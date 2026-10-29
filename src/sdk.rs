use std::path::PathBuf;

use anyhow::{anyhow, bail, Context, Result};
use md5::{Digest, Md5};
use prost::Message;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use rsa::pkcs1::DecodeRsaPrivateKey;
use rsa::pkcs1v15::SigningKey;
use rsa::signature::{SignatureEncoding, Signer};
use rsa::RsaPrivateKey;
use serde_json::{json, Map, Number, Value};
use sha2::Sha256;

use crate::credentials::Credentials;
use crate::crypto::{build_nova_message, decode_nova_message};
use crate::net::post_buffer;
use crate::proto::{ErrorMsg, LoginReq, LoginResp, Official, OfficialOverseas};
use crate::regions::{AuthMode, LoginField, Region, CLIENT_VERSION, DEVICE_MODEL};

pub const MSG_LOGIN_REQ: u16 = 4;
pub const MSG_LOGIN_SUCCEED_ACK: u16 = 5;
pub const MSG_LOGIN_FAILED_ACK: u16 = 6;
pub const MSG_PLAYER_DATA_REQ: u16 = 1001;
pub const MSG_PLAYER_DATA_SUCCEED_ACK: u16 = 1002;
pub const MSG_GACHA_INFORMATION_REQ: u16 = 6004;
pub const MSG_GACHA_INFORMATION_SUCCEED_ACK: u16 = 6005;
pub const MSG_GACHA_HISTORIES_REQ: u16 = 6007;
pub const MSG_GACHA_HISTORIES_SUCCEED_ACK: u16 = 6008;
pub const MSG_SYSTEM_FAILED_ACK: u16 = 10000;

const SALT: &str = "886c085e4a8d30a703367b120dd8353948405ec2";

const PRIVATE_KEY_CN: &str = "-----BEGIN RSA PRIVATE KEY-----
MIICXQIBAAKBgQDBdHUbBS5ZEnYbmLzUagVUD+WnG3h7vtPIb6lxLsIONtE35JEI
PY3Q6+Klv+mMAR2f8yoxp/OAD2QGwOhpMVT7HK8HyDN6W4mjx6D61/5k3FhHRfok
Fk4pyJWxCiccqO6tzMgbyqNB8W2vH9qryGdKKmjoeKfyhCKjfI5mI5QSrQIDAQAB
AoGBALUUrn0xV1X26ukgCjkGWKB8FOpb02Z5lEG9C60vKGQnuaUI5R0CiRmzgz7F
KfZsIO/UrH04ibwoK+gKcEHu9GxL0+qjqoq1KgNRhy3nqSqk1EhPmLxfUaaCdGBB
I5p4Bb8RAvtCwRABzkYobeZBQcOwSa/az0lpx4+IJu1wZAsdAkEA7RVtp6ia/QAO
JWg2ikQpYDAWB7XtVb8Q/kpGodkLCDbJbxS9+6iXHpnjoGQicwR/AnIqWyp8liPC
6agU4hoGowJBANDj4sQj+F8l24KjM04aTf0V+NCXr0rhjvKNm1GEIhz56iNxlq5O
dMcsv9B88puwvDMN7pvjm/4O3Pgt6tVlJm8CQHa9L7EzoYQq3ergOcigALT7AF2W
QSqbyfAB1UREBuFzRwmKMuhydFVQL4/p9tLyIoZgFVSZf9JWUM1jcCoZbM8CQBc+
A/FG/0+26qGyfvblvl+2niS3e+2dA0bBstJzHUiUFhUzgkyKtjujIwSTonroQ0h6
+F2oCadPCA8lLMfppUkCQQCAUYlW1Ys3DgUi+3gDZbhWqQajojfYU5PbhxhlZwc5
2TnwB+07yrrBu1yzzb0LbDWSrJUlNL2UZ438GGqrWJ0l
-----END RSA PRIVATE KEY-----";

