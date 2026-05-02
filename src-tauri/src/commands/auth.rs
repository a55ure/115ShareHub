use crate::db::Database;
use crate::pan115::auth::{AuthClient, LoginStatus};
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QrCodeResponse {
    pub uid: String,
    pub qr_image_base64: String,
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
    let result = client.get_qrcode().await.map_err(|e| e.to_string())?;
    Ok(QrCodeResponse {
        uid: result.uid,
        qr_image_base64: result.qr_image_base64,
    })
}

#[tauri::command]
pub async fn poll_qrcode_login(
    state: State<'_, Database>,
    uid: String,
) -> Result<PollResponse, String> {
    let client = AuthClient::new();
    let result = client.poll_qrcode(&uid).await.map_err(|e| e.to_string())?;

    if result.status == 2 {
        // Confirmed — exchange for login cookie
        let cookie = if let Some(c) = result.cookie {
            c
        } else {
            client
                .exchange_qrcode_login(&uid)
                .await
                .map_err(|e| e.to_string())?
        };

        // Validate and get user info
        let user_info = client
            .validate_cookie(&cookie)
            .await
            .map_err(|e| e.to_string())?;

        // Save to DB
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

    let client = AuthClient::new();
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
        // Validate existing cookie
        let client = AuthClient::new();
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
                // Cookie expired, clear it
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
