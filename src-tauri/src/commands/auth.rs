use crate::db::Database;
use crate::pan115::auth::{AuthClient, LoginStatus};
use crate::pan115::client::ProxyConfig;
use serde::{Deserialize, Serialize};
use tauri::State;

fn get_proxy_config(db: &Database) -> ProxyConfig {
    let config_str = db
        .get_setting("proxy_config")
        .ok()
        .flatten()
        .unwrap_or_default();
    serde_json::from_str(&config_str).unwrap_or_default()
}

fn make_auth_client(db: Option<&Database>) -> AuthClient {
    if let Some(db) = db {
        let cfg = get_proxy_config(db);
        AuthClient::with_proxy(&cfg)
    } else {
        AuthClient::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QrCodeResponse {
    pub token: String,
    pub qr_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollResponse {
    pub status: i32,
    pub logged_in: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CookieLoginRequest {
    pub cookie: String,
}

#[tauri::command]
pub async fn init_qrcode_login() -> Result<QrCodeResponse, String> {
    let client = AuthClient::new();
    let result = client.get_qr_token().await.map_err(|e| e.to_string())?;
    Ok(QrCodeResponse {
        qr_url: format!("https://115.com/login/?qrcode={}", result.token),
        token: result.token,
    })
}

#[tauri::command]
pub async fn poll_qrcode_login(
    state: State<'_, Database>,
    token: String,
) -> Result<PollResponse, String> {
    let client = make_auth_client(Some(&*state));
    let result = client.poll_qr_token(&token).await.map_err(|e| e.to_string())?;

    if result.status == 2 {
        let cookie = result.cookie.ok_or_else(|| "登录成功但未获取到cookie".to_string())?;

        let user_info = client
            .validate_cookie(&cookie)
            .await
            .map_err(|e| format!("Cookie验证失败: {}", e))?;

        state.set_setting("auth_cookie", &cookie).map_err(|e| e.to_string())?;
        state
            .set_setting("auth_user_name", &user_info.user_name)
            .map_err(|e| e.to_string())?;
        state
            .set_setting("auth_user_id", &user_info.user_id)
            .map_err(|e| e.to_string())?;
        state
            .set_setting("auth_face", &user_info.face)
            .map_err(|e| e.to_string())?;
        state
            .set_setting("auth_login_time", &chrono::Utc::now().to_rfc3339())
            .map_err(|e| e.to_string())?;

        Ok(PollResponse {
            status: 2,
            logged_in: true,
        })
    } else {
        Ok(PollResponse {
            status: result.status,
            logged_in: false,
        })
    }
}

#[tauri::command]
pub async fn login_by_cookie(
    state: State<'_, Database>,
    request: CookieLoginRequest,
) -> Result<LoginStatus, String> {
    let cookie = request.cookie.trim().to_string();
    if cookie.is_empty() {
        return Err("Cookie不能为空".to_string());
    }

    let client = make_auth_client(Some(&*state));
    let user_info = client
        .validate_cookie(&cookie)
        .await
        .map_err(|e| format!("Cookie验证失败: {}", e))?;

    state
        .set_setting("auth_cookie", &cookie)
        .map_err(|e| e.to_string())?;
    state
        .set_setting("auth_user_name", &user_info.user_name)
        .map_err(|e| e.to_string())?;
    state
        .set_setting("auth_user_id", &user_info.user_id)
        .map_err(|e| e.to_string())?;
    state
        .set_setting("auth_face", &user_info.face)
        .map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();
    state
        .set_setting("auth_login_time", &now)
        .map_err(|e| e.to_string())?;

    Ok(LoginStatus {
        logged_in: true,
        user_name: user_info.user_name,
        user_id: user_info.user_id,
        face: user_info.face,
        login_time: Some(now),
    })
}

#[tauri::command]
pub async fn get_login_status(state: State<'_, Database>) -> Result<LoginStatus, String> {
    let cookie = state.get_setting("auth_cookie").map_err(|e| e.to_string())?;

    if let Some(cookie_str) = cookie {
        let client = make_auth_client(Some(&*state));
        match client.validate_cookie(&cookie_str).await {
            Ok(info) => {
                let login_time = state
                    .get_setting("auth_login_time")
                    .map_err(|e| e.to_string())?;
                Ok(LoginStatus {
                    logged_in: true,
                    user_name: info.user_name,
                    user_id: info.user_id,
                    face: info.face,
                    login_time,
                })
            }
            Err(_) => {
                clear_auth_settings(&state)?;
                Ok(LoginStatus {
                    logged_in: false,
                    user_name: String::new(),
                    user_id: String::new(),
                    face: String::new(),
                    login_time: None,
                })
            }
        }
    } else {
        Ok(LoginStatus {
            logged_in: false,
            user_name: String::new(),
            user_id: String::new(),
            face: String::new(),
            login_time: None,
        })
    }
}

#[tauri::command]
pub async fn logout(state: State<'_, Database>) -> Result<(), String> {
    clear_auth_settings(&state)?;
    Ok(())
}

fn clear_auth_settings(db: &Database) -> Result<(), String> {
    let conn = db.get_conn();
    conn.execute("DELETE FROM settings WHERE key LIKE 'auth_%'", [])
        .map_err(|e| e.to_string())?;
    Ok(())
}
