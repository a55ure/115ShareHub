use crate::db::models::*;
use crate::db::Database;
use crate::pan115::client::{Pan115Client, ProxyConfig};
use crate::pan115::parser::ShareLinkParser;
use tauri::{AppHandle, Emitter, State};
use url::Url;

fn get_proxy_configs_from_db(db: &Database) -> Vec<crate::pan115::client::ProxyConfig> {
    let config_str = db
        .get_setting("proxy_configs")
        .ok()
        .flatten()
        .unwrap_or_default();

    if config_str.is_empty() {
        // Fall back to old single-proxy format
        return get_single_proxy_config_from_db(db)
            .into_iter()
            .filter(|c| c.enabled && !c.host.is_empty())
            .collect();
    }

    serde_json::from_str(&config_str).unwrap_or_default()
}

fn get_single_proxy_config_from_db(db: &Database) -> Vec<crate::pan115::client::ProxyConfig> {
    let config_str = db
        .get_setting("proxy_config")
        .ok()
        .flatten()
        .unwrap_or_default();
    if config_str.is_empty() {
        return vec![];
    }
    serde_json::from_str::<crate::pan115::client::ProxyConfig>(&config_str)
        .map(|c| vec![c])
        .unwrap_or_default()
}

fn get_rate_limit_from_db(db: &Database) -> u32 {
    db.get_setting("rate_limit_rps")
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2)
        .clamp(1, 10)
}

fn get_auth_cookie_from_db(db: &Database) -> Option<String> {
    db.get_setting("auth_cookie").ok().flatten()
        .and_then(|c| if c.is_empty() { None } else { Some(c) })
}

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
        let proxy_configs = get_proxy_configs_from_db(&db);
        let rate = get_rate_limit_from_db(&db);
        let cookie = get_auth_cookie_from_db(&db);
        let client = Pan115Client::with_proxy_pool(rate, &proxy_configs).with_cookie(cookie);
        let parser = ShareLinkParser::new(client, 1150);

        db.update_share_link_status(id, "parsing", None).ok();

        match parser
            .parse_share_link(&share_code_clone, &receive_code_clone, id, &app_handle, &db)
            .await
        {
            Ok(result) => {
                let title = result.share_info.as_ref().map(|si| si.share_title.clone()).unwrap_or_default();
                let user_id = result.user_info.as_ref().map(|ui| ui.user_id.clone()).unwrap_or_default();
                let user_name = result.user_info.as_ref().map(|ui| ui.user_name.clone()).unwrap_or_default();

                db.update_share_link_metadata(
                    id, &title, &user_id, &user_name,
                    result.total_files as i64, result.total_size,
                ).ok();

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
                db.update_share_link_status(id, "error", Some(&e.to_string())).ok();
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
    Ok(PaginatedResult { items, total, page, page_size })
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
    state.update_share_link_status(id, "pending", None).map_err(|e| e.to_string())?;

    let db: Database = state.inner().clone();
    let app_handle = app.clone();
    let share_code = share_link.share_code.clone();
    let receive_code = share_link.receive_code.clone();

    tokio::spawn(async move {
        let proxy_configs = get_proxy_configs_from_db(&db);
        let rate = get_rate_limit_from_db(&db);
        let cookie = get_auth_cookie_from_db(&db);
        let client = Pan115Client::with_proxy_pool(rate, &proxy_configs).with_cookie(cookie);
        let parser = ShareLinkParser::new(client, 1150);

        db.update_share_link_status(id, "parsing", None).ok();

        match parser
            .parse_share_link(&share_code, &receive_code, id, &app_handle, &db)
            .await
        {
            Ok(result) => {
                let title = result.share_info.as_ref().map(|si| si.share_title.clone()).unwrap_or_default();
                let user_id = result.user_info.as_ref().map(|ui| ui.user_id.clone()).unwrap_or_default();
                let user_name = result.user_info.as_ref().map(|ui| ui.user_name.clone()).unwrap_or_default();

                db.update_share_link_metadata(
                    id, &title, &user_id, &user_name,
                    result.total_files as i64, result.total_size,
                ).ok();

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
                db.update_share_link_status(id, "error", Some(&e.to_string())).ok();
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
pub async fn update_share_link(
    state: State<'_, Database>,
    id: i64,
    title: String,
    receive_code: String,
) -> Result<(), String> {
    let conn = state.get_conn();
    conn.execute(
        "UPDATE share_links SET title = ?1, receive_code = ?2 WHERE id = ?3",
        rusqlite::params![title, receive_code, id],
    ).map_err(|e| e.to_string())?;
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

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ReceiveFileRequest {
    pub file_id: String,
    pub share_link_id: i64,
    pub cid: String,
}

#[tauri::command]
pub async fn receive_share_file(
    state: State<'_, Database>,
    request: ReceiveFileRequest,
) -> Result<String, String> {
    // Get share_link info for share_code / receive_code
    let share_link = state
        .get_share_link(request.share_link_id)
        .map_err(|e| e.to_string())?
        .ok_or("分享链接不存在".to_string())?;

    let cookie = state
        .get_setting("auth_cookie")
        .map_err(|e| e.to_string())?
        .ok_or("请先登录115账号".to_string())?;

    if cookie.is_empty() {
        return Err("请先登录115账号".to_string());
    }

    let proxy_configs = get_proxy_configs_from_db(&state);
    let client = Pan115Client::with_proxy_pool(1, &proxy_configs).with_cookie(Some(cookie));

    client
        .receive_share_file(
            &share_link.share_code,
            &share_link.receive_code,
            &request.file_id,
            &request.cid,
            "0", // target: user root folder
        )
        .await
        .map_err(|e| e.to_string())?;

    Ok(format!("已保存到115网盘根目录"))
}
