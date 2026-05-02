mod commands;
mod db;
mod pan115;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
            let app_data_dir = app.path().app_data_dir().expect("no app data dir");
            let db_path = app_data_dir.join("115_resource_hub.db");
            let database = db::Database::new(&db_path).expect("failed to open database");
            database.run_migrations().expect("failed to run migrations");
            app.manage(database);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::share_links::add_share_link,
            commands::share_links::remove_share_link,
            commands::share_links::list_share_links,
            commands::share_links::refresh_share_link,
            commands::share_links::update_share_link,
            commands::share_links::get_share_link_detail,
            commands::search::search_files,
            commands::search::get_file_stats,
            commands::search::list_files,
            commands::auth::init_qrcode_login,
            commands::auth::poll_qrcode_login,
            commands::auth::login_by_cookie,
            commands::auth::get_login_status,
            commands::auth::logout,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
