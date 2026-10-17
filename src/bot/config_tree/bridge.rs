//! Pushes the options the runtime understands from the settings tree into
//! `BotSettings` (and so into every module's `ModuleConfig`), and seeds the
//! tree from existing `modules.json` values on first run.
//!
//! `BRIDGED` lists options explicitly copied into `BotSettings`. The complete
//! resolved tree is also retained in `BotSettings.config_tree` for shell and
//! engine consumers; support metadata and the gap catalogue are checked in
//! `config_tree/tests.rs`.
use super::schema::{get_path, set_path};
use super::store::ConfigStore;
use serde_json::{Value, json};
use xenomit_bot_runtime::settings::{BotSettings, ClientKind, FlashBackend};
use xenomit_game_api::ClientBackend;

/// Option paths with runtime effect.
#[cfg(test)]
pub(crate) const BRIDGED: &[&str] = &[
    "general.current_module",
    "general.working_map",
    "general.offensive",
    "general.roam",
    "general.run",
    "general.safety.repair_hp_range",
    "general.safety.repair_hp_no_npc",
    "general.safety.repair_to_shield",
    "general.safety.repair",
    "general.safety.max_deaths",
    "general.safety.revive",
    "general.safety.wait_before_revive",
    "general.safety.instant_repair",
    "general.running.run_from_enemies",
    "general.running.run_from_enemies_sight",
    "general.running.max_sight_distance",
    "general.roaming.only_kill_preferred",
    "collect.stay_away_from_enemies",
    "collect.auto_cloack",
    "collect.radius",
    "collect.ignore_contested_boxes",
    "collect.box_infos",
    "loot.sab.enabled",
    "loot.sab.percent",
    "loot.sab.npc_amount",
    "loot.npc_infos",
    "loot.npc_distance_ignore",
    "pet.enabled",
    "pet.module_id",
    "group.accept_invites",
    "miscellaneous.refresh_time",
    "bot_settings.api_config.browser_api",
    "xenomit.safety.enabled",
    "xenomit.safety.auto_revive",
    "xenomit.safety.retreat_low_health",
    "xenomit.safety.defensive_ability",
    "xenomit.collect.collect_boxes",
    "xenomit.collect.collect_cargo",
    "xenomit.collect.collect_palladium",
    "xenomit.collect.pickup_radius",
    "xenomit.collect.max_box_distance",
    "xenomit.collect.wait_ms",
    "xenomit.combat.prioritize_npcs",
    "xenomit.combat.switch_configs",
    "xenomit.combat.npc_radius",
    "xenomit.combat.stationary_radius",
    "xenomit.combat.offensive_ability",
    "xenomit.combat.offensive_ability_range",
    "xenomit.combat.laser_ammo",
    "xenomit.combat.rocket_ammo",
    "xenomit.pet.pet_auto_loot",
    "xenomit.pet.pet_resource_collector",
    "xenomit.pet.locator_target",
    "xenomit.group.follow_leader",
    "xenomit.group.assist",
    "xenomit.engine.tick_ms",
    "xenomit.engine.game_api_backend",
    "xenomit.engine.flash_backend",
    "xenomit.engine.behavior_config",
];

