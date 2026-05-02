use super::client::{ApiError, Pan115Client};
use super::types::{ShareInfo, ShareSnapItem, ShareUserInfo};
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
    pub files: Vec<ParsedFile>,
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

    pub async fn parse_share_link<F>(
        &self,
        share_code: &str,
        receive_code: &str,
        share_link_id: i64,
        progress_callback: &F,
        app_handle: &tauri::AppHandle,
    ) -> Result<ParseResult, ApiError>
    where
        F: Fn(ParseProgress),
    {
        let first = self
            .client
            .fetch_share_snap(share_code, receive_code, "0", self.page_size, 0)
            .await?;

        let data = first.data.ok_or_else(|| {
            ApiError::Api("No data in share snap response".to_string())
        })?;

        let share_info = data.shareinfo.clone();
        let user_info = data.userinfo.clone();

        let mut all_files = Vec::new();
        let mut total_files: u64 = 0;
        let mut total_dirs: u64 = 0;
        let mut total_size: i64 = 0;

        let root_items = self
            .fetch_all_pages(share_code, receive_code, "0")
            .await?;

        for item in &root_items {
            if item.is_file != 0 {
                total_files += 1;
                total_size += item.size;
            } else {
                total_dirs += 1;
            }
        }

        let root_parsed = self.items_to_parsed(&root_items, "", 0);
        all_files.extend(root_parsed);

        self.emit_progress(
            share_link_id,
            "/",
            total_files,
            total_dirs,
            progress_callback,
            app_handle,
        );

        let mut stack: Vec<DirTask> = root_items
            .iter()
            .filter(|i| i.is_file == 0)
            .map(|i| DirTask {
                cid: i.category_id.clone(),
                path_prefix: i.name.clone(),
                depth: 1,
            })
            .collect();

        while let Some(task) = stack.pop() {
            let items = self
                .fetch_all_pages(share_code, receive_code, &task.cid)
                .await?;

            for item in &items {
                if item.is_file != 0 {
                    total_files += 1;
                    total_size += item.size;
                } else {
                    total_dirs += 1;
                }
            }

            let parsed = self.items_to_parsed(&items, &task.path_prefix, task.depth);
            all_files.extend(parsed);

            self.emit_progress(
                share_link_id,
                &task.path_prefix,
                total_files,
                total_dirs,
                progress_callback,
                app_handle,
            );

            for item in items.iter().filter(|i| i.is_file == 0) {
                let sub_path = if task.path_prefix.is_empty() {
                    item.name.clone()
                } else {
                    format!("{}/{}", task.path_prefix, item.name)
                };
                stack.push(DirTask {
                    cid: item.category_id.clone(),
                    path_prefix: sub_path,
                    depth: task.depth + 1,
                });
            }
        }

        Ok(ParseResult {
            files: all_files,
            share_info,
            user_info,
            total_files,
            total_dirs,
            total_size,
        })
    }

    fn emit_progress<F>(
        &self,
        share_link_id: i64,
        current_path: &str,
        files_found: u64,
        dirs_found: u64,
        progress_callback: &F,
        app_handle: &tauri::AppHandle,
    ) where
        F: Fn(ParseProgress),
    {
        let progress = ParseProgress {
            share_link_id,
            current_path: current_path.to_string(),
            files_found,
            dirs_found,
        };
        progress_callback(progress.clone());
        let _ = app_handle.emit("share-link-progress", &progress);
    }

    async fn fetch_all_pages(
        &self,
        share_code: &str,
        receive_code: &str,
        cid: &str,
    ) -> Result<Vec<ShareSnapItem>, ApiError> {
        let mut all_items = Vec::new();
        let mut offset: u32 = 0;

        loop {
            let resp = self
                .client
                .fetch_share_snap(share_code, receive_code, cid, self.page_size, offset)
                .await?;

            let data = match resp.data {
                Some(d) => d,
                None => break,
            };

            let count = data.list.len() as u32;
            all_items.extend(data.list);
            offset += count;

            if count < self.page_size || offset as i64 >= data.count {
                break;
            }
        }

        Ok(all_items)
    }

    fn items_to_parsed(&self, items: &[ShareSnapItem], path_prefix: &str, depth: i32) -> Vec<ParsedFile> {
        items
            .iter()
            .map(|item| {
                let full_path = if path_prefix.is_empty() {
                    item.name.clone()
                } else {
                    format!("{}/{}", path_prefix, item.name)
                };
                let is_dir = item.is_file == 0;
                let file_id = if is_dir {
                    item.category_id.clone()
                } else {
                    item.file_id.clone()
                };
                ParsedFile {
                    file_id,
                    parent_id: item.parent_id.clone(),
                    name: item.name.clone(),
                    size: item.size,
                    sha1: item.sha1.clone(),
                    is_dir,
                    file_type: derive_file_type(&item.ico, &item.name),
                    full_path,
                    depth,
                    thumbnail_url: item.thumb_url.clone().unwrap_or_default(),
                }
            })
            .collect()
    }
}

fn derive_file_type(ico: &str, filename: &str) -> String {
    match ico {
        "1" => "document".to_string(),
        "2" => "image".to_string(),
        "3" => "audio".to_string(),
        "4" => "video".to_string(),
        "5" => "archive".to_string(),
        "6" => "software".to_string(),
        "7" => "book".to_string(),
        _ => extension_based_type(filename),
    }
}

fn extension_based_type(filename: &str) -> String {
    let lower = filename.to_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");

    let video_exts = ["mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "ts", "rmvb", "rm", "3gp", "mpg", "mpeg", "vob"];
    let audio_exts = ["mp3", "flac", "wav", "aac", "ogg", "wma", "m4a", "ape", "alac", "opus"];
    let image_exts = ["jpg", "jpeg", "png", "gif", "bmp", "webp", "svg", "ico", "tiff", "tif"];
    let doc_exts = ["pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "txt", "csv", "rtf", "odt", "ods"];
    let archive_exts = ["zip", "rar", "7z", "tar", "gz", "bz2", "xz", "zst", "iso"];
    let software_exts = ["exe", "msi", "dmg", "pkg", "deb", "rpm", "apk", "app"];
    let book_exts = ["epub", "mobi", "azw3", "fb2", "lit"];

    if video_exts.contains(&ext) { "video".to_string() }
    else if audio_exts.contains(&ext) { "audio".to_string() }
    else if image_exts.contains(&ext) { "image".to_string() }
    else if doc_exts.contains(&ext) { "document".to_string() }
    else if archive_exts.contains(&ext) { "archive".to_string() }
    else if software_exts.contains(&ext) { "software".to_string() }
    else if book_exts.contains(&ext) { "book".to_string() }
    else { "other".to_string() }
}
