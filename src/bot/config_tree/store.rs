//! Values, profiles and persistence for the settings tree.
//!
//! The file layout mirrors the reference implementation's `ConfigManager`: `config.json` is the
//! default profile, other profiles live in `configs/<name>.json`, and each
//! save first moves the previous file to `<name>_old.json`. Only explicitly
//! set values are written, so new options pick up their defaults without a
//! migration. A file that fails to parse disables saving for the session.
use super::schema::{Kind, Schema, get_path, remove_path, set_path};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub(crate) const DEFAULT_PROFILE: &str = "config";
const FOLDER: &str = "configs";
const BACKUP: &str = "_old";
const EXTENSION: &str = ".json";
const LAST_PROFILE: &str = "config_profile.txt";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Loaded {
    /// A profile file was read.
    File,
    /// No file existed; the profile starts from defaults.
    Missing,
    /// A file existed but could not be parsed; saving is disabled.
    Failed,
}

pub(crate) struct ConfigStore {
    schema: Arc<Schema>,
    dir: PathBuf,
    profile: String,
    /// Sparse nested object of explicitly set values.
    values: Value,
    failed: bool,
    dirty: bool,
}

impl ConfigStore {
    pub(crate) fn new(schema: Arc<Schema>, dir: PathBuf) -> Self {
        Self {
            schema,
            dir,
            profile: DEFAULT_PROFILE.into(),
            values: json!({}),
            failed: false,
            dirty: false,
        }
    }

    pub(crate) fn profile(&self) -> &str {
        &self.profile
    }
    #[cfg(test)]
    pub(crate) fn failed(&self) -> bool {
        self.failed
    }

    // Values ------------------------------------------------------------------

    /// Stored value if valid for the option, otherwise its default.
    pub(crate) fn get(&self, path: &str) -> Value {
        let Some(node) = self.schema.find(path) else {
            return Value::Null;
        };
        let Some(kind) = &node.kind else {
            return Value::Null;
        };
        get_path(&self.values, path)
            .and_then(|stored| kind.normalize(stored))
            .unwrap_or_else(|| node.default.clone())
    }

    /// Materialize every schema value (including defaults) for runtime
    /// consumers. The saved profile remains sparse; this is only the resolved
    /// view sent through `BotSettings`.
    pub(crate) fn resolved(&self) -> Value {
        let mut resolved = json!({});
        for node in self.schema.leaves() {
            set_path(&mut resolved, &node.path, self.get(&node.path));
        }
        resolved
    }

    /// Interface preferences (visibility level, last tab) saved beside the
    /// options, under a reserved key no option path can collide with.
    pub(crate) fn ui(&self, key: &str) -> Option<&Value> {
        self.values.get("_ui")?.get(key)
    }

    pub(crate) fn set_ui(&mut self, key: &str, value: Value) {
        if self.ui(key) == Some(&value) {
            return;
        }
        set_path(&mut self.values, &format!("_ui.{key}"), value);
        self.dirty = true;
    }

    pub(crate) fn is_default(&self, path: &str) -> bool {
        match self.schema.find(path) {
            Some(node) => self.get(path) == node.default,
            None => true,
        }
    }

    /// Validates (clamping, like the reference implementation) and stores. Returns the stored value.
    pub(crate) fn set(&mut self, path: &str, value: Value) -> Result<Value, String> {
        let node = self
            .schema
            .find(path)
            .ok_or_else(|| format!("unknown setting {path}"))?;
        let kind: &Kind = node
            .kind
            .as_ref()
            .ok_or_else(|| format!("{path} is a group"))?;
        let normalized = kind
            .normalize(&value)
            .ok_or_else(|| format!("{} has an invalid value", node.label))?;
        if self.get(path) != normalized || get_path(&self.values, path).is_none() {
            set_path(&mut self.values, path, normalized.clone());
            self.dirty = true;
        }
        Ok(normalized)
    }

    pub(crate) fn reset(&mut self, path: &str) {
        let Some(node) = self.schema.find(path) else {
            return;
        };
        let paths: Vec<String> = if node.is_group() {
            fn collect(node: &super::schema::Node, out: &mut Vec<String>) {
                for child in &node.children {
                    if child.is_group() {
                        collect(child, out);
                    } else {
                        out.push(child.path.clone());
                    }
                }
            }
            let mut out = Vec::new();
            collect(node, &mut out);
            out
        } else {
            vec![path.to_owned()]
        };
        for path in paths {
            remove_path(&mut self.values, &path);
        }
        self.dirty = true;
    }

    // Profiles ----------------------------------------------------------------

    fn file_for(&self, name: &str) -> PathBuf {
        if name == DEFAULT_PROFILE {
            self.dir.join(format!("{DEFAULT_PROFILE}{EXTENSION}"))
        } else {
            self.dir.join(FOLDER).join(format!("{name}{EXTENSION}"))
        }
    }
    fn backup_for(&self, name: &str) -> PathBuf {
        if name == DEFAULT_PROFILE {
            self.dir
                .join(format!("{DEFAULT_PROFILE}{BACKUP}{EXTENSION}"))
        } else {
            self.dir
                .join(FOLDER)
                .join(format!("{name}{BACKUP}{EXTENSION}"))
        }
    }

