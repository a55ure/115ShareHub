use super::mock::{self, MockMode};
use super::types::ShareSnapResponse;
use rand::Rng;
use reqwest::{Client, Proxy};
use serde::Deserialize;
use std::sync::{Arc, Mutex as StdMutex};
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

#[derive(Debug, Clone, Deserialize)]
pub struct ProxyConfig {
    pub enabled: bool,
    #[serde(rename = "proxyType")]
    pub proxy_type: String,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
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

/// A pool of HTTP clients, each potentially using a different proxy.
/// When a WAF block is detected the pool can rotate to the next client,
/// giving the request a fresh outbound IP.
pub struct ProxyPool {
    clients: Vec<Client>,
    active: Arc<StdMutex<usize>>,
    /// Human labels for logging (e.g. "direct", "proxy 1: 127.0.0.1:1080").
    labels: Vec<String>,
}

impl ProxyPool {
    /// Build a pool from proxy configs. Always includes at least one direct client.
    pub fn from_configs(configs: &[ProxyConfig]) -> Self {
        let mut clients: Vec<Client> = Vec::new();
        let mut labels: Vec<String> = Vec::new();

        // Direct client (no proxy) always available as fallback.
        match Self::build_client(None) {
            Ok(c) => {
                clients.push(c);
                labels.push("direct".to_string());
            }
            Err(e) => log::error!("Failed to build direct HTTP client: {}", e),
        }

        for cfg in configs {
            if !cfg.enabled || cfg.host.is_empty() {
                continue;
            }
            match Self::build_client(Some(cfg)) {
                Ok(c) => {
                    clients.push(c);
                    labels.push(format!("proxy: {}:{}", cfg.host, cfg.port));
                }
                Err(e) => log::error!("Failed to build proxy client for {}:{}: {}", cfg.host, cfg.port, e),
            }
        }

        ProxyPool {
            clients,
            active: Arc::new(StdMutex::new(0)),
            labels,
        }
    }

    fn build_client(proxy_cfg: Option<&ProxyConfig>) -> Result<Client, reqwest::Error> {
        let mut builder = Client::builder()
            .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
            .timeout(Duration::from_secs(30));

        if let Some(cfg) = proxy_cfg {
            let proxy_url = format!("{}://{}:{}", cfg.proxy_type, cfg.host, cfg.port);
            let mut proxy = Proxy::all(&proxy_url)?;
            if let (Some(u), Some(p)) = (&cfg.username, &cfg.password) {
                if !u.is_empty() {
                    proxy = proxy.basic_auth(u, p);
                }
            }
            builder = builder.proxy(proxy);
        }

        builder.build()
    }

    pub fn active_client(&self) -> &Client {
        let idx = *self.active.lock().expect("proxy pool lock poisoned");
        &self.clients[idx.min(self.clients.len().saturating_sub(1))]
    }

    pub fn rotate(&self) {
        let mut idx = self.active.lock().expect("proxy pool lock poisoned");
        let old = *idx;
        *idx = if self.clients.len() > 1 { (*idx + 1) % self.clients.len() } else { 0 };
        log::info!(
            "Proxy rotated: {} -> {}",
            self.labels.get(old).map(|s| s.as_str()).unwrap_or("?"),
            self.labels.get(*idx).map(|s| s.as_str()).unwrap_or("?")
        );
    }

    /// Number of clients in the pool.
    pub fn len(&self) -> usize {
        self.clients.len()
    }
}

/// Rate-limited 115 API client with randomized intervals and proxy rotation.
pub struct Pan115Client {
    proxy_pool: ProxyPool,
    last_request: Arc<Mutex<std::time::Instant>>,
    min_interval_ms: u64,
    max_interval_ms: u64,
    cookie: Option<String>,
    mock_mode: MockMode,
}

impl Pan115Client {
    pub fn new(requests_per_second: u32) -> Self {
        Self::with_proxy_pool(requests_per_second, &[ProxyConfig::default()])
    }

    pub fn with_proxy(requests_per_second: u32, proxy_config: &ProxyConfig) -> Self {
        Self::with_proxy_pool(requests_per_second, &[proxy_config.clone()])
    }

    pub fn with_proxy_pool(requests_per_second: u32, proxy_configs: &[ProxyConfig]) -> Self {
        let rps = requests_per_second.clamp(1, 10);
        let base = 1000.0 / rps as f64;
        let min_interval_ms = (base * 0.7).max(300.0) as u64;
        let max_interval_ms = ((base * 1.3).max(min_interval_ms as f64 + 100.0)) as u64;

        let proxy_pool = ProxyPool::from_configs(proxy_configs);
        let mock_mode = MockMode::from_env();
        log::info!(
            "Pan115Client: {} proxies, mock_mode={:?}, rate={}rps",
            proxy_pool.len(),
            mock_mode,
            rps
        );

        Pan115Client {
            proxy_pool,
            last_request: Arc::new(Mutex::new(std::time::Instant::now())),
            min_interval_ms,
            max_interval_ms,
            cookie: None,
            mock_mode,
        }
    }

