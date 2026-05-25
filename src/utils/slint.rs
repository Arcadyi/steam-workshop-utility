use crate::AppWindow;

pub fn post_status(weak: slint::Weak<AppWindow>, text: String) {
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = weak.upgrade() {
            ui.set_status_text(text.into());
        }
    });
}

pub fn post_progress(weak: slint::Weak<AppWindow>, progress: f32) {
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = weak.upgrade() {
            ui.set_progress(progress);
        }
    });
}