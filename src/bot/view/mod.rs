use std::cell::Cell;
use std::collections::BTreeMap;
use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gpui_kit::component::Root;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::component::{
    Sizable,
    select::{Select, SelectEvent, SelectItem, SelectState},
};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, Bounds, Context, ElementId, Entity, Focusable, FontWeight, Pixels, Render,
    SharedString, Stateful, TitlebarOptions, Window, WindowBounds, WindowControlArea,
    WindowDecorations, WindowHandle, WindowOptions, canvas, div, px, rgb, size,
};
use serde_json::json;
use xenomit_backpage::session::Session;
use xenomit_behavior_engine::HostAction;
use xenomit_bot_runtime::panel::PanelLink;
use xenomit_bot_runtime::session::{BackpageSessionProvider, SessionCredentials, SessionProvider};
use xenomit_bot_runtime::settings::{
    BotSettings, ClientKind, FileSettingsStore, FlashBackend, MapZone, MapZoneKind, SettingsStore,
};
use xenomit_bot_runtime::statistics::{self, SharedStats, Statistics};
use xenomit_bot_runtime::{behavior, packet_client, panel, star_catalog};
use xenomit_game_api::live::{LiveEntity, LiveFrame, LiveKind};
use xenomit_game_api::navigation::StarSystem;
use xenomit_game_api::{
    BehaviorRuntime, ClientBackend, FunctionCall, GameClient, GameSnapshot, PluginDescriptor,
    SettingDescriptor, SettingKind,
};

use super::config_tree::{ConfigChanged, ConfigTreePage};
use super::live_map::{self, HeroTrail};
use super::module_settings::{ModuleSettingsPage, SettingsChanged};
use super::settings_window::SettingsHost;
use xenomit_game_api::ModuleDescriptor;

mod account;
mod catalog;
mod chrome;
mod client;
mod launch;
mod lifecycle;
mod live;
mod login;
mod map;
mod map_zones;
mod pages;
mod platform;
mod remote_panel;
mod run_control;
mod saved_credentials;
mod settings;
mod stats_page;
mod theme;
mod window;

use catalog::*;
use launch::*;
use map::*;
use platform::*;
use saved_credentials::{SavedCredential, SavedCredentialStore};
use settings::*;
pub(crate) use theme::*;

