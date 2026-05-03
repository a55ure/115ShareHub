use super::mock::{self, MockMode};
use super::types::ShareSnapResponse;
use rand::seq::SliceRandom;
use rand::Rng;
use reqwest::{Client, Proxy};
use serde::Deserialize;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use thiserror::Error;
use tokio::sync::Mutex;

const USER_AGENTS: &[&str] = &[
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.2 Safari/605.1.15",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.6 Safari/605.1.15",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:133.0) Gecko/20100101 Firefox/133.0",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:133.0) Gecko/20100101 Firefox/133.0",
];

/// Build a Chrome-like `sec-ch-ua` header value from a UA string.
fn sec_ch_ua_from_ua(ua: &str) -> (&'static str, &'static str, &'static str) {
    if ua.contains("Chrome/131") {
        (
            "\"Google Chrome\";v=\"131\", \"Chromium\";v=\"131\", \"Not_A Brand\";v=\"24\"",
            "\"Google Chrome\";v=\"131\", \"Chromium\";v=\"131\", \"Not_A Brand\";v=\"24\"",
            "macOS",
        )
    } else if ua.contains("Chrome/130") {
        (
            "\"Google Chrome\";v=\"130\", \"Chromium\";v=\"130\", \"Not?A_Brand\";v=\"99\"",
            "\"Google Chrome\";v=\"130\", \"Chromium\";v=\"130\", \"Not?A_Brand\";v=\"99\"",
            "macOS",
        )
    } else if ua.contains("Safari/") && !ua.contains("Chrome") {
        (
            "\"Safari\";v=\"18\", \"Not=A?Brand\";v=\"8\"",
            "\"Safari\"",
            "macOS",
        )
    } else {
        (
            "\"Not A(Brand\";v=\"8\", \"Chromium\";v=\"131\"",
            "\"Chromium\";v=\"131\"",
            "Windows",
        )
    }
}

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
    /// Per-client User-Agent strings (rotated on WAF block).
    user_agents: Vec<String>,
}

impl ProxyPool {
    /// Build a pool from proxy configs. Always includes at least one direct client.
    pub fn from_configs(configs: &[ProxyConfig]) -> Self {
        let mut clients: Vec<Client> = Vec::new();
        let mut labels: Vec<String> = Vec::new();
        let mut user_agents: Vec<String> = Vec::new();
        let mut rng = rand::thread_rng();

        // Direct client (no proxy) always available as fallback.
        let ua = USER_AGENTS.choose(&mut rng).unwrap_or(&USER_AGENTS[0]);
        match Self::build_client(None, ua) {
            Ok(c) => {
                clients.push(c);
                labels.push("direct".to_string());
                user_agents.push(ua.to_string());
            }
            Err(e) => log::error!("Failed to build direct HTTP client: {}", e),
        }

        for cfg in configs {
            if !cfg.enabled || cfg.host.is_empty() {
                continue;
            }
            let ua = USER_AGENTS.choose(&mut rng).unwrap_or(&USER_AGENTS[0]);
            match Self::build_client(Some(cfg), ua) {
                Ok(c) => {
                    clients.push(c);
                    labels.push(format!("proxy: {}:{}", cfg.host, cfg.port));
                    user_agents.push(ua.to_string());
                }
                Err(e) => log::error!("Failed to build proxy client for {}:{}: {}", cfg.host, cfg.port, e),
            }
        }

        ProxyPool {
            clients,
            active: Arc::new(StdMutex::new(0)),
            labels,
            user_agents,
        }
    }

