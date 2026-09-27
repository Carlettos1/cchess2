#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use cchess_session::{Config, Session};
use std::sync::{Arc, Mutex};
use tauri::Manager;
#[tauri::command]
async fn game_request(
    state: tauri::State<'_, Arc<Mutex<Session>>>,
    request: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let session = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || cchess_session::handle(&session, request))
        .await
        .map_err(|e| e.to_string())?
}
fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let storage = app.path().app_data_dir()?.join("cchess-save.json");
            app.manage(Arc::new(Mutex::new(
                Session::new(Config::default())
                    .expect("built-in game")
                    .with_storage(storage),
            )));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![game_request])
        .run(tauri::generate_context!())
        .expect("error while running CChess desktop");
}
