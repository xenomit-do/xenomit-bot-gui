//! Web-panel link: config/state push, remote commands and pairing.

use super::*;

impl BotView {
    /// `config.modules` body — `descriptor.key → value` per advertised
    /// module — pushed to the panel agent. The link dedupes unchanged bodies.
    pub(super) fn panel_config_body(&self) -> serde_json::Value {
        let mut modules = serde_json::Map::new();
        for descriptor in &self.all_module_descriptors {
            let nested = self.settings.module_config_for(&descriptor.id).settings;
            modules.insert(
                descriptor.id.clone(),
                serde_json::Value::Object(panel::module_settings_flat(descriptor, &nested)),
            );
        }
        if let Some(descriptor) = &self.behaviors_descriptor {
            modules.insert(
                descriptor.id.clone(),
                serde_json::Value::Object(panel::module_settings_flat(
                    descriptor,
                    &self.settings.behaviors,
                )),
            );
        }
        if let Some(descriptor) = &self.plugins_descriptor {
            let nested = json!({ "plugins": self.settings.plugins });
            modules.insert(
                descriptor.id.clone(),
                serde_json::Value::Object(panel::module_settings_flat(descriptor, &nested)),
            );
        }
        json!({
            "active_module": self.settings.active_module,
            "modules": modules,
        })
    }

    pub(super) fn push_panel_config(&self) {
        if self.catalog_ready {
            self.panel.push_config(self.panel_config_body());
        }
    }

    /// Refresh the agent's `hello`/`status` payloads from live view state.
    /// Runs on a one-second pump whether or not a game session is attached.
    pub(super) fn push_panel_state(&self) {
        let client = match self.settings.client {
            ClientKind::Ruffle => "ruffle",
            ClientKind::Flash => "flash",
            ClientKind::Packet => "packet",
        };
        let backend = match self.settings.client {
            ClientKind::Packet => "tcp",
            ClientKind::Flash => "native",
            ClientKind::Ruffle => match self.client_backend {
                ClientBackend::Native => "native",
                ClientBackend::LegacyPipe => "legacy_pipe",
            },
        };
        let session = self.session.as_ref();
        let username = panel::hero_name(&self.live_frame)
            .or_else(|| {
                (!self.settings.panel.username.is_empty())
                    .then(|| self.settings.panel.username.clone())
            })
            .or_else(|| session.map(|session| session.user_id.to_string()))
            .unwrap_or_default();
        let modules: Vec<serde_json::Value> = self
            .all_module_descriptors
            .iter()
            .chain(self.behaviors_descriptor.iter())
            .chain(self.plugins_descriptor.iter())
            .filter_map(|descriptor| serde_json::to_value(descriptor).ok())
            .collect();
        self.panel.set_hello(json!({
            "instance": {
                "server": session
                    .map(|session| session.server.clone())
                    .unwrap_or_else(|| self.settings.panel.server.clone()),
                "user_id": session
                    .map(|session| session.user_id.to_string())
                    .unwrap_or_else(|| self.settings.panel.user_id.clone()),
                "username": username,
            },
            "client": client,
            "backend": backend,
            "modules": modules,
            "config": self.panel_config_body(),
        }));

        let now = statistics::now_ms();
        let (balances, rates, uptime_s) = self
            .statistics
            .as_ref()
            .and_then(|stats| {
                stats.lock().ok().map(|stats| {
                    (
                        stats.balances,
                        stats.rates(now),
                        now.saturating_sub(stats.started) / 1000,
                    )
                })
            })
            .unwrap_or(([None; 4], [0.0; 4], 0));
        let capture_us = if self.live_frame.capture_us > 0 {
            self.live_frame.capture_us
        } else {
            self.flash_capture_us
        };
        let mut status = json!({
            "bot_running": self.behavior_running,
            "module_id": self.settings.active_module,
            "activity": self.activity,
            "uptime_s": uptime_s,
            "sid_expires_at": session.and_then(|session| session.expires_at).map(|end| end * 1000),
            "client": client,
            "backend": backend,
            "capture_ms": capture_us as f64 / 1000.0,
            "cpu_pct": self.cpu_percent,
            "ram_mb": self.mem_bytes / 1_048_576,
            "deaths": self.deaths,
            "balances": balances.map(|balance| balance.unwrap_or(0.0)),
            "rates": {
                "cre_h": rates[0],
                "uri_h": rates[1],
                "exp_h": rates[3],
                "hon_h": rates[2],
            },
        });
        if let Some(ping) = self.bridge_ping_ms {
            status["bridge_ping_ms"] = json!(ping);
        }
        self.panel.set_status(status);
    }

