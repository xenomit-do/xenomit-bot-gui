//! Sign-in dialog and the authentication/connection flow.

use super::*;

impl BotView {
    pub(super) fn render_login_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let client_picker = div().flex().flex_col().items_end().child(
            div()
                .flex()
                .items_center()
                .gap(px(6.0))
                .child(label("CLIENT", FAINT, 10.0))
                .child(
                    button(
                        "client-picker",
                        format!("{}  ▾", self.settings.client.label()),
                        Btn::Subtle,
                    )
                    .w(px(148.0))
                    .py(px(5.0))
                    .text_size(px(12.0))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if this.busy {
                            return;
                        }
                        this.client_picker_open = !this.client_picker_open;
                        cx.notify();
                    })),
                ),
        );
        let client_options = if self.client_picker_open {
            let mut options = div()
                .id("client-picker-options")
                .w(px(148.0))
                .flex()
                .flex_col()
                .gap(px(2.0))
                .p(px(3.0))
                .bg(rgb(PANEL_DARK))
                .rounded_md()
                .border_1()
                .border_color(rgb(LINE_SOFT));
            for kind in [ClientKind::Ruffle, ClientKind::Flash, ClientKind::Packet] {
                let selected = self.settings.client == kind;
                options = options.child(
                    div()
                        .id(format!("client-option-{}", kind.label()))
                        .w_full()
                        .px(px(8.0))
                        .py(px(5.0))
                        .rounded_sm()
                        .bg(rgb(if selected { BLUE_TINT } else { PANEL_DARK }))
                        .hover(move |style| {
                            style.bg(rgb(if selected { BLUE_TINT } else { PANEL_HOVER }))
                        })
                        .text_size(px(12.0))
                        .text_color(rgb(if selected { TEXT } else { MUTED }))
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if this.busy {
                                return;
                            }
                            this.settings.client = kind;
                            this.client_picker_open = false;
                            this.save_settings();
                            cx.notify();
                        }))
                        .child(kind.label()),
                );
            }
            Some(options)
        } else {
            None
        };

        let mut header = div().flex().items_start().justify_between().gap(px(10.0));
        if self.session_connected {
            header = header.child(
                div()
                    .text_size(px(14.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(TEXT))
                    .child("Login"),
            );
        } else {
            header = header.child(div());
        }
        header = header.child(client_picker);
        if self.session_connected {
            header = header.child(
                button("close-login", "×", Btn::Ghost)
                    .w(px(28.0))
                    .h(px(28.0))
                    .py(px(2.0))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.login_dialog_open = false;
                        this.client_picker_open = false;
                        cx.notify();
                    })),
            );
        }

        let mut panel = div().flex().flex_col().relative().gap(px(10.0));
        if self.session_connected {
            panel = panel
                .w(px(420.0))
                .p(px(14.0))
                .bg(rgb(PANEL))
                .rounded_lg()
                .border_1()
                .border_color(rgb(LINE_SOFT))
                .shadow_lg()
                .on_mouse_down(
                    gpui_kit::MouseButton::Left,
                    cx.listener(|_, _, _, cx| cx.stop_propagation()),
                );
        } else {
            panel = panel.size_full().p(px(14.0)).bg(rgb(PANEL));
        }
        panel = panel.child(header);

        let mut modes = div()
            .flex()
            .gap(px(3.0))
            .p(px(3.0))
            .rounded_md()
            .bg(rgb(PANEL_DARK))
            .border_1()
            .border_color(rgb(LINE_SOFT));
        for (tab, title) in [
            (LoginTab::Account, "User & pass"),
            (LoginTab::ExistingSession, "SID login"),
            (LoginTab::SavedCreds, "Saved"),
        ] {
            let selected = self.login_tab == tab;
            modes = modes.child(
                div()
                    .id(format!(
                        "login-tab-{}",
                        title
                            .to_ascii_lowercase()
                            .replace(' ', "-")
                            .replace('&', "")
                    ))
                    .flex_1()
                    .py(px(6.0))
                    .rounded_sm()
                    .bg(rgb(if selected { BLUE_TINT } else { PANEL_DARK }))
                    .hover(move |style| {
                        style.bg(rgb(if selected { BLUE_TINT } else { PANEL_HOVER }))
                    })
                    .text_center()
                    .text_size(px(12.0))
                    .font_weight(if selected {
                        FontWeight::MEDIUM
                    } else {
                        FontWeight::NORMAL
                    })
                    .text_color(rgb(if selected { TEXT } else { MUTED }))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if this.busy {
                            return;
                        }
                        this.login_tab = tab;
                        this.login_error = None;
                        if tab != LoginTab::SavedCreds {
                            let input = if tab == LoginTab::ExistingSession {
                                &this.server
                            } else {
                                &this.username
                            };
                            window.focus(&input.read(cx).focus_handle(cx), cx);
                        }
                        cx.notify();
                    }))
                    .child(title),
            );
        }
        panel = panel.child(modes);

        match self.login_tab {
            LoginTab::Account | LoginTab::ExistingSession => {
                let fields = if self.login_tab == LoginTab::ExistingSession {
                    vec![
                        ("Server", "server-input", &self.server),
                        ("Session dosid", "sid-input", &self.sid),
                    ]
                } else {
                    vec![
                        ("Username", "username-input", &self.username),
                        ("Password", "password-input", &self.password),
                    ]
                };
                for (index, (title, id, state)) in fields.into_iter().enumerate() {
                    panel = panel.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .w(px(84.0))
                                    .flex_shrink_0()
                                    .text_size(px(12.0))
                                    .text_color(rgb(MUTED))
                                    .child(title),
                            )
                            .child(
                                div().flex_1().child(
                                    Input::new(state)
                                        .id(id)
                                        .w_full()
                                        .tab_index(index as isize + 1)
                                        .disabled(self.busy)
                                        .h(px(34.0))
                                        .text_size(px(13.0)),
                                ),
                            ),
                    );
                }
                if self.login_tab == LoginTab::Account {
                    panel = panel.child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(8.0))
                            .child(label("Save for faster sign-in", FAINT, 11.0))
                            .child(
                                button("save-login-credentials", "Save account", Btn::Ghost)
                                    .py(px(5.0))
                                    .text_size(px(11.5))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.save_login_credentials(window, cx)
                                    })),
                            ),
                    );
                    panel = panel.child(label(
                        "For accounts with CAPTCHA, use SID login.",
                        FAINT,
                        10.5,
                    ));
                }
            }
            LoginTab::SavedCreds => {
                panel = panel.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(3.0))
                        .child(section("SAVED ACCOUNTS"))
                        .child(label("Encrypted for this Windows account", FAINT, 10.5)),
                );
                if self.saved_credentials.is_empty() {
                    panel = panel.child(
                        div()
                            .w_full()
                            .p(px(12.0))
                            .rounded_md()
                            .bg(rgb(PANEL_DARK))
                            .border_1()
                            .border_color(rgb(LINE_SOFT))
                            .child(label(
                                "No saved accounts yet. Add one from User & pass.",
                                MUTED,
                                12.0,
                            )),
                    );
                } else {
                    let credentials = self.saved_credentials.clone();
                    let count = credentials.len();
                    let mut saved_rows = div().w_full().flex().flex_col().gap(px(6.0));
                    for (index, credential) in credentials.into_iter().enumerate() {
                        let connecting =
                            self.busy && self.connecting_saved_credential == Some(index);
                        let can_connect = !self.busy;
                        let mut connect_button = button(
                            format!("saved-credential-connect-{index}"),
                            if connecting {
                                "⟳ Connecting…"
                            } else {
                                "Connect"
                            },
                            if connecting {
                                Btn::Selected
                            } else if can_connect {
                                Btn::Primary
                            } else {
                                Btn::Subtle
                            },
                        )
                        .py(px(5.0))
                        .text_size(px(11.5));
                        if can_connect {
                            connect_button =
                                connect_button.on_click(cx.listener(move |this, _, _, cx| {
                                    this.sign_in_saved_credential(index, cx);
                                }));
                        }
                        let mut remove_button = button(
                            format!("saved-credential-remove-{index}"),
                            "Remove",
                            Btn::Subtle,
                        )
                        .py(px(5.0))
                        .text_size(px(11.5));
                        if can_connect {
                            remove_button =
                                remove_button.on_click(cx.listener(move |this, _, _, cx| {
                                    this.remove_saved_credential(index, cx);
                                }));
                        }
                        saved_rows = saved_rows.child(
                            div()
                                .w_full()
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap(px(8.0))
                                .p(px(8.0))
                                .rounded_md()
                                .bg(rgb(PANEL_DARK))
                                .border_1()
                                .border_color(rgb(LINE_SOFT))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w(px(0.0))
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .text_ellipsis()
                                        .child(label(&credential.username, TEXT, 12.0)),
                                )
                                .child(
                                    div()
                                        .flex_shrink_0()
                                        .flex()
                                        .gap(px(5.0))
                                        .child(connect_button)
                                        .child(remove_button),
                                ),
                        );
                    }
                    panel = panel.child(if count > 3 {
                        saved_rows
                            .id("saved-credentials-list")
                            .h(px(190.0))
                            .overflow_y_scrollbar()
                            .into_any_element()
                    } else {
                        saved_rows.into_any_element()
                    });
                }
                if let Some(error) = &self.saved_credentials_error {
                    panel = panel.child(
                        div()
                            .w_full()
                            .p(px(9.0))
                            .rounded_md()
                            .bg(rgb(RED_TINT))
                            .border_1()
                            .border_color(rgb(0x6b4046))
                            .child(label(error, 0xf0b9b9, 12.0)),
                    );
                }
            }
        }

        if let Some(error) = &self.login_error {
            panel = panel.child(
                div()
                    .w_full()
                    .p(px(9.0))
                    .rounded_md()
                    .bg(rgb(RED_TINT))
                    .border_1()
                    .border_color(rgb(0x6b4046))
                    .child(label(error, 0xf0b9b9, 12.0)),
            );
        }

        let mut footer = div().flex().items_center().justify_between().gap(px(10.0));
        if self.busy {
            let status = if let Some(index) = self.connecting_saved_credential {
                self.saved_credentials
                    .get(index)
                    .map(|credential| format!("Signing in as {}…", credential.username))
                    .unwrap_or_else(|| "Connecting…".into())
            } else {
                "Signing in and starting the client…".into()
            };
            footer = footer.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .child(if self.connecting_saved_credential.is_some() {
                        label("⟳", BLUE, 13.0)
                    } else {
                        div().size(px(8.0)).rounded_full().bg(rgb(BLUE))
                    })
                    .child(label(&status, MUTED, 11.0)),
            );
        } else {
            footer = footer.child(div());
        }
        if self.login_tab != LoginTab::SavedCreds {
            footer = footer.child(
                button(
                    "submit-login",
                    if self.busy { "Connecting…" } else { "Log in" },
                    if self.busy { Btn::Subtle } else { Btn::Primary },
                )
                .w(px(100.0))
                .py(px(8.0))
                .text_size(px(12.5))
                .font_weight(FontWeight::SEMIBOLD)
                .on_click(cx.listener(|this, _, window, cx| this.submit_login(window, cx))),
            );
        }
        panel = panel.child(footer);
        if let Some(options) = client_options {
            panel = panel.child(
                div()
                    .absolute()
                    .top(px(48.0))
                    .right(px(14.0))
                    .child(options),
            );
        }

        div()
            .id("login-page")
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .bg(if self.session_connected {
                rgb(BG).alpha(0.72)
            } else {
                rgb(PANEL).alpha(1.0)
            })
            .font_family("Segoe UI")
            .text_color(rgb(TEXT))
            .on_mouse_down(
                gpui_kit::MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if this.session_connected {
                        this.login_dialog_open = false;
                        this.client_picker_open = false;
                        cx.notify();
                    }
                }),
            )
            .capture_key_down(
                cx.listener(|this, event: &gpui_kit::KeyDownEvent, window, cx| {
                    if event.keystroke.key == "enter" {
                        cx.stop_propagation();
                        this.submit_login(window, cx);
                    }
                }),
            )
            .child(panel)
    }
    pub(super) fn submit_login(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.login_tab {
            LoginTab::ExistingSession => self.connect(cx),
            LoginTab::Account => self.submit_credentials(window, cx),
            LoginTab::SavedCreds => {}
        }
    }

    pub(super) fn save_login_credentials(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let username = self.username.read(cx).value().trim().to_string();
        let password = self.password.read(cx).value().to_string();
        if username.is_empty() || password.is_empty() {
            self.login_error =
                Some("Enter both your username and password to save this account.".into());
            cx.notify();
            return;
        }

        let mut credentials = self.saved_credentials.clone();
        if let Some(existing) = credentials
            .iter_mut()
            .find(|credential| credential.username.eq_ignore_ascii_case(&username))
        {
            existing.username = username;
            existing.password = password;
        } else {
            credentials.push(SavedCredential { username, password });
        }

        match SavedCredentialStore::save(&self.saved_credentials_path, &credentials) {
            Ok(()) => {
                self.saved_credentials = credentials;
                self.saved_credentials_error = None;
                self.login_error = None;
                self.login_tab = LoginTab::SavedCreds;
                self.password
                    .update(cx, |state, cx| state.set_value("", window, cx));
            }
            Err(error) => self.saved_credentials_error = Some(error),
        }
        cx.notify();
    }

    pub(super) fn remove_saved_credential(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.busy || index >= self.saved_credentials.len() {
            return;
        }
        let mut credentials = self.saved_credentials.clone();
        credentials.remove(index);
        match SavedCredentialStore::save(&self.saved_credentials_path, &credentials) {
            Ok(()) => {
                self.saved_credentials = credentials;
                self.saved_credentials_error = None;
            }
            Err(error) => self.saved_credentials_error = Some(error),
        }
        cx.notify();
    }

    pub(super) fn sign_in_saved_credential(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        if self.session_connected {
            self.login_dialog_open = false;
            self.stop_client(cx);
            cx.notify();
            return;
        }
        let Some(credential) = self.saved_credentials.get(index).cloned() else {
            return;
        };
        self.begin_authentication(
            SessionCredentials::Password {
                username: credential.username,
                password: credential.password,
            },
            Some(index),
            cx,
        );
    }

    pub(super) fn connect(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        if self.session_connected {
            self.stop_client(cx);
            cx.notify();
            return;
        }

        let server = self.server.read(cx).value().trim().to_string();
        let sid = self.sid.read(cx).value().trim().to_string();
        if server.is_empty() || sid.is_empty() {
            self.login_error = Some("Enter a server and session dosid first.".into());
            cx.notify();
            return;
        }

        self.begin_authentication(SessionCredentials::SessionId { server, sid }, None, cx);
    }

    pub(super) fn submit_credentials(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        if self.session_connected {
            self.login_dialog_open = false;
            self.stop_client(cx);
            cx.notify();
            return;
        }
        let username = self.username.read(cx).value().trim().to_string();
        let password = self.password.read(cx).value().to_string();
        if username.is_empty() || password.is_empty() {
            self.login_error = Some("Enter both your username and password.".into());
            cx.notify();
            return;
        }
        self.password
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.begin_authentication(
            SessionCredentials::Password { username, password },
            None,
            cx,
        );
    }

    pub(super) fn begin_authentication(
        &mut self,
        auth: SessionCredentials,
        saved_credential_index: Option<usize>,
        cx: &mut Context<Self>,
    ) {
        self.login_error = None;
        if let SessionCredentials::Password { username, .. } = &auth {
            self.session_username = username.clone();
        }

        self.busy = true;
        self.client_picker_open = false;
        self.connecting_saved_credential = saved_credential_index;
        self.session_label = "Authenticating…".into();
        self.activity = match &auth {
            SessionCredentials::Password { .. } => "Signing in and starting the client…".into(),
            SessionCredentials::SessionId { .. } => {
                "Checking the session and starting the client…".into()
            }
        };
        cx.notify();

        let backend = self.settings.game_api_backend;
        let kind = self.settings.client;
        let flash_backend = self.settings.flash_backend;
        let api_options = launch::game_api_options(&self.settings.config_tree);
        let locale = self
            .settings
            .config_tree
            .pointer("/bot_settings/bot_gui/locale")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("en")
            .to_owned();
        self.connection_generation += 1;
        let generation = self.connection_generation;
        let provider: Arc<dyn SessionProvider> = Arc::new(BackpageSessionProvider::new(auth));
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    authenticate_and_launch(provider, kind, backend, flash_backend, api_options, locale)
                })
                .await;
            let monitor = result
                .as_ref()
                .ok()
                .and_then(|launch| launch.bridge.as_ref().map(|bridge| Arc::clone(bridge)));
            let _ = this.update(cx, |view, cx| {
                view.busy = false;
                view.connecting_saved_credential = None;
                match result {
                    Ok(launch) => {
                        if let Ok(mut stats) = launch.statistics.lock() {
                            stats.panel = Some(view.panel.clone());
                        }
                        view.statistics = Some(launch.statistics.clone());
                        view.update_statistics_policy(false);
                        view.panel.set_logs(launch.client_logs.clone());
                        view.session = Some(launch.session.clone());
                        behavior::set_backpage_session(Some(launch.session.clone()));
                        view.status_refreshes = 0;
                        view.last_reload = Instant::now();
                        view.session_started_secs = statistics::now_ms() / 1000;
                        view.bridge_ping_ms = None;
                        Self::spawn_statistics_monitor(
                            this.clone(),
                            generation,
                            launch.session,
                            cx,
                        );
                        view.login_dialog_open = false;
                        view.login_error = None;
                        view.session_connected = true;
                        view.session_label = "Backpage session verified".into();
                        view.game_pid = launch.pid;
                        view.client_window_visible = true;
                        let fully_hide_api = view
                            .settings
                            .config_tree
                            .pointer("/bot_settings/api_config/fully_hide_api")
                            .and_then(serde_json::Value::as_bool)
                            .unwrap_or(true);
                        view.hide_flash_after_startup = kind == ClientKind::Flash
                            && fully_hide_api
                            && std::env::var("XENOMIT_FLASH_KEEP_VISIBLE").as_deref() != Ok("1");
                        #[cfg(windows)]
                        if let Some(pid) = launch.pid {
                            if kind != ClientKind::Flash && fully_hide_api {
                                view.client_window_visible = false;
                                Self::hide_client_window_when_ready(
                                    this.clone(),
                                    generation,
                                    pid,
                                    cx,
                                );
                            }
                        }
                        view.game_process = launch.child;
                        view.bridge = launch.bridge;
                        view.star_catalog = launch.star_catalog;
                        view.client_backend = backend;
                        view.client_flash_backend = flash_backend;
                        view.bridge_ready = false;
                        view.client_logs = launch.client_logs;
                        view.bridge_label = "Checking game connection…".into();
                        view.activity =
                            format!("{} connected. Waiting for game state.", kind.label());
                    }
                    Err(error) => {
                        view.resume_after_reload = false;
                        view.refresh_pause_after_reload = None;
                        view.session_label = "Authentication failed".into();
                        let lower = error.to_ascii_lowercase();
                        let captcha_detected = ["captcha", "verify you are human", "robot check"]
                            .iter()
                            .any(|marker| lower.contains(marker));
                        let manual_handoff = captcha_detected
                            && (view.settings.config_tree
                                .pointer("/miscellaneous/solve_backpage_captcha")
                                .and_then(serde_json::Value::as_bool)
                                .unwrap_or(false)
                                || view.settings.config_tree
                                    .pointer("/bot_settings/other/always_show_captcha")
                                    .and_then(serde_json::Value::as_bool)
                                    .unwrap_or(false));
                        if manual_handoff {
                            view.login_tab = LoginTab::ExistingSession;
                            view.login_error = Some(format!(
                                "{error}\nComplete the captcha in your browser, then paste its session dosid in SID login."
                            ));
                            view.activity = "Backpage captcha detected; waiting for a manual session-ID login.".into();
                        } else {
                            view.login_error = Some(error.clone());
                            view.activity = error;
                        }
                    }
                }
                if let Some((minutes, resume)) = view.refresh_pause_after_reload.take() {
                    view.activity = format!("Client refreshed; paused for {minutes} minutes.");
                    let this = this.clone();
                    cx.spawn(async move |_, cx| {
                        cx.background_executor()
                            .timer(Duration::from_secs(minutes.saturating_mul(60)))
                            .await;
                        let _ = this.update(cx, |view, cx| {
                            if view.connection_generation != generation || !view.session_connected {
                                return;
                            }
                            if resume && !view.behavior_running && !view.behavior_starting {
                                if view.bridge_ready && view.catalog_ready {
                                    view.run_behavior(cx, None);
                                } else {
                                    view.resume_after_reload = true;
                                }
                            }
                            cx.notify();
                        });
                    })
                    .detach();
                }
                cx.notify();
            });
            if monitor.is_some() {
                // Shared-memory frames drive the map in the native backend;
                // the pipe only carries a cheap status request once per
                // second, never state reads. The legacy backend polls STATE
                // slowly instead — see spawn_live_monitor.
                Self::spawn_live_monitor(this.clone(), generation, cx);
                let mut polls: u32 = 0;
                loop {
                    cx.background_executor().timer(Duration::from_secs(1)).await;
                    polls = polls.wrapping_add(1);
                    // Read the bridge live so a backend switch rebinds this
                    // monitor without relaunching the client.
                    let bridge = this
                        .update(cx, |view, _cx| {
                            if view.connection_generation != generation || !view.session_connected {
                                return None;
                            }
                            view.bridge.clone()
                        })
                        .unwrap_or(None);
                    let Some(bridge) = bridge else {
                        break;
                    };
                    let ping_at = Instant::now();
                    let polled_bridge = bridge.clone();
                    let result = cx
                        .background_executor()
                        .spawn(async move { bridge.status().map_err(|error| error.to_string()) })
                        .await;
                    let ping_ms = ping_at.elapsed().as_millis() as u64;
                    // Boosters change on the order of minutes and the graphical
                    // clients observe them on the player thread, so poll slowly.
                    let boosters = if polls % 10 == 1
                        && result.as_ref().is_ok_and(|status| status.ready)
                    {
                        let client = Arc::clone(&polled_bridge);
                        cx.background_executor()
                            .spawn(async move {
                                client.feature_state().ok().and_then(|state| state.boosters)
                            })
                            .await
                    } else {
                        None
                    };
                    let stopped_tick = this
                        .update(cx, |view, _| {
                            if view.connection_generation != generation
                                || !view.session_connected
                                || view.engine.as_ref().is_some_and(|engine| engine.is_running())
                            {
                                return None;
                            }
                            let engine = view.engine.clone()?;
                            Some((
                                engine,
                                Arc::clone(&polled_bridge),
                                view.settings.module_config(),
                                result
                                    .as_ref()
                                    .ok()
                                    .filter(|status| status.ready)
                                    .and_then(|_| view.snapshot.clone()),
                            ))
                        })
                        .unwrap_or(None);
                    if let Some((engine, client, config, snapshot)) = stopped_tick {
                        let result = cx
                            .background_executor()
                            .spawn(async move {
                                engine.tick_stopped(client, &config, snapshot.as_ref())
                            })
                            .await;
                        if let Err(error) = result {
                            let _ = this.update(cx, |view, cx| {
                                if view.connection_generation == generation {
                                    view.activity = format!("Stopped behavior tick failed: {error}");
                                    cx.notify();
                                }
                            });
                        }
                    }
                    let still_active = this
                        .update(cx, |view, cx| {
                            if view.connection_generation != generation || !view.session_connected {
                                return false;
                            }
                            if !view.bridge.as_ref().is_some_and(|current| Arc::ptr_eq(current, &polled_bridge)) {
                                return true;
                            }
                            view.status_refreshes += 1;
                            view.bridge_ping_ms = Some(ping_ms);
                            if let Some(boosters) = boosters {
                                view.boosters = boosters;
                                view.boosters_at = Instant::now();
                            }
                            view.sample_process_usage();
                            match result {
                                Ok(status) => {
                                    if view.settings.client == ClientKind::Flash {
                                        view.live_fps = status
                                            .details
                                            .get("fps_milli")
                                            .and_then(|v| v.as_u64())
                                            .unwrap_or(0)
                                            as f32
                                            / 1000.0;
                                        view.flash_capture_us = status
                                            .details
                                            .get("capture_us_avg")
                                            .and_then(|v| v.as_u64())
                                            .unwrap_or(0)
                                            .min(u32::MAX as u64)
                                            as u32;
                                    }
                                    let was_ready = view.bridge_ready;
                                    view.bridge_ready = status.ready;
                                    view.bridge_label = if status.ready {
                                        "Game ready".into()
                                    } else if status.connected {
                                        "Game client connected; loading world".into()
                                    } else {
                                        "Game disconnected".into()
                                    };
                                    if status.ready && !was_ready {
                                        #[cfg(windows)]
                                        if kind == ClientKind::Flash
                                            && view.hide_flash_after_startup
                                        {
                                            view.hide_flash_after_startup = false;
                                            if let Some(pid) = view.game_pid {
                                                // Flash must activate visibly before Flex/game startup.
                                                // Hide only after the loaded hero/map proves startup completed.
                                                if set_client_window_visible(pid, false).is_ok() {
                                                    view.client_window_visible = false;
                                                }
                                            }
                                        }
                                        view.activity = format!(
                                            "{} is ready for the behavior engine.",
                                            kind.label()
                                        );
                                    }
                                }
                                Err(error) => {
                                    view.bridge_ready = false;
                                    view.bridge_label = format!("Game connection unavailable: {error}");
                                    view.snapshot = None;
                                }
                            }
                            if let Some(engine) = view.engine.clone() {
                                if view.behavior_running {
                                    // The worker reports its state every tick —
                                    // surface it so a silently idle or failed
                                    // module is visible instead of the stale
                                    // "accepted" message.
                                    if !engine.is_running() {
                                        view.behavior_running = false;
                                        view.update_statistics_policy(false);
                                        if let Some(page) = view.module_pages.get("__plugins") {
                                            page.update(cx, |page, cx| page.set_locked(false, cx));
                                        }
                                    }
                                    if let Some(line) = engine.report() {
                                        view.activity = line;
                                    }
                                }
                            }
                            if view.resume_after_reload
                                && view.bridge_ready
                                && view.catalog_ready
                                && !view.behavior_starting
                            {
                                view.resume_after_reload = false;
                                view.run_behavior(cx, None);
                            }
                            for action in behavior::take_host_actions() {
                                match action {
                                    HostAction::Refresh => view.refresh_with_policy(cx),
                                    HostAction::RefreshIdle => view.reload_client_with_resume(Some(false), cx),
                                    HostAction::RequestRefresh => {}, // Runtime must approve cooperative requests.
                                    HostAction::Disconnect => {
                                        view.clear_safe_stop_request();
                                        view.stop_client(cx);
                                    }
                                    HostAction::Close => {
                                        view.clear_safe_stop_request();
                                        view.close_application(cx);
                                    }
                                    HostAction::Profile(name) => view.start_profile(name, cx),
                                    HostAction::TagPlayer { id, tag } => {
                                        view.settings.player_tags.insert(id, tag);
                                        view.save_settings();
                                    }
                                    HostAction::SelectTarget(_) => {}
                                    HostAction::AcceptUrgentQuest | HostAction::ClaimCalendar => {
                                        view.activity = "A requested game service is unavailable in this client session.".into();
                                    }
                                }
                            }
                            cx.notify();
                            true
                        })
                        .unwrap_or(false);
                    if !still_active {
                        break;
                    }
                }
            }
        })
        .detach();
    }
}
