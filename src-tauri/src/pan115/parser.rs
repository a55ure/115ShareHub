use super::client::{ApiError, Pan115Client};
use super::types::{ShareInfo, ShareSnapItem, ShareUserInfo};
use crate::db::Database;
use serde::Serialize;
use tauri::Emitter;

#[derive(Debug, Clone, Serialize)]
pub struct ParseProgress {
    pub share_link_id: i64,
    pub current_path: String,
    pub files_found: u64,
    pub dirs_found: u64,
}

#[derive(Debug)]
pub struct ParsedFile {
    pub file_id: String,
    pub parent_id: String,
    pub name: String,
    pub size: i64,
    pub sha1: String,
    pub is_dir: bool,
    pub file_type: String,
    pub full_path: String,
    pub depth: i32,
    pub thumbnail_url: String,
}

#[derive(Debug)]
pub struct ParseResult {
    pub share_info: Option<ShareInfo>,
    pub user_info: Option<ShareUserInfo>,
    pub total_files: u64,
    pub total_dirs: u64,
    pub total_size: i64,
}

struct DirTask {
    cid: String,
    path_prefix: String,
    depth: i32,
}

pub struct ShareLinkParser {
    client: Pan115Client,
    page_size: u32,
}

impl ShareLinkParser {
    pub fn new(client: Pan115Client, page_size: u32) -> Self {
        ShareLinkParser { client, page_size }
    }

    pub async fn parse_share_link(
        &self,
        share_code: &str,
        receive_code: &str,
        share_link_id: i64,
        app_handle: &tauri::AppHandle,
        db: &Database,
    ) -> Result<ParseResult, ApiError> {
        let warn_cb = |attempt: u32, max_retries: u32, wait_secs: u64| {
            let _ = app_handle.emit("share-link-warn", serde_json::json!({
                "share_link_id": share_link_id,
                "message": format!("被115服务器拦截，等待{}秒后重试 ({}/{})", wait_secs, attempt, max_retries),
                "attempt": attempt,
                "max_retries": max_retries,
                "wait_secs": wait_secs,
            }));
        };

        let first = self.client
            .fetch_share_snap_with_backoff(share_code, receive_code, "0", self.page_size, 0, 3, Some(&warn_cb))
            .await?;

        let data = first.data.ok_or_else(|| {
            ApiError::Api("No data in share snap response".to_string())
        })?;

        let share_info = data.shareinfo.clone();
        let user_info = data.userinfo.clone();
        let mut total_files: u64 = 0;
        let mut total_dirs: u64 = 0;
        let mut total_size: i64 = 0;

        self.emit_log(share_link_id, "info", &format!("开始解析，根目录共 {} 项", data.count), app_handle);

        let root_items = self.fetch_all_pages(share_code, receive_code, "0", Some(&warn_cb)).await?;
        for item in &root_items {
            if item.is_file != 0 { total_files += 1; total_size += item.size; }
            else { total_dirs += 1; }
        }

        self.emit_log(share_link_id, "scan", &format!("/ — {} 文件, {} 目录", 
            root_items.iter().filter(|i| i.is_file != 0).count(),
            root_items.iter().filter(|i| i.is_file == 0).count()), app_handle);

        let root_parsed = items_to_parsed(&root_items, "", 0);
        self.flush_to_db(db, share_link_id, &root_parsed);
        self.emit_progress(share_link_id, "/", total_files, total_dirs, app_handle);

        let mut stack: Vec<DirTask> = root_items
            .iter()
            .filter(|i| i.is_file == 0)
            .map(|i| DirTask { cid: i.category_id.clone(), path_prefix: i.name.clone(), depth: 1 })
            .collect();

        while let Some(task) = stack.pop() {
            self.emit_log(share_link_id, "scan", &format!("扫描目录: {}", task.path_prefix), app_handle);

            let items = match self.fetch_all_pages(share_code, receive_code, &task.cid, Some(&warn_cb)).await {
                Ok(items) => items,
                Err(e) => {
                    self.emit_log(share_link_id, "error", &format!("目录 {} 获取失败: {}", task.path_prefix, e), app_handle);
                    log::warn!("Failed to fetch dir {}: {}, skipping", task.path_prefix, e);
                    continue;
                }
            };

            let dir_count = items.iter().filter(|i| i.is_file == 0).count();
            let file_count = items.iter().filter(|i| i.is_file != 0).count();

            for item in &items {
                if item.is_file != 0 { total_files += 1; total_size += item.size; }
                else { total_dirs += 1; }
            }

            self.emit_log(share_link_id, "scan", &format!("{} — {} 文件, {} 子目录", task.path_prefix, file_count, dir_count), app_handle);

            let parsed = items_to_parsed(&items, &task.path_prefix, task.depth);
            self.flush_to_db(db, share_link_id, &parsed);
            self.emit_progress(share_link_id, &task.path_prefix, total_files, total_dirs, app_handle);

            for item in items.iter().filter(|i| i.is_file == 0) {
                stack.push(DirTask {
                    cid: item.category_id.clone(),
                    path_prefix: format!("{}/{}", task.path_prefix, item.name),
                    depth: task.depth + 1,
                });
            }
        }

        self.emit_log(share_link_id, "info", &format!("解析完成: {} 文件, {} 目录, {}", total_files, total_dirs, format_size(total_size)), app_handle);

        Ok(ParseResult { share_info, user_info, total_files, total_dirs, total_size })
    }