pub struct QuickLogin {
    pub account_login_token: String,
    pub account_uid: String,
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn token_head(
    region: &Region,
    credentials: &Credentials,
    saved_token: Option<&str>,
) -> Result<Map<String, Value>> {
    let mut head = Map::new();

    if region.auth_mode == AuthMode::Cn {
        let uid = credentials
            .uid
            .parse::<u64>()
            .context("CN account UID is not a number")?;
        head.insert("Channel".into(), Value::String("official".into()));
        head.insert("Platform".into(), Value::String("pc".into()));
        head.insert("Lang".into(), Value::String(region.auth_lang.into()));
        head.insert("DeviceID".into(), Value::String(credentials.device.clone()));
        head.insert("Version".into(), Value::String(region.sdk_version.into()));
        head.insert("GVersionNo".into(), Value::String(CLIENT_VERSION.into()));
        head.insert("GBuildNo".into(), Value::String(String::new()));
        head.insert("PID".into(), Value::String(region.pid.into()));
        head.insert("DeviceModel".into(), Value::String(DEVICE_MODEL.into()));
        head.insert("Time".into(), Value::Number(Number::from(unix_now())));
        head.insert("UID".into(), Value::Number(Number::from(uid)));
        head.insert("Token".into(), Value::String(credentials.token.clone()));
        head.insert(
            "RID".into(),
            Value::String(uuid::Uuid::new_v4().to_string()),
        );
        return Ok(head);
    }

    head.insert("Channel".into(), Value::String("pc".into()));
    head.insert("Platform".into(), Value::String("pc".into()));
    head.insert("Lang".into(), Value::String(region.auth_lang.into()));
    head.insert("DeviceID".into(), Value::String(credentials.device.clone()));
    head.insert("Version".into(), Value::String(region.sdk_version.into()));
    head.insert("GVersionNo".into(), Value::String(CLIENT_VERSION.into()));
    head.insert("GBuildNo".into(), Value::String(String::new()));
    head.insert("PID".into(), Value::String(region.pid.into()));
    head.insert("DeviceModel".into(), Value::String(DEVICE_MODEL.into()));
    head.insert("Time".into(), Value::Number(Number::from(unix_now())));
    if !credentials.uid.is_empty() {
        head.insert("UID".into(), Value::String(credentials.uid.clone()));
    }
    head.insert("Token".into(), Value::String(String::new()));
    if let Some(saved_token) = saved_token {
        head.insert("Token".into(), Value::String(saved_token.to_string()));
    }

    Ok(head)
}

pub fn generate_yostar_auth_header(
    region: &Region,
    credentials: &Credentials,
    saved_token: Option<&str>,
    body: &Value,
) -> Result<String> {
    let head = token_head(region, credentials, saved_token)?;
    let head_value = Value::Object(head);
    let to_sign = format!(
        "{}{}",
        serde_json::to_string(&head_value)?,
        serde_json::to_string(body)?
    );

    let sign = if region.auth_mode == AuthMode::Cn {
        let private_key = RsaPrivateKey::from_pkcs1_pem(PRIVATE_KEY_CN)
            .map_err(|error| anyhow!("Could not parse CN signing key: {error}"))?;
        let signing_key = SigningKey::<Sha256>::new(private_key);
        let signature = signing_key.sign(to_sign.as_bytes());
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(signature.to_bytes())
    } else {
        let mut hasher = Md5::new();
        hasher.update(to_sign.as_bytes());
        hasher.update(SALT.as_bytes());
        hex::encode_upper(hasher.finalize())
    };

    let mut wrapper = Map::new();
    wrapper.insert("Head".into(), head_value);
    wrapper.insert("Sign".into(), Value::String(sign));
    Ok(serde_json::to_string(&Value::Object(wrapper))?)
}

fn truncate(text: &str, limit: usize) -> String {
    text.chars().take(limit).collect()
}

fn login_credentials(device: &str, uid: &str) -> Credentials {
    Credentials {
        uid: uid.to_string(),
        token: String::new(),
        device: device.to_string(),
        file: PathBuf::new(),
        region: None,
        mtime: 0,
        login_time: 0,
    }
}

async fn post_sdk_json(
    client: &reqwest::Client,
    region: &Region,
    endpoint: &str,
    body: &Value,
    device: &str,
    uid: &str,
) -> Result<(Value, String)> {
    let url = format!("{}{endpoint}", region.sdk_url);
    let credentials = login_credentials(device, uid);
    let auth_header = generate_yostar_auth_header(region, &credentials, None, body)?;

    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&auth_header).context("Invalid Authorization header")?,
    );

    let response = post_buffer(client, &url, serde_json::to_vec(body)?, headers).await?;
    let text = String::from_utf8_lossy(&response).into_owned();
    let object: Value = serde_json::from_str(&text)
        .map_err(|_| anyhow!("{endpoint} returned invalid JSON: {}", truncate(&text, 300)))?;
    Ok((object, text))
}

