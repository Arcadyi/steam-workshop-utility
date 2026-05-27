use tauri::Manager;
use crate::steam::library::{enrich_workshop_items_for_game, get_workshop_entries};
use crate::steam::types::{GameEntry, WorkshopItem};

mod steam;
mod utils;

#[tauri::command]
fn get_games() -> Result<Vec<GameEntry>, String> {
    let games = steam::library::get_games().map_err(|e| e.to_string())?;
    let filtered = games.into_iter().filter(|game| {
        steam::library::get_workshop_entries(game)
            .map(|items| !items.is_empty())
            .unwrap_or(false)
    }).collect();
    Ok(filtered)
}

#[tauri::command]
async fn get_workshop_items(game: GameEntry) -> Result<Vec<WorkshopItem>, String> {
    let items = get_workshop_entries(&game).map_err(|e| e.to_string())?;
    let client = reqwest::Client::new();
    let enriched = enrich_workshop_items_for_game(&client, items)
        .await
        .map_err(|e| e.to_string())?;
    Ok(enriched)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();
            window.set_shadow(true);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_games])
        .invoke_handler(tauri::generate_handler![get_games, get_workshop_items])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}