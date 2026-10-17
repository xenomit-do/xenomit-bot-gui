//! One retained page per immutable module descriptor. Switching modules reuses
//! the page, input entities, subscriptions and unfinished edit buffers.
use gpui_base::{Checkbox as SemanticCheckbox, CheckboxIndicator, CheckboxState};
use gpui_kit::{
    Context, Entity, EventEmitter, FontWeight, Render, SharedString, Subscription, Window,
    component::{
        Sizable,
        input::{Input, InputEvent, InputState},
        scroll::ScrollableElement,
        select::{Select, SelectEvent, SelectItem, SelectState},
    },
    div,
    prelude::*,
    px, rgb,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use xenomit_game_api::{
    EntityCategory, ModuleDescriptor, SettingDescriptor, SettingKind, set_setting_value,
    setting_value,
};

use super::view::{BLUE, FAINT, LINE, LINE_SOFT, MUTED, PANEL_DARK, TEXT, label};

#[derive(Clone)]
pub(super) struct SettingsChanged {
    pub module_id: String,
    pub settings: Value,
}
pub(super) struct ModuleSettingsPage {
    pub descriptor: ModuleDescriptor,
    values: Value,
    inputs: BTreeMap<String, Entity<InputState>>,
    subscriptions: Vec<Subscription>,
    errors: BTreeMap<String, String>,
    /// Section titles the user has folded. Everything starts open.
    collapsed: BTreeSet<String>,
    map_selects: BTreeMap<String, Entity<SelectState<Vec<MapChoice>>>>,
    map_search_inputs: BTreeMap<String, Entity<InputState>>,
    maps: Vec<xenomit_game_api::MapDescriptor>,
    maps_dirty: bool,
    pet_gears: Option<BTreeSet<i32>>,
    locked: bool,
    visible_rules: Option<BTreeSet<String>>,
}
impl EventEmitter<SettingsChanged> for ModuleSettingsPage {}
impl ModuleSettingsPage {
    pub fn new(
        mut descriptor: ModuleDescriptor,
        values: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // Group once at setup; keep declaration order within each group.
        let mut groups = Vec::new();
        for field in &descriptor.settings {
            if !groups.contains(&field.group) {
                groups.push(field.group.clone());
            }
        }
        descriptor.settings.sort_by_key(|field| {
            groups
                .iter()
                .position(|group| group == &field.group)
                .unwrap()
        });
        let values = descriptor
            .prepare_settings(values.clone())
            .unwrap_or(values);
        let mut page = Self {
            descriptor,
            values,
            inputs: BTreeMap::new(),
            subscriptions: Vec::new(),
            errors: BTreeMap::new(),
            collapsed: BTreeSet::new(),
            map_selects: BTreeMap::new(),
            map_search_inputs: BTreeMap::new(),
            maps: vec![],
            maps_dirty: false,
            pet_gears: None,
            locked: false,
            visible_rules: None,
        };
        for field in &page.descriptor.settings {
            let value = setting_value(&page.values, &field.key).unwrap_or(&field.default);
            if let Err(error) = field.validate(value) {
                page.errors.insert(field.key.clone(), error);
            }
            if matches!(field.kind, SettingKind::Map) {
                let choices = map_choices(&[], value);
                let select =
                    cx.new(|cx| SelectState::new(choices, None, window, cx).searchable(true));
                let selected = resolved_map_value(&[], value);
                select.update(cx, |state, cx| {
                    state.set_selected_value(&selected, window, cx)
                });
                let key = field.key.clone();
                page.subscriptions.push(cx.subscribe(
                    &select,
                    move |page, _, event: &SelectEvent<Vec<MapChoice>>, cx| {
                        let SelectEvent::Confirm(value) = event;
                        page.set_value(&key, value.clone().unwrap_or(Value::Null), cx);
                    },
                ));
                page.map_selects.insert(field.key.clone(), select);
                continue;
            }
            if matches!(field.kind, SettingKind::BooleanMap { .. }) {
                let search = cx.new(|cx| InputState::new(window, cx).placeholder("Filter names"));
                page.subscriptions.push(cx.subscribe(
                    &search,
                    |_page, _, event: &InputEvent, cx| {
                        if matches!(event, InputEvent::Change) {
                            cx.notify();
                        }
                    },
                ));
                page.map_search_inputs.insert(field.key.clone(), search);
                continue;
            }
            if matches!(
                field.kind,
                SettingKind::Boolean
                    | SettingKind::Choice { .. }
                    | SettingKind::BooleanMap { discover: Some(_) }
            ) {
                continue;
            }
            let text = if matches!(field.kind, SettingKind::Text) {
                value.as_str().unwrap_or_default().to_owned()
            } else {
                value.to_string()
            };
            let input = cx.new(|cx| InputState::new(window, cx).default_value(text));
            let field = field.clone();
            let field_key = field.key.clone();
            page.subscriptions.push(cx.subscribe(
                &input,
                move |page, input, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) {
                        let text = input.read(cx).value().to_string();
                        match parse_edit(&field, &text) {
                            Ok(value) => {
                                page.errors.remove(&field.key);
                                page.set_value(&field.key, value, cx);
                            }
                            Err(error) => {
                                page.errors.insert(field.key.clone(), error);
                                cx.notify();
                            }
                        }
                    }
                },
            ));
            page.inputs.insert(field_key, input);
        }
        page
    }
    pub fn has_errors(&self) -> bool {
        self.errors.keys().any(|key| {
            self.visible_rules.as_ref().is_none_or(|visible| {
                key.split_once('.')
                    .is_none_or(|(rule, _)| visible.contains(rule))
            })
        })
    }
    /// Replace the retained editor values after a named profile is selected.
    /// InputState::set_value does not emit Change, so this only updates the
    /// controls and validation state without writing stale values back.
    pub fn sync_settings(&mut self, values: Value, window: &mut Window, cx: &mut Context<Self>) {
        self.values = self
            .descriptor
            .prepare_settings(values.clone())
            .unwrap_or(values);
        self.errors.clear();
        for field in &self.descriptor.settings {
            let value = setting_value(&self.values, &field.key).unwrap_or(&field.default);
            if let Err(error) = field.validate(value) {
                self.errors.insert(field.key.clone(), error);
            }
            if let Some(input) = self.inputs.get(&field.key) {
                let text = if matches!(field.kind, SettingKind::Text) {
                    value.as_str().unwrap_or_default().to_owned()
                } else {
                    value.to_string()
                };
                input.update(cx, |state, cx| state.set_value(text, window, cx));
            }
            if let Some(select) = self.map_selects.get(&field.key) {
                let choices = map_choices(&self.maps, value);
                let selected = resolved_map_value(&self.maps, value);
                select.update(cx, |state, cx| {
                    state.set_items(choices, window, cx);
                    state.set_selected_value(&selected, window, cx);
                });
            }
        }
        cx.notify();
    }
    pub fn observe_maps(
        &mut self,
        maps: &[xenomit_game_api::MapDescriptor],
        cx: &mut Context<Self>,
    ) {
        if self.maps != maps {
            self.maps = maps.to_vec();
            self.maps_dirty = true;
            cx.notify();
        }
    }
    pub fn observe_pet_gears(&mut self, gears: Option<Vec<i32>>, cx: &mut Context<Self>) {
        let gears = gears.map(|gears| gears.into_iter().collect());
        if self.pet_gears != gears {
            self.pet_gears = gears;
            cx.notify();
        }
    }
    fn set_value(&mut self, key: &str, value: Value, cx: &mut Context<Self>) {
        if self.locked {
            return;
        }
        if let Some(field) = self
            .descriptor
            .settings
            .iter()
            .find(|field| field.key == key)
        {
            if let Err(error) = field.validate(&value) {
                self.errors.insert(key.into(), error);
                cx.notify();
                return;
            }
        }
        match set_setting_value(&mut self.values, key, value) {
            Ok(()) => {
                self.errors.remove(key);
                cx.emit(SettingsChanged {
                    module_id: self.descriptor.id.clone(),
                    settings: self.values.clone(),
                });
            }
            Err(error) => {
                self.errors.insert(key.into(), error);
            }
        }
        cx.notify();
    }
    pub fn set_locked(&mut self, locked: bool, cx: &mut Context<Self>) {
        if self.locked != locked {
            self.locked = locked;
            cx.notify();
        }
    }
    pub fn set_visible_rules(&mut self, rules: BTreeSet<String>, cx: &mut Context<Self>) {
        if self.visible_rules.as_ref() != Some(&rules) {
            self.visible_rules = Some(rules);
            cx.notify();
        }
    }
    /// Discovery is metadata-driven, including for custom entity dictionaries.
    pub fn observe(&mut self, entities: &[(EntityCategory, String)], cx: &mut Context<Self>) {
        let mut changed = false;
        for field in &self.descriptor.settings {
            if let SettingKind::BooleanMap {
                discover: Some(category),
            } = field.kind
            {
                let mut value = setting_value(&self.values, &field.key)
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                if let Some(map) = value.as_object_mut() {
                    for (_, name) in entities
                        .iter()
                        .filter(|(kind, name)| *kind == category && !name.is_empty())
                    {
                        if !map.contains_key(name) {
                            map.insert(name.clone(), json!(true));
                            changed = true;
                        }
                    }
                }
                let _ = set_setting_value(&mut self.values, &field.key, value);
            }
        }
        if changed {
            cx.emit(SettingsChanged {
                module_id: self.descriptor.id.clone(),
                settings: self.values.clone(),
            });
            cx.notify();
        }
    }

    /// Populate shared target tables from their source entity kinds while
    /// keeping the same first-seen, enabled-by-default behavior.
    pub fn observe_named_lists(&mut self, entities: &[(&str, String)], cx: &mut Context<Self>) {
        let mut changed = false;
        for field in &self.descriptor.settings {
            if !matches!(field.kind, SettingKind::BooleanMap { discover: None }) {
                continue;
            }
            let mut value = setting_value(&self.values, &field.key)
                .cloned()
                .unwrap_or_else(|| json!({}));
            if let Some(map) = value.as_object_mut() {
                for (_, name) in entities
                    .iter()
                    .filter(|(list, name)| *list == field.key.as_str() && !name.trim().is_empty())
                {
                    if !map.contains_key(name) {
                        map.insert(name.clone(), json!(true));
                        changed = true;
                    }
                }
            }
            let _ = set_setting_value(&mut self.values, &field.key, value);
        }
        if changed {
            cx.emit(SettingsChanged {
                module_id: self.descriptor.id.clone(),
                settings: self.values.clone(),
            });
            cx.notify();
        }
    }
}

mod edit;
mod fields;
pub(crate) mod maps;
mod render;
#[cfg(test)]
mod tests;

use edit::parse_edit;
use maps::*;
