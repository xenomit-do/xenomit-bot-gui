//! Start/stop of the behavior runtime and panel command replies.

use super::*;

impl BotView {
    /// Optional `command_result` sink for panel-issued start/stop.
    pub(super) fn run_behavior(
        &mut self,
        cx: &mut Context<Self>,
        panel_reply: Option<panel::CommandReply>,
    ) {
        if self.behavior_starting || self.behavior_running {
            Self::panel_fail(&panel_reply, "Module is already starting or running");
            return;
        }
        if self.catalog_ready && self.module_descriptors.is_empty() {
            self.activity = "Enable a plugin with an objective module before starting.".into();
            Self::panel_fail(&panel_reply, self.activity.clone());
            cx.notify();
            return;
        }
        if !self.catalog_ready
            || !self
                .module_descriptors
                .iter()
                .any(|module| module.id == self.settings.active_module)
            || self
                .module_pages
                .get(&self.settings.active_module)
                .is_none_or(|page| page.read(cx).has_errors())
            || self
                .module_pages
                .get("__behaviors")
                .is_none_or(|page| page.read(cx).has_errors())
            || self
                .module_pages
                .get("__plugins")
                .is_none_or(|page| page.read(cx).has_errors())
        {
            self.activity =
                "Wait for module discovery and correct invalid settings before starting.".into();
            Self::panel_fail(&panel_reply, self.activity.clone());
            cx.notify();
            return;
        }
        if self.busy {
            self.activity = "Wait for the client to finish stopping or starting first.".into();
            Self::panel_fail(&panel_reply, self.activity.clone());
            cx.notify();
            return;
        }
        let Some(engine) = self.engine.clone() else {
            self.activity = "The behavior engine adapter is unavailable.".into();
            Self::panel_fail(&panel_reply, self.activity.clone());
            cx.notify();
            return;
        };
        let Some(client) = self.bridge.clone() else {
            self.activity = "The game must finish loading before the bot can start.".into();
            Self::panel_fail(&panel_reply, self.activity.clone());
            cx.notify();
            return;
        };
        let config = BotSettings {
            game_api_backend: self.client_backend,
            ..self.settings.clone()
        }
        .module_config();
        if let Some(page) = self.module_pages.get("__plugins") {
            page.update(cx, |page, cx| page.set_locked(true, cx));
        }
        self.activity = format!("Starting {}…", self.module_name(&config.module_id));
        self.behavior_starting = true;
        let generation = self.connection_generation;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let start_engine = engine.clone();
            let result = cx
                .background_executor()
                .spawn(async move { start_engine.start(client, config) })
                .await;
            let stale = this.update(cx, |view, cx| {
                view.behavior_starting = false;
                if view.connection_generation != generation {
                    if let Some(page) = view.module_pages.get("__plugins") {
                        page.update(cx, |page, cx| page.set_locked(view.plugins_locked(), cx));
                    }
                    Self::panel_fail(&panel_reply, "Connection reset while starting");
                    cx.notify();
                    return true;
                }
                match result {
                    Ok(()) => {
                        view.behavior_running = engine.is_running();
                        view.update_statistics_policy(view.behavior_running);
                        view.activity = "Behavior engine accepted the selected module.".into();
                        Self::panel_ok(&panel_reply);
                    }
                    Err(error) => {
                        view.activity = format!("Could not start behavior: {error}");
                        view.update_statistics_policy(false);
                        Self::panel_fail(&panel_reply, error);
                    }
                }
                if let Some(page) = view.module_pages.get("__plugins") {
                    page.update(cx, |page, cx| page.set_locked(view.plugins_locked(), cx));
                }
                cx.notify();
                false
            });
            if stale.unwrap_or(true) {
                let _ = cx
                    .background_executor()
                    .spawn(async move { engine.stop() })
                    .await;
            }
        })
        .detach();
    }

    pub(super) fn stop_behavior(
        &mut self,
        cx: &mut Context<Self>,
        panel_reply: Option<panel::CommandReply>,
    ) {
        let Some(engine) = self.engine.clone() else {
            self.activity = "The behavior engine adapter is unavailable.".into();
            Self::panel_fail(&panel_reply, self.activity.clone());
            cx.notify();
            return;
        };
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { engine.stop() })
                .await;
            let _ = this.update(cx, |view, cx| {
                view.behavior_running = false;
                view.update_statistics_policy(false);
                match result {
                    Ok(()) => {
                        view.activity = "Behavior stopped.".into();
                        Self::panel_ok(&panel_reply);
                    }
                    Err(error) => {
                        view.activity = format!("Behavior stopped with an error: {error}");
                        Self::panel_fail(&panel_reply, error);
                    }
                }
                if let Some(page) = view.module_pages.get("__plugins") {
                    page.update(cx, |page, cx| page.set_locked(view.plugins_locked(), cx));
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn panel_ok(reply: &Option<panel::CommandReply>) {
        if let Some((link, id)) = reply {
            link.reply(id, true, None, None);
        }
    }

    pub(super) fn panel_fail(reply: &Option<panel::CommandReply>, error: impl Into<String>) {
        if let Some((link, id)) = reply {
            link.reply(id, false, Some(error.into()), None);
        }
    }
}