pub(super) struct BotView {
    module_descriptors: Vec<ModuleDescriptor>,
    all_module_descriptors: Vec<ModuleDescriptor>,
    plugin_descriptors: Vec<PluginDescriptor>,
    module_pages: BTreeMap<String, Entity<ModuleSettingsPage>>,
    /// Reference style settings tree (the General tab); see `config_tree/`.
    config_page: Entity<ConfigTreePage>,
    settings_module_selector: Option<Entity<SelectState<Vec<ModuleChoice>>>>,
    settings_module_selector_value: String,
    settings_category: String,
    module_subscriptions: Vec<gpui_kit::Subscription>,
    pending_profile_sync: Option<(String, serde_json::Value)>,
    resume_after_reload: bool,
    refresh_pause_after_reload: Option<(u64, bool)>,
    pending_catalog: Option<Result<(Vec<ModuleDescriptor>, Vec<PluginDescriptor>), String>>,
    catalog_loading: bool,
    catalog_ready: bool,
    catalog_error: Option<String>,
    behavior_starting: bool,
    statistics: Option<SharedStats>,
    statistics_range: u64,
    server: Entity<InputState>,
    sid: Entity<InputState>,
    username: Entity<InputState>,
    password: Entity<InputState>,
    settings: BotSettings,
    top_tab: String,
    settings_open: bool,
    /// The separate settings window while it is open.
    settings_window: Option<WindowHandle<Root>>,
    /// The tick/memory/FPS/ping popup under the header's metrics.
    stats_open: bool,
    /// Working-set size of the game client process.
    game_mem_bytes: u64,
    login_dialog_open: bool,
    exit_confirmation_open: bool,
    session_username: String,
    login_tab: LoginTab,
    client_picker_open: bool,
    busy: bool,
    connecting_saved_credential: Option<usize>,
    login_error: Option<String>,
    saved_credentials_path: PathBuf,
    saved_credentials: Vec<SavedCredential>,
    saved_credentials_error: Option<String>,
    session_label: String,
    session_connected: bool,
    connection_generation: u64,
    dashboard_visible: bool,
    game_process: Option<Child>,
    game_pid: Option<u32>,
    client_window_visible: bool,
    hide_flash_after_startup: bool,
    bridge: Option<Arc<dyn GameClient>>,
    /// Backend the running client was launched with.
    client_backend: ClientBackend,
    client_flash_backend: FlashBackend,
    bridge_ready: bool,
    bridge_label: String,
    snapshot: Option<GameSnapshot>,
    live_frame: LiveFrame,
    live_fps: f32,
    flash_capture_us: u32,
    live_fps_frames: u32,
    live_fps_since: Instant,
    hero_trail: HeroTrail,
    /// Last map notification; `map_display.refresh_delay` throttles painting.
    last_map_repaint_at: Instant,
    /// Ship destructions counted this session from the killed-state edge.
    deaths: u32,
    /// Previous frame's killed flag — rising edges bump `deaths`.
    hero_dead: bool,
    /// Star catalog fetched from the CDN at login (`maps-config` +
    /// `spacemap-config`); fills `navigation.star_system`/`portals` on
    /// backends without live capture. `None` when the fetch failed.
    star_catalog: Option<StarSystem>,
    /// Last painted bounds of the map canvas, for click-to-move.
    map_canvas_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// Set while the left button is held after a press inside the map
    /// canvas; the drag keeps retargeting until the button is released.
    map_dragging: bool,
    map_click_target: Option<(u64, i32, i32, i32, Instant)>,
    /// Last click/drag navigation call, used to throttle drag updates.
    map_nav_at: Instant,
    zone_drawing: bool,
    zone_drag_start: Option<(i32, f64, f64)>,
    zone_draft: Option<MapZone>,
    zone_hovered: Option<(i32, usize)>,
    client_logs: Arc<Mutex<VecDeque<String>>>,
    activity: String,
    engine: Option<Arc<dyn BehaviorRuntime>>,
    behavior_running: bool,
    config_path: PathBuf,
    /// `BotSettings` persistence — the file store today; the seam lets a
    /// front-end swap in read-only/control-plane storage.
    settings_store: Arc<dyn SettingsStore>,
    /// Backpage session behind the connected client — powers the Backpage
    /// link and the SID expiry countdown in the telemetry strip.
    session: Option<Session>,
    /// Status polls completed this session.
    status_refreshes: u32,
    /// Last observed active booster categories, with when they were read so
    /// the countdown keeps ticking between polls.
    boosters: Vec<xenomit_game_api::BoosterState>,
    boosters_at: Instant,
    /// Game client reloads since sign-in — the reference implementation's refresh counter.
    reload_count: u32,
    /// When the current game client connection was established.
    last_reload: Instant,
    /// Unix seconds when the current Backpage session was established.
    session_started_secs: u64,
    /// Round-trip of the last `bridge.status()` poll.
    bridge_ping_ms: Option<u64>,
    /// Last CPU sample (instant, total CPU 100ns ticks) for the delta rate.
    cpu_sample: Option<(Instant, u64)>,
    cpu_percent: f64,
    /// Working-set size of this process.
    mem_bytes: u64,
    /// Pin-window state: the app floats above other windows when set.
    always_on_top: bool,
    /// Hamburger-menu dropdown open state.
    menu_open: bool,
    /// Cached HWND of this window for `SetWindowPos` (pin window).
    #[cfg(windows)]
    main_hwnd: Option<isize>,
    /// Shared endpoint for the web-panel agent thread. Spawned once at
    /// startup; idles while no instance token is configured.
    panel: Arc<PanelLink>,
    panel_url: Entity<InputState>,
    panel_code: Entity<InputState>,
    /// Descriptors of the retained `__behaviors`/`__plugins` pages, exported
    /// to the panel so global rules are remotely editable too.
    behaviors_descriptor: Option<ModuleDescriptor>,
    plugins_descriptor: Option<ModuleDescriptor>,
    /// Remote config edits waiting to be applied to the retained settings
    /// pages on the next render.
    pending_remote_sync: Vec<(String, serde_json::Value)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LoginTab {
    Account,
    ExistingSession,
    SavedCreds,
}

impl BotView {
    pub(crate) fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
        engine: Option<Arc<dyn BehaviorRuntime>>,
    ) -> Self {
        let settings_store: Arc<dyn SettingsStore> = Arc::new(FileSettingsStore::default());
        let settings = settings_store.load();
        let client_backend = settings.game_api_backend;
        let client_flash_backend = settings.flash_backend;
        let server = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value("int1")
                .placeholder("e.g. int1")
        });
        let sid = cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .placeholder("Paste the session dosid")
        });
        let username = cx.new(|cx| InputState::new(window, cx).placeholder("Backpage username"));
        let password = cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .placeholder("Password")
        });
        let config_path = settings_path();
        let saved_credentials_path = config_path.with_file_name("saved-credentials.bin");
        let (saved_credentials, saved_credentials_error) =
            match SavedCredentialStore::load(&saved_credentials_path) {
                Ok(credentials) => (credentials, None),
                Err(error) => (Vec::new(), Some(error)),
            };
        window.focus(&username.read(cx).focus_handle(cx), cx);
        let panel_link = Arc::new(PanelLink::default());
        panel_link.configure(settings.panel.clone());
        panel::spawn_agent(panel_link.clone());
        let panel_url = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(settings.panel.url.clone())
                .placeholder("https://panel.example.com")
        });
        let panel_code =
            cx.new(|cx| InputState::new(window, cx).placeholder("Pairing code from the panel"));

        let config_page = cx.new(|cx| {
            ConfigTreePage::new(xenomit_bot_runtime::settings::config_root(), window, cx)
        });
        let mut view = Self {
            module_descriptors: Vec::new(),
            all_module_descriptors: Vec::new(),
            plugin_descriptors: Vec::new(),
            module_pages: BTreeMap::new(),
            config_page: config_page.clone(),
            settings_module_selector: None,
            settings_module_selector_value: String::new(),
            settings_category: "General".into(),
            module_subscriptions: Vec::new(),
            pending_profile_sync: None,
            resume_after_reload: false,
            refresh_pause_after_reload: None,
            pending_catalog: None,
            catalog_loading: false,
            catalog_ready: false,
            catalog_error: None,
            behavior_starting: false,
            statistics: None,
            statistics_range: 24 * 3_600_000,
            server,
            sid,
            username,
            password,
            settings,
            top_tab: "general".into(),
            settings_open: false,
            settings_window: None,
            stats_open: false,
            game_mem_bytes: 0,
            login_dialog_open: true,
            exit_confirmation_open: false,
            session_username: String::new(),
            login_tab: LoginTab::Account,
            client_picker_open: false,
            busy: false,
            connecting_saved_credential: None,
            login_error: None,
            saved_credentials_path,
            saved_credentials,
            saved_credentials_error,
            session_label: "Not connected".into(),
            session_connected: false,
            connection_generation: 0,
            dashboard_visible: false,
            game_process: None,
            game_pid: None,
            client_window_visible: true,
            hide_flash_after_startup: false,
            bridge: None,
            client_backend,
            client_flash_backend,
            bridge_ready: false,
            bridge_label: "No game client connected".into(),
            snapshot: None,
            live_frame: LiveFrame::default(),
            live_fps: 0.0,
            flash_capture_us: 0,
            live_fps_frames: 0,
            live_fps_since: Instant::now(),
            hero_trail: HeroTrail::default(),
            last_map_repaint_at: Instant::now(),
            deaths: 0,
            hero_dead: false,
            star_catalog: None,
            map_canvas_bounds: Rc::new(Cell::new(None)),
            map_dragging: false,
            map_click_target: None,
            map_nav_at: Instant::now(),
            zone_drawing: false,
            zone_drag_start: None,
            zone_draft: None,
            zone_hovered: None,
            client_logs: Arc::new(Mutex::new(VecDeque::new())),
            activity: "Ready. Authenticate a Backpage session to begin.".into(),
            engine,
            behavior_running: false,
            config_path,
            settings_store,
            session: None,
            status_refreshes: 0,
            boosters: Vec::new(),
            boosters_at: Instant::now(),
            reload_count: 0,
            last_reload: Instant::now(),
            session_started_secs: 0,
            bridge_ping_ms: None,
            cpu_sample: None,
            cpu_percent: 0.0,
            mem_bytes: 0,
            always_on_top: false,
            menu_open: false,
            #[cfg(windows)]
            main_hwnd: None,
            panel: panel_link,
            panel_url,
            panel_code,
            behaviors_descriptor: None,
            plugins_descriptor: None,
            pending_remote_sync: Vec::new(),
        };
        let close_view = cx.entity().downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            let bounds = window.bounds();
            let _ = close_view.update(cx, |view, cx| {
                let save_position = view
                    .settings
                    .config_tree
                    .pointer("/bot_settings/bot_gui/save_gui_pos")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);
                if save_position {
                    view.settings.gui_window_bounds = Some([
                        f32::from(bounds.origin.x) as i32,
                        f32::from(bounds.origin.y) as i32,
                        f32::from(bounds.size.width) as i32,
                        f32::from(bounds.size.height) as i32,
                    ]);
                    view.save_settings();
                }
                let confirm = view
                    .settings
                    .config_tree
                    .pointer("/bot_settings/bot_gui/confirm_exit")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(true);
                if confirm {
                    view.exit_confirmation_open = true;
                    cx.notify();
                } else {
                    view.close_application(cx);
                }
            });
            false
        });
        // Independent background setup overlaps authentication/client loading.
        view.module_subscriptions.push(
            cx.subscribe(&config_page, |view, _page, _: &ConfigChanged, cx| {
                view.apply_config_tree(cx)
            }),
        );
        // Pushes a previously saved tree into the settings (or waits for the
        // module catalog when the profile still needs seeding).
        view.apply_config_tree(cx);
        view.load_module_catalog(cx);
        // The panel pump ticks independently of a game session so remote
        // commands and status reach the agent even while signed out.
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let alive = this
                    .update(cx, |view, cx| {
                        view.push_panel_state();
                        for command in view.panel.take_commands() {
                            view.handle_panel_command(command, cx);
                        }
                        let acks = view.panel.take_stat_acks();
                        if let (Some(stats), false) = (&view.statistics, acks.is_empty())
                            && let Ok(mut stats) = stats.lock()
                        {
                            for cursor in acks {
                                stats.ack(cursor);
                            }
                        }
                    })
                    .is_ok();
                if !alive {
                    break;
                }
            }
        })
        .detach();
        view
    }
}

