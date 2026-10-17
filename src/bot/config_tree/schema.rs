//! Typed description of the settings tree.
//!
//! This is the Rust counterpart of the reference implementation's annotated `Config` class: a tree
//! of groups and options, each with a kind (the "annotation"), a default, a
//! visibility level and a translation-derived label/description. The tree is
//! pure data; `store` holds values, `page` renders it and `bridge` pushes the
//! options the runtime understands into `BotSettings`.
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::OnceLock;

/// the reference implementation's `@Visibility(Level)`. A level shows every option at or below it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Level {
    #[default]
    Basic,
    Intermediate,
    Advanced,
    Developer,
}

impl Level {
    pub(crate) const ALL: [Level; 4] = [
        Level::Basic,
        Level::Intermediate,
        Level::Advanced,
        Level::Developer,
    ];
    pub(crate) fn label(self) -> &'static str {
        match self {
            Level::Basic => "Basic",
            Level::Intermediate => "Intermediate",
            Level::Advanced => "Advanced",
            Level::Developer => "Developer",
        }
    }
    pub(crate) fn id(self) -> &'static str {
        match self {
            Level::Basic => "basic",
            Level::Intermediate => "intermediate",
            Level::Advanced => "advanced",
            Level::Developer => "developer",
        }
    }
    pub(crate) fn from_id(id: &str) -> Level {
        Level::ALL
            .into_iter()
            .find(|level| level.id() == id)
            .unwrap_or_default()
    }
}

/// How much of an option the current runtime honours.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Status {
    /// Pushed into the runtime and acted on.
    Live,
    /// Pushed into the runtime, but the runtime only honours part of it.
    Partial,
    /// Persisted and shown, but nothing consumes it yet. See `gaps`.
    Gap,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Support {
    pub status: Status,
    /// Human-readable runtime target, shown in the tooltip.
    pub target: &'static str,
    /// Id of the entry in `gaps::GAPS` describing what is missing.
    pub gap: &'static str,
}

pub(crate) const fn live(target: &'static str) -> Support {
    Support {
        status: Status::Live,
        target,
        gap: "",
    }
}
pub(crate) const fn partial(target: &'static str, gap: &'static str) -> Support {
    Support {
        status: Status::Partial,
        target,
        gap,
    }
}
pub(crate) const fn gap(gap: &'static str) -> Support {
    Support {
        status: Status::Gap,
        target: "",
        gap,
    }
}

/// `@Tag(Tag.Default.*)`: which implicit entries the tag dropdown offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TagDefault {
    None,
    All,
}

