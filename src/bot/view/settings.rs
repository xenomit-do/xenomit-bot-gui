//! Settings persistence helpers.

use super::*;

pub(super) fn settings_path() -> PathBuf {
    FileSettingsStore::default_path()
}

impl BotView {
    pub(super) fn update_statistics_policy(&self, behavior_active: bool) {
        let update_while_paused = self
            .settings
            .config_tree
            .pointer("/miscellaneous/update_stats_while_paused")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true);
        if let Some(stats) = &self.statistics
            && let Ok(mut stats) = stats.lock()
        {
            stats.set_updates_enabled(behavior_active || update_while_paused);
        }
    }

    fn active_module_descriptor(&self) -> Option<ModuleDescriptor> {
        self.all_module_descriptors
            .iter()
            .find(|descriptor| descriptor.id == self.settings.active_module)
            .cloned()
    }

    /// Pushes the settings tree's runtime-backed options into `BotSettings`.
    pub(super) fn apply_config_tree(&mut self, cx: &mut Context<Self>) {
        let page = self.config_page.clone();
        if page.read(cx).pending_seed() {
            // A fresh profile is seeded from modules.json once the module
            // catalog is known; until then nothing is written.
            if !self.catalog_ready {
                return;
            }
            let (settings, descriptor) = (self.settings.clone(), self.active_module_descriptor());
            page.update(cx, |page, cx| page.seed(&settings, descriptor.as_ref(), cx));
        }
        // The module dropdown in the tree switches objective: adopt that
        // module's own settings instead of overwriting them.
        if let Some(module) = page.read(cx).pending_module(&self.settings) {
            let busy = self.behavior_running || self.behavior_starting;
            let known = self
                .all_module_descriptors
                .iter()
                .any(|descriptor| descriptor.id == module);
            if !busy && known {
                self.settings.active_module = module;
                self.settings.active_behavior_profile.clear();
                self.refresh_enabled_modules();
                self.save_settings();
            }
            self.sync_config_tree(cx);
            cx.notify();
            return;
        }
        let old_client = self.settings.client;
        let old_backend = self.settings.game_api_backend;
        let old_flash_backend = self.settings.flash_backend;
        let old_launch_options = launch::game_api_options(&self.settings.config_tree);
        page.read(cx).apply(&mut self.settings);
        let launch_options_changed =
            old_launch_options != launch::game_api_options(&self.settings.config_tree);
        self.save_settings();
        self.update_statistics_policy(self.behavior_running || self.behavior_starting);
        if self.session_connected && !self.busy && old_client != self.settings.client {
            let resume = self.behavior_running
                || self
                    .engine
                    .as_ref()
                    .is_some_and(|engine| engine.is_running());
            self.activity = format!(
                "Switching to {} and relaunching the client…",
                self.settings.client.label()
            );
            self.reload_client_with_resume(Some(resume), cx);
            return;
        }
        if self.session_connected
            && !self.busy
            && (old_backend != self.settings.game_api_backend
                || old_flash_backend != self.settings.flash_backend)
        {
            self.apply_backend_change(cx);
            return;
        }
        if self.session_connected
            && !self.busy
            && launch_options_changed
            && self.settings.client != ClientKind::Packet
        {
            let resume = self.behavior_running
                || self
                    .engine
                    .as_ref()
                    .is_some_and(|engine| engine.is_running());
            self.activity = "Applying game client launch settings…".into();
            self.reload_client_with_resume(Some(resume), cx);
            return;
        }
        cx.notify();
    }

    /// Re-reads runtime-backed values after another settings page edited
    /// `BotSettings` or the active module changed, so the tree never shows
    /// stale numbers.
    pub(super) fn sync_config_tree(&mut self, cx: &mut Context<Self>) {
        let (settings, descriptor) = (self.settings.clone(), self.active_module_descriptor());
        self.config_page.update(cx, |page, cx| {
            page.sync_from(&settings, descriptor.as_ref(), cx)
        });
    }

    pub(super) fn save_settings(&self) {
        let _ = self.settings_store.save(&self.settings);
        // A running module holds a config snapshot from Start; push the
        // updated one so NPC list toggles and feature flags apply live.
        if self.behavior_running
            && let Some(engine) = &self.engine
        {
            engine.update(&self.settings.module_config());
        }
        self.push_panel_config();
    }
}