pub async fn send_auth_code(
    client: &reqwest::Client,
    region: &Region,
    email: &str,
    device: &str,
    uid: &str,
) -> Result<()> {
    let body = json!({ "Account": email });
    let (object, text) =
        post_sdk_json(client, region, "/yostar/send-code", &body, device, uid).await?;
    if object.get("Code").and_then(Value::as_i64) != Some(200) {
        bail!("/yostar/send-code failed: {}", truncate(&text, 500));
    }
    Ok(())
}

pub async fn login_with_email_code(
    client: &reqwest::Client,
    region: &Region,
    email: &str,
    code: &str,
    device: &str,
    uid: &str,
) -> Result<QuickLogin> {
    let auth_body = json!({ "Account": email, "Code": code });
    let (auth_object, auth_text) =
        post_sdk_json(client, region, "/yostar/get-auth", &auth_body, device, uid).await?;
    let auth_token = auth_object
        .get("Data")
        .and_then(|data| data.get("Token"))
        .and_then(Value::as_str)
        .filter(|token| !token.is_empty())
        .map(str::to_string)
        .with_context(|| format!("/yostar/get-auth failed: {}", truncate(&auth_text, 500)))?;

    let login_body = json!({
        "OpenID": email,
        "Token": auth_token,
        "Type": "yostar",
        "UserName": email,
        "Secret": "",
        "CheckAccount": 0,
    });
    let (login_object, login_text) =
        post_sdk_json(client, region, "/user/login", &login_body, device, uid).await?;
    let user = extract_sdk_user(&login_object)
        .with_context(|| format!("/user/login failed: {}", truncate(&login_text, 500)))?;

    let token = user
        .get("Token")
        .and_then(Value::as_str)
        .filter(|token| !token.is_empty())
        .map(str::to_string)
        .with_context(|| format!("/user/login failed: {}", truncate(&login_text, 500)))?;
    let uid = user
        .get("ID")
        .and_then(js_string)
        .filter(|uid| !uid.is_empty())
        .with_context(|| {
            format!(
                "/user/login response has no account ID: {}",
                truncate(&login_text, 500)
            )
        })?;

    Ok(QuickLogin {
        account_login_token: token,
        account_uid: uid,
    })
}

fn extract_sdk_user(value: &Value) -> Option<&Value> {
    let data = value.get("Data")?;
    data.get("User")
        .filter(|user| user.is_object())
        .or_else(|| data.get("UserInfo").filter(|user| user.is_object()))
}

fn js_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(boolean) => Some(boolean.to_string()),
        _ => None,
    }
}

pub async fn quick_login(
    client: &reqwest::Client,
    region: &Region,
    credentials: &Credentials,
    saved_token: &str,
) -> Result<QuickLogin> {
    let url = format!("{}/user/quick-login", region.sdk_url);
    let body = json!({});
    let saved = if region.auth_mode == AuthMode::Cn {
        None
    } else {
        Some(saved_token)
    };
    let auth_header = generate_yostar_auth_header(region, credentials, saved, &body)?;

    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&auth_header).context("Invalid Authorization header")?,
    );

    let response = post_buffer(client, &url, b"{}".to_vec(), headers).await?;
    let text = String::from_utf8_lossy(&response).into_owned();
    let object: Value = serde_json::from_str(&text).map_err(|_| {
        anyhow!(
            "/user/quick-login returned invalid JSON: {}",
            truncate(&text, 300)
        )
    })?;

    let user = extract_sdk_user(&object);
    let token = user
        .and_then(|user| user.get("Token"))
        .and_then(Value::as_str)
        .filter(|token| !token.is_empty());
    let code = object.get("Code").and_then(Value::as_i64);
    if code != Some(200) || token.is_none() {
        bail!("/user/quick-login failed: {}", truncate(&text, 500));
    }

    let uid = user
        .and_then(|user| user.get("ID"))
        .and_then(js_string)
        .filter(|uid| !uid.is_empty())
        .context("/user/quick-login response has no account ID")?;

    Ok(QuickLogin {
        account_login_token: token.unwrap().to_string(),
        account_uid: uid,
    })
}