    fn flush_to_db(&self, db: &Database, share_link_id: i64, files: &[ParsedFile]) {
        if files.is_empty() { return; }
        let file_data: Vec<(i64, &str, &str, &str, i64, &str, bool, &str, &str, i32, &str)> = files
            .iter()
            .map(|f| (share_link_id, f.file_id.as_str(), f.parent_id.as_str(), f.name.as_str(),
                f.size, f.sha1.as_str(), f.is_dir, f.file_type.as_str(), f.full_path.as_str(),
                f.depth, f.thumbnail_url.as_str()))
            .collect();
        if let Err(e) = db.insert_files_batch(&file_data) {
            log::error!("Batch insert error: {}", e);
        }
    }

    fn emit_progress(&self, share_link_id: i64, path: &str, files: u64, dirs: u64, app_handle: &tauri::AppHandle) {
        let progress = ParseProgress { share_link_id, current_path: path.to_string(), files_found: files, dirs_found: dirs };
        let _ = app_handle.emit("share-link-progress", &progress);
    }

    fn emit_log(&self, share_link_id: i64, level: &str, message: &str, app_handle: &tauri::AppHandle) {
        let _ = app_handle.emit("share-link-log", serde_json::json!({
            "share_link_id": share_link_id,
            "level": level,
            "message": message,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }));
    }

    async fn fetch_all_pages(
        &self,
        share_code: &str,
        receive_code: &str,
        cid: &str,
        on_block: Option<&(dyn Fn(u32, u32, u64) + Sync)>,
    ) -> Result<Vec<ShareSnapItem>, ApiError> {
        let mut all_items = Vec::new();
        let mut offset: u32 = 0;
        loop {
            let resp = self.client.fetch_share_snap_with_backoff(share_code, receive_code, cid, self.page_size, offset, 3, on_block).await?;
            let data = match resp.data { Some(d) => d, None => break };
            let count = data.list.len() as u32;
            all_items.extend(data.list);
            offset += count;
            if count < self.page_size || offset as i64 >= data.count { break; }
        }
        Ok(all_items)
    }
}

fn items_to_parsed(items: &[ShareSnapItem], path_prefix: &str, depth: i32) -> Vec<ParsedFile> {
    items.iter().map(|item| {
        let full_path = if path_prefix.is_empty() { item.name.clone() } else { format!("{}/{}", path_prefix, item.name) };
        let is_dir = item.is_file == 0;
        ParsedFile {
            file_id: if is_dir { item.category_id.clone() } else { item.file_id.clone() },
            parent_id: item.parent_id.clone(),
            name: item.name.clone(),
            size: item.size,
            sha1: item.sha1.clone(),
            is_dir,
            file_type: if is_dir { "folder".to_string() } else { derive_file_type(&item.ico, &item.name) },
            full_path,
            depth,
            thumbnail_url: item.thumb_url.clone().unwrap_or_default(),
        }
    }).collect()
}

fn derive_file_type(ico: &str, filename: &str) -> String {
    match ico {
        "1" => "document", "2" => "image", "3" => "audio", "4" => "video",
        "5" => "archive", "6" => "software", "7" => "book",
        _ => return extension_based_type(filename),
    }.to_string()
}

fn extension_based_type(filename: &str) -> String {
    let lower = filename.to_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");
    let video_exts = ["mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "ts", "rmvb", "rm", "3gp", "mpg", "mpeg", "vob"];
    let audio_exts = ["mp3", "flac", "wav", "aac", "ogg", "wma", "m4a", "ape", "alac", "opus"];
    let image_exts = ["jpg", "jpeg", "png", "gif", "bmp", "webp", "svg", "ico", "tiff", "tif"];
    let doc_exts = ["pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "txt", "csv", "rtf", "odt", "ods"];
    let archive_exts = ["zip", "rar", "7z", "tar", "gz", "bz2", "xz", "zst"];
    let software_exts = ["exe", "msi", "dmg", "pkg", "deb", "rpm", "apk", "app"];
    let book_exts = ["epub", "mobi", "azw3", "fb2", "lit"];
    let iso_exts = ["iso", "img", "mdf", "nrg"];
    let subtitle_exts = ["srt", "ass", "ssa", "sub", "idx", "sup", "vtt", "lrc"];

    let t = if video_exts.contains(&ext) { "video" }
        else if audio_exts.contains(&ext) { "audio" }
        else if image_exts.contains(&ext) { "image" }
        else if doc_exts.contains(&ext) { "document" }
        else if archive_exts.contains(&ext) { "archive" }
        else if software_exts.contains(&ext) { "software" }
        else if book_exts.contains(&ext) { "book" }
        else if iso_exts.contains(&ext) { "iso" }
        else if subtitle_exts.contains(&ext) { "subtitle" }
        else { "other" };
    t.to_string()
}

fn format_size(bytes: i64) -> String {
    if bytes < 1024 { return format!("{} B", bytes); }
    let kb = bytes as f64 / 1024.0;
    if kb < 1024.0 { return format!("{:.1} KB", kb); }
    let mb = kb / 1024.0;
    if mb < 1024.0 { return format!("{:.1} MB", mb); }
    let gb = mb / 1024.0;
    format!("{:.2} GB", gb)
}
