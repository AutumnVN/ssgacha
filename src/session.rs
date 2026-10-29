use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use anyhow::Result;

const ENV_FILE: &str = ".env";

fn env_path() -> Result<PathBuf> {
    Ok(std::env::current_dir()?.join(ENV_FILE))
}

fn parse_env(text: &str) -> HashMap<String, String> {
    let text = text.trim_start_matches('\u{feff}');
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (key, value) = line.split_once('=')?;
            let key = key.trim();
            if key.is_empty() {
                return None;
            }
            let value = value
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string();
            Some((key.to_string(), value))
        })
        .collect()
}

fn load_env() -> HashMap<String, String> {
    let Ok(path) = env_path() else {
        return HashMap::new();
    };
    let Ok(text) = fs::read_to_string(path) else {
        return HashMap::new();
    };
    parse_env(&text)
}

fn value(env: &HashMap<String, String>, key: &str) -> Option<String> {
    env.get(key)
        .cloned()
        .filter(|value| !value.is_empty())
        .or_else(|| std::env::var(key).ok().filter(|value| !value.is_empty()))
}

pub fn configured_device() -> Option<String> {
    value(&load_env(), "DEVICE")
}

#[derive(Clone, Debug)]
pub struct SavedLogin {
    pub uid: String,
    pub token: String,
    pub device: String,
}

pub fn load_saved_login(region_key: &str) -> Option<SavedLogin> {
    let env = load_env();
    let suffix = region_key.to_ascii_uppercase();
    let token = value(&env, &format!("TOKEN_{suffix}"))?;
    Some(SavedLogin {
        uid: value(&env, &format!("UID_{suffix}")).unwrap_or_default(),
        token,
        device: value(&env, "DEVICE").unwrap_or_default(),
    })
}

pub fn save_login(region_key: &str, uid: &str, token: &str, device: &str) -> Result<()> {
    let path = env_path()?;
    let existing = fs::read_to_string(&path).unwrap_or_default();
    fs::write(path, merge_login(&existing, region_key, uid, token, device))?;
    Ok(())
}

fn merge_login(existing: &str, region_key: &str, uid: &str, token: &str, device: &str) -> String {
    let suffix = region_key.to_ascii_uppercase();
    let token_key = format!("TOKEN_{suffix}");
    let uid_key = format!("UID_{suffix}");
    let mut lines: Vec<String> = existing.lines().map(str::to_string).collect();

    let has_device = lines.iter().any(|line| {
        line.trim()
            .strip_prefix("DEVICE=")
            .map(|value| !value.trim().trim_matches('"').trim_matches('\'').is_empty())
            .unwrap_or(false)
    });

    lines.retain(|line| {
        let trimmed = line.trim();
        !trimmed.starts_with(&format!("{token_key}="))
            && !trimmed.starts_with(&format!("{uid_key}="))
    });

    if !has_device {
        lines.retain(|line| !line.trim().starts_with("DEVICE="));
        lines.push(format!("DEVICE={device}"));
    }
    lines.push(format!("{token_key}={token}"));
    lines.push(format!("{uid_key}={uid}"));

    let mut content = lines.join("\n");
    if !content.is_empty() {
        content.push('\n');
    }
    content
}