fn frame_id(plain: &[u8]) -> Result<u16> {
    if plain.len() < 2 {
        bail!("Nova response is too short");
    }
    Ok(u16::from_be_bytes([plain[0], plain[1]]))
}

pub async fn get_account_login_token(
    client: &reqwest::Client,
    region: &Region,
    credentials: &Credentials,
) -> Result<QuickLogin> {
    println!("Found saved Yostar token; trying quick-login...");
    quick_login(client, region, credentials, &credentials.token).await
}

pub async fn do_player_login(
    client: &reqwest::Client,
    region: &Region,
    credentials: &Credentials,
    ike_token: &str,
    cipher: i32,
    session_key: &[u8],
    account_login_token: &str,
    account_uid: &str,
) -> Result<String> {
    let mut login = LoginReq {
        platform: Some(3),
        language: Some(region.language.to_string()),
        channel: Some("Official".to_string()),
        device: Some(credentials.device.clone()),
        version: Some(CLIENT_VERSION.to_string()),
        ..Default::default()
    };

    match region.login_field {
        LoginField::OfficialOverseas => {
            login.official_overseas = Some(OfficialOverseas {
                uid: Some(account_uid.to_string()),
                token: Some(account_login_token.to_string()),
            });
        }
        LoginField::Official => {
            login.official = Some(Official {
                uid: Some(
                    account_uid
                        .parse::<u64>()
                        .context("CN account UID is not a number")?,
                ),
                token: Some(account_login_token.to_string()),
            });
        }
    }

    let body = login.encode_to_vec();
    let payload = build_nova_message(MSG_LOGIN_REQ, &body, cipher, session_key, region.garble_key)?;

    let mut headers = HeaderMap::new();
    headers.insert(
        "X-Token",
        HeaderValue::from_str(ike_token).context("Invalid X-Token header")?,
    );
    let response = post_buffer(
        client,
        &format!("{}/agent-zone-1/", region.server_url),
        payload,
        headers,
    )
    .await?;
    let plain = decode_nova_message(&response, cipher, session_key)?;
    let msg_id = frame_id(&plain)?;

    if msg_id == MSG_LOGIN_SUCCEED_ACK {
        let login_response = LoginResp::decode(&plain[2..])?;
        return login_response
            .token
            .filter(|token| !token.is_empty())
            .context("Player login returned an empty session token");
    }

    if msg_id == MSG_LOGIN_FAILED_ACK || msg_id == MSG_SYSTEM_FAILED_ACK {
        let detail = ErrorMsg::decode(&plain[2..]).unwrap_or_default();
        bail!("Player login failed (msgId {msg_id}): {detail:?}");
    }

    bail!("Unexpected player login response msgId: {msg_id}");
}

pub async fn send_game_request(
    client: &reqwest::Client,
    region: &Region,
    token: &str,
    cipher: i32,
    session_key: &[u8],
    msg_id: u16,
    body: &[u8],
    expected_ack: u16,
) -> Result<Vec<u8>> {
    let payload = build_nova_message(msg_id, body, cipher, session_key, region.garble_key)?;

    let mut headers = HeaderMap::new();
    headers.insert(
        "X-Token",
        HeaderValue::from_str(token).context("Invalid X-Token header")?,
    );
    let response = post_buffer(
        client,
        &format!("{}/agent-zone-1/", region.server_url),
        payload,
        headers,
    )
    .await?;
    let plain = decode_nova_message(&response, cipher, session_key)?;
    let response_id = frame_id(&plain)?;

    if response_id == expected_ack {
        return Ok(plain[2..].to_vec());
    }
    if response_id == MSG_SYSTEM_FAILED_ACK {
        let detail = ErrorMsg::decode(&plain[2..]).unwrap_or_default();
        bail!("Server returned system_failed_ack: {detail:?}");
    }
    bail!("Expected ack {expected_ack}, got {response_id}");
}