impl Drop for BotView {
    fn drop(&mut self) {
        self.panel.shutdown();
        if self.behavior_running {
            if let Some(engine) = &self.engine {
                let _ = engine.stop();
            }
        }
        if let Some(mut process) = self.game_process.take() {
            let _ = process.kill();
            let _ = process.wait();
        }
    }
}

impl Render for BotView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.install_module_catalog(window, cx);
        self.apply_pending_profile_sync(window, cx);
        if !self.session_connected {
            if self.dashboard_visible {
                window.resize(gpui_kit::size(px(460.0), px(390.0)));
                window.set_window_title("Login");
                self.dashboard_visible = false;
            }
            let mut root = div().size_full().child(self.render_login_dialog(cx));
            if self.exit_confirmation_open {
                root = root.child(self.render_exit_confirmation(cx));
            }
            return root.into_any_element();
        }
        if !self.dashboard_visible {
            let save_position = self
                .settings
                .config_tree
                .pointer("/bot_settings/bot_gui/save_gui_pos")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            let restored_size = save_position
                .then_some(self.settings.gui_window_bounds)
                .flatten()
                .filter(|bounds| bounds[2] >= 600 && bounds[3] >= 420)
                .map(|bounds| size(px(bounds[2] as f32), px(bounds[3] as f32)))
                .unwrap_or_else(|| size(px(900.0), px(640.0)));
            window.resize(restored_size);
            self.dashboard_visible = true;
        }
        let title = self.app_title();
        window.set_window_title(&title);
        let mut root = div()
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .bg(rgb(BG))
            .text_color(rgb(TEXT))
            .font_family("Segoe UI")
            .child(self.render_header(window, cx, true))
            .child(div().flex_1().child(self.render_main_content(cx)));
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
        if self.login_dialog_open {
            root = root.child(
                div()
                    .absolute()
                    .inset_0()
                    .child(self.render_login_dialog(cx)),
            );
        }
        if self.exit_confirmation_open {
            root = root.child(self.render_exit_confirmation(cx));
        }
        root.into_any_element()
    }
}
