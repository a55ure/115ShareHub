use super::client::{ApiError, ProxyConfig};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QrTokenResult {
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollResult {
    pub status: i32,
    pub cookie: Option<String>,
}

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
        Self::with_proxy(&ProxyConfig::default())
    }

    pub fn with_proxy(proxy_config: &ProxyConfig) -> Self {
        let mut builder = Client::builder()
            .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
            .timeout(Duration::from_secs(30));

        if proxy_config.enabled && !proxy_config.host.is_empty() {
            let proxy_url = format!(
                "{}://{}:{}",
                proxy_config.proxy_type, proxy_config.host, proxy_config.port
            );
            match reqwest::Proxy::all(&proxy_url) {
                Ok(mut proxy) => {
                    if let (Some(u), Some(p)) = (&proxy_config.username, &proxy_config.password) {
                        if !u.is_empty() {
                            proxy = proxy.basic_auth(u, p);
                        }
                    }
                    builder = builder.proxy(proxy);
                    log::info!("AuthClient: attached proxy {}", proxy_url);
                }
                Err(e) => {
                    log::error!("AuthClient: failed to create proxy {}: {}", proxy_url, e);
                }
            }
        }

        let http = builder.build().expect("failed to build HTTP client");
        AuthClient { http }
    }

    /// Get QR login token from 115 QR code API.
    /// Ref: AList uses qrcodeapi.115.com, not passportapi.115.com.
    pub async fn get_qr_token(&self) -> Result<QrTokenResult, ApiError> {
        let resp = self
            .http
            .get("https://qrcodeapi.115.com/api/1.0/web/1.0/token")
            .header("Accept", "application/json, text/plain, */*")
            .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
            .header("Referer", "https://115.com/")
            .header("Origin", "https://115.com")
            .send()
            .await?;

        let status = resp.status();
        let text = resp.text().await.map_err(ApiError::Network)?;
        log::info!("QR token response (HTTP {}): {} bytes", status, text.len());

        if !status.is_success() || text.trim().starts_with('<') {
            return Err(ApiError::Api("获取二维码token失败: 被115服务器拦截".to_string()));
        }

        let json: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| ApiError::Parse(format!("JSON解析失败: {}", e)))?;

        // AList QRCodeTokenResp: { state: 1 (success), data: { uid: "...", ... } }
        let token = json
            .get("data")
            .and_then(|d| d.get("uid"))
            .or_else(|| json.get("data").and_then(|d| d.get("token")))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| ApiError::Parse("响应中未找到token/uid".to_string()))?;

        Ok(QrTokenResult { token })
    }

    /// Poll QR code scan status. Uses qrcodeapi.115.com.
    /// status: 0=等待扫码, 1=已扫码待确认, 2=已确认登录, -1=已过期
    pub async fn poll_qr_token(&self, uid: &str) -> Result<PollResult, ApiError> {
        let resp = self
            .http
            .get(&format!("https://qrcodeapi.115.com/get/status/?uid={}", uid))
            .header("Accept", "application/json, text/plain, */*")
            .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
            .header("Referer", "https://115.com/")
            .header("Origin", "https://115.com")
            .send()
            .await?;

        let status_code = resp.status();
        let text = resp.text().await.map_err(ApiError::Network)?;
        log::info!("QR poll response (HTTP {}): {} bytes", status_code, text.len());

        if !status_code.is_success() || text.trim().starts_with('<') {
            return Err(ApiError::Api("轮询失败: 被115服务器拦截".to_string()));
        }

        let json: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| ApiError::Parse(format!("JSON解析失败: {}", e)))?;

        let poll_status = json
            .get("data")
            .and_then(|d| d.get("status"))
            .and_then(|v| v.as_i64())
            .unwrap_or(-1) as i32;

        // When status=2, extract cookie from data.cookie
        let cookie = if poll_status == 2 {
            json.get("data")
                .and_then(|d| d.get("cookie"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        } else {
            None
        };

        Ok(PollResult { status: poll_status, cookie })
    }

    /// Validate cookie and get user info.
    /// Step 1: call check/sso to verify login and get user_id.
    /// Step 2: call my.115.com/?ct=ajax&ac=nav to get user_name and face.
    pub async fn validate_cookie(&self, cookie: &str) -> Result<UserInfo115, ApiError> {
        // Step 1: login check — verifies cookie validity, returns user_id.
        // Response: { state: 0 (success), data: { user_id: N, expire: ..., link: ... } }
        let check_resp = self
            .http
            .get("https://passportapi.115.com/app/1.0/web/1.0/check/sso")
            .header("Cookie", cookie)
            .header("Accept", "application/json, text/plain, */*")
            .header("Referer", "https://115.com/")
            .send()
            .await?;

        let check_text = check_resp.text().await.map_err(ApiError::Network)?;
        log::info!("check/sso: {} bytes, body: {}", check_text.len(), &check_text[..check_text.len().min(300)]);

        if check_text.trim().starts_with('<') {
            return Err(ApiError::Api("Cookie验证被115拦截，请检查代理".to_string()));
        }

        let check_json: serde_json::Value =
            serde_json::from_str(&check_text).map_err(|e| ApiError::Parse(format!("check/sso JSON解析失败: {}", e)))?;

        // AList LoginResp: state: 0 = success
        let state_val = check_json.get("state").and_then(|v| v.as_i64()).unwrap_or(-1);
        if state_val != 0 {
            let err = check_json.get("error").and_then(|v| v.as_str()).unwrap_or("Cookie无效或已过期");
            return Err(ApiError::Api(err.to_string()));
        }

        let user_id = check_json
            .get("data")
            .and_then(|d| d.get("user_id"))
            .and_then(|v| if v.is_number() { Some(v.to_string()) } else { v.as_str().map(|s| s.to_string()) })
            .unwrap_or_default();

        if user_id.is_empty() || user_id == "0" {
            return Err(ApiError::Api("Cookie已过期或无效".to_string()));
        }

        // Step 2: get user info — user_name, face
        let info_resp = self
            .http
            .get("https://my.115.com/?ct=ajax&ac=nav")
            .header("Cookie", cookie)
            .header("Accept", "application/json, text/plain, */*")
            .header("Referer", "https://115.com/")
            .send()
            .await?;

        let info_text = info_resp.text().await.map_err(ApiError::Network)?;
        log::info!("my.115.com/nav: {} bytes, body: {}", info_text.len(), &info_text[..info_text.len().min(500)]);

        let info_json: serde_json::Value = serde_json::from_str(&info_text).unwrap_or_default();

        let user_name = info_json
            .get("data")
            .and_then(|d| d.get("user_name"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let face = info_json
            .get("data")
            .and_then(|d| d.get("face"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        log::info!("validate_cookie success: user_id={}, user_name={}", user_id, user_name);

        Ok(UserInfo115 {
            user_id,
            user_name: if user_name.is_empty() { "115用户".to_string() } else { user_name },
            face,
        })
    }
}