/// Dynamic dropdown sources, resolved by the page from live state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Source {
    Modules,
    Maps,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Disabled {
    /// Value stored when the checkbox is unchecked (`@Number.Disabled.value`).
    pub value: f64,
    /// Value shown when the checkbox is checked again (`def`).
    pub def: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Column {
    pub key: String,
    pub label: String,
    pub desc: String,
    pub kind: Kind,
    pub default: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TableSpec {
    /// Heading of the first (key) column.
    pub key_label: String,
    pub key_desc: String,
    pub columns: Vec<Column>,
    /// Rows are created by the user (Add/Remove) when true.
    pub editable_rows: bool,
    /// Shared name list the runtime discovers rows from (`npcs`, `boxes`).
    pub discover: Option<&'static str>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Kind {
    Bool,
    /// `@Number` on an integer field.
    Int {
        min: i64,
        max: i64,
        step: i64,
        disabled: Option<Disabled>,
    },
    /// `@Number` on a floating point field.
    Float {
        min: f64,
        max: f64,
        step: f64,
    },
    /// `@Percentage`; stored as a 0..1 fraction, edited as 0..100 %.
    Percent {
        min: f64,
        max: f64,
        step: f64,
    },
    Text,
    /// A nullable hotkey (`Character` field edited by `CharacterEditor`): one
    /// character or a key name such as `F1`; see `keys.rs`.
    Char,
    /// `@Dropdown` over a fixed option list (`multi` for a `Set<Enum>`).
    Choice {
        options: Vec<(String, Value)>,
        multi: bool,
    },
    Dynamic(Source),
    /// `PercentRange`: object `{min, max}` of fractions.
    Range,
    /// `ShipConfig`: object `{config, formation}`.
    Ship,
    /// `PlayerTag`: tag name or null.
    Tag(TagDefault),
    Color {
        alpha: bool,
    },
    /// Condition expression (stored as its source text).
    Condition,
    Table(Box<TableSpec>),
    Json,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Node {
    /// Last path segment, lowercase.
    pub key: String,
    /// Full dotted path, filled in by [`Schema::new`].
    pub path: String,
    pub label: String,
    pub desc: String,
    pub level: Level,
    /// `None` for a group.
    pub kind: Option<Kind>,
    pub default: Value,
    pub support: Option<Support>,
    pub children: Vec<Node>,
    /// Overrides the translation key (`@Option("config.some.key")`).
    pub i18n: String,
}

impl Node {
    pub(crate) fn is_group(&self) -> bool {
        self.kind.is_none()
    }
    pub(crate) fn lvl(mut self, level: Level) -> Self {
        self.level = level;
        self
    }
    pub(crate) fn sup(mut self, support: Support) -> Self {
        self.support = Some(support);
        self
    }
    pub(crate) fn i18n(mut self, key: &str) -> Self {
        self.i18n = key.to_owned();
        self
    }
    pub(crate) fn label(mut self, label: &str) -> Self {
        self.label = label.to_owned();
        self
    }
    pub(crate) fn desc(mut self, desc: &str) -> Self {
        self.desc = desc.to_owned();
        self
    }
}

pub(crate) fn grp(key: &str, children: Vec<Node>) -> Node {
    Node {
        key: key.to_owned(),
        path: String::new(),
        label: String::new(),
        desc: String::new(),
        level: Level::Basic,
        kind: None,
        default: Value::Null,
        support: None,
        children,
        i18n: String::new(),
    }
}

pub(crate) fn opt(key: &str, kind: Kind, default: Value) -> Node {
    Node {
        key: key.to_owned(),
        path: String::new(),
        label: String::new(),
        desc: String::new(),
        level: Level::Basic,
        kind: Some(kind),
        default,
        support: Some(gap("UNSPECIFIED")),
        children: Vec::new(),
        i18n: String::new(),
    }
}

// Kind shorthands -------------------------------------------------------------

pub(crate) fn int(min: i64, max: i64, step: i64) -> Kind {
    Kind::Int {
        min,
        max,
        step,
        disabled: None,
    }
}
/// `@Number` + `@Number.Disabled`.
pub(crate) fn int_off(min: i64, max: i64, step: i64, value: i64, def: i64) -> Kind {
    Kind::Int {
        min,
        max,
        step,
        disabled: Some(Disabled {
            value: value as f64,
            def: def as f64,
        }),
    }
}
pub(crate) fn pct() -> Kind {
    Kind::Percent {
        min: 0.0,
        max: 1.0,
        step: 0.05,
    }
}
pub(crate) fn choice(options: &[(&str, &str)]) -> Kind {
    Kind::Choice {
        options: options
            .iter()
            .map(|(label, value)| ((*label).to_owned(), json!(value)))
            .collect(),
        multi: false,
    }
}
pub(crate) fn ship(config: i64, formation: &str) -> Value {
    json!({ "config": config, "formation": formation })
}
pub(crate) fn range(min: f64, max: f64) -> Value {
    json!({ "min": min, "max": max })
}

// Translations ----------------------------------------------------------------

/// The reference `strings_en.properties`, restricted to `config.*` keys.
const STRINGS: &str = include_str!("settings_en.properties");

fn strings() -> &'static BTreeMap<String, String> {
    static MAP: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    MAP.get_or_init(|| {
        STRINGS
            .lines()
            .filter_map(|line| line.split_once('='))
            .map(|(key, text)| (key.trim().to_owned(), clean_text(text)))
            .collect()
    })
}

/// Drops the Swing `<html>` wrapper and converts `<br>` and `\n` to spaces.
fn clean_text(raw: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    let text = raw.replace("\\n", " ").replace("<br>", " ");
    for ch in text.chars() {
        match ch {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(crate) fn translate(key: &str) -> Option<&'static str> {
    strings().get(key).map(String::as_str)
}

/// `bot_gui` -> `Bot gui`.
pub(crate) fn humanize(key: &str) -> String {
    let mut text = key.replace('_', " ");
    if let Some(first) = text.get(..1) {
        text = format!("{}{}", first.to_uppercase(), &text[1..]);
    }
    text
}

// Schema ----------------------------------------------------------------------

pub(crate) struct Schema {
    pub roots: Vec<Node>,
}

impl Schema {
    /// Fills in paths, labels and descriptions from the translation table.
    pub(crate) fn new(mut roots: Vec<Node>) -> Self {
        for node in &mut roots {
            finalize(node, "");
        }
        Self { roots }
    }

    pub(crate) fn find(&self, path: &str) -> Option<&Node> {
        let mut nodes = &self.roots;
        let mut found = None;
        for segment in path.split('.') {
            let node = nodes.iter().find(|node| node.key == segment)?;
            nodes = &node.children;
            found = Some(node);
        }
        found
    }

    /// Every option (non-group) in declaration order.
    pub(crate) fn leaves(&self) -> Vec<&Node> {
        fn walk<'a>(nodes: &'a [Node], out: &mut Vec<&'a Node>) {
            for node in nodes {
                if node.is_group() {
                    walk(&node.children, out);
                } else {
                    out.push(node);
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.roots, &mut out);
        out
    }
}

fn finalize(node: &mut Node, parent: &str) {
    node.path = if parent.is_empty() {
        node.key.clone()
    } else {
        format!("{parent}.{}", node.key)
    };
    let i18n = if node.i18n.is_empty() {
        format!("config.{}", node.path)
    } else {
        node.i18n.clone()
    };
    if node.label.is_empty() {
        node.label = translate(&i18n)
            .map(str::to_owned)
            .unwrap_or_else(|| humanize(&node.key));
    }
    if node.desc.is_empty() {
        node.desc = translate(&format!("{i18n}.desc"))
            .unwrap_or_default()
            .to_owned();
    }
    if let Some(Kind::Table(spec)) = &mut node.kind {
        // Column text lives under the row-class key (`config.loot.npc_table`).
        let base = format!("config.{}", table_i18n_base(&node.path));
        if let Some(text) = translate(&base) {
            spec.key_label = text.to_owned();
        }
        if let Some(text) = translate(&format!("{base}.desc")) {
            spec.key_desc = text.to_owned();
        }
        for column in &mut spec.columns {
            let key = format!("{base}.{}", column.key);
            if let Some(text) = translate(&key) {
                column.label = text.to_owned();
            }
            if let Some(text) = translate(&format!("{key}.desc")) {
                column.desc = text.to_owned();
            }
        }
    }
    let path = node.path.clone();
    for child in &mut node.children {
        finalize(child, &path);
    }
}

/// `loot.npc_infos` -> `loot.npc_table` (the reference implementation's `@Configuration` key).
pub(crate) fn table_i18n_base(path: &str) -> String {
    path.strip_suffix("_infos")
        .map(|base| format!("{base}_table"))
        .unwrap_or_else(|| path.to_owned())
}

pub(crate) fn column(key: &str, kind: Kind, default: Value) -> Column {
    Column {
        key: key.to_owned(),
        label: humanize(key),
        desc: String::new(),
        kind,
        default,
    }
}

pub(crate) fn table(
    columns: Vec<Column>,
    editable_rows: bool,
    discover: Option<&'static str>,
) -> Kind {
    Kind::Table(Box::new(TableSpec {
        key_label: "Name".into(),
        key_desc: String::new(),
        columns,
        editable_rows,
        discover,
    }))
}

// Value helpers ----------------------------------------------------------------

pub(crate) fn get_path<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.')
        .try_fold(root, |value, part| value.as_object()?.get(part))
}

pub(crate) fn set_path(root: &mut Value, path: &str, value: Value) {
    let mut current = root;
    let mut parts = path.split('.').peekable();
    while let Some(part) = parts.next() {
        if !current.is_object() {
            *current = json!({});
        }
        let object = current.as_object_mut().expect("object ensured above");
        if parts.peek().is_none() {
            object.insert(part.to_owned(), value);
            return;
        }
        current = object.entry(part.to_owned()).or_insert_with(|| json!({}));
    }
}

pub(crate) fn remove_path(root: &mut Value, path: &str) {
    let mut parts: Vec<&str> = path.split('.').collect();
    let Some(last) = parts.pop() else { return };
    let mut current = root;
    for part in parts {
        match current.get_mut(part) {
            Some(next) => current = next,
            None => return,
        }
    }
    if let Some(object) = current.as_object_mut() {
        object.remove(last);
    }
}

impl Kind {
    /// Coerces an arbitrary JSON value into a valid one for this kind,
    /// mirroring the reference implementation's `ValueHandler.validate` (clamp rather than reject).
    /// `None` means the value cannot be represented at all.
    pub(crate) fn normalize(&self, value: &Value) -> Option<Value> {
        match self {
            Kind::Bool => value.as_bool().map(Value::Bool),
            Kind::Int {
                min, max, disabled, ..
            } => {
                let number = value.as_f64().filter(|n| n.is_finite())?;
                if disabled.as_ref().is_some_and(|d| number == d.value) {
                    return Some(json!(number as i64));
                }
                Some(json!(number.round().clamp(*min as f64, *max as f64) as i64))
            }
            Kind::Float { min, max, .. } | Kind::Percent { min, max, .. } => {
                let number = value.as_f64().filter(|n| n.is_finite())?;
                Some(json!(number.clamp(*min, *max)))
            }
            Kind::Text | Kind::Condition => value.as_str().map(|s| json!(s)),
            Kind::Char => match value {
                Value::Null => Some(Value::Null),
                Value::String(s) if s.is_empty() => Some(Value::Null),
                Value::String(s) => super::keys::normalize(s).map(|key| json!(key)),
                _ => None,
            },
            Kind::Choice { options, multi } => {
                if *multi {
                    let items = value.as_array()?;
                    let kept: Vec<Value> = options
                        .iter()
                        .filter(|(_, option)| items.contains(option))
                        .map(|(_, option)| option.clone())
                        .collect();
                    Some(Value::Array(kept))
                } else {
                    options
                        .iter()
                        .any(|(_, option)| option == value)
                        .then(|| value.clone())
                }
            }
            Kind::Dynamic(Source::Modules) => value.as_str().map(|s| json!(s)),
            Kind::Dynamic(Source::Maps) => (value.is_null()
                || value.as_i64().is_some_and(|id| id > 0)
                || value.as_str().is_some_and(|s| !s.trim().is_empty()))
            .then(|| value.clone()),
            Kind::Range => {
                let min = value.get("min")?.as_f64()?.clamp(0.0, 1.0);
                let max = value.get("max")?.as_f64()?.clamp(0.0, 1.0);
                Some(json!({ "min": min.min(max), "max": max.max(min) }))
            }
            Kind::Ship => {
                let config = value.get("config")?.as_i64()?.clamp(1, 2);
                let formation = match value.get("formation") {
                    Some(Value::String(s)) if !s.is_empty() => super::keys::normalize(s)
                        .map(|key| json!(key))
                        .unwrap_or(Value::Null),
                    _ => Value::Null,
                };
                Some(json!({ "config": config, "formation": formation }))
            }
            Kind::Tag(_) => match value {
                Value::Null => Some(Value::Null),
                Value::String(_) => Some(value.clone()),
                _ => None,
            },
            Kind::Color { alpha } => {
                let text = value.as_str()?.trim().trim_start_matches('#');
                let digits = if *alpha { [6usize, 8] } else { [6, 6] };
                (digits.contains(&text.len()) && text.chars().all(|c| c.is_ascii_hexdigit()))
                    .then(|| json!(format!("#{}", text.to_ascii_lowercase())))
            }
            Kind::Table(spec) => {
                let rows = value.as_object()?;
                let mut out = serde_json::Map::new();
                for (name, row) in rows {
                    let row = row.as_object()?;
                    let mut clean = serde_json::Map::new();
                    for column in &spec.columns {
                        let raw = row.get(&column.key).unwrap_or(&column.default);
                        clean.insert(
                            column.key.clone(),
                            column
                                .kind
                                .normalize(raw)
                                .unwrap_or_else(|| column.default.clone()),
                        );
                    }
                    out.insert(name.clone(), Value::Object(clean));
                }
                Some(Value::Object(out))
            }
            Kind::Json => Some(value.clone()),
        }
    }
}
