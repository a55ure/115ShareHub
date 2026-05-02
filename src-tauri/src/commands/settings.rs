use crate::db::Database;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProxyConfig {
    pub enabled: bool,
    #[serde(rename = "proxyType")]
    pub proxy_type: String,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppSettings {
    pub rate_limit_rps: u32,
    pub page_size: u32,
    pub theme: String,
    pub language: String,
    pub proxy: ProxyConfig,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            proxy_type: "http".to_string(),
            host: String::new(),
            port: 1080,
            username: None,
            password: None,
        }
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            rate_limit_rps: 2,
            page_size: 1150,
            theme: "light".to_string(),
            language: "zh-CN".to_string(),
            proxy: ProxyConfig::default(),
        }
    }
}

#[tauri::command]
pub fn get_proxy_configs(db: State<'_, Database>) -> Result<Vec<ProxyConfig>, String> {
    // Try new multi-proxy format first
    let config_str = db
        .get_setting("proxy_configs")
        .map_err(|e| e.to_string())?
        .unwrap_or_default();

    if !config_str.is_empty() {
        return serde_json::from_str(&config_str).map_err(|e| e.to_string());
    }

    // Fall back to old single-proxy format
    let old_str = db
        .get_setting("proxy_config")
        .map_err(|e| e.to_string())?
        .unwrap_or_default();

    if old_str.is_empty() {
        return Ok(vec![]);
    }

    let single: ProxyConfig = serde_json::from_str(&old_str).map_err(|e| e.to_string())?;
    Ok(vec![single])
}

#[tauri::command]
pub fn save_proxy_configs(db: State<'_, Database>, configs: Vec<ProxyConfig>) -> Result<(), String> {
    let config_str = serde_json::to_string(&configs).map_err(|e| e.to_string())?;
    db.set_setting("proxy_configs", &config_str)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_proxy_config(db: State<'_, Database>) -> Result<ProxyConfig, String> {
    let config_str = db
        .get_setting("proxy_config")
        .map_err(|e| e.to_string())?
        .unwrap_or_default();

    if config_str.is_empty() {
        return Ok(ProxyConfig::default());
    }

    serde_json::from_str(&config_str).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_proxy_config(db: State<'_, Database>, config: ProxyConfig) -> Result<(), String> {
    let config_str = serde_json::to_string(&config).map_err(|e| e.to_string())?;
    db.set_setting("proxy_config", &config_str)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_app_setting(db: State<'_, Database>, key: String, value: String) -> Result<(), String> {
    db.set_setting(&key, &value).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_app_settings(db: State<'_, Database>) -> Result<AppSettings, String> {
    let rate_limit = db
        .get_setting("rate_limit_rps")
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| "2".to_string())
        .parse()
        .unwrap_or(2);

    let page_size = db
        .get_setting("page_size")
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| "1150".to_string())
        .parse()
        .unwrap_or(1150);

    let theme = db
        .get_setting("theme")
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| "light".to_string());

    let language = db
        .get_setting("language")
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| "zh-CN".to_string());

    let proxy = get_proxy_config_inner(&db)?;

    Ok(AppSettings {
        rate_limit_rps: rate_limit,
        page_size,
        theme,
        language,
        proxy,
    })
}

fn get_proxy_config_inner(db: &Database) -> Result<ProxyConfig, String> {
    let config_str = db
        .get_setting("proxy_config")
        .map_err(|e| e.to_string())?
        .unwrap_or_default();

    if config_str.is_empty() {
        return Ok(ProxyConfig::default());
    }

    serde_json::from_str(&config_str).map_err(|e| e.to_string())
}
