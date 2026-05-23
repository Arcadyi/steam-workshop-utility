#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod app;
pub mod steam;
pub mod types;
pub mod utils;
pub mod icons;

slint::include_modules!();
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ui = AppWindow::new()?;
    
    ui.run()?;

    Ok(())
}
