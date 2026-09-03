// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod types;

use commands::{parse_dccon_url, fetch_dccon_info, download_dccon};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            parse_dccon_url,
            fetch_dccon_info,
            download_dccon
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