/// Identity or scaled mappings onto a module payload key.
/// `(tree path, engine key, factor)`; the tree value is multiplied by `factor`.
const DIRECT: &[(&str, &str, f64)] = &[
    (
        "general.running.run_from_enemies",
        "safety.run_from_enemy",
        1.0,
    ),
    (
        "general.running.run_from_enemies_sight",
        "safety.run_in_sight",
        1.0,
    ),
    (
        "general.running.max_sight_distance",
        "safety.sight_distance",
        1.0,
    ),
    (
        "general.roaming.only_kill_preferred",
        "only_kill_preferred",
        1.0,
    ),
    (
        "collect.stay_away_from_enemies",
        "stay_away_from_enemies",
        1.0,
    ),
    ("collect.auto_cloack", "auto_cloak", 1.0),
    ("collect.radius", "collect_radius", 1.0),
    (
        "collect.ignore_contested_boxes",
        "ignore_contested_boxes",
        1.0,
    ),
    ("loot.sab.enabled", "sab_enabled", 1.0),
    ("loot.sab.percent", "sab_shield_percent", 100.0),
    ("loot.sab.npc_amount", "sab_target_shield_min", 1.0),
    ("loot.npc_distance_ignore", "max_npc_distance", 1.0),
    ("pet.enabled", "features.pet_active", 1.0),
    ("pet.module_id", "pet_gear", 1.0),
    ("xenomit.safety.enabled", "safety.enabled", 1.0),
    ("xenomit.safety.auto_revive", "safety.auto_revive", 1.0),
    ("xenomit.safety.defensive_ability", "defensive_ability", 1.0),
    (
        "xenomit.collect.collect_boxes",
        "features.collect_boxes",
        1.0,
    ),
    (
        "xenomit.collect.collect_cargo",
        "features.collect_cargo",
        1.0,
    ),
    ("xenomit.collect.pickup_radius", "pickup_radius", 1.0),
    ("xenomit.collect.max_box_distance", "max_box_distance", 1.0),
    ("xenomit.collect.wait_ms", "wait_ms", 1.0),
    (
        "xenomit.combat.switch_configs",
        "features.switch_configs",
        1.0,
    ),
    ("xenomit.combat.npc_radius", "npc_radius", 1.0),
    ("xenomit.combat.stationary_radius", "stationary_radius", 1.0),
    ("xenomit.combat.offensive_ability", "offensive_ability", 1.0),
    (
        "xenomit.combat.offensive_ability_range",
        "offensive_ability_range",
        1.0,
    ),
    ("xenomit.combat.laser_ammo", "laser_ammo", 1.0),
    ("xenomit.combat.rocket_ammo", "rocket_ammo", 1.0),
    ("xenomit.pet.locator_target", "pet_locator_target", 1.0),
];

/// Options stored in `BotSettings.feature_flags` (global, read by every
/// module as `features.*`). Flags a module also declares (`pet_active`,
/// `collect_boxes`, ...) are written to the module payload only, through
/// `DIRECT`, so the global value other modules inherit is left alone.
/// `(tree path, flag name)`.
const FLAGS: &[(&str, &str)] = &[
    ("group.accept_invites", "group_invites"),
    ("xenomit.safety.retreat_low_health", "retreat_low_health"),
    ("xenomit.collect.collect_palladium", "collect_palladium"),
    ("xenomit.combat.prioritize_npcs", "prioritize_npcs"),
    ("xenomit.pet.pet_auto_loot", "pet_auto_loot"),
    (
        "xenomit.pet.pet_resource_collector",
        "pet_resource_collector",
    ),
    ("xenomit.group.follow_leader", "group_follow_leader"),
    ("xenomit.group.assist", "group_assist"),
];

#[cfg(test)]
pub(crate) fn handles(path: &str) -> bool {
    BRIDGED.contains(&path)
}

fn percent_u8(fraction: f64) -> u8 {
    (fraction * 100.0).round().clamp(0.0, 100.0) as u8
}

fn module_set(settings: &mut BotSettings, ids: &[String], key: &str, value: Value) {
    for id in ids {
        let payload = settings
            .modules
            .entry(id.clone())
            .or_insert_with(|| json!({}));
        if !payload.is_object() {
            *payload = json!({});
        }
        set_path(payload, key, value.clone());
    }
}

/// Module the tree asks to activate, when it differs from `settings`.
pub(crate) fn pending_module(store: &ConfigStore, settings: &BotSettings) -> Option<String> {
    store
        .get("general.current_module")
        .as_str()
        .filter(|module| !module.is_empty() && *module != settings.active_module)
        .map(str::to_owned)
}

