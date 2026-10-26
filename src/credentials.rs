use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::UNIX_EPOCH;

use anyhow::Result;
use serde_json::Value;

use crate::regions::{normalize_region, Region};

#[derive(Clone, Debug)]
pub struct Credentials {
    pub uid: String,
    pub token: String,
    pub device: String,
    pub file: PathBuf,
    pub region: Option<&'static str>,
    pub mtime: u128,
    pub login_time: i64,
}

static YOSDK_CACHE: OnceLock<HashMap<String, Vec<Credentials>>> = OnceLock::new();

pub fn load_yosdk_credentials() -> &'static HashMap<String, Vec<Credentials>> {
    YOSDK_CACHE.get_or_init(|| {
        let files = find_yosdk_files();
        let mut users: Vec<Credentials> = files
            .iter()
            .filter(|file| {
                file.file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name.to_lowercase().starts_with("yosdk_userinfo"))
                    .unwrap_or(false)
            })
            .flat_map(|file| parse_yosdk_user_info(file).unwrap_or_default())
            .collect();

        users.sort_by(|a, b| {
            b.mtime
                .cmp(&a.mtime)
                .then_with(|| b.login_time.cmp(&a.login_time))
        });

        let mut by_region: HashMap<String, Vec<Credentials>> = HashMap::new();
        for user in users {
            let Some(region) = user.region else {
                continue;
            };
            let entries = by_region.entry(region.to_string()).or_default();
            if let Some(existing) = entries.iter_mut().find(|entry| entry.uid == user.uid) {
                let newer = user.mtime > existing.mtime
                    || (user.mtime == existing.mtime && user.login_time > existing.login_time);
                if newer {
                    *existing = user;
                }
            } else {
                entries.push(user);
            }
        }

        for entries in by_region.values_mut() {
            entries.sort_by(|a, b| compare_uids(&a.uid, &b.uid));
        }

        by_region
    })
}

pub fn find_yosdk_credentials(region_key: &str) -> Option<&'static [Credentials]> {
    load_yosdk_credentials().get(region_key).map(Vec::as_slice)
}

pub fn region_credentials(region: &Region) -> &'static [Credentials] {
    find_yosdk_credentials(region.key).unwrap_or(&[])
}

fn compare_uids(left: &str, right: &str) -> std::cmp::Ordering {
    match (left.parse::<u128>(), right.parse::<u128>()) {
        (Ok(left), Ok(right)) => left.cmp(&right),
        _ => left.cmp(right),
    }
}

fn stella_sora_directories() -> Vec<PathBuf> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    let local_low = home.join("AppData").join("LocalLow");
    let Ok(entries) = fs::read_dir(&local_low) else {
        return Vec::new();
    };

    entries
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .map(|name| name.to_ascii_lowercase().contains("yostar"))
                .unwrap_or(false)
                && entry.path().is_dir()
        })
        .map(|entry| entry.path().join("StellaSora"))
        .filter(|dir| dir.exists())
        .collect()
}

fn find_yosdk_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    for dir in stella_sora_directories() {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            if entry.path().is_file()
                && entry
                    .file_name()
                    .to_str()
                    .map(|name| name.to_ascii_lowercase().starts_with("yosdk_"))
                    .unwrap_or(false)
            {
                files.push(entry.path());
            }
        }
    }
    files
}

fn read_json_file(file: &Path) -> Result<Option<Value>> {
    let text = fs::read_to_string(file)?;
    let text = text.trim_start_matches('\u{feff}').trim();
    Ok(serde_json::from_str(text).ok())
}

fn parse_json_value(value: &Value) -> Option<Value> {
    if value.is_object() || value.is_array() {
        return Some(value.clone());
    }
    let text = value.as_str()?.trim();
    if text.is_empty() {
        return None;
    }
    serde_json::from_str(text).ok()
}

fn js_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(boolean) => Some(boolean.to_string()),
        _ => None,
    }
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|text| !text.is_empty())
}

fn to_i64(value: &Value) -> i64 {
    if let Some(number) = value.as_i64() {
        return number;
    }
    if let Some(number) = value.as_f64() {
        return number as i64;
    }
    if let Some(text) = value.as_str() {
        return text.parse::<i64>().unwrap_or(0);
    }
    0
}

fn login_time(entry: &Value) -> i64 {
    entry.get("loginTime").map(to_i64).unwrap_or(0)
}

fn yosdk_region_from_file(file: &Path) -> Option<&'static str> {
    let name = file.file_name()?.to_str()?;
    if !name.to_ascii_lowercase().starts_with("yosdk_userinfo") {
        return None;
    }
    let rest = &name["YoSDK_UserInfo".len()..];
    let region = rest.trim_start_matches(['_', '-']);
    if region.is_empty() {
        return None;
    }
    normalize_region(region)
}

fn parse_yosdk_user_info(file: &Path) -> Result<Vec<Credentials>> {
    let Some(data) = read_json_file(file)? else {
        return Ok(Vec::new());
    };
    let Some(history_value) = data.get("all_user_login_history") else {
        return Ok(Vec::new());
    };
    let Some(history) = parse_json_value(history_value) else {
        return Ok(Vec::new());
    };
    let Some(history) = history.as_array() else {
        return Ok(Vec::new());
    };

    let mut entries: Vec<Value> = history.iter().filter_map(parse_json_value).collect();
    entries.sort_by(|a, b| {
        login_time(b)
            .partial_cmp(&login_time(a))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let current_uid = non_empty(data.get("ID").and_then(js_string));
    let current_token = non_empty(data.get("Token").and_then(js_string));
    let fallback_device = entries
        .iter()
        .find_map(|entry| non_empty(entry.get("DeviceID").and_then(js_string)));

    let mtime = fs::metadata(file)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis())
        .unwrap_or(0);

    let region = yosdk_region_from_file(file);
    let mut users = Vec::new();
    for entry in entries {
        let Some(uid) = non_empty(entry.get("ID").and_then(js_string)) else {
            continue;
        };
        let token = non_empty(entry.get("Token").and_then(js_string)).or_else(|| {
            if current_uid.as_deref() == Some(uid.as_str()) {
                current_token.clone()
            } else {
                None
            }
        });
        let Some(token) = token else {
            continue;
        };
        let device = non_empty(entry.get("DeviceID").and_then(js_string))
            .or_else(|| fallback_device.clone());
        let Some(device) = device else {
            continue;
        };

        users.push(Credentials {
            uid,
            token,
            device,
            file: file.to_path_buf(),
            region,
            mtime,
            login_time: login_time(&entry),
        });
    }

    Ok(users)
}
