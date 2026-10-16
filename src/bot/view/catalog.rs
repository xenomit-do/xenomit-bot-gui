//! Module/plugin catalog discovery and the module selector in the settings UI.

use super::*;

#[derive(Clone)]
pub(super) struct ModuleChoice {
    pub(super) id: String,
    pub(super) name: String,
}

impl SelectItem for ModuleChoice {
    type Value = String;

    fn title(&self) -> SharedString {
        self.name.clone().into()
    }

    fn value(&self) -> &String {
        &self.id
    }
}

pub(super) fn plugin_settings_descriptor(plugins: &[PluginDescriptor]) -> ModuleDescriptor {
    ModuleDescriptor {
        id: "__plugins".into(),
        name: "Plugins".into(),
        description: "Enable or disable extensions. Changes apply to the next run.".into(),
        settings: plugins
            .iter()
            .map(|plugin| SettingDescriptor {
                key: format!("plugins.{}", plugin.id),
                label: plugin.name.clone(),
                description: plugin.description.clone(),
                group: "Available plugins".into(),
                kind: SettingKind::Boolean,
                default: json!(plugin.default_enabled),
            })
            .collect(),
        required_services: vec![],
    }
}

impl BotView {
    pub(super) fn load_module_catalog(&mut self, cx: &mut Context<Self>) {
        if self.catalog_ready || self.catalog_loading {
            return;
        }
        let Some(engine) = self.engine.clone() else {
            self.catalog_error = Some("Behavior engine unavailable".into());
            return;
        };
        self.catalog_loading = true;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let plugins = engine.plugin_descriptors()?;
                    let all_enabled = plugins
                        .iter()
                        .map(|plugin| (plugin.id.clone(), true))
                        .collect::<BTreeMap<_, _>>();
                    let descriptors = engine.modules_for(&all_enabled)?;
                    Ok((descriptors, plugins))
                })
                .await;
            let _ = this.update(cx, |view, cx| {
                view.pending_catalog = Some(result);
                view.catalog_loading = false;
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn install_module_catalog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(result) = self.pending_catalog.take() else {
            return;
        };
        if self.catalog_ready {
            return;
        }
        let (descriptors, plugins) = match result {
            Ok(catalog) => catalog,
            Err(error) => {
                self.catalog_error = Some(error);
                return;
            }
        };
        let behavior_descriptor = descriptors
            .iter()
            .find(|descriptor| descriptor.id == "__behaviors")
            .cloned()
            .unwrap_or_else(xenomit_behavior_engine::behavior_settings_descriptor);
        let behavior_page = cx.new(|cx| {
            ModuleSettingsPage::new(
                behavior_descriptor.clone(),
                self.settings.behaviors.clone(),
                window,
                cx,
            )
        });
        self.behaviors_descriptor = Some(behavior_descriptor.clone());
        self.module_subscriptions.push(cx.subscribe(
            &behavior_page,
            |view, _page, event: &SettingsChanged, cx| {
                if event.module_id == "__behaviors" {
                    view.settings.behaviors = event.settings.clone();
                    view.save_settings();
                    view.sync_config_tree(cx);
                    cx.notify();
                }
            },
        ));
        self.module_pages
            .insert("__behaviors".into(), behavior_page);
        let mut plugin_descriptor = descriptors
            .iter()
            .find(|descriptor| descriptor.id == "__plugins")
            .cloned()
            .unwrap_or_else(|| plugin_settings_descriptor(&plugins));
        for setting in &mut plugin_descriptor.settings {
            if !setting.key.starts_with("plugins.") {
                setting.key = format!("plugins.{}", setting.key);
            }
        }
        for plugin in &plugins {
            self.settings
                .plugins
                .entry(plugin.id.clone())
                .or_insert(plugin.default_enabled);
        }
        for (id, enabled) in &self.settings.plugins {
            if !plugins.iter().any(|plugin| &plugin.id == id) {
                plugin_descriptor.settings.push(SettingDescriptor {
                    key: format!("plugins.{id}"),
                    label: format!("Missing plugin: {id}"),
                    description: "Its manifest was not discovered at startup. Disable this saved activation or restore the manifest.".into(),
                    group: "Missing manifests".into(),
                    kind: SettingKind::Boolean,
                    default: json!(*enabled),
                });
            }
        }
        let plugin_values = json!({ "plugins": self.settings.plugins.clone() });
        self.plugins_descriptor = Some(plugin_descriptor.clone());
        let plugin_page = cx.new(|cx| {
            ModuleSettingsPage::new(plugin_descriptor.clone(), plugin_values, window, cx)
        });
        self.module_subscriptions.push(cx.subscribe(
            &plugin_page,
            |view, page, event: &SettingsChanged, cx| {
                if event.module_id == "__plugins" {
                    view.settings.plugins = event
                        .settings
                        .get("plugins")
                        .cloned()
                        .and_then(|value| {
                            serde_json::from_value::<BTreeMap<String, bool>>(value).ok()
                        })
                        .unwrap_or_default();
                    view.refresh_enabled_modules();
                    view.refresh_behavior_rule_visibility(cx);
                    page.update(cx, |page, cx| page.set_locked(view.plugins_locked(), cx));
                    view.save_settings();
                    cx.notify();
                }
            },
        ));
        self.module_pages.insert("__plugins".into(), plugin_page);
        let targets_descriptor = ModuleDescriptor::new("__targets", "Targets")
            .description("Shared entity lists used by every objective module. New names are enabled on first encounter.")
            .setting(
                SettingDescriptor::new(
                    "npcs",
                    "NPCs to kill",
                    SettingKind::BooleanMap { discover: None },
                    json!({}),
                )
                .group("NPCs"),
            )
            .setting(
                SettingDescriptor::new(
                    "boxes",
                    "Bonus boxes",
                    SettingKind::BooleanMap { discover: None },
                    json!({}),
                )
                .group("Boxes"),
            )
            .setting(
                SettingDescriptor::new(
                    "collectables",
                    "Resources",
                    SettingKind::BooleanMap { discover: None },
                    json!({}),
                )
                .group("Resources"),
            );
        let targets_values = json!({
            "npcs": self.settings.npcs,
            "boxes": self.settings.boxes,
            "collectables": self.settings.collectables,
        });
        let targets_page =
            cx.new(|cx| ModuleSettingsPage::new(targets_descriptor, targets_values, window, cx));
        self.module_subscriptions.push(cx.subscribe(
            &targets_page,
            |view, _page, event: &SettingsChanged, cx| {
                if event.module_id != "__targets" {
                    return;
                }
                view.settings.npcs = serde_json::from_value(
                    event
                        .settings
                        .get("npcs")
                        .cloned()
                        .unwrap_or_else(|| json!({})),
                )
                .unwrap_or_default();
                view.settings.boxes = serde_json::from_value(
                    event
                        .settings
                        .get("boxes")
                        .cloned()
                        .unwrap_or_else(|| json!({})),
                )
                .unwrap_or_default();
                view.settings.collectables = serde_json::from_value(
                    event
                        .settings
                        .get("collectables")
                        .cloned()
                        .unwrap_or_else(|| json!({})),
                )
                .unwrap_or_default();
                view.save_settings();
                cx.notify();
            },
        ));
        self.module_pages.insert("__targets".into(), targets_page);
        let descriptors = descriptors
            .into_iter()
            .filter(|descriptor| !matches!(descriptor.id.as_str(), "__plugins" | "__behaviors"))
            .collect::<Vec<_>>();
        for descriptor in &descriptors {
            let values = self.settings.module_config_for(&descriptor.id).settings;
            let page = cx.new(|cx| ModuleSettingsPage::new(descriptor.clone(), values, window, cx));
            self.module_subscriptions.push(cx.subscribe(
                &page,
                |view, _page, event: &SettingsChanged, cx| {
                    if event.module_id != "__behaviors" {
                        view.settings
                            .modules
                            .insert(event.module_id.clone(), event.settings.clone());
                    }
                    view.save_settings();
                    view.sync_config_tree(cx);
                    cx.notify();
                },
            ));
            self.module_pages.insert(descriptor.id.clone(), page);
        }
        let module_names: Vec<(String, String)> = descriptors
            .iter()
            .map(|descriptor| (descriptor.id.clone(), descriptor.name.clone()))
            .collect();
        self.all_module_descriptors = descriptors;
        self.plugin_descriptors = plugins;
        self.config_page
            .update(cx, |page, cx| page.observe_modules(module_names, cx));
        self.refresh_enabled_modules();
        self.refresh_behavior_rule_visibility(cx);
        if let Some(page) = self.module_pages.get("__plugins") {
            page.update(cx, |page, cx| page.set_locked(self.plugins_locked(), cx));
        }
        self.catalog_ready = true;
        self.apply_config_tree(cx);
    }

    pub(super) fn plugins_locked(&self) -> bool {
        self.behavior_starting
            || self.behavior_running
            || self
                .engine
                .as_ref()
                .is_some_and(|engine| engine.is_running())
    }

    pub(super) fn refresh_enabled_modules(&mut self) {
        self.module_descriptors = self
            .all_module_descriptors
            .iter()
            .filter(|module| {
                let owners = self
                    .plugin_descriptors
                    .iter()
                    .filter(|plugin| plugin.module_ids.iter().any(|id| id == &module.id));
                owners.into_iter().all(|plugin| {
                    self.settings
                        .plugins
                        .get(&plugin.id)
                        .copied()
                        .unwrap_or(plugin.default_enabled)
                })
            })
            .cloned()
            .collect();
        if !self
            .module_descriptors
            .iter()
            .any(|module| module.id == self.settings.active_module)
            && let Some(first) = self.module_descriptors.first()
        {
            self.settings.active_module = first.id.clone();
        } else if self.module_descriptors.is_empty() {
            self.settings.active_module.clear();
        }
    }

    pub(super) fn refresh_behavior_rule_visibility(&self, cx: &mut Context<Self>) {
        let Some(page) = self.module_pages.get("__behaviors") else {
            return;
        };
        let rule_ids: std::collections::BTreeSet<String> = page
            .read(cx)
            .descriptor
            .settings
            .iter()
            .filter_map(|setting| setting.key.split_once('.').map(|(rule, _)| rule.to_owned()))
            .collect();
        let visible = rule_ids
            .into_iter()
            .filter(|rule| {
                self.plugin_descriptors
                    .iter()
                    .filter(|plugin| plugin.behavior_ids.iter().any(|id| id == rule))
                    .all(|plugin| {
                        self.settings
                            .plugins
                            .get(&plugin.id)
                            .copied()
                            .unwrap_or(plugin.default_enabled)
                    })
            })
            .collect();
        page.update(cx, |page, cx| page.set_visible_rules(visible, cx));
    }

    pub(super) fn module_name<'a>(&'a self, id: &'a str) -> &'a str {
        self.module_descriptors
            .iter()
            .find(|d| d.id == id)
            .map(|d| d.name.as_str())
            .unwrap_or(id)
    }

    pub(super) fn sync_settings_module_selector(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.settings_module_selector.is_none() && self.catalog_ready {
            let choices = self
                .module_descriptors
                .iter()
                .map(|descriptor| ModuleChoice {
                    id: descriptor.id.clone(),
                    name: descriptor.name.clone(),
                })
                .collect::<Vec<_>>();
            let selector =
                cx.new(|cx| SelectState::new(choices, None, window, cx).searchable(true));
            self.module_subscriptions.push(cx.subscribe(
                &selector,
                |view, _, event: &SelectEvent<Vec<ModuleChoice>>, cx| {
                    let SelectEvent::Confirm(Some(module_id)) = event else {
                        return;
                    };
                    view.select_module(module_id, cx);
                    view.settings_module_selector_value.clear();
                    cx.notify();
                },
            ));
            self.settings_module_selector = Some(selector);
        }

        let selected = self.settings.active_module.clone();
        if selected != self.settings_module_selector_value {
            if let Some(selector) = &self.settings_module_selector {
                selector.update(cx, |state, cx| {
                    state.set_selected_value(&selected, window, cx)
                });
                self.settings_module_selector_value = selected;
            }
        }
    }

    pub(super) fn select_module(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.behavior_running
            || self.behavior_starting
            || self
                .engine
                .as_ref()
                .is_some_and(|engine| engine.is_running())
        {
            cx.notify();
            return;
        }
        if !self.module_descriptors.iter().any(|module| module.id == id) {
            return;
        }
        self.settings.active_module = id.into();
        self.settings.active_behavior_profile.clear();
        self.settings_category = "General".into();
        self.save_settings();
        self.sync_config_tree(cx);
        cx.notify();
    }
}