    /// Attach a login cookie. Logged-in requests usually get higher rate quotas from 115.
    pub fn with_cookie(mut self, cookie: Option<String>) -> Self {
        self.cookie = cookie;
        self
    }

    /// Raw HTTP GET with rate-limit delay. Returns the response body text.
    async fn do_fetch(&self, url: &str, referer: &str) -> Result<String, ApiError> {
        // Rate limit — same lock as before
        {
            let mut last = self.last_request.lock().await;
            let delay_ms = rand::thread_rng().gen_range(self.min_interval_ms..=self.max_interval_ms);
            let delay = Duration::from_millis(delay_ms);
            let elapsed = last.elapsed();
            if elapsed < delay {
                tokio::time::sleep(delay - elapsed).await;
            }
            *last = std::time::Instant::now();
        }

        let mut req = self
            .proxy_pool
            .active_client()
            .get(url)
            .header("Referer", referer)
            .header("Accept", "application/json, text/plain, */*")
            .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8");

        if let Some(ref cookie) = self.cookie {
            req = req.header("Cookie", cookie);
        }

        let resp = req.send().await?;
        let status = resp.status();
        let text = resp.text().await.map_err(ApiError::Network)?;

        log::info!("115 API status: {}, body: {} bytes", status, text.len());

        if !status.is_success() {
            let preview = &text[..text.len().min(100)];
            if preview.trim().starts_with('<') {
                log::warn!("115 returned HTML (WAF block), status: {}", status);
                return Err(ApiError::Api(format!("被115服务器拦截 (HTTP {}), 请稍后再试", status)));
            }
            return Err(ApiError::Api(format!("HTTP {}: {}", status, &text[..text.len().min(200)])));
        }

        if text.trim().starts_with('<') {
            return Err(ApiError::Parse("Got HTML instead of JSON".to_string()));
        }

        Ok(text)
    }

    /// Parse a raw JSON response body into a `ShareSnapResponse`.
    fn parse_response(text: &str) -> Result<ShareSnapResponse, ApiError> {
        let body: ShareSnapResponse = serde_json::from_str(text).map_err(|e| {
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

    pub async fn fetch_share_snap(
        &self,
        share_code: &str,
        receive_code: &str,
        cid: &str,
        limit: u32,
        offset: u32,
    ) -> Result<ShareSnapResponse, ApiError> {
        let referer = format!(
            "https://115cdn.com/s/{}?password={}&",
            share_code, receive_code
        );

        let url = format!(
            "https://webapi.115.com/share/snap?share_code={}&receive_code={}&cid={}&limit={}&offset={}",
            share_code, receive_code, cid, limit, offset
        );

        // ── Mock / record mode ──────────────────────────────
        match &self.mock_mode {
            MockMode::Playback(dir) => {
                let path = mock::cache_path(dir, share_code, receive_code, cid, offset);
                let cached = mock::read_cache(&path)
                    .ok_or_else(|| ApiError::Api(format!("Mock playback: no fixture at {}", path.display())))?;
                log::info!("Mock playback: {} ({} bytes)", path.display(), cached.len());
                return Self::parse_response(&cached);
            }
            MockMode::Record(dir) => {
                let path = mock::cache_path(dir, share_code, receive_code, cid, offset);
                // Fall through to real request; save response after.
                let result = self.do_fetch(&url, &referer).await;
                if let Ok(ref body) = result {
                    mock::write_cache(&path, body);
                    log::info!("Mock record: saved {} ({} bytes)", path.display(), body.len());
                }
                return result.and_then(|text| Self::parse_response(&text));
            }
            MockMode::Off => {}
        }

        // ── Normal path ─────────────────────────────────────
        let text = self.do_fetch(&url, &referer).await?;
        Self::parse_response(&text)
    }

    /// Fetch with exponential backoff on WAF blocks. Retries up to `max_retries` times.
    pub async fn fetch_share_snap_with_backoff(
        &self,
        share_code: &str,
        receive_code: &str,
        cid: &str,
        limit: u32,
        offset: u32,
        max_retries: u32,
    ) -> Result<ShareSnapResponse, ApiError> {
        let mut attempt = 0;
        loop {
            match self.fetch_share_snap(share_code, receive_code, cid, limit, offset).await {
                Ok(resp) => return Ok(resp),
                Err(ApiError::Api(msg)) if msg.contains("被115服务器拦截") => {
                    attempt += 1;
                    if attempt > max_retries {
                        return Err(ApiError::Api(format!("WAF拦截, 已重试{}次仍失败", max_retries)));
                    }
                    // Rotate to next proxy before retrying
                    if self.proxy_pool.len() > 1 {
                        self.proxy_pool.rotate();
                    }
                    let wait = std::time::Duration::from_secs(15u64.saturating_mul(attempt as u64));
                    log::warn!("WAF block detected, backoff {}/{}: waiting {}s", attempt, max_retries, wait.as_secs());
                    tokio::time::sleep(wait).await;
                }
                Err(e) => return Err(e),
            }
        }
    }
}
