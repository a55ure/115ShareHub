//! Offline mock / record mode for 115 API.
//!
//! Controlled by env `MOCK_115_API`:
//!   - unset          → real requests, no caching
//!   - `record`       → real request, save response to disk, return response
//!   - `playback`     → read from disk only; return error on cache miss
//!
//! Cache directory defaults to `test_fixtures/` in the workspace root.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub enum MockMode {
    /// Normal operation — real HTTP requests, nothing cached.
    Off,
    /// Real request + persist the raw JSON body to disk.
    Record(PathBuf),
    /// Serve from disk only; fail if the fixture is missing.
    Playback(PathBuf),
}

impl MockMode {
    /// Read `MOCK_115_API` env var.  Recognised values (case-insensitive):
    /// `record`, `playback`, `1`, `true`, `yes`.
    pub fn from_env() -> Self {
        let val = match std::env::var("MOCK_115_API") {
            Ok(v) => v.to_lowercase(),
            Err(_) => return MockMode::Off,
        };

        let dir = match std::env::var("MOCK_115_CACHE_DIR") {
            Ok(d) => PathBuf::from(d),
            Err(_) => {
                // Default: workspace-root/test_fixtures
                let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
                p.pop(); // src-tauri
                p.pop(); // workspace root
                p.push("test_fixtures");
                p
            }
        };

        match val.as_str() {
            "record" => MockMode::Record(dir),
            "playback" => MockMode::Playback(dir),
            "1" | "true" | "yes" => MockMode::Playback(dir), // bare flag defaults to playback
            _ => MockMode::Off,
        }
    }

    pub fn is_off(&self) -> bool {
        matches!(self, MockMode::Off)
    }
}

/// Build a cache-file path for a share-snap request.
pub fn cache_path(dir: &Path, share_code: &str, receive_code: &str, cid: &str, offset: u32) -> PathBuf {
    let safe_cid = if cid == "0" { "root".to_string() } else { cid.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_") };
    let filename = format!("share_snap_{}_{}_{}_p{}.json", share_code, receive_code, safe_cid, offset);
    dir.join(filename)
}

/// Try to read a cached response body.  Returns `None` on any I/O error
/// (missing file, permission, …).
pub fn read_cache(path: &Path) -> Option<String> {
    match std::fs::read_to_string(path) {
        Ok(body) if !body.trim().is_empty() => Some(body),
        _ => None,
    }
}

/// Write a response body to the cache directory.  Creates parent dirs as needed.
pub fn write_cache(path: &Path, body: &str) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, body);
}
