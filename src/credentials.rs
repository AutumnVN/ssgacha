use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result};
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
}

static YOSDK_CACHE: OnceLock<HashMap<String, Credentials>> = OnceLock::new();

pub fn load_yosdk_credentials() -> &'static HashMap<String, Credentials> {
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
            .filter_map(|file| parse_yosdk_user_info(file).ok().flatten())
            .collect();
        users.sort_by(|a, b| b.mtime.cmp(&a.mtime));

        let mut by_region = HashMap::new();
        for user in users {
            if let Some(region) = user.region {
                by_region.entry(region.to_string()).or_insert(user);
            }
        }
        by_region
    })
}

pub fn find_yosdk_credentials(region_key: &str) -> Option<&'static Credentials> {
    load_yosdk_credentials().get(region_key)
}

pub fn region_credentials(region: &Region) -> Option<&'static Credentials> {
    let credentials = find_yosdk_credentials(region.key)?;
    println!(
        "Using {} ({}, UID {})",
        credentials.file.display(),
        region.label,
        credentials.uid
    );
    Some(credentials)
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

fn login_time(value: &Value) -> i64 {
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

fn parse_yosdk_user_info(file: &Path) -> Result<Option<Credentials>> {
    let Some(data) = read_json_file(file)? else {
        return Ok(None);
    };
    let Some(history_value) = data.get("all_user_login_history") else {
        return Ok(None);
    };
    let Some(history) = parse_json_value(history_value) else {
        return Ok(None);
    };
    let Some(history) = history.as_array() else {
        return Ok(None);
    };

    let mut entries: Vec<Value> = history.iter().filter_map(parse_json_value).collect();
    entries.sort_by(|a, b| {
        login_time(b)
            .partial_cmp(&login_time(a))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let latest = entries.first();
    let Some(latest) = latest else {
        return Ok(None);
    };
    let Some(device) = latest
        .get("DeviceID")
        .and_then(Value::as_str)
        .filter(|device| !device.is_empty())
    else {
        return Ok(None);
    };
    let Some(uid) = data
        .get("ID")
        .and_then(js_string)
        .filter(|uid| !uid.is_empty())
    else {
        return Ok(None);
    };
    let Some(token) = data
        .get("Token")
        .and_then(js_string)
        .filter(|token| !token.is_empty())
    else {
        return Ok(None);
    };

    let mtime = fs::metadata(file)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis())
        .unwrap_or(0);

    Ok(Some(Credentials {
        uid,
        token,
        device: device.to_string(),
        file: file.to_path_buf(),
        region: yosdk_region_from_file(file),
        mtime,
    }))
}

#[allow(dead_code)]
pub fn read_credentials_for_test(file: &Path) -> Result<Option<Credentials>> {
    parse_yosdk_user_info(file).with_context(|| format!("reading {}", file.display()))
}
