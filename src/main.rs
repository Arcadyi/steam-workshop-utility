#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod app;
mod config;
pub mod steam;
pub mod types;
pub mod utils;
pub mod icons;
fn main() -> cosmic::iced::Result {
    let settings = cosmic::app::Settings::default()
        .size(cosmic::iced::Size::new(1280.0, 800.0))
        .size_limits(
            cosmic::iced::Limits::NONE
                .min_width(500.0)
                .min_height(300.0)
        );
    cosmic::app::run::<app::AppModel>(settings, ())
}
