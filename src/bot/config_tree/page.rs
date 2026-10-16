//! The settings tree window: DarkBot's `AdvancedConfig` in GPUI.
//!
//! Left: a tree of groups. Right: the selected group's options, rendered
//! recursively with one editor per option kind. Above: search, visibility
//! level, support filter and profile controls. Inputs and selects are created
//! lazily the first time an option is rendered and then retained.
use gpui_base::{Checkbox as SemanticCheckbox, CheckboxIndicator, CheckboxState};
use gpui_kit::{
    AnyElement, Context, Entity, EventEmitter, FocusHandle, FontWeight, Render, SharedString,
    Subscription, Window,
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
use std::path::PathBuf;
use std::sync::Arc;
use xenomit_bot_runtime::settings::BotSettings;

use super::bridge;
use super::darkbot;
use super::edit::{format_value, parse_text, step_of};
use super::filter::Filter;
use super::gaps;
use super::schema::{Kind, Level, Node, Schema, Source, Status, TableSpec, TagDefault};
use super::store::{ConfigStore, DEFAULT_PROFILE, Loaded};
use crate::bot::module_settings::maps::{map_choices, resolved_map_value};
use crate::bot::view::{
    AMBER, BLUE, BLUE_TINT, Btn, FAINT, GREEN, GREEN_TINT, LINE, LINE_SOFT, MUTED, PANEL,
    PANEL_DARK, PANEL_HOVER, RED_TINT, TEXT, button, label,
};

/// Emitted after any change that must reach `BotSettings`.
#[derive(Clone)]
pub(crate) struct ConfigChanged;

#[derive(Clone)]
struct ChoiceItem {
    label: String,
    value: Value,
}
impl SelectItem for ChoiceItem {
    type Value = Value;
    fn title(&self) -> SharedString {
        self.label.clone().into()
    }
    fn value(&self) -> &Value {
        &self.value
    }
}

/// What a retained text input edits.
#[derive(Clone, Debug)]
enum Role {
    Value(String),
    Range {
        path: String,
        max: bool,
    },
    Cell {
        path: String,
        row: String,
        col: String,
    },
    NewRow(String),
    Search,
    ProfileName,
}

impl Role {
    fn key(&self) -> String {
        match self {
            Role::Value(path) => path.clone(),
            Role::Range { path, max } => format!("{path}#{}", if *max { "max" } else { "min" }),
            Role::Cell { path, row, col } => format!("{path}/{row}/{col}"),
            Role::NewRow(path) => format!("{path}#new"),
            Role::Search => "#search".into(),
            Role::ProfileName => "#profile".into(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SupportFilter {
    All,
    Working,
    Planned,
}

type SelectEntity = Entity<SelectState<Vec<ChoiceItem>>>;

struct SelectEntry {
    state: SelectEntity,
    signature: Vec<String>,
    shown: Value,
}

pub(crate) struct ConfigTreePage {
    store: ConfigStore,
    schema: Arc<Schema>,
    selected: String,
    tabs_open: BTreeSet<String>,
    folded: BTreeSet<String>,
    /// Option paths whose support/gap detail card is expanded.
    info_open: BTreeSet<String>,
    cells_open: BTreeSet<String>,
    level: Level,
    support_filter: SupportFilter,
    all_results: bool,
    inputs: BTreeMap<String, (Entity<InputState>, Role)>,
    selects: BTreeMap<String, SelectEntry>,
    subscriptions: Vec<Subscription>,
    /// Focus handles of the key-capture boxes, keyed by `KeyTarget::id`.
    key_focus: BTreeMap<String, FocusHandle>,
    errors: BTreeMap<String, String>,
    notice: Option<String>,
    modules: Vec<(String, String)>,
    maps: Vec<xenomit_game_api::MapDescriptor>,
    tags: Vec<String>,
    pending_profile: Option<String>,
    /// A new profile with no file: seed it from `BotSettings` once the module
    /// catalog is known, so untouched options take the engine's defaults.
    pending_seed: bool,
    /// Option paths whose retained inputs must be re-read from the store.
    stale: BTreeSet<String>,
}

impl EventEmitter<ConfigChanged> for ConfigTreePage {}

impl ConfigTreePage {
    pub(crate) fn new(dir: PathBuf, _window: &mut Window, _cx: &mut Context<Self>) -> Self {
        let schema = Arc::new(darkbot::schema());
        let mut store = ConfigStore::new(schema.clone(), dir);
        let profile = store.last_profile();
        let outcome = store.load_profile(&profile);
        let level = store
            .ui("level")
            .and_then(Value::as_str)
            .map(Level::from_id)
            .unwrap_or_default();
        let selected = store
            .ui("tab")
            .and_then(Value::as_str)
            .filter(|path| schema.find(path).is_some_and(Node::is_group))
            .map(str::to_owned)
            .or_else(|| schema.roots.first().map(|node| node.path.clone()))
            .unwrap_or_default();
        let mut page = Self {
            store,
            schema,
            selected: String::new(),
            tabs_open: BTreeSet::new(),
            folded: BTreeSet::new(),
            info_open: BTreeSet::new(),
            cells_open: BTreeSet::new(),
            level,
            support_filter: SupportFilter::All,
            all_results: false,
            inputs: BTreeMap::new(),
            selects: BTreeMap::new(),
            subscriptions: Vec::new(),
            key_focus: BTreeMap::new(),
            errors: BTreeMap::new(),
            notice: (outcome == Loaded::Failed).then(|| {
                "The settings file could not be read; changes will not be saved this session."
                    .to_owned()
            }),
            modules: Vec::new(),
            maps: Vec::new(),
            tags: Vec::new(),
            pending_seed: outcome == Loaded::Missing,
            pending_profile: None,
            stale: BTreeSet::new(),
        };
        page.select_group(&selected);
        page
    }

    // Host integration -----------------------------------------------------------

    /// Writes every runtime-backed option into `settings` (active module only).
    /// A profile still waiting for its seed writes nothing.
    pub(crate) fn apply(&self, settings: &mut BotSettings) {
        if !self.pending_seed {
            bridge::apply(&self.store, settings);
        }
    }

    pub(crate) fn pending_seed(&self) -> bool {
        self.pending_seed
    }

    /// Module the tree wants active, when it differs from `settings`.
    pub(crate) fn pending_module(&self, settings: &BotSettings) -> Option<String> {
        if self.pending_seed {
            return None;
        }
        bridge::pending_module(&self.store, settings)
    }

    /// Fills a fresh profile from the existing settings.
    pub(crate) fn seed(
        &mut self,
        settings: &BotSettings,
        descriptor: Option<&xenomit_game_api::ModuleDescriptor>,
        cx: &mut Context<Self>,
    ) {
        bridge::import(settings, &mut self.store, descriptor);
        self.pending_seed = false;
        self.mark_all_stale();
        self.store.save();
        cx.notify();
    }

    fn mark_all_stale(&mut self) {
        let paths: Vec<String> = self
            .inputs
            .values()
            .filter_map(|(_, role)| match role {
                Role::Value(path) => Some(path.clone()),
                Role::Range { path, .. } | Role::Cell { path, .. } => Some(path.clone()),
                _ => None,
            })
            .collect();
        self.stale.extend(paths);
    }

    /// Re-reads runtime-backed values after another page edited `BotSettings`
    /// or the active module changed. Does not emit `ConfigChanged`, so the
    /// two cannot ping-pong.
    pub(crate) fn sync_from(
        &mut self,
        settings: &BotSettings,
        descriptor: Option<&xenomit_game_api::ModuleDescriptor>,
        cx: &mut Context<Self>,
    ) {
        if self.pending_seed {
            return;
        }
        bridge::import(settings, &mut self.store, descriptor);
        self.mark_all_stale();
        self.store.save();
        cx.notify();
    }

    pub(crate) fn observe_modules(
        &mut self,
        modules: Vec<(String, String)>,
        cx: &mut Context<Self>,
    ) {
        if self.modules != modules {
            self.modules = modules;
            cx.notify();
        }
    }

    pub(crate) fn observe_maps(
        &mut self,
        maps: &[xenomit_game_api::MapDescriptor],
        cx: &mut Context<Self>,
    ) {
        if self.maps != maps {
            self.maps = maps.to_vec();
            cx.notify();
        }
    }

    pub(crate) fn observe_tags(&mut self, tags: Vec<String>, cx: &mut Context<Self>) {
        if self.tags != tags {
            self.tags = tags;
            cx.notify();
        }
    }

    /// Adds table rows for names the game reported (`list` is `npcs`/`boxes`).
    pub(crate) fn observe_names(&mut self, list: &str, names: &[String], cx: &mut Context<Self>) {
        let schema = self.schema.clone();
        let mut changed = false;
        for leaf in schema.leaves() {
            let Some(Kind::Table(spec)) = &leaf.kind else {
                continue;
            };
            if spec.discover != Some(list) {
                continue;
            }
            let mut table = self.store.get(&leaf.path);
            let Some(rows) = table.as_object_mut() else {
                continue;
            };
            let defaults = row_defaults(spec);
            let before = rows.len();
            for name in names.iter().filter(|name| !name.trim().is_empty()) {
                rows.entry(name.clone()).or_insert_with(|| defaults.clone());
            }
            if rows.len() != before && self.store.set(&leaf.path, table).is_ok() {
                changed = true;
            }
        }
        if changed {
            cx.emit(ConfigChanged);
            cx.notify();
        }
    }

    // State changes -----------------------------------------------------------------

    fn select_group(&mut self, path: &str) {
        self.selected = path.to_owned();
        let mut prefix = String::new();
        for segment in path.split('.') {
            if !prefix.is_empty() {
                prefix.push('.');
            }
            prefix.push_str(segment);
            self.tabs_open.insert(prefix.clone());
        }
        self.store.set_ui("tab", json!(path));
        self.all_results = false;
    }

    fn set(&mut self, path: &str, value: Value, cx: &mut Context<Self>) {
        match self.store.set(path, value) {
            Ok(_) => {
                self.errors.remove(path);
                self.store.save();
                cx.emit(ConfigChanged);
            }
            Err(error) => {
                self.errors.insert(path.to_owned(), error);
            }
        }
        cx.notify();
    }

    fn input_changed(&mut self, role: &Role, text: &str, cx: &mut Context<Self>) {
        let key = role.key();
        let result: Result<(String, Value), String> = match role {
            Role::Search => {
                cx.notify();
                return;
            }
            Role::ProfileName | Role::NewRow(_) => return,
            Role::Value(path) => {
                let Some(kind) = self.schema.find(path).and_then(|node| node.kind.clone()) else {
                    return;
                };
                parse_text(&kind, text).map(|value| (path.clone(), value))
            }
            Role::Range { path, max } => {
                let percent = Kind::Percent {
                    min: 0.0,
                    max: 1.0,
                    step: 0.05,
                };
                parse_text(&percent, text).map(|fraction| {
                    let mut range = self.store.get(path);
                    range[if *max { "max" } else { "min" }] = fraction;
                    (path.clone(), range)
                })
            }
            Role::Cell { path, row, col } => {
                let kind = self.column_kind(path, col);
                match kind {
                    Some(kind) => {
                        let validation = if col == "action" {
                            xenomit_behavior_engine::parse_darkbot_action(text).map(|_| ())
                        } else {
                            Ok(())
                        };
                        validation
                            .and_then(|_| parse_text(&kind, text))
                            .map(|value| {
                                let mut table = self.store.get(path);
                                table[row.as_str()][col.as_str()] = value;
                                (path.clone(), table)
                            })
                    }
                    None => return,
                }
            }
        };
        match result {
            Ok((path, value)) => {
                self.errors.remove(&key);
                self.set(&path, value, cx);
            }
            Err(error) => {
                self.errors.insert(key, error);
                cx.notify();
            }
        }
    }

    fn column_kind(&self, path: &str, col: &str) -> Option<Kind> {
        match self.schema.find(path)?.kind.as_ref()? {
            Kind::Table(spec) => spec
                .columns
                .iter()
                .find(|column| column.key == col)
                .map(|column| column.kind.clone()),
            _ => None,
        }
    }

    fn text_for(&self, role: &Role) -> String {
        match role {
            Role::Value(path) => self
                .schema
                .find(path)
                .and_then(|node| node.kind.as_ref())
                .map(|kind| format_value(kind, &self.store.get(path)))
                .unwrap_or_default(),
            Role::Range { path, max } => {
                let value = self.store.get(path);
                value[if *max { "max" } else { "min" }]
                    .as_f64()
                    .map(|fraction| super::edit::format_number(fraction * 100.0))
                    .unwrap_or_default()
            }
            Role::Cell { path, row, col } => {
                let table = self.store.get(path);
                match self.column_kind(path, col) {
                    Some(kind) => format_value(&kind, &table[row.as_str()][col.as_str()]),
                    None => String::new(),
                }
            }
            Role::NewRow(_) | Role::Search | Role::ProfileName => String::new(),
        }
    }

    /// Pushes the stored values back into the retained inputs. `set_value`
    /// does not emit `Change`, so this cannot write stale text back.
    fn refresh_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let entries: Vec<(Entity<InputState>, Role)> = self
            .inputs
            .values()
            .filter(|(_, role)| !matches!(role, Role::Search | Role::ProfileName | Role::NewRow(_)))
            .cloned()
            .collect();
        for (input, role) in entries {
            let text = self.text_for(&role);
            input.update(cx, |state, cx| state.set_value(text, window, cx));
        }
        self.errors.clear();
    }

    fn switch_profile(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        if name == self.store.profile() {
            return;
        }
        match self.store.load_profile(name) {
            Loaded::Failed => {
                self.notice = Some(format!(
                    "Profile {name} could not be read; changes will not be saved."
                ));
            }
            Loaded::Missing => self.notice = None,
            Loaded::File => self.notice = None,
        }
        self.refresh_inputs(window, cx);
        cx.emit(ConfigChanged);
        cx.notify();
    }

    fn input_text(&self, role: &Role, cx: &Context<Self>) -> String {
        self.inputs
            .get(&role.key())
            .map(|(input, _)| input.read(cx).value().to_string())
            .unwrap_or_default()
    }

    fn ensure_input(
        &mut self,
        role: Role,
        placeholder: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        let key = role.key();
        if let Some((input, _)) = self.inputs.get(&key) {
            return input.clone();
        }
        let initial = self.text_for(&role);
        let placeholder = placeholder.to_owned();
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(initial)
                .placeholder(placeholder)
        });
        let handler_role = role.clone();
        self.subscriptions.push(cx.subscribe(
            &input,
            move |page, input, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    let text = input.read(cx).value().to_string();
                    page.input_changed(&handler_role, &text, cx);
                }
            },
        ));
        self.inputs.insert(key, (input.clone(), role));
        input
    }

    fn ensure_select(
        &mut self,
        key: &str,
        items: Vec<ChoiceItem>,
        selected: &Value,
        searchable: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> SelectEntity {
        let signature: Vec<String> = items
            .iter()
            .map(|item| format!("{}\u{1}{}", item.label, item.value))
            .collect();
        if let Some(entry) = self.selects.get_mut(key) {
            let state = entry.state.clone();
            if entry.signature != signature {
                state.update(cx, |state, cx| state.set_items(items, window, cx));
                entry.signature = signature;
                entry.shown = Value::Bool(false); // force reselect below
            }
            if entry.shown != *selected {
                state.update(cx, |state, cx| {
                    state.set_selected_value(selected, window, cx)
                });
                entry.shown = selected.clone();
            }
            return state;
        }
        let state = cx.new(|cx| {
            let state = SelectState::new(items, None, window, cx);
            if searchable {
                state.searchable(true)
            } else {
                state
            }
        });
        state.update(cx, |state, cx| {
            state.set_selected_value(selected, window, cx)
        });
        let route = key.to_owned();
        self.subscriptions.push(cx.subscribe(
            &state,
            move |page, _, event: &SelectEvent<Vec<ChoiceItem>>, cx| {
                let SelectEvent::Confirm(value) = event;
                page.select_confirmed(&route, value.clone().unwrap_or(Value::Null), cx);
            },
        ));
        self.selects.insert(
            key.to_owned(),
            SelectEntry {
                state: state.clone(),
                signature,
                shown: selected.clone(),
            },
        );
        state
    }

    fn select_confirmed(&mut self, route: &str, value: Value, cx: &mut Context<Self>) {
        if route == "#profile" {
            if let Some(name) = value.as_str() {
                self.pending_profile = Some(name.to_owned());
                cx.notify();
            }
            return;
        }
        self.set(route, value, cx);
    }
}

/// Default row for a table: every column at its default.
pub(super) fn row_defaults(spec: &TableSpec) -> Value {
    let mut row = serde_json::Map::new();
    for column in &spec.columns {
        row.insert(column.key.clone(), column.default.clone());
    }
    Value::Object(row)
}

mod render;

/// What a key-capture box edits.
#[derive(Clone, Debug)]
pub(super) enum KeyTarget {
    Value(String),
    /// The `formation` field of a ship config.
    Formation(String),
    Cell {
        path: String,
        row: String,
        col: String,
    },
}

impl KeyTarget {
    fn id(&self) -> String {
        match self {
            KeyTarget::Value(path) => path.clone(),
            KeyTarget::Formation(path) => format!("{path}#formation"),
            KeyTarget::Cell { path, row, col } => format!("{path}/{row}/{col}"),
        }
    }
}

impl ConfigTreePage {
    fn key_current(&self, target: &KeyTarget) -> Option<String> {
        let value = match target {
            KeyTarget::Value(path) => self.store.get(path),
            KeyTarget::Formation(path) => self.store.get(path)["formation"].clone(),
            KeyTarget::Cell { path, row, col } => {
                self.store.get(path)[row.as_str()][col.as_str()].clone()
            }
        };
        value.as_str().map(str::to_owned)
    }

    /// Stores a captured key (`None` clears it).
    fn key_picked(&mut self, target: &KeyTarget, key: Option<String>, cx: &mut Context<Self>) {
        let value = key.map_or(Value::Null, |key| json!(key));
        match target {
            KeyTarget::Value(path) => self.set(path, value, cx),
            KeyTarget::Formation(path) => {
                let mut ship = self.store.get(path);
                ship["formation"] = value;
                self.set(path, ship, cx);
            }
            KeyTarget::Cell { path, row, col } => {
                let mut table = self.store.get(path);
                table[row.as_str()][col.as_str()] = value;
                self.set(path, table, cx);
            }
        }
    }
}
