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
            if let Ok(mut proxy) = reqwest::Proxy::all(&proxy_url) {
                if let (Some(u), Some(p)) = (&proxy_config.username, &proxy_config.password) {
                    if !u.is_empty() {
                        proxy = proxy.basic_auth(u, p);
                    }
                }
                builder = builder.proxy(proxy);
            }
        }

        let http = builder.build().expect("failed to build HTTP client");
        AuthClient { http }
    }

    /// Step 1: Get QR login token from 115 passport API.
    pub async fn get_qr_token(&self) -> Result<QrTokenResult, ApiError> {
        let resp = self
            .http
            .get("https://passportapi.115.com/app/1.0/web/1.0/qrcode/token")
            .header("Accept", "application/json, text/plain, */*")
            .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
            .header("Referer", "https://115.com/")
            .header("Origin", "https://115.com")
            .send()
            .await?;

        let status = resp.status();
        let text = resp.text().await.map_err(ApiError::Network)?;
        log::info!(
            "QR token response (HTTP {}): {} bytes, body: {}",
            status,
            text.len(),
            &text[..text.len().min(500)]
        );

        if !status.is_success() {
            let preview = &text[..text.len().min(200)];
            if preview.trim().starts_with('<') {
                return Err(ApiError::Api("获取二维码token失败: 被115服务器拦截, 请稍后再试".to_string()));
            }
            return Err(ApiError::Api(format!("获取二维码token失败: HTTP {}", status)));
        }

        if text.trim().is_empty() {
            return Err(ApiError::Parse("获取二维码token失败: 服务器返回空响应".to_string()));
        }

        if text.trim().starts_with('<') {
            return Err(ApiError::Parse("获取二维码token失败: 服务器返回HTML而非JSON (可能被WAF拦截)".to_string()));
        }

        let json: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| ApiError::Parse(format!("JSON解析失败: {}", e)))?;

        let token = json
            .get("data")
            .and_then(|d| d.get("token"))
            .or_else(|| json.get("token"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| ApiError::Parse(format!("响应中未找到token: {}", &text[..text.len().min(200)])))?;

        Ok(QrTokenResult { token })
    }

    /// Step 2: Poll QR code scan status with token.
    /// status: 0=等待扫码, 1=已扫码待确认, 2=已确认登录, -1=已过期
    pub async fn poll_qr_token(&self, token: &str) -> Result<PollResult, ApiError> {
        let resp = self
            .http
            .get(&format!(
                "https://passportapi.115.com/app/1.0/web/1.0/qrcode/poll?token={}",
                token
            ))
            .header("Accept", "application/json, text/plain, */*")
            .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
            .header("Referer", "https://115.com/")
            .header("Origin", "https://115.com")
            .send()
            .await?;

        let status = resp.status();
        let text = resp.text().await.map_err(ApiError::Network)?;
        log::info!(
            "QR poll response (HTTP {}): {} bytes, body: {}",
            status,
            text.len(),
            &text[..text.len().min(500)]
        );

        if !status.is_success() {
            return Err(ApiError::Api(format!("轮询失败: HTTP {}", status)));
        }

        if text.trim().is_empty() {
            return Err(ApiError::Parse("轮询失败: 服务器返回空响应".to_string()));
        }

        if text.trim().starts_with('<') {
            return Err(ApiError::Parse("轮询失败: 服务器返回HTML而非JSON".to_string()));
        }

        let json: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| ApiError::Parse(format!("JSON解析失败: {}", e)))?;

        // Extract status from data
        let poll_status = json
            .get("data")
            .and_then(|d| d.get("status"))
            .or_else(|| json.get("status"))
            .and_then(|v| v.as_i64())
            .unwrap_or(-1) as i32;

        // When status=2, extract cookie from response
        let cookie = if poll_status == 2 {
            json.get("data")
                .and_then(|d| d.get("cookie"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        } else {
            None
        };

        Ok(PollResult {
            status: poll_status,
            cookie,
        })
    }

    /// Validate a cookie string by fetching user info from 115.
    pub async fn validate_cookie(&self, cookie: &str) -> Result<UserInfo115, ApiError> {
        let resp = self
            .http
            .get("https://webapi.115.com/user/info")
            .header("Cookie", cookie)
            .header("Accept", "application/json, text/plain, */*")
            .header("Referer", "https://115.com/")
            .send()
            .await?;

        let status = resp.status();
        let text = resp.text().await.map_err(ApiError::Network)?;
        log::info!("validate_cookie HTTP {}: {} bytes", status, text.len());

        if !status.is_success() {
            return Err(ApiError::Api(format!("Cookie验证失败: HTTP {}", status)));
        }

        if text.trim().is_empty() {
            return Err(ApiError::Api("Cookie验证失败: 服务器返回空响应".to_string()));
        }

        if text.trim().starts_with('<') {
            log::warn!("validate_cookie: got HTML instead of JSON (WAF block)");
            return Err(ApiError::Api("Cookie验证被115拦截, 请检查代理设置或稍后再试".to_string()));
        }

        let json: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| ApiError::Parse(format!("JSON解析失败: {}", e)))?;

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
