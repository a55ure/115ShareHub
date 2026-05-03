use crate::db::models::*;
use crate::db::Database;
use crate::pan115::client::Pan115Client;
use crate::pan115::parser::ShareLinkParser;
use tauri::{AppHandle, Emitter, State};
use url::Url;

pub(crate) fn get_proxy_configs_from_db(db: &Database) -> Vec<crate::pan115::client::ProxyConfig> {
    let config_str = db
        .get_setting("proxy_configs")
        .ok()
        .flatten()
        .unwrap_or_default();

    if config_str.is_empty() {
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
    db.get_setting("auth_cookie")
        .ok()
        .flatten()
        .and_then(|c| if c.is_empty() { None } else { Some(c) })
}

fn extract_share_info(input: &str) -> Option<(String, String)> {
    if input.starts_with("http") {
        if let Ok(parsed) = Url::parse(input) {
            let path = parsed.path();
            if let Some(code) = path.strip_prefix("/s/") {
                let share_code = code.trim_end_matches('/').to_string();
                let receive_code = parsed
                    .query_pairs()
                    .find(|(k, _)| k == "password" || k == "receive_code" || k == "code")
                    .map(|(_, v)| v.to_string())
                    .unwrap_or_default();
                return Some((share_code, receive_code));
            }
        }
        if let Some(rest) = input.strip_prefix("https://115cdn.com/s/") {
            let share_code = rest
                .trim_end_matches('/')
                .split('?')
                .next()
                .unwrap_or(rest)
                .to_string();
            let receive_code = if let Some(qs) = rest.split('?').nth(1) {
                url::form_urlencoded::parse(qs.as_bytes())
                    .find(|(k, _)| k == "password" || k == "receive_code" || k == "code")
                    .map(|(_, v)| v.to_string())
                    .unwrap_or_default()
            } else {
                String::new()
            };
            return Some((share_code, receive_code));
        }
    }
    if !input.contains('/') && !input.contains('.') && !input.is_empty() {
        Some((input.to_string(), String::new()))
    } else {
        None
    }
}

/// Start the next pending share link if no other link is currently parsing.
/// Called on startup, after a link finishes, and after a link is deleted.
pub(crate) fn start_next_pending_link(db: &Database, app_handle: &AppHandle) {
    // If a link is already being parsed, do nothing (prevents double-start)
    let parsing_count: i64 = {
        let conn = db.get_conn();
        conn.query_row(
            "SELECT COUNT(*) FROM share_links WHERE status = 'parsing'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0)
    };
    if parsing_count > 0 {
        log::info!("start_next_pending_link: {} link(s) already parsing, skipping", parsing_count);
        return;
    }

    // Find the oldest pending link
    let pending = {
        let conn = db.get_conn();
        conn.query_row(
            "SELECT id, share_code, receive_code FROM share_links WHERE status = 'pending' ORDER BY id ASC LIMIT 1",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
    };

    let (id, share_code, receive_code) = match pending {
        Ok(row) => row,
        Err(_) => {
            log::info!("start_next_pending_link: no pending links found");
            return;
        }
    };

    log::info!("start_next_pending_link: starting parse for link {} ({})", id, share_code);

    // Mark as parsing
    if db.update_share_link_status(id, "parsing", None).is_err() {
        log::warn!("start_next_pending_link: failed to update status for link {}", id);
        return;
    }

    // Spawn background parse task
    let db_clone = db.clone();
    let app_clone = app_handle.clone();
    tokio::spawn(async move {
        let proxy_configs = get_proxy_configs_from_db(&db_clone);
        let rate = get_rate_limit_from_db(&db_clone);
        let cookie = get_auth_cookie_from_db(&db_clone);
        let client = Pan115Client::with_proxy_pool(rate, &proxy_configs).with_cookie(cookie);
        let parser = ShareLinkParser::new(client, 1150);

        match parser
            .parse_share_link(&share_code, &receive_code, id, &app_clone, &db_clone)
            .await
        {
            Ok(result) => {
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

                db_clone
                    .update_share_link_metadata(
                        id,
                        &title,
                        &user_id,
                        &user_name,
                        result.total_files as i64,
                        result.total_size,
                    )
                    .ok();

                let _ = app_clone.emit(
                    "share-link-completed",
                    serde_json::json!({
                        "share_link_id": id,
                        "total_files": result.total_files,
                        "total_size": result.total_size,
                    }),
                );
            }
            Err(e) => {
                db_clone
                    .update_share_link_status(id, "error", Some(&e.to_string()))
                    .ok();
                let _ = app_clone.emit(
                    "share-link-error",
                    serde_json::json!({ "share_link_id": id, "error": e.to_string() }),
                );
            }
        }

        // Chain: start the next pending link
        start_next_pending_link(&db_clone, &app_clone);
    });
}

/// On startup, reset any links stuck in "parsing" state (e.g., from a crash)
/// back to "pending", then start the queue.
pub(crate) fn recover_stale_parsing_links(db: &Database) {
    let conn = db.get_conn();
    if let Err(e) = conn.execute(
        "UPDATE share_links SET status = 'pending' WHERE status = 'parsing'",
        [],
    ) {
        log::warn!("recover_stale_parsing_links: {}", e);
    }
}

#[tauri::command]
pub async fn add_share_link(
    state: State<'_, Database>,
    app: AppHandle,
    request: AddShareLinkRequest,
) -> Result<ShareLink, String> {
    let (share_code, auto_receive_code) = extract_share_info(&request.url)
        .ok_or_else(|| format!("Invalid share link URL: {}", request.url))?;

    let receive_code = if request.receive_code.is_empty() {
        auto_receive_code
    } else {
        request.receive_code
    };

    // Dedup: check if this share_code + receive_code already exists
    {
        let conn = state.get_conn();
        let existing: Option<i64> = conn.query_row(
            "SELECT id FROM share_links WHERE share_code = ?1 AND receive_code = ?2",
            rusqlite::params![share_code, receive_code],
            |row| row.get(0),
        ).ok();

        if let Some(id) = existing {
            drop(conn);
            let link = state.get_share_link(id).map_err(|e| e.to_string())?.ok_or("链接不存在")?;
            return Err(format!("该分享链接已存在（状态: {}）", link.status));
        }
    }

    let id = state
        .insert_share_link(&share_code, &receive_code)
        .map_err(|e| e.to_string())?;

    let share_link = state
        .get_share_link(id)
        .map_err(|e| e.to_string())?
        .ok_or("Failed to retrieve created share link")?;

    let db: Database = state.inner().clone();
    let app_handle = app.clone();

    // Try to start parsing (will only start if no other link is parsing)
    start_next_pending_link(&db, &app_handle);

    Ok(share_link)
}

#[tauri::command]
pub async fn remove_share_link(
    state: State<'_, Database>,
    app: AppHandle,
    id: i64,
) -> Result<(), String> {
    state.delete_share_link(id).map_err(|e| e.to_string())?;

    // After deletion, try starting the next pending link
    let db: Database = state.inner().clone();
    start_next_pending_link(&db, &app);

    Ok(())
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

    state
        .delete_files_by_share_link(id)
        .map_err(|e| e.to_string())?;
    state
        .update_share_link_status(id, "pending", None)
        .map_err(|e| e.to_string())?;

    let db: Database = state.inner().clone();
    let app_handle = app.clone();

    // Try to start parsing (queue handles the rest)
    start_next_pending_link(&db, &app_handle);

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
    )
    .map_err(|e| e.to_string())?;
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

    let target_cid = state
        .get_setting("receive_target_cid")
        .map_err(|e| e.to_string())?
        .filter(|v| !v.is_empty() && v != "0")
        .unwrap_or_else(|| "0".to_string());

    let proxy_configs = get_proxy_configs_from_db(&state);
    let client = Pan115Client::with_proxy_pool(1, &proxy_configs).with_cookie(Some(cookie));

    client
        .receive_share_file(
            &share_link.share_code,
            &share_link.receive_code,
            &request.file_id,
            &request.cid,
            &target_cid,
        )
        .await
        .map_err(|e| e.to_string())?;

    let target_name = state
        .get_setting("receive_target_name")
        .ok()
        .flatten()
        .unwrap_or_else(|| "根目录".to_string());
    Ok(format!("已保存到: {}", target_name))
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ReceiveFolderRequest {
    pub folder_id: String,
    pub share_link_id: i64,
}

#[tauri::command]
pub async fn receive_share_folder(
    state: State<'_, Database>,
    request: ReceiveFolderRequest,
) -> Result<String, String> {
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

    let target_cid = state
        .get_setting("receive_target_cid")
        .map_err(|e| e.to_string())?
        .filter(|v| !v.is_empty() && v != "0")
        .unwrap_or_else(|| "0".to_string());

    let proxy_configs = get_proxy_configs_from_db(&state);
    let client = Pan115Client::with_proxy_pool(1, &proxy_configs).with_cookie(Some(cookie));

    // Directly pass the folder's category_id to 115 API — it handles the whole tree
    client
        .receive_share_file(
            &share_link.share_code,
            &share_link.receive_code,
            &request.folder_id,
            "",
            &target_cid,
        )
        .await
        .map_err(|e| e.to_string())?;

    let target_name = state
        .get_setting("receive_target_name")
        .ok()
        .flatten()
        .unwrap_or_else(|| "根目录".to_string());
    Ok(format!("已转存文件夹到: {}", target_name))
}
