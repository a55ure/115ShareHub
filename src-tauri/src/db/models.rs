use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareLink {
    pub id: i64,
    pub share_code: String,
    pub receive_code: String,
    pub title: String,
    pub share_user_id: String,
    pub share_user_name: String,
    pub total_file_count: i64,
    pub total_size: i64,
    pub status: String,
    pub error_message: Option<String>,
    pub last_parsed_at: Option<String>,
    pub added_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub id: i64,
    pub share_link_id: i64,
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

#[derive(Debug, Clone, Deserialize)]
pub struct SearchParams {
    pub query: Option<String>,
    pub file_type: Option<String>,
    pub size_min: Option<i64>,
    pub size_max: Option<i64>,
    pub share_link_id: Option<i64>,
    pub is_dir: Option<bool>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub items: Vec<FileEntry>,
    pub total_count: i64,
    pub page: u32,
    pub page_size: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppStats {
    pub total_share_links: i64,
    pub total_files: i64,
    pub total_size: i64,
    pub files_by_type: HashMap<String, i64>,
    pub parsing_count: i64,
    pub completed_count: i64,
    pub error_count: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListFilesParams {
    pub file_type: Option<String>,
    pub keyword: Option<String>,
    pub share_link_id: Option<i64>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AddShareLinkRequest {
    pub url: String,
    pub receive_code: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ShareLinkDetail {
    pub share_link: ShareLink,
    pub files_by_type: HashMap<String, i64>,
    pub top_level_dirs: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PaginatedResult<T> {
    pub items: Vec<T>,
    pub total: i64,
    pub page: u32,
    pub page_size: u32,
}