/// Writes every bridged option into `settings`. Module-level keys go to the
/// active module's payload only, so other modules keep their own values until
/// they are activated (which re-imports, see `import`). The active module
/// itself is switched by the host via [`pending_module`], never here.
pub(crate) fn apply(store: &ConfigStore, settings: &mut BotSettings) {
    settings.config_tree = store.resolved();
    let ids = vec![settings.active_module.clone()];
    let ids = &ids[..];

    for (path, key, factor) in DIRECT {
        let value = store.get(path);
        let value = match (value.as_f64(), *factor != 1.0) {
            (Some(number), true) => json!(number * factor),
            _ => value,
        };
        module_set(settings, ids, key, value);
    }
    for (path, flag) in FLAGS {
        if let Some(on) = store.get(path).as_bool() {
            settings.feature_flags.insert((*flag).to_owned(), on);
        }
    }

    let map = store.get("general.working_map");
    match &map {
        Value::Null => {}
        Value::Number(number) => settings.working_map = number.to_string(),
        Value::String(name) => settings.working_map = name.clone(),
        _ => {}
    }
    module_set(settings, ids, "working_map", map);

    for (path, key, slot) in [
        ("general.offensive", "offensive_config", 0),
        ("general.roam", "roam_config", 1),
        ("general.run", "run_config", 2),
        ("general.safety.repair", "repair_config", 3),
    ] {
        let config = store.get(path)["config"].as_i64().unwrap_or(1).clamp(1, 2);
        module_set(settings, ids, key, json!(config));
        let config = config as u8;
        match slot {
            0 => settings.offensive_config = config,
            1 => settings.roam_config = config,
            2 => settings.run_config = config,
            _ => settings.repair_config = config,
        }
    }

    let range = store.get("general.safety.repair_hp_range");
    let (min, max) = (
        range["min"].as_f64().unwrap_or(0.4),
        range["max"].as_f64().unwrap_or(0.95),
    );
    module_set(
        settings,
        ids,
        "safety.min_health_percent",
        json!(min * 100.0),
    );
    module_set(
        settings,
        ids,
        "safety.max_health_percent",
        json!(max * 100.0),
    );
    settings.min_health = percent_u8(min);
    settings.max_health = percent_u8(max);

    let roaming = store
        .get("general.safety.repair_hp_no_npc")
        .as_f64()
        .unwrap_or(0.5);
    module_set(
        settings,
        ids,
        "safety.roaming_repair_health_percent",
        json!(roaming * 100.0),
    );
    settings.roaming_repair_health = percent_u8(roaming);

    let shield = store
        .get("general.safety.repair_to_shield")
        .as_f64()
        .unwrap_or(1.0);
    module_set(
        settings,
        ids,
        "safety.repair_shield_percent",
        json!(shield * 100.0),
    );
    settings.repair_shield = percent_u8(shield);

    // Legacy's "disabled" sentinel (-1) means unlimited; the engine uses 0.
    // Legacy allows up to 9999; the engine schema stops at 1000.
    let deaths = store
        .get("general.safety.max_deaths")
        .as_i64()
        .unwrap_or(10)
        .clamp(0, 1000);
    module_set(settings, ids, "safety.max_deaths", json!(deaths));
    settings.max_deaths = deaths.min(255) as u8;

    let wait = store
        .get("general.safety.wait_before_revive")
        .as_i64()
        .unwrap_or(5);
    module_set(settings, ids, "safety.revive_wait_seconds", json!(wait));
    settings.revive_wait_seconds = wait.clamp(0, 255) as u8;

    settings.revive_location = match store.get("general.safety.revive").as_str() {
        Some("portal") => "Portal",
        Some("spot") => "Spot",
        _ => "Base",
    }
    .into();
    settings.instant_repair_above = store
        .get("general.safety.instant_repair")
        .as_i64()
        .unwrap_or(0)
        .clamp(0, i64::from(u16::MAX)) as u16;

    // Refresh: 0 minutes disables it, like Legacy.
    let minutes = store
        .get("miscellaneous.refresh_time")
        .as_i64()
        .unwrap_or(60);
    if !settings.behaviors.is_object() {
        settings.behaviors = json!({});
    }
    set_path(
        &mut settings.behaviors,
        "refresh.enabled",
        json!(minutes > 0),
    );
    set_path(
        &mut settings.behaviors,
        "refresh.interval_minutes",
        json!(minutes.max(1)),
    );
    let extra_events = extra_action_events(store);
    set_path(
        &mut settings.behaviors,
        "custom_events.enabled",
        json!(extra_events.iter().any(|event| event["active"] == true)),
    );
    set_path(
        &mut settings.behaviors,
        "custom_events.events",
        json!(extra_events),
    );
    let sab_condition = parse_condition(&store.get("loot.sab.condition"));
    module_set(settings, ids, "sab_condition", sab_condition);

    if let Ok(client) =
        serde_json::from_value::<ClientKind>(store.get("bot_settings.api_config.browser_api"))
    {
        settings.client = client;
    }
    if let Ok(backend) =
        serde_json::from_value::<ClientBackend>(store.get("xenomit.engine.game_api_backend"))
    {
        settings.game_api_backend = backend;
    }
    if let Ok(flash) =
        serde_json::from_value::<FlashBackend>(store.get("xenomit.engine.flash_backend"))
    {
        settings.flash_backend = flash;
    }

    // 0 ("disabled") leaves whatever the payload/backend default already is.
    let tick = store.get("xenomit.engine.tick_ms").as_i64().unwrap_or(0);
    if tick > 0 {
        module_set(settings, ids, "tick_ms", json!(tick));
    }
    let config = store.get("xenomit.engine.behavior_config");
    settings.behavior_config = if config.is_null() {
        None
    } else {
        serde_json::from_value(config).ok()
    };

    // Shared target tables: rows are the authority once the tree owns them.
    settings.npcs = rows_as_flags(&store.get("loot.npc_infos"), "kill");
    settings.boxes = rows_as_flags(&store.get("collect.box_infos"), "collect");
    module_set(settings, ids, "npc_table", store.get("loot.npc_infos"));
    module_set(settings, ids, "box_table", store.get("collect.box_infos"));
}

