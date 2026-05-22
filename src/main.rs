mod app;
mod config;
pub mod steam;
pub mod types;
pub mod utils;
pub mod icons;
pub mod build;

fn main() -> cosmic::iced::Result {
    let settings = cosmic::app::Settings::default().size_limits(
        cosmic::iced::Limits::NONE
            .min_width(360.0)
            .min_height(180.0),
    );

    cosmic::app::run::<app::AppModel>(settings, ())
}
