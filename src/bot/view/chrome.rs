//! Window chrome: titlebar, header row, hamburger menu, stats popup and top tabs.

use super::*;

pub(super) const TOP_TABS: [(&str, &str); 7] = [
    ("general", "General"),
    ("modules", "Modules"),
    ("map_zones", "Map Zones"),
    ("targets", "Targets"),
    ("players", "Players"),
    ("config", "Config"),
    ("statistics", "Statistics"),
];

impl BotView {
    /// Legacy's single-row header: icon buttons (menu, settings, start/stop,
    /// home) on the left, the centered title, then tick/CPU/ping/memory
    /// readouts, client-window and pin toggles and the caption buttons.
    pub(super) fn render_header(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
        native_titlebar: bool,
    ) -> impl IntoElement {
        let running = self.behavior_running;
        let left = div()
            .flex()
            .items_center()
            .gap(px(2.0))
            .px(px(6.0))
            .child(
                header_icon("hdr-menu", ICON_MENU, false, true).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.menu_open = !this.menu_open;
                        cx.notify();
                    },
                )),
            )
            .child(
                header_icon("hdr-settings", ICON_SETTINGS, self.settings_open, true)
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_settings_window(cx))),
            )
            .child(
                header_icon(
                    "hdr-start",
                    if running { ICON_PAUSE } else { ICON_PLAY },
                    false,
                    !self.behavior_starting,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    if this.behavior_starting {
                        return;
                    }
                    if this.behavior_running {
                        this.stop_behavior(cx, None);
                    } else {
                        this.run_behavior(cx, None);
                    }
                })),
            )
            .child(header_icon("hdr-home", ICON_HOME, false, true))
            .child(div().w(px(1.0)).h(px(22.0)).mx(px(5.0)).bg(rgb(LINE)));

        let tick = if self.live_frame.capture_us > 0 {
            format!("{:.1}", self.live_frame.capture_us as f64 / 1000.0)
        } else {
            "-".to_owned()
        };
        let ping = self
            .bridge_ping_ms
            .map_or_else(|| "-".to_owned(), |ping| ping.to_string());
        let cpu = if cfg!(windows) && self.mem_bytes > 0 {
            format!("{:.1}", self.cpu_percent)
        } else {
            "-".to_owned()
        };
        let memory = if cfg!(windows) && self.game_mem_bytes > 0 {
            (self.game_mem_bytes / 1_048_576).to_string()
        } else {
            "-".to_owned()
        };
        let readouts = div()
            .id("hdr-metrics")
            .flex()
            .items_center()
            .gap(px(8.0))
            .mr(px(8.0))
            .cursor_pointer()
            .on_click(cx.listener(|this, _, _, cx| {
                this.stats_open = !this.stats_open;
                cx.notify();
            }))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(1.0))
                    .child(readout(ReadoutIcon::Heart, &tick))
                    .child(readout(ReadoutIcon::Signal, &ping)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(1.0))
                    .child(readout(ReadoutIcon::Cpu, &cpu))
                    .child(readout(ReadoutIcon::Memory, &memory)),
            );

        let mut right = div()
            .flex()
            .items_center()
            .gap(px(2.0))
            .child(readouts)
            .child(
                header_icon(
                    "hdr-client",
                    ICON_VIEW,
                    self.game_pid.is_some() && self.client_window_visible,
                    self.game_pid.is_some(),
                )
                .on_click(cx.listener(|this, _, _, cx| this.toggle_client_window(cx))),
            )
            .child(
                header_icon("hdr-pin", ICON_PIN, self.always_on_top, true)
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_always_on_top(cx))),
            )
            .child(
                header_icon("hdr-tray", "_", false, true)
                    .on_click(cx.listener(|this, _, _, cx| this.minimize_to_tray(cx))),
            );
        if !native_titlebar {
            right = right.child(
                div()
                    .flex()
                    .h_full()
                    .ml(px(4.0))
                    .child(caption_button(
                        "titlebar-min",
                        WindowControlArea::Min,
                        "–",
                        false,
                    ))
                    .child(caption_button(
                        "titlebar-max",
                        WindowControlArea::Max,
                        if window.is_maximized() { "❐" } else { "□" },
                        false,
                    ))
                    .child(caption_button(
                        "titlebar-close",
                        WindowControlArea::Close,
                        "✕",
                        true,
                    )),
            );
        }

        let mut header = div()
            .id("titlebar")
            .w_full()
            .h(px(38.0))
            .flex()
            .items_center()
            .bg(rgb(TOP))
            .child(left);
        if !native_titlebar {
            header = header.child(
                div()
                    .id("titlebar-drag")
                    .flex_1()
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .window_control_area(WindowControlArea::Drag)
                    .text_size(px(13.0))
                    .text_color(rgb(MUTED))
                    .child(self.app_title()),
            );
        } else {
            header = header.child(div().flex_1());
        }
        header.child(right)
    }

    /// The ☰ dropdown, rendered at window level so a transparent backdrop
    /// can dismiss it on an outside click. Anchored under the toolbar.
    pub(super) fn render_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let connected = self.session_connected;
        div()
            .id("toolbar-menu-popup")
            .absolute()
            .left(px(8.0))
            .top(px(40.0))
            .w(px(200.0))
            .flex()
            .flex_col()
            .gap(px(2.0))
            .p(px(6.0))
            .bg(rgb(PANEL))
            .rounded_lg()
            .border_1()
            .border_color(rgb(LINE))
            .shadow_lg()
            .on_mouse_down(
                gpui_kit::MouseButton::Left,
                cx.listener(|_, _, _, cx| cx.stop_propagation()),
            )
            .child(
                button("menu-refresh", "Refresh client status", Btn::Ghost)
                    .w_full()
                    .justify_start()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.menu_open = false;
                        this.refresh_client(cx);
                    })),
            )
            .child(
                button("menu-client-log", "Open client log", Btn::Ghost)
                    .w_full()
                    .justify_start()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.menu_open = false;
                        this.open_client_log(cx);
                    })),
            )
            .child(
                button("menu-settings-folder", "Open settings folder", Btn::Ghost)
                    .w_full()
                    .justify_start()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.menu_open = false;
                        this.open_settings_folder(cx);
                    })),
            )
            .when(connected, |this| {
                this.child(div().h(px(1.0)).w_full().my(px(4.0)).bg(rgb(LINE_SOFT)))
                    .child(
                        button("menu-disconnect", "Disconnect", Btn::Danger)
                            .w_full()
                            .justify_start()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.menu_open = false;
                                this.stop_client(cx);
                            })),
                    )
            })
    }

    /// Bot / in-game readouts popup anchored under the header's metrics.
    pub(super) fn render_stats_popup(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tick = if self.live_frame.capture_us > 0 {
            format!("{:.1} ms tick", self.live_frame.capture_us as f64 / 1000.0)
        } else {
            "- ms tick".to_owned()
        };
        let api = format!("{} API Version", self.settings.client.label());
        let memory = if self.game_mem_bytes > 0 {
            format!("{} MB", self.game_mem_bytes / 1_048_576)
        } else {
            "- MB".to_owned()
        };
        let ping = self
            .bridge_ping_ms
            .map_or_else(|| "- ping".to_owned(), |ping| format!("{ping} ping"));
        let line = |text: String| div().pl(px(12.0)).text_size(px(13.0)).child(text);
        div()
            .id("stats-popup")
            .absolute()
            .right(px(180.0))
            .top(px(40.0))
            .flex()
            .flex_col()
            .gap(px(2.0))
            .px(px(10.0))
            .py(px(8.0))
            .bg(rgb(PANEL))
            .border_1()
            .border_color(rgb(LINE))
            .rounded_sm()
            .shadow_lg()
            .text_color(rgb(TEXT))
            .on_mouse_down(
                gpui_kit::MouseButton::Left,
                cx.listener(|_, _, _, cx| cx.stop_propagation()),
            )
            .child(div().text_size(px(13.0)).child("Bot"))
            .child(line(tick))
            .child(line(api))
            .child(div().text_size(px(13.0)).child("In-game"))
            .child(line(memory))
            .child(line(format!("{:.0} FPS", self.live_fps)))
            .child(line(ping))
    }

    pub(super) fn render_top_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut tabs = div().flex().items_center().gap(px(2.0));
        for (id, title) in TOP_TABS {
            let selected = self.top_tab == id;
            let id_owned = id.to_string();
            tabs = tabs.child(
                div()
                    .id(format!("top-{id}"))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.top_tab = id_owned.clone();
                        cx.notify();
                    }))
                    .px(px(11.0))
                    .py(px(5.0))
                    .rounded_sm()
                    .text_size(px(12.0))
                    .font_weight(if selected {
                        FontWeight::MEDIUM
                    } else {
                        FontWeight::NORMAL
                    })
                    .text_color(rgb(if selected { TEXT } else { MUTED }))
                    .bg(rgb(if selected { BLUE_TINT } else { TOP }))
                    .hover(move |s| s.bg(rgb(if selected { BLUE_TINT } else { PANEL_HOVER })))
                    .cursor_pointer()
                    .child(title),
            );
        }
        tabs
    }
}