fn rows_as_flags(table: &Value, column: &str) -> std::collections::BTreeMap<String, bool> {
    table
        .as_object()
        .map(|rows| {
            rows.iter()
                .map(|(name, row)| (name.clone(), row[column].as_bool().unwrap_or(true)))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_condition(value: &Value) -> Value {
    value
        .as_str()
        .and_then(|source| xenomit_behavior_engine::parse_rule_condition(source).ok())
        .unwrap_or_else(|| json!({"unsupported_condition": true}))
}

fn extra_action_events(store: &ConfigStore) -> Vec<Value> {
    let global_condition = parse_condition(&store.get("extra.condition"));
    store
        .get("extra.action_infos")
        .as_object()
        .map(|rows| {
            rows.iter()
                .filter_map(|(_, row)| {
                    let active = row["active"].as_bool().unwrap_or(false);
                    if !active {
                        return None;
                    }
                    let condition = parse_condition(&row["condition"]);
                    let condition = match (
                        global_condition.as_object().is_some_and(|v| !v.is_empty()),
                        condition.as_object().is_some_and(|v| !v.is_empty()),
                    ) {
                        (true, true) => json!({"all": [global_condition.clone(), condition]}),
                        (true, false) => global_condition.clone(),
                        (false, true) => condition,
                        (false, false) => json!({}),
                    };
                    let action = row["action"]
                        .as_str()
                        .and_then(|source| {
                            xenomit_behavior_engine::parse_rule_action(source).ok()
                        })
                        .unwrap_or_else(|| json!({"unsupported_action": true}));
                    Some(json!({
                        "active": true,
                        "check_every_ms": row["check_every"].as_u64().unwrap_or(0),
                        "condition": condition,
                        "cooldown_ms": row["cooldown"].as_u64().unwrap_or(0),
                        "action": action,
                        "limit": row["limit"].as_u64().unwrap_or(0),
                    }))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Seeds the tree from existing settings so users keep what they configured
/// before the tree existed. Called once when a profile has no file.
pub(crate) fn import(
    settings: &BotSettings,
    store: &mut ConfigStore,
    descriptor: Option<&xenomit_game_api::ModuleDescriptor>,
) {
    // The active module's payload with the descriptor's defaults filled in, so
    // keys the user never touched take the engine's value, not Legacy's.
    let raw = settings.module_config().settings;
    let payload = descriptor
        .and_then(|descriptor| descriptor.prepare_settings(raw.clone()).ok())
        .unwrap_or(raw);
    let put = |store: &mut ConfigStore, path: &str, value: Value| {
        let _ = store.set(path, value);
    };

    for (path, flag) in FLAGS {
        if let Some(on) = settings.feature_flags.get(*flag) {
            put(store, path, json!(on));
        }
    }
    for (path, key, factor) in DIRECT {
        if let Some(value) = get_path(&payload, key) {
            let value = match (value.as_f64(), *factor != 1.0) {
                (Some(number), true) => json!(number / factor),
                _ => value.clone(),
            };
            put(store, path, value);
        }
    }
    put(
        store,
        "general.current_module",
        json!(settings.active_module),
    );
    if let Some(map) = payload.get("working_map") {
        put(store, "general.working_map", map.clone());
    }
    // Module payload first (it overrides the shared fields), shared field second.
    let number = |key: &str| get_path(&payload, key).and_then(Value::as_f64);
    for (path, key, shared) in [
        (
            "general.offensive",
            "offensive_config",
            settings.offensive_config,
        ),
        ("general.roam", "roam_config", settings.roam_config),
        ("general.run", "run_config", settings.run_config),
        (
            "general.safety.repair",
            "repair_config",
            settings.repair_config,
        ),
    ] {
        let config = number(key).map(|n| n as i64).unwrap_or(i64::from(shared));
        let mut current = store.get(path);
        current["config"] = json!(config);
        put(store, path, current);
    }
    let fraction =
        |key: &str, shared: u8| number(key).map_or(f64::from(shared) / 100.0, |n| n / 100.0);
    put(
        store,
        "general.safety.repair_hp_range",
        json!({
            "min": fraction("safety.min_health_percent", settings.min_health),
            "max": fraction("safety.max_health_percent", settings.max_health),
        }),
    );
    put(
        store,
        "general.safety.repair_hp_no_npc",
        json!(fraction(
            "safety.roaming_repair_health_percent",
            settings.roaming_repair_health
        )),
    );
    put(
        store,
        "general.safety.repair_to_shield",
        json!(fraction(
            "safety.repair_shield_percent",
            settings.repair_shield
        )),
    );
    // The engine uses 0 for "unlimited"; the tree's disabled sentinel is -1.
    let deaths = number("safety.max_deaths").map_or(i64::from(settings.max_deaths), |n| n as i64);
    put(
        store,
        "general.safety.max_deaths",
        json!(if deaths <= 0 { -1 } else { deaths }),
    );
    put(
        store,
        "general.safety.wait_before_revive",
        json!(
            number("safety.revive_wait_seconds")
                .map_or(i64::from(settings.revive_wait_seconds), |n| n as i64)
        ),
    );
    put(
        store,
        "general.safety.revive",
        json!(settings.revive_location.to_ascii_lowercase()),
    );
    put(
        store,
        "general.safety.instant_repair",
        json!(settings.instant_repair_above),
    );
    if let Some(refresh) = settings.behaviors.get("refresh") {
        let enabled = refresh["enabled"].as_bool().unwrap_or(true);
        let minutes = refresh["interval_minutes"].as_i64().unwrap_or(60);
        put(
            store,
            "miscellaneous.refresh_time",
            json!(if enabled { minutes } else { 0 }),
        );
    }
    if let Ok(client) = serde_json::to_value(settings.client) {
        put(store, "bot_settings.api_config.browser_api", client);
    }
    if let Ok(backend) = serde_json::to_value(settings.game_api_backend) {
        put(store, "xenomit.engine.game_api_backend", backend);
    }
    if let Ok(flash) = serde_json::to_value(settings.flash_backend) {
        put(store, "xenomit.engine.flash_backend", flash);
    }
    if let Some(config) = &settings.behavior_config
        && let Ok(config) = serde_json::to_value(config)
    {
        put(store, "xenomit.engine.behavior_config", config);
    }
    if let Some(tick) = settings
        .modules
        .get(&settings.active_module)
        .and_then(|payload| payload.get("tick_ms"))
    {
        put(store, "xenomit.engine.tick_ms", tick.clone());
    }
    merge_flags(store, "loot.npc_infos", "kill", &settings.npcs);
    merge_flags(store, "collect.box_infos", "collect", &settings.boxes);
}

/// Adds rows for names present in the shared flag map but missing from the
/// table, preserving existing row data.
pub(crate) fn merge_flags(
    store: &mut ConfigStore,
    path: &str,
    column: &str,
    names: &std::collections::BTreeMap<String, bool>,
) -> bool {
    let mut table = store.get(path);
    let Some(rows) = table.as_object_mut() else {
        return false;
    };
    let mut changed = false;
    for (name, flag) in names {
        match rows.get_mut(name).and_then(Value::as_object_mut) {
            Some(row) => {
                if row.get(column).and_then(Value::as_bool) != Some(*flag) {
                    row.insert(column.to_owned(), json!(flag));
                    changed = true;
                }
            }
            None => {
                rows.insert(name.clone(), json!({ column: flag }));
                changed = true;
            }
        }
    }
    if changed {
        let _ = store.set(path, table);
    }
    changed
}
