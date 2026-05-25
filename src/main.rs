#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use crate::steam::library::{enrich_workshop_items_for_game, get_games, get_workshop_entries};
use crate::types::types::{GameEntry, WorkshopItem};
use crate::utils::slint::{post_progress, post_status};

mod app;
pub mod icons;
pub mod steam;
pub mod types;
pub mod utils;

slint::include_modules!();
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ui = AppWindow::new()?;
    // Core Variables
    let version = format!("v{}", env!("CARGO_PKG_VERSION"));
    ui.set_app_version(version.into());

    // Setup Drag
    let ui_weak = ui.as_weak();
    ui.on_drag(move |dx, dy| {
        if let Some(ui) = ui_weak.upgrade() {
            let window = ui.window();
            let mut pos = window.position();
            pos.x += dx as i32;
            pos.y += dy as i32;
            window.set_position(pos);
        }
    });

    ui.on_minimize(|| { /* ui.window().set_minimized(true) if available */ });
    ui.on_maximize(|| { /* ui.window().set_maximized(true) if available */ });

    let ui_weak = ui.as_weak();
    ui.on_close_window(move || {
        if let Some(ui) = ui_weak.upgrade() {
            ui.window().hide().unwrap();
        }
    });


    ui.set_loading(true);
    ui.set_status_text("Starting...".into());

    let weak = ui.as_weak();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");

        post_status(weak.clone(), "Getting games...".to_string());
        let result = rt.block_on(scan_steam_with_status(weak.clone()));

        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = weak.upgrade() {
                match result {
                    Ok(results) => {
                        ui.set_loading(false);
                        ui.set_status_text(format!("Loaded {} games", results.len()).into());
                    }
                    Err(err) => {
                        ui.set_loading(false);
                        ui.set_status_text(format!("Scan failed: {}", err).into());
                    }
                }
            }
        });
    });

    ui.run()?;
    Ok(())
}

async fn scan_steam_with_status(
    weak: slint::Weak<AppWindow>,
) -> Result<Vec<(GameEntry, Vec<WorkshopItem>)>, String> {
    post_status(weak.clone(), "Creating HTTP client...".to_string());
    post_progress(weak.clone(), 0.01);
    let client = reqwest::Client::new();

    post_status(weak.clone(), "Getting games...".into());
    post_progress(weak.clone(), 0.05);
    let games = get_games().map_err(|e| e.to_string())?;
    let total_games = games.len().max(1);

    let start_progress = 0.10_f32;
    let usable_progress = 0.90_f32;

    let mut results = Vec::new();

    for (i, game) in games.into_iter().enumerate() {
        let base_progress = start_progress + (i as f32 / total_games as f32) * usable_progress;
        post_status(
            weak.clone(),
            format!(
                "Loading workshop entries for {} ({}/{})...",
                game.name.clone().unwrap(),
                i + 1,
                total_games
            ),
        );

        post_progress(weak.clone(), base_progress);

        let items = get_workshop_entries(&game).map_err(|e| e.to_string())?;

        post_status(
            weak.clone(),
            format!(
                "Fetching workshop metadata for {} ({}/{})...",
                game.name.clone().unwrap(),
                i + 1,
                total_games
            ),
        );
        post_progress(
            weak.clone(),
            base_progress + (usable_progress / total_games as f32) * 0.5,
        );

        let items = enrich_workshop_items_for_game(&client, items)
            .await
            .map_err(|e| e.to_string())?;

        results.push((game, items));

        let completed_progress =
            start_progress
                + ((i + 1) as f32 / total_games as f32) * usable_progress;

        post_progress(weak.clone(), completed_progress);
    }

    post_status(weak.clone(), "Finished loading workshop data.".into());
    post_progress(weak.clone(), 1.0);

    Ok(results)
}
