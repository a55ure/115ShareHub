use super::client::ApiError;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QrCodeResult {
    pub uid: String,
    pub qr_image_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollResult {
    pub status: i32,
    pub cookie: Option<String>,
}

/// 115 user info returned by /user/info API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo115 {
    pub user_id: String,
    pub user_name: String,
    pub face: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginStatus {
    pub logged_in: bool,
    pub user_name: String,
    pub user_id: String,
    pub face: String,
    pub login_time: Option<String>,
}

pub struct AuthClient {
    http: Client,
}

impl AuthClient {
    pub fn new() -> Self {
        let http = Client::builder()
            .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
            .timeout(Duration::from_secs(30))
            .build()
            .expect("failed to build HTTP client");
        AuthClient { http }
    }

    /// Step 1: Fetch QR code PNG from 115, return base64 image + uid.
    pub async fn get_qrcode(&self) -> Result<QrCodeResult, ApiError> {
        let resp = self
            .http
            .get("https://qrcodeapi.115.com/api/2.0/pb.png?qrfrom=1&client=0")
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            return Err(ApiError::Api(format!("获取二维码失败: HTTP {}", status)));
        }

        // Extract uid from Set-Cookie header
        let uid = resp
            .headers()
            .get_all("set-cookie")
            .iter()
            .find_map(|v| {
                let s = v.to_str().ok()?;
                let prefix = "uid=";
                let start = s.find(prefix)?;
                let rest = &s[start + prefix.len()..];
                let end = rest.find(';').unwrap_or(rest.len());
                Some(rest[..end].to_string())
            })
            .ok_or_else(|| ApiError::Parse("二维码响应中未找到 uid".to_string()))?;

        let bytes = resp.bytes().await.map_err(ApiError::Network)?;
        let base64 = base64_encode(&bytes);

        Ok(QrCodeResult {
            uid,
            qr_image_base64: format!("data:image/png;base64,{}", base64),
        })
    }

    /// Step 2: Poll QR code scan status.
    /// status: 0=等待扫码, 1=已扫码待确认, 2=已确认登录, -1=已过期
    pub async fn poll_qrcode(&self, uid: &str) -> Result<PollResult, ApiError> {
        let resp = self
            .http
            .post("https://qrcodeapi.115.com/get/status")
            .header("Cookie", format!("uid={}", uid))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(format!("uid={}", uid))
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            return Err(ApiError::Api(format!("轮询失败: HTTP {}", status)));
        }

        let text = resp.text().await.map_err(ApiError::Network)?;
        let json: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| ApiError::Parse(format!("JSON解析失败: {}", e)))?;

        let data = json
            .get("data")
            .ok_or_else(|| ApiError::Parse("响应中无data字段".to_string()))?;

        let poll_status = data
            .get("status")
            .and_then(|v| v.as_i64())
            .unwrap_or(-1) as i32;

        // When status=2 (confirmed), try to extract cookie from response
        let cookie = if poll_status == 2 {
            data.get("cookie").and_then(|v| v.as_str()).map(|s| s.to_string())
        } else {
            None
        };

        Ok(PollResult {
            status: poll_status,
            cookie,
        })
    }

    /// After QR code confirmed (status=2), exchange uid for login cookies.
    pub async fn exchange_qrcode_login(&self, uid: &str) -> Result<String, ApiError> {
        let resp = self
            .http
            .post("https://passportapi.115.com/app/1.0/web/1.0/login/qrcode")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(format!("uid={}", uid))
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            return Err(ApiError::Api(format!("登录交换失败: HTTP {}", status)));
        }

        // Extract cookies from response headers
        let cookies: Vec<String> = resp
            .headers()
            .get_all("set-cookie")
            .iter()
            .filter_map(|v| {
                let s = v.to_str().ok()?;
                let end = s.find(';').unwrap_or(s.len());
                Some(s[..end].to_string())
            })
            .collect();

        if cookies.is_empty() {
            // Fallback: try to parse JSON body for token/cookie
            let text = resp.text().await.map_err(ApiError::Network)?;
            let json: serde_json::Value =
                serde_json::from_str(&text).map_err(|e| ApiError::Parse(format!("{}", e)))?;

            json.get("data")
                .and_then(|d| d.get("cookie"))
                .and_then(|c| c.as_str())
                .map(|s| s.to_string())
                .ok_or_else(|| ApiError::Parse("登录响应中未找到cookie".to_string()))
        } else {
            Ok(cookies.join("; "))
        }
    }

    /// Validate a cookie string by fetching user info from 115.
    pub async fn validate_cookie(&self, cookie: &str) -> Result<UserInfo115, ApiError> {
        let resp = self
            .http
            .get("https://webapi.115.com/user/info")
            .header("Cookie", cookie)
            .send()
            .await?;

        let status = resp.status();
        let text = resp.text().await.map_err(ApiError::Network)?;

        if !status.is_success() {
            return Err(ApiError::Api(format!("Cookie验证失败: HTTP {}", status)));
        }

        let json: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| ApiError::Parse(format!("{}", e)))?;

        if json.get("state").and_then(|v| v.as_bool()).unwrap_or(false) {
            let data = json
                .get("data")
                .ok_or_else(|| ApiError::Parse("响应中无data字段".to_string()))?;

            Ok(UserInfo115 {
                user_id: data
                    .get("user_id")
                    .and_then(|v| v.as_i64())
                    .map(|v| v.to_string())
                    .or_else(|| data.get("user_id").and_then(|v| v.as_str()).map(|s| s.to_string()))
                    .unwrap_or_default(),
                user_name: data
                    .get("user_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                face: data
                    .get("face")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            })
        } else {
            let err = json
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("Cookie无效或已过期");
            Err(ApiError::Api(err.to_string()))
        }
    }
}

fn base64_encode(bytes: &[u8]) -> String {
    use std::fmt::Write;
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity((bytes.len() + 2) / 3 * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        write!(s, "{}", TABLE[((triple >> 18) & 0x3F) as usize] as char).unwrap();
        write!(s, "{}", TABLE[((triple >> 12) & 0x3F) as usize] as char).unwrap();
        if chunk.len() > 1 {
            write!(s, "{}", TABLE[((triple >> 6) & 0x3F) as usize] as char).unwrap();
        } else {
            s.push('=');
        }
        if chunk.len() > 2 {
            write!(s, "{}", TABLE[(triple & 0x3F) as usize] as char).unwrap();
        } else {
            s.push('=');
        }
    }
    s
}
