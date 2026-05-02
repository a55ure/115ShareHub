use super::types::ShareSnapResponse;
use reqwest::Client;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::Mutex;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("API error: {0}")]
    Api(String),
    #[error("Parse error: {0}")]
    Parse(String),
}

/// Rate-limited 115 API client.
/// Uses a mutex + timestamp to enforce a minimum interval between requests,
/// ensuring they are truly spaced out rather than bursting.
pub struct Pan115Client {
    http: Client,
    last_request: Arc<Mutex<std::time::Instant>>,
    min_interval: Duration,
}

impl Pan115Client {
    pub fn new(requests_per_second: u32) -> Self {
        let interval = Duration::from_millis(1000 / requests_per_second.max(1) as u64);
        let http = Client::builder()
            .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
            .timeout(Duration::from_secs(30))
            .build()
            .expect("failed to build HTTP client");
        Pan115Client {
            http,
            last_request: Arc::new(Mutex::new(std::time::Instant::now())),
            min_interval: interval,
        }
    }

    pub async fn fetch_share_snap(
        &self,
        share_code: &str,
        receive_code: &str,
        cid: &str,
        limit: u32,
        offset: u32,
    ) -> Result<ShareSnapResponse, ApiError> {
        {
            let mut last = self.last_request.lock().await;
            let elapsed = last.elapsed();
            if elapsed < self.min_interval {
                tokio::time::sleep(self.min_interval - elapsed).await;
            }
            *last = std::time::Instant::now();
        }

        let referer = format!(
            "https://115cdn.com/s/{}?password={}&",
            share_code, receive_code
        );

        let url = format!(
            "https://webapi.115.com/share/snap?share_code={}&receive_code={}&cid={}&limit={}&offset={}",
            share_code, receive_code, cid, limit, offset
        );

        let resp = self
            .http
            .get(&url)
            .header("Referer", &referer)
            .header("Accept", "application/json, text/plain, */*")
            .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
            .send()
            .await?;

        let status = resp.status();
        let text = resp.text().await.map_err(ApiError::Network)?;

        log::info!("115 API status: {}, body: {} bytes, path: cid={}", status, text.len(), cid);

        if !status.is_success() {
            let preview = &text[..text.len().min(100)];
            if preview.trim().starts_with('<') {
                log::warn!("115 returned HTML (WAF block), status: {}", status);
                return Err(ApiError::Api(format!("被115服务器拦截 (HTTP {}), 请稍后再试或降低并发", status)));
            }
            return Err(ApiError::Api(format!("HTTP {}: {}", status, &text[..text.len().min(200)])));
        }

        if text.trim().starts_with('<') {
            return Err(ApiError::Parse("Got HTML instead of JSON".to_string()));
        }

        let body: ShareSnapResponse = serde_json::from_str(&text).map_err(|e| {
            let preview = &text[..text.len().min(500)];
            log::error!("JSON parse error: {}, preview: {}", e, preview);
            ApiError::Parse(format!("JSON decode error: {}", e))
        })?;

        if !body.state {
            let err_msg = if body.error.is_empty() {
                format!("API error (errno={})", body.errno)
            } else {
                body.error.clone()
            };
            return Err(ApiError::Api(err_msg));
        }

        Ok(body)
    }
}
