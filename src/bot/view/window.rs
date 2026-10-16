//! Native window handling: pin on top, tray, and the separate settings window.

use super::*;

impl BotView {
    pub(super) fn app_title(&self) -> String {
        let include_username = self
            .settings
            .config_tree
            .pointer("/miscellaneous/username_on_title")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        if !include_username {
            return "Xenomit Bot".into();
        }
        let name = if self.session_username.trim().is_empty() {
            self.session
                .as_ref()
                .map(|session| session.user_id.to_string())
        } else {
            Some(self.session_username.clone())
        };
        name.map_or_else(
            || "Xenomit Bot".into(),
            |name| format!("Xenomit Bot — {name}"),
        )
    }

    pub(super) fn render_exit_confirmation(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgb(0x000000).alpha(0.68))
            .child(
                div()
                    .w(px(360.0))
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    .p(px(18.0))
                    .rounded_lg()
                    .bg(rgb(PANEL))
                    .border_1()
                    .border_color(rgb(LINE))
                    .shadow_lg()
                    .child(section("EXIT XENOMIT"))
                    .child(label(
                        "Close the bot and stop the game client?",
                        MUTED,
                        12.0,
                    ))
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap(px(8.0))
                            .child(button("cancel-exit", "Cancel", Btn::Ghost).on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.exit_confirmation_open = false;
                                    cx.notify();
                                }),
                            ))
                            .child(button("confirm-exit", "Exit", Btn::Danger).on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.exit_confirmation_open = false;
                                    this.close_application(cx);
                                }),
                            )),
                    ),
            )
    }

    /// Open the settings window, or close it when it is already open.
    pub(super) fn toggle_settings_window(&mut self, cx: &mut Context<Self>) {
        if let Some(handle) = self.settings_window.take() {
            let _ = handle.update(cx, |_, window, _| window.remove_window());
            self.settings_open = false;
            cx.notify();
            return;
        }
        if self.settings_open {
            // The open is queued until this entity update completes. A second
            // click cancels that pending open.
            self.settings_open = false;
            cx.notify();
            return;
        }

        self.settings_open = true;
        cx.notify();
        let bot = cx.entity();
        cx.defer(move |cx| {
            // SettingsHost renders BotView in the new window. Open only after
            // the header's BotView update has returned, or GPUI can reject the
            // nested update while the window is first laid out.
            if !bot.read(cx).settings_open {
                return;
            }
            let bounds = Bounds::centered(None, size(px(680.0), px(520.0)), cx);
            let bot_for_window = bot.clone();
            let opened = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Xenomit Bot".into()),
                        appears_transparent: true,
                        ..Default::default()
                    }),
                    window_decorations: Some(WindowDecorations::Client),
                    window_min_size: Some(size(px(600.0), px(420.0))),
                    ..Default::default()
                },
                move |window, cx| {
                    let title = bot_for_window.read(cx).app_title();
                    window.set_window_title(&title);
                    let weak = bot_for_window.downgrade();
                    window.on_window_should_close(cx, move |_, cx| {
                        let _ = weak.update(cx, |bot, cx| {
                            bot.settings_window = None;
                            bot.settings_open = false;
                            cx.notify();
                        });
                        true
                    });
                    let host = cx.new(|_| SettingsHost {
                        bot: bot_for_window.clone(),
                    });
                    cx.new(|cx| Root::new(host, window, cx))
                },
            );
            match opened {
                Ok(handle) => {
                    let _ = bot.update(cx, |bot, cx| {
                        if bot.settings_open {
                            bot.settings_window = Some(handle);
                            cx.notify();
                        } else {
                            let _ = handle.update(cx, |_, window, _| window.remove_window());
                        }
                    });
                }
                Err(error) => {
                    let _ = bot.update(cx, |bot, cx| {
                        bot.settings_open = false;
                        bot.activity = format!("Could not open the settings window: {error}");
                        cx.notify();
                    });
                }
            }
        });
    }

    /// Hide the main window to the notification area (Windows).
    pub(super) fn minimize_to_tray(&mut self, cx: &mut Context<Self>) {
        #[cfg(windows)]
        {
            if let Some(handle) = self.settings_window.take() {
                let _ = handle.update(cx, |_, window, _| window.remove_window());
                self.settings_open = false;
            }
            match self.own_hwnd() {
                Some(hwnd) => {
                    if let Err(error) = crate::bot::tray::hide_to_tray(hwnd as isize) {
                        self.activity = error;
                    }
                }
                None => self.activity = "The main window could not be located.".into(),
            }
        }
        #[cfg(not(windows))]
        {
            self.activity = "Minimize to tray is supported on Windows only.".into();
        }
        cx.notify();
    }

    /// The settings UI for the separate window.
    pub(crate) fn render_settings_window(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.install_module_catalog(window, cx);
        self.sync_settings_module_selector(window, cx);
        let mut root = div()
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .bg(rgb(BG))
            .text_color(rgb(TEXT))
            .font_family("Segoe UI")
            .child(self.render_header(window, cx, false))
            .child(self.render_settings_body(cx));
        if self.menu_open {
            root = root
                .child(div().absolute().inset_0().on_mouse_down(
                    gpui_kit::MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.menu_open = false;
                        cx.notify();
                    }),
                ))
                .child(self.render_menu(cx));
        }
        if self.stats_open {
            root = root
                .child(div().absolute().inset_0().on_mouse_down(
                    gpui_kit::MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.stats_open = false;
                        cx.notify();
                    }),
                ))
                .child(self.render_stats_popup(cx));
        }
        root.into_any_element()
    }

    /// Pin window: toggle the always-on-top state through `SetWindowPos`.
    pub(super) fn toggle_always_on_top(&mut self, cx: &mut Context<Self>) {
        #[cfg(windows)]
        {
            let pin = !self.always_on_top;
            match self.apply_topmost(pin) {
                Ok(()) => {
                    self.always_on_top = pin;
                    self.activity = if pin {
                        "Window pinned above other windows.".into()
                    } else {
                        "Window unpinned.".into()
                    };
                }
                Err(error) => self.activity = error,
            }
        }
        #[cfg(not(windows))]
        {
            self.activity = "Pin window is supported on Windows only.".into();
        }
        cx.notify();
    }

    /// Locate (and cache) this window's HWND by process id and the
    /// "Xenomit" title prefix used by both the sign-in and dashboard titles.
    #[cfg(windows)]
    pub(super) fn own_hwnd(&mut self) -> Option<windows_sys::Win32::Foundation::HWND> {
        use windows_sys::Win32::Foundation::{HWND, LPARAM};
        use windows_sys::Win32::System::Threading::GetCurrentProcessId;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            EnumWindows, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
            WNDENUMPROC,
        };
        if let Some(hwnd) = self.main_hwnd {
            return Some(hwnd as HWND);
        }
        struct Search {
            pid: u32,
            hwnd: HWND,
        }
        unsafe extern "system" fn find(hwnd: HWND, lparam: LPARAM) -> i32 {
            let search = unsafe { &mut *(lparam as *mut Search) };
            let mut pid = 0;
            unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
            if pid != search.pid {
                return 1;
            }
            let len = unsafe { GetWindowTextLengthW(hwnd) };
            if len <= 0 {
                return 1;
            }
            let mut buf = vec![0u16; len as usize + 1];
            let written = unsafe { GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
            if written <= 0 {
                return 1;
            }
            let title = String::from_utf16_lossy(&buf[..written as usize]);
            if title.starts_with("Xenomit") {
                search.hwnd = hwnd;
                0
            } else {
                1
            }
        }
        let mut search = Search {
            pid: unsafe { GetCurrentProcessId() },
            hwnd: std::ptr::null_mut(),
        };
        let callback: WNDENUMPROC = Some(find);
        unsafe { EnumWindows(callback, &mut search as *mut Search as LPARAM) };
        if search.hwnd.is_null() {
            None
        } else {
            self.main_hwnd = Some(search.hwnd as isize);
            Some(search.hwnd)
        }
    }

    #[cfg(windows)]
    pub(super) fn apply_topmost(&mut self, pin: bool) -> Result<(), String> {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos,
        };
        let hwnd = self
            .own_hwnd()
            .ok_or("Could not find the Xenomit window to pin.")?;
        let ok = unsafe {
            SetWindowPos(
                hwnd,
                if pin { HWND_TOPMOST } else { HWND_NOTOPMOST },
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            )
        };
        if ok == 0 {
            Err("Could not change the window's always-on-top state.".into())
        } else {
            Ok(())
        }
    }
}
