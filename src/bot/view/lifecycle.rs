//! Session lifecycle: safe stop, profile switching, client reload and shutdown.

use super::*;

impl BotView {
    pub(super) fn refresh_with_policy(&mut self, cx: &mut Context<Self>) {
        let pause_minutes = self
            .settings
            .config_tree
            .pointer("/miscellaneous/pause_for")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
            .min(720);
        let resume = self.behavior_running
            || self
                .engine
                .as_ref()
                .is_some_and(|engine| engine.is_running());
        if pause_minutes > 0 {
            self.refresh_pause_after_reload = Some((pause_minutes, resume));
            self.reload_client_with_resume(Some(false), cx);
        } else {
            self.reload_client(cx);
        }
    }

    pub(super) fn request_safe_stop(&mut self, close: bool, cx: &mut Context<Self>) {
        if !self.settings.behaviors.is_object() {
            self.settings.behaviors = json!({});
        }
        let behaviors = self
            .settings
            .behaviors
            .as_object_mut()
            .expect("behavior settings initialized as object");
        let stop = behaviors.entry("stop_button").or_insert_with(|| json!({}));
        if !stop.is_object() {
            *stop = json!({});
        }
        if !stop
            .get("options")
            .is_some_and(serde_json::Value::is_object)
        {
            stop["options"] = json!({});
        }
        stop["enabled"] = json!(true);
        stop["tick_stopped"] = json!(true);
        stop["options"]["requested"] = json!(true);
        stop["options"]["close"] = json!(close);
        self.activity = if close {
            "Safe stop requested; the game will close after reaching safety.".into()
        } else {
            "Safe stop requested; the game will disconnect after reaching safety.".into()
        };
        self.save_settings();
        cx.notify();
    }

    pub(super) fn clear_safe_stop_request(&mut self) {
        if let Some(stop) = self
            .settings
            .behaviors
            .get_mut("stop_button")
            .and_then(serde_json::Value::as_object_mut)
        {
            if let Some(options) = stop
                .get_mut("options")
                .and_then(serde_json::Value::as_object_mut)
            {
                options.insert("requested".into(), json!(false));
            }
            self.save_settings();
        }
    }

    pub(super) fn start_profile(&mut self, name: String, cx: &mut Context<Self>) {
        if self.settings.active_behavior_profile == name {
            self.activity = format!("Profile {name} is already active.");
            cx.notify();
            return;
        }
        let Some(mut profile) = self.settings.behavior_profiles.get(&name).cloned() else {
            self.activity = format!("Behavior requested unknown profile: {name}.");
            cx.notify();
            return;
        };
        if !profile.settings.is_object() {
            self.activity = format!("Profile {name} has invalid settings.");
            cx.notify();
            return;
        }
        let Some(engine) = self.engine.clone() else {
            self.activity = "The behavior engine adapter is unavailable.".into();
            cx.notify();
            return;
        };
        let Some(client) = self.bridge.clone() else {
            self.activity =
                format!("Profile {name} is saved, but the game client is disconnected.");
            cx.notify();
            return;
        };
        profile.settings["behaviors"] = self.settings.behaviors.clone();
        let id = profile.module_id.clone();
        let settings = profile.settings.clone();
        self.settings.active_module = id.clone();
        self.settings.modules.insert(id.clone(), settings.clone());
        self.settings.active_behavior_profile = name.clone();
        self.settings_category = "General".into();
        self.behavior_starting = true;
        self.pending_profile_sync = Some((id, settings));
        self.save_settings();
        self.activity = format!("Switching to profile {name}…");
        let generation = self.connection_generation;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let stop_engine = engine.clone();
            let start_engine = engine.clone();
            let result = cx
                .background_executor()
                .spawn(async move {
                    stop_engine.stop()?;
                    start_engine.start(client, profile)
                })
                .await;
            let _ = this.update(cx, |view, cx| {
                if view.connection_generation != generation {
                    return;
                }
                view.behavior_starting = false;
                match result {
                    Ok(()) => {
                        view.behavior_running = engine.is_running();
                        view.update_statistics_policy(view.behavior_running);
                        view.activity = format!("Profile {name} is running.");
                    }
                    Err(error) => {
                        view.behavior_running = false;
                        view.update_statistics_policy(false);
                        view.activity = format!("Could not start profile {name}: {error}");
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn reload_client(&mut self, cx: &mut Context<Self>) {
        self.reload_client_with_resume(None, cx);
    }

    pub(super) fn reload_client_with_resume(
        &mut self,
        resume: Option<bool>,
        cx: &mut Context<Self>,
    ) {
        let Some(session) = self.session.clone() else {
            self.activity = "Cannot reload without the authenticated session.".into();
            cx.notify();
            return;
        };
        let resume_objective = resume.unwrap_or(
            self.behavior_running
                || self
                    .engine
                    .as_ref()
                    .is_some_and(|engine| engine.is_running()),
        );
        let engine = self.engine.clone();
        let process = self.game_process.take();
        let host = self.flash_host.take();
        let bridge = self.bridge.take();
        let auth = SessionCredentials::SessionId {
            server: session.server.clone(),
            sid: session.sid.clone(),
        };
        let generation = self.connection_generation;
        self.connection_generation += 1;
        self.busy = true;
        self.session_connected = false;
        self.behavior_running = false;
        self.behavior_starting = false;
        self.resume_after_reload = resume_objective;
        self.reload_count += 1;
        self.activity = "Reloading the game client…".into();
        self.session_label = "Reloading…".into();
        self.bridge_ready = false;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .spawn(async move {
                    if let Some(engine) = engine {
                        let _ = engine.stop();
                    }
                    // The in-process host must be stopped before it is started again.
                    if let Some(host) = host {
                        host.shutdown();
                    }
                    drop(bridge);
                    if let Some(mut process) = process {
                        let _ = process.kill();
                        let _ = process.wait();
                    }
                })
                .await;
            let _ = this.update(cx, |view, cx| {
                if view.connection_generation != generation + 1 {
                    return;
                }
                view.session_connected = false;
                view.session = None;
                view.game_pid = None;
                view.bridge = None;
                view.bridge_ready = false;
                view.game_process = None;
                view.begin_authentication(auth, None, cx);
            });
        })
        .detach();
    }

    pub(super) fn close_application(&mut self, cx: &mut Context<Self>) {
        let engine = self.engine.clone();
        let process = self.game_process.take();
        let host = self.flash_host.take();
        let bridge = self.bridge.take();
        self.session_connected = false;
        self.behavior_running = false;
        self.behavior_starting = false;
        self.busy = true;
        self.activity = "Closing after stopping the behavior and client…".into();
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .spawn(async move {
                    if let Some(engine) = engine {
                        let _ = engine.stop();
                    }
                    if let Some(host) = host {
                        host.shutdown();
                    }
                    drop(bridge);
                    if let Some(mut process) = process {
                        let _ = process.kill();
                        let _ = process.wait();
                    }
                })
                .await;
            let _ = this.update(cx, |_, cx| cx.quit());
        })
        .detach();
    }

    pub(super) fn apply_pending_profile_sync(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some((id, values)) = self.pending_profile_sync.take() {
            if let Some(page) = self.module_pages.get(&id) {
                page.update(cx, |page, cx| page.sync_settings(values, window, cx));
            } else {
                self.pending_profile_sync = Some((id, values));
            }
        }
        for (id, values) in std::mem::take(&mut self.pending_remote_sync) {
            if let Some(page) = self.module_pages.get(&id) {
                page.update(cx, |page, cx| page.sync_settings(values, window, cx));
            }
        }
    }
}
