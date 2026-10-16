//! Account/session panel: sign-in state, client controls and web-panel pairing UI.

use super::*;

impl BotView {
    pub(super) fn render_account_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let connected = self.session_connected;
        let session_color = if self.session_connected { GREEN } else { MUTED };
        let pid = self
            .game_pid
            .map(|pid| pid.to_string())
            .unwrap_or_else(|| "—".into());
        let bridge_color = if self.bridge_ready { GREEN } else { MUTED };
        let mut log_lines: Vec<String> = self
            .client_logs
            .lock()
            .map(|lines| lines.iter().rev().take(20).cloned().collect())
            .unwrap_or_default();
        log_lines.reverse();
        let mut log_panel = div()
            .max_h(px(190.0))
            .overflow_y_scrollbar()
            .flex()
            .flex_col()
            .gap(px(2.0))
            .p(px(10.0))
            .bg(rgb(PANEL_DARK))
            .rounded_md()
            .border_1()
            .border_color(rgb(LINE_SOFT))
            .font_family("Consolas");
        if log_lines.is_empty() {
            log_panel = log_panel.child(
                div()
                    .text_size(px(12.0))
                    .text_color(rgb(FAINT))
                    .child("Client output will appear here after launch."),
            );
        } else {
            for line in log_lines {
                log_panel =
                    log_panel.child(div().text_size(px(11.0)).text_color(rgb(TEXT)).child(line));
            }
        }
        let (panel_phase, panel_detail) = self.panel.state();
        let (panel_label, panel_color) = match panel_phase {
            "online" => ("Connected".to_owned(), GREEN),
            "connecting" => ("Connecting…".into(), AMBER),
            "offline" if !panel_detail.is_empty() => (format!("Offline — {panel_detail}"), RED),
            "offline" => ("Offline".into(), RED),
            _ => ("Not linked".into(), MUTED),
        };

