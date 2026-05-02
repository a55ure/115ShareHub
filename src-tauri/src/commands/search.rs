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
