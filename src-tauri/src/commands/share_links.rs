use crate::db::models::*;
use crate::db::Database;
use crate::pan115::client::Pan115Client;
use crate::pan115::parser::ShareLinkParser;
use tauri::{AppHandle, Emitter, State};
use url::Url;

fn extract_share_code(input: &str) -> Option<String> {
    if input.starts_with("http") {
        if let Ok(parsed) = Url::parse(input) {
            let path = parsed.path();
            if let Some(code) = path.strip_prefix("/s/") {
                return Some(code.trim_end_matches('/').to_string());
            }
        }
        if let Some(code) = input.strip_prefix("https://115cdn.com/s/") {
            return Some(code.trim_end_matches('/').split('?').next().unwrap_or(code).to_string());
        }
    }
    if !input.contains('/') && !input.contains('.') && !input.is_empty() {
        Some(input.to_string())
    } else {
        None
    }
}

#[tauri::command]
pub async fn add_share_link(
    state: State<'_, Database>,
    app: AppHandle,
    request: AddShareLinkRequest,
) -> Result<ShareLink, String> {
    let share_code = extract_share_code(&request.url)
        .ok_or_else(|| format!("Invalid share link URL: {}", request.url))?;

    let id = state
        .insert_share_link(&share_code, &request.receive_code)
        .map_err(|e| e.to_string())?;

    let share_link = state
        .get_share_link(id)
        .map_err(|e| e.to_string())?
        .ok_or("Failed to retrieve created share link")?;

    let db: Database = state.inner().clone();
    let app_handle = app.clone();
    let share_code_clone = share_code.clone();
    let receive_code_clone = request.receive_code.clone();

    tokio::spawn(async move {
        let client = Pan115Client::new(2);
        let parser = ShareLinkParser::new(client, 1150);

        db.update_share_link_status(id, "parsing", None)
            .ok();

        match parser
            .parse_share_link(&share_code_clone, &receive_code_clone, id, &|_| {}, &app_handle)
            .await
        {
            Ok(result) => {
                let file_data: Vec<(i64, &str, &str, &str, i64, &str, bool, &str, &str, i32, &str)> = result
                    .files
                    .iter()
                    .map(|f| {
                        (
                            id,
                            f.file_id.as_str(),
                            f.parent_id.as_str(),
                            f.name.as_str(),
                            f.size,
                            f.sha1.as_str(),
                            f.is_dir,
                            f.file_type.as_str(),
                            f.full_path.as_str(),
                            f.depth,
                            f.thumbnail_url.as_str(),
                        )
                    })
                    .collect();

                if let Err(e) = db.insert_files_batch(&file_data) {
                    db.update_share_link_status(id, "error", Some(&e.to_string()))
                        .ok();
                    let _ = app_handle.emit(
                        "share-link-error",
                        serde_json::json!({ "share_link_id": id, "error": e.to_string() }),
                    );
                    return;
                }

                let title = result
                    .share_info
                    .as_ref()
                    .map(|si| si.share_title.clone())
                    .unwrap_or_default();
                let user_id = result
                    .user_info
                    .as_ref()
                    .map(|ui| ui.user_id.clone())
                    .unwrap_or_default();
                let user_name = result
                    .user_info
                    .as_ref()
                    .map(|ui| ui.user_name.clone())
                    .unwrap_or_default();

                db.update_share_link_metadata(
                    id,
                    &title,
                    &user_id,
                    &user_name,
                    result.total_files as i64,
                    result.total_size,
                )
                .ok();

                let _ = app_handle.emit(
                    "share-link-completed",
                    serde_json::json!({
                        "share_link_id": id,
                        "total_files": result.total_files,
                        "total_size": result.total_size,
                    }),
                );
            }
            Err(e) => {
                db.update_share_link_status(id, "error", Some(&e.to_string()))
                    .ok();
                let _ = app_handle.emit(
                    "share-link-error",
                    serde_json::json!({ "share_link_id": id, "error": e.to_string() }),
                );
            }
        }
    });

    Ok(share_link)
}