        div()
            .w_full()
            .max_w(px(560.0))
            .flex()
            .flex_col()
            .gap(px(8.0))
            .p(px(12.0))
            .child(section("BACKPAGE SESSION"))
            .child(label(
                "Account credentials are requested only in the login dialog. They are not saved with module settings.",
                MUTED,
                12.0,
            ))
            .child(
                button(
                    "open-login-dialog",
                    if self.busy {
                        "Connecting…"
                    } else if connected {
                        "Stop game client"
                    } else {
                        "Log in to Backpage"
                    },
                    if connected { Btn::Danger } else { Btn::Primary },
                )
                .w_full()
                .py(px(9.0))
                .on_click(cx.listener(|this, _, _, cx| {
                    if this.busy {
                        return;
                    }
                    if this.session_connected {
                        this.stop_client(cx);
                    } else {
                        this.login_dialog_open = true;
                        this.login_error = None;
                    }
                    cx.notify();
                })),
            )
            .child(div().mt(px(4.0)).border_b_1().border_color(rgb(LINE_SOFT)).h(px(1.0)))
            .child(section("CLIENT API"))
            .child(status_row("Session", &self.session_label, session_color))
            .child(status_row("Game", &self.bridge_label, bridge_color))
            .child(status_row("Client PID", &pid, TEXT))
            .child(status_row("Client", self.settings.client.label(), TEXT))
            .child(status_row("Backend", self.settings.client.backend_label(self.client_backend, self.client_flash_backend), TEXT))
            .child(
                button("refresh-api", "Refresh status & snapshot", Btn::Ghost)
                    .w_full()
                    .py(px(8.0))
                    .on_click(cx.listener(|this, _, _, cx| this.refresh_client(cx))),
            )
            .child(div().mt(px(4.0)).border_b_1().border_color(rgb(LINE_SOFT)).h(px(1.0)))
            .child(section("CLIENT LOGS"))
            .child(log_panel)
            .child(
                button("open-client-log", "Open full client log", Btn::Subtle)
                    .w_full()
                    .py(px(7.0))
                    .on_click(cx.listener(|this, _, _, cx| this.open_client_log(cx))),
            )
            .child(label(
                &format!("Saved to {}", client_log_path().display()),
                FAINT,
                11.0,
            ))
            .child(div().mt(px(4.0)).border_b_1().border_color(rgb(LINE_SOFT)).h(px(1.0)))
            .child(section("BEHAVIOR ENGINE"))
            .child(status_row(
                "Adapter",
                if self.engine.is_some() { "Linked" } else { "Unavailable" },
                if self.engine.is_some() { GREEN } else { AMBER },
            ))
            .child(label(
                "Start runs the planner against live map entities and translates movement, combat, and collection intents through the client bridge. Add behavior_config to modules.json to override the default entity rules.",
                MUTED,
                12.0,
            ))
            .child(
                button(
                    "run-behavior",
                    if self.behavior_running {
                        "● Behavior running"
                    } else if self.behavior_starting {
                        "Starting…"
                    } else {
                        "Start behavior"
                    },
                    if self.behavior_running {
                        Btn::Success
                    } else if self.behavior_starting {
                        Btn::Subtle
                    } else {
                        Btn::Primary
                    },
                )
                .w_full()
                .py(px(9.0))
                .on_click(cx.listener(|this, _, _, cx| this.run_behavior(cx, None))),
            )
            .when(self.behavior_running, |this| {
                this.child(
                    button("stop-behavior", "Stop behavior", Btn::Ghost)
                        .w_full()
                        .py(px(8.0))
                        .on_click(cx.listener(|this, _, _, cx| this.stop_behavior(cx, None))),
                )
            })
            .child(div().mt(px(4.0)).border_b_1().border_color(rgb(LINE_SOFT)).h(px(1.0)))
            .child(section("WEB PANEL"))
            .child(status_row("Panel", &panel_label, panel_color))
            .child(label(
                "Outbound-only remote control for this instance; pairing code comes from the panel's Add instance dialog.",
                MUTED,
                12.0,
            ))
            .when(self.settings.panel.token.is_empty(), |this| {
                this.child(Input::new(&self.panel_url).id("panel-url").w_full().h(px(36.0)))
                    .child(Input::new(&self.panel_code).id("panel-code").w_full().h(px(36.0)))
                    .child(
                        button("panel-pair", "Pair with panel", Btn::Primary)
                            .w_full()
                            .py(px(8.0))
                            .on_click(cx.listener(|this, _, _, cx| this.pair_panel(cx))),
                    )
            })
            .when(!self.settings.panel.token.is_empty(), |this| {
                this.child(status_row(
                    "Instance",
                    &self.settings.panel.instance_id,
                    TEXT,
                ))
                .child(
                    button(
                        "panel-toggle",
                        if self.settings.panel.enabled {
                            "Disconnect"
                        } else {
                            "Reconnect"
                        },
                        Btn::Ghost,
                    )
                    .w_full()
                    .py(px(7.0))
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_panel(cx))),
                )
                .child(
                    button("panel-unlink", "Unlink instance", Btn::Danger)
                        .w_full()
                        .py(px(7.0))
                        .on_click(cx.listener(|this, _, _, cx| this.unlink_panel(cx))),
                )
            })
    }

    pub(super) fn render_bridge_danger_zone(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let backend_label = self
            .settings
            .client
            .backend_label(self.settings.game_api_backend, self.settings.flash_backend);
        let control = if self.settings.client == ClientKind::Packet {
            button(
                "bridge-backend-unavailable",
                "Not available for Packet",
                Btn::Subtle,
            )
            .w(px(190.0))
        } else {
            button("cycle-api-backend", backend_label, Btn::Subtle)
                .w(px(190.0))
                .on_click(cx.listener(|this, _, _, cx| {
                    if this.busy {
                        return;
                    }
                    if this.settings.client == ClientKind::Flash {
                        this.settings.flash_backend = this.settings.flash_backend.toggle();
                    } else if this.settings.client == ClientKind::Ruffle {
                        this.settings.game_api_backend = match this.settings.game_api_backend {
                            ClientBackend::Native => ClientBackend::LegacyPipe,
                            ClientBackend::LegacyPipe => ClientBackend::Native,
                        };
                    }
                    this.save_settings();
                    this.apply_backend_change(cx);
                    cx.notify();
                }))
        };

        div()
            .w_full()
            .max_w(px(560.0))
            .flex()
            .flex_col()
            .gap(px(8.0))
            .p(px(12.0))
            .bg(rgb(PANEL))
            .rounded_md()
            .border_1()
            .border_color(rgb(0x6b4046))
            .child(section("DANGER ZONE"))
            .child(label(
                "Advanced bridge controls can interrupt game state capture. Change these only when troubleshooting a client connection.",
                MUTED,
                12.0,
            ))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(10.0))
                    .child(label("Bridge backend", TEXT, 12.0))
                    .child(control),
            )
            .child(label(
                if self.settings.client == ClientKind::Flash {
                    "Switching changes capture in the running Flash client. A running module stops first."
                } else if self.settings.client == ClientKind::Ruffle {
                    "Legacy pipe uses AS3 reflection and can stall the game. Switching applies immediately while connected."
                } else {
                    "Packet uses its own TCP adapter and has no bridge backend choice."
                },
                FAINT,
                11.0,
            ))
    }
}
