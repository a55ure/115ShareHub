use super::types::ShareSnapResponse;
use reqwest::Client;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("API error: {0}")]
    Api(String),
    #[error("Parse error: {0}")]
    Parse(String),
}

pub struct Pan115Client {
    http: Client,
    delay: Duration,
}

impl Pan115Client {
    pub fn new(rate_limit_rps: u32) -> Self {
        let delay = Duration::from_millis(1000 / rate_limit_rps.max(1) as u64);
        let http = Client::builder()
            .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .timeout(Duration::from_secs(30))
            .build()
            .expect("failed to build HTTP client");
        Pan115Client { http, delay }
    }

    pub async fn fetch_share_snap(
        &self,
        share_code: &str,
        receive_code: &str,
        cid: &str,
        limit: u32,
        offset: u32,
    ) -> Result<ShareSnapResponse, ApiError> {
        tokio::time::sleep(self.delay).await;

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
            .send()
            .await?;

        let status = resp.status();
        let text = resp.text().await.map_err(|e| ApiError::Network(e))?;

        log::info!("115 API response status: {}, body length: {}", status, text.len());
        log::debug!("115 API response body (first 2000 chars): {}", &text[..text.len().min(2000)]);

        if !status.is_success() {
            return Err(ApiError::Api(format!("HTTP {}: {}", status, &text[..text.len().min(500)])));
        }

        if text.trim().starts_with('<') {
            return Err(ApiError::Parse(format!("Got HTML instead of JSON (length {}), API may require auth or the link is invalid", text.len())));
        }

        let body: ShareSnapResponse = serde_json::from_str(&text).map_err(|e| {
            let preview = &text[..text.len().min(500)];
            log::error!("JSON parse error: {}, body preview: {}", e, preview);
            ApiError::Parse(format!("JSON decode error: {} - body starts with: {}", e, &preview[..preview.len().min(200)]))
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