    /// Execute one queued panel command; `command_result` replies go through
    /// the link's outbox, deferred to command completion where the action is
    /// asynchronous (start/stop/game_command/refresh_stats).
    pub(super) fn handle_panel_command(
        &mut self,
        command: panel::PanelCommand,
        cx: &mut Context<Self>,
    ) {
        let id = command.id.clone();
        let deferred = || Some((self.panel.clone(), id.clone()));
        match command.action.as_str() {
            "start" => {
                if self.behavior_running
                    || self
                        .engine
                        .as_ref()
                        .is_some_and(|engine| engine.is_running())
                {
                    self.panel
                        .reply(&id, false, Some("Module already running".into()), None);
                } else {
                    self.run_behavior(cx, deferred());
                }
            }
            "stop" => {
                if !self.behavior_running && !self.behavior_starting {
                    self.panel
                        .reply(&id, true, None, Some(json!("not running")));
                } else {
                    self.stop_behavior(cx, deferred());
                }
            }
            "set_module" => {
                let module_id = command.payload["module_id"].as_str().unwrap_or_default();
                if module_id.is_empty() || !self.module_pages.contains_key(module_id) {
                    self.panel.reply(
                        &id,
                        false,
                        Some(format!("Unknown module {module_id}")),
                        None,
                    );
                } else if self.behavior_running
                    || self.behavior_starting
                    || self
                        .engine
                        .as_ref()
                        .is_some_and(|engine| engine.is_running())
                {
                    self.panel.reply(
                        &id,
                        false,
                        Some("Cannot switch module while running".into()),
                        None,
                    );
                } else {
                    self.select_module(module_id, cx);
                    let applied = self.settings.active_module == module_id;
                    self.panel.reply(
                        &id,
                        applied,
                        (!applied).then(|| "Module selection was rejected".to_owned()),
                        None,
                    );
                }
            }
            "update_config" => {
                let module_id = command.payload["module_id"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned();
                match self.apply_remote_config(&module_id, &command.payload["settings"], cx) {
                    Ok(()) => self.panel.reply(&id, true, None, None),
                    Err(error) => self.panel.reply(&id, false, Some(error), None),
                }
            }
            "game_command" => {
                let Some(bridge) = self.bridge.clone() else {
                    self.panel
                        .reply(&id, false, Some("No game client attached".into()), None);
                    return;
                };
                match panel::parse_game_command(&command.payload) {
                    Ok(game_command) => {
                        let link = self.panel.clone();
                        cx.spawn(async move |_, cx| {
                            let result = cx
                                .background_executor()
                                .spawn(async move {
                                    bridge
                                        .game_command(&game_command)
                                        .map_err(|error| error.to_string())
                                })
                                .await;
                            match result {
                                Ok(result) => link.reply(&id, true, None, result.data),
                                Err(error) => link.reply(&id, false, Some(error), None),
                            }
                        })
                        .detach();
                    }
                    Err(error) => self.panel.reply(&id, false, Some(error), None),
                }
            }
            "refresh_stats" => {
                let Some(session) = self.session.clone() else {
                    self.panel
                        .reply(&id, false, Some("No Backpage session".into()), None);
                    return;
                };
                let stats = self.statistics.clone();
                let link = self.panel.clone();
                cx.spawn(async move |_, cx| {
                    let result = cx
                        .background_executor()
                        .spawn(async move { statistics::fetch_balances(session) })
                        .await;
                    match result {
                        Ok(balances) => match stats.as_ref().map(|stats| stats.lock()) {
                            Some(Ok(mut stats)) => {
                                stats.reconcile(balances, "panel", statistics::now_ms());
                                link.reply(&id, true, None, Some(json!(balances)));
                            }
                            _ => {
                                link.reply(&id, false, Some("Statistics unavailable".into()), None)
                            }
                        },
                        Err(error) => link.reply(&id, false, Some(error), None),
                    }
                })
                .detach();
            }
            "reset_stats" => match self.statistics.as_ref().map(|stats| stats.lock()) {
                Some(Ok(mut stats)) => {
                    stats.events.clear();
                    stats.balances = [None; 4];
                    stats.started = statistics::now_ms();
                    stats.ended = None;
                    self.panel.reply(&id, true, None, None);
                }
                _ => self.panel.reply(
                    &id,
                    false,
                    Some("No statistics for this session".into()),
                    None,
                ),
            },
            "shutdown" => {
                self.panel.reply(&id, true, None, None);
                self.close_application(cx);
            }
            other => self
                .panel
                .reply(&id, false, Some(format!("Unknown action {other}")), None),
        }
    }

    /// Apply a panel `update_config`: the flat `descriptor.key → value` map
    /// is unflattened into the stored nested settings, persisted, and the
    /// matching settings page is synced on the next render.
    pub(super) fn apply_remote_config(
        &mut self,
        module_id: &str,
        flat: &serde_json::Value,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let Some(flat) = flat.as_object() else {
            return Err("settings must be an object".into());
        };
        let mut nested = json!({});
        for (key, value) in flat {
            xenomit_game_api::set_setting_value(&mut nested, key, value.clone())?;
        }
        let sync = match module_id {
            "__behaviors" => {
                self.settings.behaviors = nested.clone();
                ("__behaviors".to_owned(), nested)
            }
            "__plugins" => {
                if self.plugins_locked() {
                    return Err("Plugins cannot change while a module is running".into());
                }
                let plugins = serde_json::from_value::<BTreeMap<String, bool>>(
                    nested.get("plugins").cloned().unwrap_or(json!({})),
                )
                .map_err(|error| error.to_string())?;
                self.settings.plugins = plugins;
                self.refresh_enabled_modules();
                self.refresh_behavior_rule_visibility(cx);
                (
                    "__plugins".to_owned(),
                    json!({ "plugins": self.settings.plugins }),
                )
            }
            _ => {
                if !self.module_pages.contains_key(module_id)
                    && !self
                        .all_module_descriptors
                        .iter()
                        .any(|descriptor| descriptor.id == module_id)
                {
                    return Err(format!("Unknown module {module_id}"));
                }
                self.settings.modules.insert(module_id.to_owned(), nested);
                (
                    module_id.to_owned(),
                    self.settings.module_config_for(module_id).settings,
                )
            }
        };
        self.save_settings();
        self.pending_remote_sync.push(sync);
        Ok(())
    }

    /// `POST /api/agent/pair` from the WEB PANEL card: exchange the pairing
    /// code for a permanent instance token and let the agent connect.
    pub(super) fn pair_panel(&mut self, cx: &mut Context<Self>) {
        let url = self.panel_url.read(cx).value().trim().to_string();
        let code = self.panel_code.read(cx).value().trim().to_string();
        if url.is_empty() || code.is_empty() {
            self.activity = "Enter the panel URL and a pairing code.".into();
            cx.notify();
            return;
        }
        let instance = json!({
            "server": self.session.as_ref().map(|s| s.server.clone()).unwrap_or_default(),
            "user_id": self.session.as_ref().map(|s| s.user_id.to_string()).unwrap_or_default(),
            "username": self.settings.panel.username.clone(),
        });
        self.busy = true;
        self.activity = "Pairing with the panel…".into();
        cx.notify();
        let link = self.panel.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { panel::pair(&url, &code, instance) })
                .await;
            let _ = this.update(cx, |view, cx| {
                view.busy = false;
                match result {
                    Ok(pair) => {
                        view.settings.panel = panel::PanelSettings {
                            enabled: true,
                            url: view.panel_url.read(cx).value().trim().to_string(),
                            token: pair.token,
                            instance_id: pair.instance_id.clone(),
                            server: view
                                .session
                                .as_ref()
                                .map(|s| s.server.clone())
                                .unwrap_or_default(),
                            user_id: view
                                .session
                                .as_ref()
                                .map(|s| s.user_id.to_string())
                                .unwrap_or_default(),
                            username: view.settings.panel.username.clone(),
                        };
                        view.save_settings();
                        link.configure(view.settings.panel.clone());
                        view.activity = format!("Paired with panel instance {}", pair.instance_id);
                    }
                    Err(error) => view.activity = error,
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn unlink_panel(&mut self, cx: &mut Context<Self>) {
        self.settings.panel = panel::PanelSettings {
            url: self.settings.panel.url.clone(),
            ..Default::default()
        };
        self.save_settings();
        let settings = self.settings.panel.clone();
        self.panel.configure(settings);
        self.activity = "Panel instance unlinked.".into();
        cx.notify();
    }

    pub(super) fn toggle_panel(&mut self, cx: &mut Context<Self>) {
        self.settings.panel.enabled = !self.settings.panel.enabled;
        self.save_settings();
        let settings = self.settings.panel.clone();
        self.panel.configure(settings);
        cx.notify();
    }
}
