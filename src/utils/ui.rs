use cosmic::iced::Color;
use cosmic::widget;
use cosmic::widget::icon::from_svg_bytes;
use cosmic::widget::svg;
use std::borrow::Cow;
use std::rc::Rc;

pub fn colored_icon(
    bytes: impl Into<Cow<'static, [u8]>>,
    size: u16,
    color: Color,
) -> widget::icon::Icon {
    widget::icon(from_svg_bytes(bytes))
        .size(size)
        .class(cosmic::theme::Svg::Custom(Rc::new(move |_theme| {
            svg::Style {
                color: Some(Color::from_rgb(color.r, color.g, color.b)),
            }
        })))
}