    pub(crate) fn valid_profile_name(name: &str) -> bool {
        !name.is_empty()
            && name.len() <= 48
            && name != DEFAULT_PROFILE
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ' '))
            && !name.ends_with(BACKUP)
    }

    /// Profile that was active when the application last closed.
    pub(crate) fn last_profile(&self) -> String {
        std::fs::read_to_string(self.dir.join(LAST_PROFILE))
            .ok()
            .map(|name| name.trim().to_owned())
            .filter(|name| {
                name == DEFAULT_PROFILE
                    || (Self::valid_profile_name(name) && self.file_for(name).exists())
            })
            .unwrap_or_else(|| DEFAULT_PROFILE.into())
    }

    pub(crate) fn profiles(&self) -> Vec<String> {
        let mut names = vec![DEFAULT_PROFILE.to_owned()];
        let mut found: Vec<(std::time::SystemTime, String)> =
            std::fs::read_dir(self.dir.join(FOLDER))
                .into_iter()
                .flatten()
                .flatten()
                .filter_map(|entry| {
                    let file = entry.file_name().to_string_lossy().into_owned();
                    let name = file.strip_suffix(EXTENSION)?;
                    if name.ends_with(BACKUP) {
                        return None;
                    }
                    let modified = entry.metadata().and_then(|m| m.modified()).ok()?;
                    Some((modified, name.to_owned()))
                })
                .collect();
        found.sort_by(|a, b| b.0.cmp(&a.0));
        names.extend(found.into_iter().map(|(_, name)| name));
        names
    }

    /// Switches to `name`, saving the current profile first.
    pub(crate) fn load_profile(&mut self, name: &str) -> Loaded {
        self.save();
        self.profile = name.to_owned();
        let outcome = self.read_into_values();
        let _ = std::fs::create_dir_all(&self.dir);
        let _ = std::fs::write(self.dir.join(LAST_PROFILE), name);
        outcome
    }

    fn read_into_values(&mut self) -> Loaded {
        let file = self.file_for(&self.profile);
        let backup = self.backup_for(&self.profile);
        self.dirty = false;
        let parse = |path: &Path| -> Option<Value> {
            let data = std::fs::read(path).ok()?;
            serde_json::from_slice::<Value>(&data)
                .ok()
                .filter(Value::is_object)
        };
        let (exists_file, exists_backup) = (file.exists(), backup.exists());
        let mut loaded = if exists_file { parse(&file) } else { None };
        if loaded.is_none() && exists_backup {
            loaded = parse(&backup);
            self.dirty = loaded.is_some();
        }
        self.failed = loaded.is_none() && (exists_file || exists_backup);
        let outcome = match (&loaded, self.failed) {
            (Some(_), _) => Loaded::File,
            (None, true) => Loaded::Failed,
            (None, false) => Loaded::Missing,
        };
        self.values = loaded.unwrap_or_else(|| json!({}));
        outcome
    }

    pub(crate) fn save(&mut self) {
        if self.failed || !self.dirty {
            return;
        }
        let file = self.file_for(&self.profile);
        let backup = self.backup_for(&self.profile);
        if let Some(parent) = file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if file.exists() {
            let _ = std::fs::rename(&file, &backup)
                .or_else(|_| std::fs::copy(&file, &backup).map(|_| ()));
        }
        if let Ok(data) = serde_json::to_vec_pretty(&self.values)
            && std::fs::write(&file, data).is_ok()
        {
            self.dirty = false;
        }
    }

    /// Creates a profile file, optionally copying an existing one.
    pub(crate) fn create_profile(&self, name: &str, copy: Option<&str>) -> Result<(), String> {
        if !Self::valid_profile_name(name) {
            return Err("Use letters, digits, spaces, '-' or '_' (not 'config').".into());
        }
        let target = self.file_for(name);
        if target.exists() {
            return Err(format!("Profile {name} already exists"));
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        match copy {
            Some(source) if source == self.profile => {
                let data = serde_json::to_vec_pretty(&self.values).map_err(|e| e.to_string())?;
                std::fs::write(&target, data).map_err(|e| e.to_string())
            }
            Some(source) => std::fs::copy(self.file_for(source), &target)
                .map(|_| ())
                .map_err(|e| e.to_string()),
            None => std::fs::write(&target, b"{}").map_err(|e| e.to_string()),
        }
    }

    pub(crate) fn delete_profile(&self, name: &str) -> Result<(), String> {
        if name == DEFAULT_PROFILE {
            return Err("The default profile cannot be deleted".into());
        }
        if name == self.profile {
            return Err("Switch to another profile before deleting this one".into());
        }
        for path in [self.file_for(name), self.backup_for(name)] {
            if path.exists() {
                std::fs::remove_file(path).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    /// True when the active profile has no file yet (first run).
    #[cfg(test)]
    pub(crate) fn needs_seed(&self) -> bool {
        !self.failed
            && !self.file_for(&self.profile).exists()
            && !self.backup_for(&self.profile).exists()
    }
}
