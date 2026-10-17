mod config_tree;
mod live_map;
mod module_settings;
mod settings_window;
#[cfg(windows)]
mod tray;
mod view;

use std::sync::Arc;

use gpui_kit::component::{Root, Theme, ThemeMode};
use gpui_kit::{
    App, AppContext, Bounds, TitlebarOptions, WindowBounds, WindowDecorations, WindowOptions,
    point, px, size,
};
use gpui_kit::{application, init};

use view::BotView;
use xenomit_bot_runtime::behavior::PlannerRuntime;
use xenomit_bot_runtime::settings::{FileSettingsStore, SettingsStore};

pub fn run() {
    application().run(|cx: &mut App| {
        init(cx);
        Theme::change(ThemeMode::Dark, None, cx);
        let settings = FileSettingsStore::default().load();
        let restore_bounds = settings
            .config_tree
            .pointer("/bot_settings/bot_gui/save_gui_pos")
            .and_then(serde_json::Value::as_bool)
            .filter(|enabled| *enabled)
            .and_then(|_| settings.gui_window_bounds)
            .filter(|bounds| bounds[2] >= 600 && bounds[3] >= 420)
            .map(|bounds| {
                Bounds::new(
                    point(px(bounds[0] as f32), px(bounds[1] as f32)),
                    size(px(bounds[2] as f32), px(bounds[3] as f32)),
                )
            });
        let bounds = restore_bounds
            .unwrap_or_else(|| Bounds::centered(None, size(px(460.0), px(390.0)), cx));
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Login".into()),
                    ..Default::default()
                }),
                window_decorations: Some(WindowDecorations::Server),
                window_min_size: None,
                ..Default::default()
            },
            |window, cx| {
                window.set_window_title("Login");
                let view = cx
                    .new(|cx| BotView::new(window, cx, Some(Arc::new(PlannerRuntime::default()))));
                cx.new(|cx| Root::new(view, window, cx))
            },
        )
        .expect("could not open Xenomit Bot window");
        cx.activate(true);
    });
}