    fn build_client(proxy_cfg: Option<&ProxyConfig>, user_agent: &str) -> Result<Client, reqwest::Error> {
        let mut builder = Client::builder()
            .user_agent(user_agent)
            .timeout(Duration::from_secs(30))
            .gzip(true)
            .brotli(true);

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
            "Proxy rotated: {} -> {} (UA: {})",
            self.labels.get(old).map(|s| s.as_str()).unwrap_or("?"),
            self.labels.get(*idx).map(|s| s.as_str()).unwrap_or("?"),
            self.user_agents.get(*idx).map(|s| &s[..s.len().min(50)]).unwrap_or("?"),
        );
    }

    /// Get the User-Agent string for the currently active client.
    pub fn active_user_agent(&self) -> &str {
        let idx = *self.active.lock().expect("proxy pool lock poisoned");
        self.user_agents.get(idx).map(|s| s.as_str()).unwrap_or(USER_AGENTS[0])
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
        // Random delay between 1.0s and 4.0s to avoid WAF detection
        let min_interval_ms = 1000u64;
        let max_interval_ms = 4000u64;

        let proxy_pool = ProxyPool::from_configs(proxy_configs);
        let mock_mode = MockMode::from_env();
        log::info!(
            "Pan115Client: {} proxies, mock_mode={:?}, rate={}rps, delay={}~{}ms",
            proxy_pool.len(),
            mock_mode,
            rps,
            min_interval_ms,
            max_interval_ms
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

        let ua = self.proxy_pool.active_user_agent();
        let (sec_ch_ua, _, platform) = sec_ch_ua_from_ua(ua);
        let is_safari = ua.contains("Safari/") && !ua.contains("Chrome");

        let mut req = self
            .proxy_pool
            .active_client()
            .get(url)
            .header("Referer", referer)
            .header("Accept", "application/json, text/plain, */*")
            .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
            .header("Accept-Encoding", "gzip, deflate, br")
            .header("Connection", "keep-alive")
            .header("sec-ch-ua-mobile", "?0")
            .header("sec-ch-ua-platform", format!("\"{}\"", platform));

        // Safari doesn't send sec-ch-ua headers
        if !is_safari {
            req = req
                .header("sec-ch-ua", sec_ch_ua)
                .header("sec-fetch-dest", "empty")
                .header("sec-fetch-mode", "cors")
                .header("sec-fetch-site", "same-site");
        }

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
            // 405 often means endpoint changed or WAF rejection
            if status.as_u16() == 405 {
                log::warn!("115 returned 405 Method Not Allowed — possible WAF or endpoint change");
                return Err(ApiError::Api(format!("115 API返回405, 可能被拦截或接口变更")));
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

        let params = format!(
            "share_code={}&receive_code={}&cid={}&limit={}&offset={}",
            share_code, receive_code, cid, limit, offset
        );

        // Primary + fallback endpoints to work around WAF blocks
        let endpoints = [
            format!("https://webapi.115.com/share/snap?{}", params),
            format!("https://proapi.115.com/share/snap?{}", params),
        ];

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
                let result = self.do_fetch(&endpoints[0], &referer).await;
                if let Ok(ref body) = result {
                    mock::write_cache(&path, body);
                    log::info!("Mock record: saved {} ({} bytes)", path.display(), body.len());
                }
                return result.and_then(|text| Self::parse_response(&text));
            }
            MockMode::Off => {}
        }

        // ── Normal path: try primary, fallback on 405 ──────
        let mut last_err = None;
        for (i, url) in endpoints.iter().enumerate() {
            match self.do_fetch(url, &referer).await {
                Ok(text) => return Self::parse_response(&text),
                Err(ApiError::Api(ref msg)) if msg.contains("405") && i + 1 < endpoints.len() => {
                    log::warn!("Endpoint {} returned 405, trying fallback: {}", url, endpoints[i + 1]);
                    last_err = Some(Err(ApiError::Api(msg.clone())));
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
        last_err.unwrap_or_else(|| Err(ApiError::Api("所有API端点均失败".to_string())))
    }

    /// Fetch with exponential backoff on WAF blocks. Retries up to `max_retries` times.
    /// `on_block` is called on each WAF block with (attempt, max_retries, wait_secs).
    pub async fn fetch_share_snap_with_backoff(
        &self,
        share_code: &str,
        receive_code: &str,
        cid: &str,
        limit: u32,
        offset: u32,
        max_retries: u32,
        on_block: Option<&(dyn Fn(u32, u32, u64) + Sync)>,
    ) -> Result<ShareSnapResponse, ApiError> {
        let mut attempt = 0;
        loop {
            match self.fetch_share_snap(share_code, receive_code, cid, limit, offset).await {
                Ok(resp) => return Ok(resp),
                Err(ApiError::Api(msg)) if msg.contains("被115服务器拦截") || msg.contains("405") => {
                    attempt += 1;
                    if attempt > max_retries {
                        return Err(ApiError::Api(format!("WAF拦截, 已重试{}次仍失败", max_retries)));
                    }
                    // Rotate to next proxy before retrying
                    self.proxy_pool.rotate();
                    // Exponential backoff with jitter: base 15s * attempt, ±30% jitter
                    let base_secs = 15u64.saturating_mul(attempt as u64);
                    let jitter = rand::thread_rng().gen_range(0.7..=1.3);
                    let wait_secs = (base_secs as f64 * jitter).max(10.0) as u64;
                    log::warn!(
                        "WAF block detected, backoff {}/{}: waiting {}s (base={}s, jitter={:.2})",
                        attempt, max_retries, wait_secs, base_secs, jitter
                    );
                    if let Some(cb) = on_block {
                        cb(attempt, max_retries, wait_secs);
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(wait_secs)).await;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Save a shared file to the logged-in user's 115 cloud.
    /// POST /share/receive — cid parameter IS the target folder (not the share directory).
    /// When cid is "0" or omitted, the file goes to root.
    /// Ref: TgtoDrive uses cid=pid to specify destination.
    pub async fn receive_share_file(
        &self,
        share_code: &str,
        receive_code: &str,
        file_id: &str,
        _cid: &str,       // share directory cid — not used in receive API
        target_cid: &str, // destination folder in user's cloud
    ) -> Result<(), ApiError> {
        let referer = format!(
            "https://115cdn.com/s/{}?password={}&",
            share_code, receive_code
        );

        // The /share/receive API uses 'cid' as the target folder.
        // If target_cid is "0" (root), we can omit cid entirely.
        let use_target = target_cid != "0" && !target_cid.is_empty();
        let mut form_vec: Vec<(&str, &str)> = vec![
            ("share_code", share_code),
            ("receive_code", receive_code),
            ("file_id", file_id),
        ];
        if use_target {
            form_vec.push(("cid", target_cid));
        }

        let ua = self.proxy_pool.active_user_agent();
        let (sec_ch_ua, _, platform) = sec_ch_ua_from_ua(ua);
        let is_safari = ua.contains("Safari/") && !ua.contains("Chrome");

        let mut req = self
            .proxy_pool.active_client()
            .post("https://webapi.115.com/share/receive")
            .form(&form_vec)
            .header("Referer", &referer)
            .header("Accept", "application/json, text/plain, */*")
            .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
            .header("Accept-Encoding", "gzip, deflate, br")
            .header("sec-ch-ua-mobile", "?0")
            .header("sec-ch-ua-platform", format!("\"{}\"", platform));
        if !is_safari {
            req = req
                .header("sec-ch-ua", sec_ch_ua)
                .header("sec-fetch-dest", "empty")
                .header("sec-fetch-mode", "cors")
                .header("sec-fetch-site", "same-site");
        }
        if let Some(ref cookie) = self.cookie {
            req = req.header("Cookie", cookie);
        }
        self.rate_limit_wait().await;
        let resp = req.send().await?;
        let status = resp.status();
        let text = resp.text().await.map_err(ApiError::Network)?;
        log::info!("receive HTTP {}: {} bytes, to_cid={}", status, text.len(), if use_target { target_cid } else { "root" });

        if !status.is_success() || text.trim().starts_with('<') {
            return Err(ApiError::Api("转存接收失败".to_string()));
        }
        let json: serde_json::Value =
            serde_json::from_str(&text).map_err(|_| ApiError::Parse("转存响应解析失败".to_string()))?;
        let state_ok = json.get("state").and_then(|v| v.as_bool()).unwrap_or(false);
        if !state_ok {
            let err = json.get("error").and_then(|v| v.as_str()).unwrap_or("未知错误");
            return Err(ApiError::Api(format!("转存接收失败: {}", err)));
        }

        log::info!("receive success: file_id={} -> cid={}", file_id, if use_target { target_cid } else { "0 (root)" });
        Ok(())
    }

    /// Batch-receive multiple shared files in one call.
    /// Sends comma-separated file_ids to the same /share/receive endpoint.
    pub async fn receive_share_batch(
        &self,
        share_code: &str,
        receive_code: &str,
        file_ids: &[String],
        target_cid: &str,
    ) -> Result<(), ApiError> {
        let referer = format!(
            "https://115cdn.com/s/{}?password={}&",
            share_code, receive_code
        );

        let ids_str = file_ids.join(",");
        let use_target = target_cid != "0" && !target_cid.is_empty();
        let mut form_vec: Vec<(&str, &str)> = vec![
            ("share_code", share_code),
            ("receive_code", receive_code),
            ("file_id", &ids_str),
        ];
        if use_target {
            form_vec.push(("cid", target_cid));
        }

        let ua = self.proxy_pool.active_user_agent();
        let (sec_ch_ua, _, platform) = sec_ch_ua_from_ua(ua);
        let is_safari = ua.contains("Safari/") && !ua.contains("Chrome");

        let mut req = self
            .proxy_pool.active_client()
            .post("https://webapi.115.com/share/receive")
            .form(&form_vec)
            .header("Referer", &referer)
            .header("Accept", "application/json, text/plain, */*")
            .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
            .header("Accept-Encoding", "gzip, deflate, br")
            .header("sec-ch-ua-mobile", "?0")
            .header("sec-ch-ua-platform", format!("\"{}\"", platform));
        if !is_safari {
            req = req
                .header("sec-ch-ua", sec_ch_ua)
                .header("sec-fetch-dest", "empty")
                .header("sec-fetch-mode", "cors")
                .header("sec-fetch-site", "same-site");
        }
        if let Some(ref cookie) = self.cookie {
            req = req.header("Cookie", cookie);
        }
        self.rate_limit_wait().await;
        let resp = req.send().await?;
        let status = resp.status();
        let text = resp.text().await.map_err(ApiError::Network)?;
        log::info!("receive_batch HTTP {}: {} files, to_cid={}", status, file_ids.len(), if use_target { target_cid } else { "root" });

        if !status.is_success() || text.trim().starts_with('<') {
            return Err(ApiError::Api("批量转存失败".to_string()));
        }
        let json: serde_json::Value =
            serde_json::from_str(&text).map_err(|_| ApiError::Parse("批量转存响应解析失败".to_string()))?;
        let state_ok = json.get("state").and_then(|v| v.as_bool()).unwrap_or(false);
        if !state_ok {
            let err = json.get("error").and_then(|v| v.as_str()).unwrap_or("未知错误");
            return Err(ApiError::Api(format!("批量转存失败: {}", err)));
        }

        log::info!("receive_batch success: {} files -> cid={}", file_ids.len(), if use_target { target_cid } else { "0 (root)" });
        Ok(())
    }

    async fn rate_limit_wait(&self) {
        let mut last = self.last_request.lock().await;
        let delay_ms = rand::thread_rng().gen_range(self.min_interval_ms..=self.max_interval_ms);
        let delay = Duration::from_millis(delay_ms);
        let elapsed = last.elapsed();
        if elapsed < delay {
            tokio::time::sleep(delay - elapsed).await;
        }
        *last = std::time::Instant::now();
    }

    /// List the logged-in user's own folders (for choosing a destination).
    /// Calls GET https://webapi.115.com/files?aid=1&cid={cid}&show_dir=1&format=json
    pub async fn fetch_user_folders(
        &self,
        cid: &str,
    ) -> Result<Vec<UserFolder>, ApiError> {
        let url = format!(
            "https://webapi.115.com/files?aid=1&cid={}&limit=1150&offset=0&show_dir=1&format=json",
            cid
        );

        let ua = self.proxy_pool.active_user_agent();
        let (sec_ch_ua, _, platform) = sec_ch_ua_from_ua(ua);
        let is_safari = ua.contains("Safari/") && !ua.contains("Chrome");

        let mut req = self
            .proxy_pool
            .active_client()
            .get(&url)
            .header("Accept", "application/json, text/plain, */*")
            .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
            .header("Accept-Encoding", "gzip, deflate, br")
            .header("Referer", "https://115.com/")
            .header("sec-ch-ua-mobile", "?0")
            .header("sec-ch-ua-platform", format!("\"{}\"", platform));
        if !is_safari {
            req = req
                .header("sec-ch-ua", sec_ch_ua)
                .header("sec-fetch-dest", "empty")
                .header("sec-fetch-mode", "cors")
                .header("sec-fetch-site", "same-origin");
        }
        if let Some(ref cookie) = self.cookie {
            req = req.header("Cookie", cookie);
        }

        // Rate limit
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

        let resp = req.send().await?;
        let text = resp.text().await.map_err(ApiError::Network)?;
        log::info!("fetch_user_folders cid={}: {} bytes", cid, text.len());

        if text.trim().starts_with('<') {
            return Err(ApiError::Api("获取目录被115拦截".to_string()));
        }

        let json: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| ApiError::Parse(format!("JSON解析失败: {}", e)))?;

        if !json.get("state").and_then(|v| v.as_bool()).unwrap_or(false) {
            let err = json.get("error").and_then(|v| v.as_str()).unwrap_or("未知错误");
            return Err(ApiError::Api(format!("获取目录失败: {}", err)));
        }

        let folders: Vec<UserFolder> = json
            .get("data")
            .and_then(|d| d.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|f| {
                        let is_dir = f.get("fc").and_then(|v| v.as_str()).map(|s| s == "0").unwrap_or(false)
                            || f.get("fid").is_none()
                            || f.get("fid").and_then(|v| v.as_str()).map(|s| s == "0").unwrap_or(false);
                        if !is_dir { return None; }
                        let cid = if let Some(s) = f.get("cid").and_then(|v| v.as_str()) {
                            s.to_string()
                        } else if let Some(n) = f.get("cid").and_then(|v| v.as_i64()) {
                            n.to_string()
                        } else {
                            String::new()
                        };
                        if cid.is_empty() || cid == "0" { return None; }
                        Some(UserFolder {
                            cid: cid.to_string(),
                            name: f.get("n").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(folders)
    }
}

/// A folder in the user's 115 cloud.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UserFolder {
    pub cid: String,
    pub name: String,
}
