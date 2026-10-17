//! Game client process control: stop, show/hide, backend switching and reload.

use super::*;

impl BotView {
    pub(super) fn stop_client(&mut self, cx: &mut Context<Self>) {
        if let Some(stats) = &self.statistics {
            if let Ok(mut stats) = stats.lock() {
                stats.ended = Some(statistics::now_ms());
                stats.panel = None;
            }
        }
        self.connection_generation += 1;
        self.session_connected = false;
        self.resume_after_reload = false;
        let bridge = self.bridge.take();
        let process = self.game_process.take();
        let host = self.flash_host.take();
        let engine = self.engine.clone();
        self.behavior_running = false;
        self.busy = true;
        self.session_label = "Stopping…".into();
        self.bridge_ready = false;
        self.activity = "Stopping behavior and game client…".into();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let behavior_error = engine.and_then(|engine| engine.stop().err());
                    let mut process_error = None;
                    if let Some(mut process) = process {
                        if let Err(error) = process.kill() {
                            process_error = Some(format!("could not stop client process: {error}"));
                        }
                        let _ = process.wait();
                    }
                    if let Some(host) = host {
                        host.shutdown();
                    }
                    drop(bridge);
                    behavior_error.or(process_error)
                })
                .await;
            let _ = this.update(cx, |view, cx| {
                view.busy = false;
                view.game_pid = None;
                view.client_window_visible = true;
                view.bridge = None;
                view.bridge_ready = false;
                view.snapshot = None;
                view.live_frame = LiveFrame::default();
                view.live_fps = 0.0;
                view.live_fps_frames = 0;
                view.hero_trail.clear();
                view.deaths = 0;
                view.hero_dead = false;
                view.star_catalog = None;
                view.map_canvas_bounds.set(None);
                view.map_dragging = false;
                view.session_connected = false;
                view.session = None;
                behavior::set_backpage_session(None);
                view.status_refreshes = 0;
                view.reload_count = 0;
                view.boosters.clear();
                view.bridge_ping_ms = None;
                view.session_label = "Not connected".into();
                view.bridge_label = "No game client connected".into();
                view.activity = result.unwrap_or_else(|| "Client stopped.".into());
                cx.notify();
            });
        })
        .detach();
    }

    /// Show or hide the game window: through the in-process host when Flash
    /// runs here, otherwise by finding the client process' window.
    pub(super) fn set_game_window_visible(&self, visible: bool) -> Result<(), String> {
        if let Some(host) = &self.flash_host {
            return host
                .set_window(xenomit_game_api::WindowCommand::Visible { visible })
                .map_err(|error| format!("Could not change the Flash window: {error}"));
        }
        let pid = self
            .game_pid
            .ok_or("The game client window is not available.")?;
        set_client_window_visible(pid, visible)
    }

    pub(super) fn toggle_client_window(&mut self, cx: &mut Context<Self>) {
        if self.game_pid.is_none() {
            self.activity = "The game client window is not available.".into();
            cx.notify();
            return;
        }
        let visible = !self.client_window_visible;
        match self.set_game_window_visible(visible) {
            Ok(()) => {
                self.client_window_visible = visible;
                self.activity = if visible {
                    "Game client window shown.".into()
                } else {
                    "Game client window hidden; the client is still running.".into()
                };
            }
            Err(error) => self.activity = error,
        }
        cx.notify();
    }

    /// Rolling CPU/memory sample for the telemetry strip; called once per
    /// status poll so the CPU figure is a per-second rate.
    pub(super) fn sample_process_usage(&mut self) {
        let (sample, cpu, mem) = process_usage(self.cpu_sample);
        self.cpu_sample = sample;
        self.cpu_percent = cpu;
        self.mem_bytes = mem;
        // An in-process host is already part of this process' memory.
        self.game_mem_bytes = if self.flash_host.is_some() {
            0
        } else {
            self.game_pid.map_or(0, process_memory)
        };
    }

    /// The freshly spawned client opens its window a moment after launch;
    /// poll until it exists, then hide it so sessions start with the client
    /// in the background. If it never appears, restore the "visible" state
    /// so the toolbar toggle reports the real situation.
    #[cfg(windows)]
    pub(super) fn hide_client_window_when_ready(
        this: gpui_kit::WeakEntity<Self>,
        generation: u64,
        pid: u32,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |_, cx| {
            for _ in 0..120 {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                let keep_polling = this
                    .update(cx, |view, _| {
                        view.connection_generation == generation
                            && view.session_connected
                            && !view.client_window_visible
                    })
                    .unwrap_or(false);
                if !keep_polling {
                    return;
                }
                let hidden = cx
                    .background_executor()
                    .spawn(async move { set_client_window_visible(pid, false) })
                    .await
                    .is_ok();
                if hidden {
                    return;
                }
            }
            let _ = this.update(cx, |view, cx| {
                if view.connection_generation == generation
                    && view.session_connected
                    && !view.client_window_visible
                {
                    view.client_window_visible = true;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Apply a newly selected game API backend to the running client without
    /// relaunching it: reconnect the bridge to the same process. The status
    /// and live monitors read `view.bridge`/`view.client_backend` every tick,
    /// so they pick the new backend up on their next pass.
    pub(super) fn apply_backend_change(&mut self, cx: &mut Context<Self>) {
        if self.settings.client == ClientKind::Flash {
            self.apply_flash_backend_change(cx);
            return;
        }
        if self.settings.client != ClientKind::Ruffle {
            return;
        }
        let backend = self.settings.game_api_backend;
        let Some(pid) = self.game_pid.filter(|_| self.game_process.is_some()) else {
            return;
        };
        if backend == self.client_backend {
            return;
        }
        let engine = self.behavior_running.then(|| self.engine.clone()).flatten();
        self.behavior_running = false;
        self.busy = true;
        self.activity = format!("Switching the game API backend to {}…", backend.label());
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    if let Some(engine) = engine {
                        engine.stop()?;
                    }
                    xenomit_game_api::connect(pid, backend).map_err(|error| error.to_string())
                })
                .await;
            let _ = this.update(cx, |view, cx| {
                // A later toggle superseded this connect — drop its result.
                if view.game_pid != Some(pid)
                    || view.game_process.is_none()
                    || view.settings.game_api_backend != backend
                {
                    return;
                }
                view.busy = false;
                match result {
                    Ok(bridge) => {
                        view.bridge = Some(bridge);
                        view.client_backend = backend;
                        view.bridge_ready = false;
                        view.snapshot = None;
                        view.live_frame = LiveFrame::default();
                        view.hero_trail.clear();
                        view.hero_dead = false;
                        view.activity = format!("Game API backend is now {}.", backend.label());
                    }
                    Err(error) => {
                        view.activity = format!("Could not switch the game API backend: {error}");
                        view.settings.game_api_backend = view.client_backend;
                        view.save_settings();
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn apply_flash_backend_change(&mut self, cx: &mut Context<Self>) {
        let backend = self.settings.flash_backend;
        let host = self.flash_host.clone();
        let Some(pid) = self
            .game_pid
            .filter(|_| self.game_process.is_some() || host.is_some())
        else {
            return;
        };
        if backend == self.client_flash_backend {
            return;
        }
        let engine = self.behavior_running.then(|| self.engine.clone()).flatten();
        self.behavior_running = false;
        self.busy = true;
        self.activity = "Switching Flash capture backend…".into();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    if let Some(engine) = engine {
                        engine.stop()?;
                    }
                    if let Some(host) = host {
                        // Same in-process client; only its capture mode changes.
                        host.set_backend(backend == FlashBackend::NativeMemory)
                            .map_err(|e| e.to_string())?;
                        return Ok(host as Arc<dyn GameClient>);
                    }
                    let control =
                        xenomit_game_api::FlashClient::connect(pid).map_err(|e| e.to_string())?;
                    control
                        .set_native_backend(backend == FlashBackend::NativeMemory)
                        .map_err(|e| e.to_string())?;
                    let bridge = match backend {
                        FlashBackend::NativeMemory => xenomit_game_api::connect_flash_native(pid),
                        FlashBackend::Cached => xenomit_game_api::connect_flash(pid),
                    };
                    bridge.map_err(|e| e.to_string())
                })
                .await;
            let _ = this.update(cx, |view, cx| {
                if view.game_pid != Some(pid)
                    || (view.game_process.is_none() && view.flash_host.is_none())
                {
                    return;
                }
                view.busy = false;
                match result {
                    Ok(bridge) => {
                        view.bridge = Some(bridge);
                        view.client_flash_backend = backend;
                        view.bridge_ready = false;
                        view.snapshot = None;
                        view.live_frame = LiveFrame::default();
                        view.hero_trail.clear();
                        view.hero_dead = false;
                        view.map_click_target = None;
                        view.activity =
                            "Flash capture backend switched in the running client.".into();
                    }
                    Err(error) => {
                        view.settings.flash_backend = view.client_flash_backend;
                        view.save_settings();
                        view.activity = format!("Could not switch Flash capture backend: {error}");
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn refresh_client(&mut self, cx: &mut Context<Self>) {
        let Some(bridge) = self.bridge.clone() else {
            self.activity = "Authenticate and launch the game client first.".into();
            cx.notify();
            return;
        };
        self.bridge_label = "Checking game connection…".into();
        self.activity = "Reading status and the current game snapshot.".into();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let status = bridge.status().map_err(|error| error.to_string())?;
                    let snapshot = if status.connected {
                        bridge.snapshot(None).ok()
                    } else {
                        None
                    };
                    Ok::<_, String>((status, snapshot))
                })
                .await;
            let _ = this.update(cx, |view, cx| {
                match result {
                    Ok((status, snapshot)) => {
                        view.bridge_ready = status.ready;
                        view.bridge_label = if status.ready {
                            "Game ready".into()
                        } else if status.connected {
                            "Game client connected; loading world".into()
                        } else {
                            "Game disconnected".into()
                        };
                        view.snapshot = snapshot;
                        view.activity = if let Some(snapshot) = &view.snapshot {
                            format!(
                                "Snapshot received: {} visible entities.",
                                snapshot.entities.len()
                            )
                        } else {
                            "Client status updated; no game snapshot is available yet.".into()
                        };
                    }
                    Err(error) => {
                        view.bridge_ready = false;
                        view.bridge_label = "Game connection unavailable".into();
                        view.activity = format!("Game client request failed: {error}");
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}
