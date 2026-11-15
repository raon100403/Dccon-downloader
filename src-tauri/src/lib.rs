// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod state;
#[cfg(test)]
mod test_support;
mod types;

use commands::downloader::cancel_download;
use commands::{download_dccon, fetch_dccon_info, parse_dccon_url};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(state::AppState::new().expect("HTTP 클라이언트 초기화 실패"))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            parse_dccon_url,
            fetch_dccon_info,
            download_dccon,
            cancel_download
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
