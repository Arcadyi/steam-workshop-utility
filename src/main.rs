#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod app;
mod config;
pub mod steam;
pub mod types;
pub mod utils;
pub mod icons;
fn main() -> cosmic::iced::Result {
    let max_width = if cfg!(target_os = "windows") { 1280.0 } else { f32::INFINITY };
    let max_height = if cfg!(target_os = "windows") { 900.0 } else { f32::INFINITY };

    let settings = cosmic::app::Settings::default()
        .size(cosmic::iced::Size::new(1280.0, 900.0))
        .size_limits(
            cosmic::iced::Limits::NONE
                .min_width(1280.0)
                .min_height(900.0)
                .max_width(max_width)
                .max_height(max_height),
        );

    cosmic::app::run::<app::AppModel>(settings, ())
}
