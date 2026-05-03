use crate::db::models::*;
use crate::db::Database;
use tauri::State;

#[tauri::command]
pub async fn search_files(
    state: State<'_, Database>,
    params: SearchParams,
) -> Result<SearchResult, String> {
    state.search_files(&params).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_file_stats(state: State<'_, Database>) -> Result<AppStats, String> {
    state.get_file_stats().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_files(
    state: State<'_, Database>,
    params: ListFilesParams,
) -> Result<PaginatedResult<FileEntry>, String> {
    let (items, total) = state.list_files(&params).map_err(|e| e.to_string())?;
    let page = params.page.unwrap_or(1);
    let page_size = params.page_size.unwrap_or(50);
    Ok(PaginatedResult { items, total, page, page_size })
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct BrowseDirParams {
    pub share_link_id: i64,
    pub parent_id: String,
    pub file_type: Option<String>,
    pub keyword: Option<String>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

#[tauri::command]
pub async fn browse_share_dir(
    state: State<'_, Database>,
    params: BrowseDirParams,
) -> Result<PaginatedResult<FileEntry>, String> {
    let page = params.page.unwrap_or(1);
    let page_size = params.page_size.unwrap_or(50);
    let (items, total) = state
        .list_files_in_dir(
            params.share_link_id,
            &params.parent_id,
            params.file_type.as_deref(),
            params.keyword.as_deref(),
            page,
            page_size,
        )
        .map_err(|e| e.to_string())?;
    Ok(PaginatedResult { items, total, page, page_size })
}