#[tauri::command]
pub async fn remove_share_link(state: State<'_, Database>, id: i64) -> Result<(), String> {
    state.delete_share_link(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_share_links(
    state: State<'_, Database>,
    page: u32,
    page_size: u32,
) -> Result<PaginatedResult<ShareLink>, String> {
    let (items, total) = state
        .list_share_links(page, page_size)
        .map_err(|e| e.to_string())?;
    Ok(PaginatedResult {
        items,
        total,
        page,
        page_size,
    })
}

#[tauri::command]
pub async fn refresh_share_link(
    state: State<'_, Database>,
    app: AppHandle,
    id: i64,
) -> Result<(), String> {
    let share_link = state
        .get_share_link(id)
        .map_err(|e| e.to_string())?
        .ok_or("Share link not found")?;

    state.delete_files_by_share_link(id).map_err(|e| e.to_string())?;
    state
        .update_share_link_status(id, "pending", None)
        .map_err(|e| e.to_string())?;

    let db: Database = state.inner().clone();
    let app_handle = app.clone();
    let share_code = share_link.share_code.clone();
    let receive_code = share_link.receive_code.clone();

    tokio::spawn(async move {
        let client = Pan115Client::new(2);
        let parser = ShareLinkParser::new(client, 1150);

        db.update_share_link_status(id, "parsing", None)
            .ok();

        match parser
            .parse_share_link(
                &share_code,
                &receive_code,
                id,
                &|_| {},
                &app_handle,
            )
            .await
        {
            Ok(result) => {
                let file_data: Vec<(i64, &str, &str, &str, i64, &str, bool, &str, &str, i32, &str)> = result
                    .files
                    .iter()
                    .map(|f| {
                        (
                            id,
                            f.file_id.as_str(),
                            f.parent_id.as_str(),
                            f.name.as_str(),
                            f.size,
                            f.sha1.as_str(),
                            f.is_dir,
                            f.file_type.as_str(),
                            f.full_path.as_str(),
                            f.depth,
                            f.thumbnail_url.as_str(),
                        )
                    })
                    .collect();

                if let Err(e) = db.insert_files_batch(&file_data) {
                    db.update_share_link_status(id, "error", Some(&e.to_string()))
                        .ok();
                    return;
                }

                let title = result
                    .share_info
                    .as_ref()
                    .map(|si| si.share_title.clone())
                    .unwrap_or_default();
                let user_id = result
                    .user_info
                    .as_ref()
                    .map(|ui| ui.user_id.clone())
                    .unwrap_or_default();
                let user_name = result
                    .user_info
                    .as_ref()
                    .map(|ui| ui.user_name.clone())
                    .unwrap_or_default();

                db.update_share_link_metadata(
                    id,
                    &title,
                    &user_id,
                    &user_name,
                    result.total_files as i64,
                    result.total_size,
                )
                .ok();

                let _ = app_handle.emit(
                    "share-link-completed",
                    serde_json::json!({
                        "share_link_id": id,
                        "total_files": result.total_files,
                        "total_size": result.total_size,
                    }),
                );
            }
            Err(e) => {
                db.update_share_link_status(id, "error", Some(&e.to_string()))
                    .ok();
                let _ = app_handle.emit(
                    "share-link-error",
                    serde_json::json!({ "share_link_id": id, "error": e.to_string() }),
                );
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn get_share_link_detail(
    state: State<'_, Database>,
    id: i64,
) -> Result<ShareLinkDetail, String> {
    let share_link = state
        .get_share_link(id)
        .map_err(|e| e.to_string())?
        .ok_or("Share link not found")?;

    let stats = state.get_file_stats().map_err(|e| e.to_string())?;

    Ok(ShareLinkDetail {
        share_link,
        files_by_type: stats.files_by_type,
        top_level_dirs: vec![],
    })
}
