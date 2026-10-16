use super::bridge::{self, BRIDGED};
use super::darkbot;
use super::edit::{format_number, format_value, parse_text};
use super::filter::Filter;
use super::gaps::{self, GAPS};
use super::schema::*;
use super::store::{ConfigStore, DEFAULT_PROFILE, Loaded};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;
use xenomit_bot_runtime::behavior::PlannerRuntime;
use xenomit_bot_runtime::settings::BotSettings;
use xenomit_game_api::BehaviorRuntime;

fn schema() -> Arc<Schema> {
    Arc::new(darkbot::schema())
}

struct TempDir(PathBuf);
impl TempDir {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("xenomit-cfg-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn store(dir: &TempDir) -> ConfigStore {
    ConfigStore::new(schema(), dir.0.clone())
}

// Schema ----------------------------------------------------------------------

/// Every option in DarkBot's `Config.java` (paths lowercase, `@Option` only).
const DARKBOT_OPTIONS: &[&str] = &[
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
    "general.safety.wait_after_revive",
    "general.safety.instant_repair",
    "general.running.run_from_enemies",
    "general.running.enemies_tag",
    "general.running.remember_enemies_for",
    "general.running.run_from_enemies_sight",
    "general.running.stop_running_no_sight",
    "general.running.max_sight_distance",
    "general.running.ship_ability",
    "general.running.ship_ability_min",
    "general.running.run_furthest_port",
    "general.roaming.keep",
    "general.roaming.sequential",
    "general.roaming.only_kill_preferred",
    "general.roaming.enemy_cbs_invisible",
    "collect.stay_away_from_enemies",
    "collect.auto_cloack",
    "collect.auto_cloack_key",
    "collect.radius",
    "collect.ignore_contested_boxes",
    "collect.box_infos",
    "loot.sab.enabled",
    "loot.sab.key",
    "loot.sab.percent",
    "loot.sab.npc_amount",
    "loot.sab.condition",
    "loot.rsb.enabled",
    "loot.rsb.key",
    "loot.ammo_key",
    "loot.ship_ability",
    "loot.ship_ability_min",
    "loot.max_circle_iterations",
    "loot.run_config_in_circle",
    "loot.group_npcs",
    "loot.npc_infos",
    "loot.npc_distance_ignore",
    "pet.enabled",
    "pet.module_id",
    "group.accept_invites",
    "group.whitelist_tag",
    "group.invite_tag",
    "group.open_invites",
    "group.block_invites",
    "group.leave_no_whitelisted",
    "group.kick_no_invited",
    "miscellaneous.refresh_after_revive",
    "miscellaneous.refresh_time",
    "miscellaneous.pause_for",
    "miscellaneous.reset_refresh",
    "miscellaneous.solve_backpage_captcha",
    "miscellaneous.update_stats_while_paused",
    "miscellaneous.drone_repair_percentage",
    "miscellaneous.honor_lost_exact",
    "miscellaneous.log_chat",
    "miscellaneous.log_deaths",
    "miscellaneous.avoid_mines",
    "miscellaneous.avoid_cbs",
    "miscellaneous.avoid_radiation",
    "miscellaneous.username_on_title",
    "miscellaneous.auto_refine",
    "bot_settings.bot_gui.locale",
    "bot_settings.bot_gui.confirm_exit",
    "bot_settings.bot_gui.save_gui_pos",
    "bot_settings.bot_gui.button_size",
    "bot_settings.api_config.browser_api",
    "bot_settings.api_config.fully_hide_api",
    "bot_settings.api_config.force_game_language",
    "bot_settings.api_config.enforce_hw_accel",
    "bot_settings.api_config.use_3d",
    "bot_settings.api_config.use_proxy",
    "bot_settings.api_config.clear_cache_on_stuck",
    "bot_settings.api_config.block_patterns",
    "bot_settings.map_display.toggle",
    "bot_settings.map_display.trail_length",
    "bot_settings.map_display.round_entities",
    "bot_settings.map_display.map_zoom",
    "bot_settings.map_display.refresh_delay",
    "bot_settings.map_display.map_start_stop",
    "bot_settings.map_display.cs.background",
    "bot_settings.map_display.cs.radiation",
    "bot_settings.map_display.cs.hero",
    "bot_settings.map_display.cs.zone_editor.zone",
    "bot_settings.map_display.cs.safety_editor.zone_solid",
    "bot_settings.map_display.cs.fonts.big",
    "bot_settings.custom_background.enabled",
    "bot_settings.custom_background.use_game_background",
    "bot_settings.custom_background.opacity",
    "bot_settings.custom_background.image",
    "bot_settings.performance.always_enabled",
    "bot_settings.performance.disable_render",
    "bot_settings.performance.max_fps",
    "bot_settings.performance.min_tick",
    "bot_settings.other.show_instructions",
    "bot_settings.other.disable_master_password",
    "bot_settings.other.always_show_captcha",
    "bot_settings.other.zone_resolution",
    "bot_settings.other.dev_stuff",
    "extra.condition",
    "extra.action_infos",
];

#[test]
fn every_darkbot_option_exists_as_a_leaf() {
    let schema = schema();
    for path in DARKBOT_OPTIONS {
        let node = schema
            .find(path)
            .unwrap_or_else(|| panic!("missing DarkBot option {path}"));
        assert!(!node.is_group(), "{path} should be an option");
    }
}

#[test]
fn paths_are_unique_and_defaults_are_valid_for_their_kind() {
    let schema = schema();
    let mut seen = BTreeSet::new();
    for leaf in schema.leaves() {
        assert!(
            seen.insert(leaf.path.clone()),
            "duplicate path {}",
            leaf.path
        );
        let kind = leaf.kind.as_ref().unwrap();
        let normalized = kind
            .normalize(&leaf.default)
            .unwrap_or_else(|| panic!("{} default {} is invalid", leaf.path, leaf.default));
        assert_eq!(
            normalized, leaf.default,
            "{} default is not canonical",
            leaf.path
        );
        assert!(!leaf.label.is_empty(), "{} has no label", leaf.path);
    }
    assert!(
        seen.len() > 150,
        "expected the full DarkBot tree, got {}",
        seen.len()
    );
}

#[test]
fn labels_and_descriptions_come_from_darkbot_translations() {
    let schema = schema();
    let deaths = schema.find("general.safety.max_deaths").unwrap();
    assert_eq!(deaths.label, "Max deaths");
    assert_eq!(
        deaths.desc,
        "Max amount of deaths, if reached, bot is paused"
    );
    let revive = schema.find("general.safety.revive").unwrap();
    assert_eq!(revive.label, "Revive location");
    let npc_table = schema.find("loot.npc_infos").unwrap();
    let Some(Kind::Table(spec)) = &npc_table.kind else {
        panic!("npc table")
    };
    assert_eq!(spec.key_label, "Name");
    let radius = spec.columns.iter().find(|c| c.key == "radius").unwrap();
    assert_eq!(radius.label, "Radius");
    let Kind::Choice {
        options,
        multi: true,
    } = &spec.columns.iter().find(|c| c.key == "extra").unwrap().kind
    else {
        panic!("extra flags")
    };
    assert_eq!(options.len(), 12);
    assert!(options.iter().any(|(label, _)| label == "No circle"));
}

#[test]
fn visibility_levels_match_darkbot_annotations() {
    let schema = schema();
    let level = |path: &str| schema.find(path).unwrap().level;
    assert_eq!(
        level("general.safety.wait_before_revive"),
        Level::Intermediate
    );
    assert_eq!(level("general.roaming"), Level::Intermediate);
    assert_eq!(level("bot_settings.api_config"), Level::Advanced);
    assert_eq!(level("loot.sab.condition"), Level::Advanced);
    assert_eq!(level("loot.max_circle_iterations"), Level::Advanced);
    assert_eq!(
        level("bot_settings.api_config.block_patterns"),
        Level::Developer
    );
    assert_eq!(level("bot_settings.bot_gui.button_size"), Level::Advanced);
    assert_eq!(level("general.safety.max_deaths"), Level::Basic);
}

#[test]
fn numeric_bounds_match_darkbot_annotations() {
    let schema = schema();
    let kind = |path: &str| schema.find(path).unwrap().kind.clone().unwrap();
    assert!(matches!(
        kind("general.safety.max_deaths"),
        Kind::Int { min: 1, max: 9999, disabled: Some(Disabled { value, def }), .. } if value == -1.0 && def == 10.0
    ));
    assert!(matches!(
        kind("bot_settings.performance.max_fps"),
        Kind::Int { min: 1, max: 60, disabled: Some(Disabled { value, def }), .. } if value == 0.0 && def == 15.0
    ));
    assert!(matches!(
        kind("general.safety.instant_repair"),
        Kind::Int {
            min: 0,
            max: 99999,
            step: 50,
            ..
        }
    ));
    assert!(matches!(kind("loot.sab.percent"), Kind::Percent { .. }));
}

// Support / gaps ----------------------------------------------------------------

#[test]
fn every_option_declares_support_and_gaps_resolve() {
    let schema = schema();
    let mut referenced = BTreeSet::new();
    for leaf in schema.leaves() {
        let support = leaf.support.expect("support");
        match support.status {
            Status::Live => assert!(support.gap.is_empty(), "{} live with a gap", leaf.path),
            Status::Partial | Status::Gap => {
                assert_ne!(
                    support.gap, "UNSPECIFIED",
                    "{} has no support declared",
                    leaf.path
                );
                assert!(
                    gaps::find(support.gap).is_some(),
                    "{} unknown gap {}",
                    leaf.path,
                    support.gap
                );
                referenced.insert(support.gap);
            }
        }
    }
    for gap in GAPS {
        assert!(
            referenced.contains(gap.id),
            "gap {} is never referenced",
            gap.id
        );
        assert!(!gap.needs.is_empty() && !gap.layers.is_empty());
    }
}

#[test]
fn explicitly_bridged_paths_are_declared_as_supported() {
    let schema = schema();
    let declared: BTreeSet<&str> = schema
        .leaves()
        .into_iter()
        .filter(|leaf| leaf.support.is_some_and(|s| s.status != Status::Gap))
        .map(|leaf| leaf.path.as_str())
        .collect();
    let bridged: BTreeSet<&str> = BRIDGED.iter().copied().collect();
    assert!(bridged.is_subset(&declared));
    for path in BRIDGED {
        assert!(bridge::handles(path));
    }
}

#[test]
fn gap_document_lists_every_gap() {
    let doc = include_str!("../../../docs/SETTINGS_GAPS.md");
    for gap in GAPS {
        assert!(
            doc.contains(&format!("`{}`", gap.id)),
            "docs/SETTINGS_GAPS.md is missing {}",
            gap.id
        );
    }
}

// Store -----------------------------------------------------------------------

#[test]
fn set_clamps_like_darkbot_and_honours_the_disabled_sentinel() {
    let dir = TempDir::new("clamp");
    let mut store = store(&dir);
    assert_eq!(
        store
            .set("general.safety.max_deaths", json!(100_000))
            .unwrap(),
        json!(9999)
    );
    assert_eq!(
        store.set("general.safety.max_deaths", json!(0)).unwrap(),
        json!(1)
    );
    assert_eq!(
        store.set("general.safety.max_deaths", json!(-1)).unwrap(),
        json!(-1)
    );
    assert_eq!(store.get("general.safety.max_deaths"), json!(-1));
    assert_eq!(
        store.set("loot.sab.percent", json!(3.0)).unwrap(),
        json!(1.0)
    );
    assert!(
        store
            .set("general.safety.max_deaths", json!("ten"))
            .is_err()
    );
    assert!(store.set("no.such.option", json!(1)).is_err());
    assert!(store.set("general.safety", json!(1)).is_err());
}

#[test]
fn range_ship_color_and_choice_are_normalized() {
    let dir = TempDir::new("kinds");
    let mut store = store(&dir);
    assert_eq!(
        store
            .set(
                "general.safety.repair_hp_range",
                json!({"min": 0.9, "max": 0.2})
            )
            .unwrap(),
        json!({"min": 0.2, "max": 0.9})
    );
    assert_eq!(
        store
            .set("general.offensive", json!({"config": 7, "formation": "f3"}))
            .unwrap(),
        json!({"config": 2, "formation": "F3"})
    );
    assert_eq!(
        store
            .set(
                "general.offensive",
                json!({"config": 1, "formation": "abc"})
            )
            .unwrap(),
        json!({"config": 1, "formation": null}),
        "not a key -> cleared"
    );
    assert_eq!(
        store
            .set("bot_settings.map_display.cs.hero", json!("#22CC22"))
            .unwrap(),
        json!("#22cc22")
    );
    assert!(
        store
            .set("bot_settings.map_display.cs.hero", json!("green"))
            .is_err()
    );
    assert!(store.set("general.safety.revive", json!("orbit")).is_err());
    assert_eq!(
        store
            .set(
                "bot_settings.map_display.toggle",
                json!(["ZONES", "BOGUS", "USERNAMES"])
            )
            .unwrap(),
        json!(["USERNAMES", "ZONES"])
    );
}

#[test]
fn tables_normalize_rows_and_fill_missing_columns() {
    let dir = TempDir::new("table");
    let mut store = store(&dir);
    let stored = store
        .set(
            "loot.npc_infos",
            json!({"Streuner": {"kill": false, "radius": 99999, "extra": ["NO_CIRCLE", "BOGUS"]}}),
        )
        .unwrap();
    assert_eq!(stored["Streuner"]["kill"], json!(false));
    assert_eq!(stored["Streuner"]["radius"], json!(5000.0));
    assert_eq!(stored["Streuner"]["extra"], json!(["NO_CIRCLE"]));
    assert_eq!(stored["Streuner"]["priority"], json!(0));
    assert!(store.set("loot.npc_infos", json!([])).is_err());
}

#[test]
fn reset_restores_defaults_for_options_and_groups() {
    let dir = TempDir::new("reset");
    let mut store = store(&dir);
    store.set("general.safety.max_deaths", json!(3)).unwrap();
    store
        .set("general.safety.wait_before_revive", json!(25))
        .unwrap();
    assert!(!store.is_default("general.safety.max_deaths"));
    store.reset("general.safety.max_deaths");
    assert!(store.is_default("general.safety.max_deaths"));
    assert!(!store.is_default("general.safety.wait_before_revive"));
    store.reset("general.safety");
    assert!(store.is_default("general.safety.wait_before_revive"));
}

#[test]
fn values_persist_with_a_backup_and_unknown_keys_survive() {
    let dir = TempDir::new("persist");
    std::fs::write(
        dir.0.join("config.json"),
        r#"{"general":{"safety":{"max_deaths":4}},"future":{"option":true}}"#,
    )
    .unwrap();
    let mut store = store(&dir);
    assert_eq!(store.load_profile(DEFAULT_PROFILE), Loaded::File);
    assert_eq!(store.get("general.safety.max_deaths"), json!(4));
    store.set("general.safety.max_deaths", json!(6)).unwrap();
    store.set_ui("level", json!("advanced"));
    store.save();

    let backup: Value =
        serde_json::from_slice(&std::fs::read(dir.0.join("config_old.json")).unwrap()).unwrap();
    assert_eq!(backup["general"]["safety"]["max_deaths"], json!(4));

    let mut reloaded = ConfigStore::new(schema(), dir.0.clone());
    assert_eq!(reloaded.load_profile(DEFAULT_PROFILE), Loaded::File);
    assert_eq!(reloaded.get("general.safety.max_deaths"), json!(6));
    assert_eq!(reloaded.ui("level"), Some(&json!("advanced")));
    let raw: Value =
        serde_json::from_slice(&std::fs::read(dir.0.join("config.json")).unwrap()).unwrap();
    assert_eq!(
        raw["future"]["option"],
        json!(true),
        "unknown keys are preserved"
    );
}

#[test]
fn a_corrupt_file_disables_saving_and_falls_back_to_the_backup() {
    let dir = TempDir::new("corrupt");
    std::fs::write(dir.0.join("config.json"), "{ not json").unwrap();
    let mut store = store(&dir);
    assert_eq!(store.load_profile(DEFAULT_PROFILE), Loaded::Failed);
    assert!(store.failed());
    store.set("general.safety.max_deaths", json!(2)).unwrap();
    store.save();
    assert_eq!(
        std::fs::read_to_string(dir.0.join("config.json")).unwrap(),
        "{ not json"
    );

    std::fs::write(
        dir.0.join("config_old.json"),
        r#"{"general":{"safety":{"max_deaths":8}}}"#,
    )
    .unwrap();
    assert_eq!(store.load_profile(DEFAULT_PROFILE), Loaded::File);
    assert_eq!(store.get("general.safety.max_deaths"), json!(8));
}

#[test]
fn profiles_can_be_created_copied_switched_and_deleted() {
    let dir = TempDir::new("profiles");
    let mut store = store(&dir);
    assert_eq!(store.load_profile(DEFAULT_PROFILE), Loaded::Missing);
    assert!(store.needs_seed());
    store.set("general.safety.max_deaths", json!(5)).unwrap();
    store.save();

    store.create_profile("farm", Some(DEFAULT_PROFILE)).unwrap();
    assert!(store.create_profile("farm", None).is_err());
    assert!(store.create_profile("../evil", None).is_err());
    assert!(store.create_profile("config", None).is_err());
    store.create_profile("blank", None).unwrap();
    assert_eq!(store.profiles()[0], DEFAULT_PROFILE);
    assert!(store.profiles().contains(&"farm".to_owned()));

    assert_eq!(store.load_profile("farm"), Loaded::File);
    assert_eq!(store.get("general.safety.max_deaths"), json!(5));
    assert_eq!(store.last_profile(), "farm");
    store.set("general.safety.max_deaths", json!(7)).unwrap();
    store.load_profile("blank");
    assert_eq!(
        store.get("general.safety.max_deaths"),
        json!(10),
        "blank profile uses defaults"
    );
    store.load_profile("farm");
    assert_eq!(
        store.get("general.safety.max_deaths"),
        json!(7),
        "switching saved the edit"
    );

    assert!(store.delete_profile("farm").is_err(), "active profile");
    assert!(store.delete_profile(DEFAULT_PROFILE).is_err());
    store.load_profile(DEFAULT_PROFILE);
    store.delete_profile("farm").unwrap();
    assert!(!store.profiles().contains(&"farm".to_owned()));
}

// Filter ----------------------------------------------------------------------

#[test]
fn filter_applies_level_and_search_like_the_darkbot_tree() {
    let schema = schema();
    let revive_wait = schema.find("general.safety.wait_before_revive").unwrap();
    let max_deaths = schema.find("general.safety.max_deaths").unwrap();
    let basic = Filter::new(Level::Basic, "");
    assert!(basic.visible(max_deaths));
    assert!(!basic.visible(revive_wait));
    assert!(Filter::new(Level::Intermediate, "").visible(revive_wait));

    let search = Filter::new(Level::Developer, "max deaths");
    assert!(search.visible(max_deaths));
    assert!(!search.visible(revive_wait));
    let general = schema.find("general").unwrap();
    assert!(
        search.visible(general),
        "a group shows when a descendant matches"
    );
    assert!(!search.visible(schema.find("pet").unwrap()));

    // A matching group name shows its entire subtree.
    let by_group = Filter::new(Level::Developer, "running");
    assert!(
        by_group.visible(schema.find("general.running.run_furthest_port").unwrap()) == false
            || by_group.visible(schema.find("general.running").unwrap())
    );
    let shown = by_group.children(schema.find("general.running").unwrap(), false);
    assert_eq!(shown.len(), 9);

    // Descriptions and paths are searchable too.
    assert!(Filter::new(Level::Developer, "bot is paused").visible(max_deaths));
    assert!(
        Filter::new(Level::Developer, "safety.repair")
            .visible(schema.find("general.safety.repair").unwrap())
    );
}

// Edit ------------------------------------------------------------------------

#[test]
fn text_parsing_round_trips_every_text_backed_kind() {
    let int = int(1, 10, 1);
    assert_eq!(parse_text(&int, " 7 ").unwrap(), json!(7));
    assert_eq!(parse_text(&int, "99").unwrap(), json!(10));
    assert!(parse_text(&int, "1.5").is_err());
    assert!(parse_text(&int, "").is_err());

    let percent = pct();
    assert_eq!(parse_text(&percent, "40").unwrap(), json!(0.4));
    assert_eq!(format_value(&percent, &json!(0.4)), "40");
    assert_eq!(format_value(&percent, &json!(0.125)), "12.5");
    assert!(parse_text(&percent, "NaN").is_err());
    assert!(parse_text(&percent, "inf").is_err());

    assert_eq!(parse_text(&Kind::Char, "x").unwrap(), json!("x"));
    assert_eq!(parse_text(&Kind::Char, "f12").unwrap(), json!("F12"));
    assert_eq!(parse_text(&Kind::Char, "").unwrap(), Value::Null);
    assert!(parse_text(&Kind::Char, "xy").is_err());

    assert_eq!(parse_text(&Kind::Json, "").unwrap(), Value::Null);
    assert_eq!(
        parse_text(&Kind::Json, r#"{"a":1}"#).unwrap(),
        json!({"a": 1})
    );
    assert!(parse_text(&Kind::Json, "{").is_err());
    assert_eq!(
        parse_text(&Kind::Condition, "all(hp-below(0.4), map(1-8))").unwrap(),
        json!("all(hp-below(0.4), map(1-8))")
    );
    assert!(parse_text(&Kind::Condition, "unsupported(1)").is_err());
    assert!(xenomit_behavior_engine::parse_darkbot_action("use-item(ammo)").is_ok());
    assert!(xenomit_behavior_engine::parse_darkbot_action("run-shell()").is_err());

    assert_eq!(format_number(5.0), "5");
    assert_eq!(format_number(0.25), "0.25");
}

// Bridge ----------------------------------------------------------------------

/// Modules whose own descriptor already rejects the plain defaults (for
/// example `item_restock` has a 250 ms minimum tick, but `module_config_for`
/// injects 150). That is independent of the settings tree, so skip them.
fn modules_valid_at_baseline() -> Vec<xenomit_game_api::ModuleDescriptor> {
    let baseline = BotSettings::default();
    PlannerRuntime::default()
        .modules()
        .unwrap()
        .into_iter()
        .filter(|descriptor| !descriptor.id.starts_with("__"))
        .filter(|descriptor| {
            descriptor
                .prepare_settings(baseline.module_config_for(&descriptor.id).settings)
                .is_ok()
        })
        .collect()
}

#[test]
fn defaults_apply_to_settings_the_engine_accepts() {
    let dir = TempDir::new("bridge-defaults");
    let store = store(&dir);
    let descriptors = modules_valid_at_baseline();
    assert!(
        descriptors.len() >= 4,
        "built-in objective modules must validate"
    );
    for descriptor in &descriptors {
        let mut settings = BotSettings::default();
        settings.active_module = descriptor.id.clone();
        bridge::apply(&store, &mut settings);
        descriptor
            .prepare_settings(settings.module_config_for(&descriptor.id).settings)
            .unwrap_or_else(|error| panic!("{}: {error}", descriptor.id));
    }
    let mut settings = BotSettings::default();
    bridge::apply(&store, &mut settings);
    assert_eq!(settings.min_health, 40);
    assert_eq!(settings.max_health, 95);
    assert_eq!(settings.max_deaths, 10);
    assert_eq!(settings.behaviors["refresh"]["interval_minutes"], json!(60));
    assert_eq!(settings.behaviors["refresh"]["enabled"], json!(true));
}

#[test]
fn extreme_values_still_satisfy_every_engine_schema() {
    let descriptors = modules_valid_at_baseline();
    let schema = schema();
    for pick_max in [false, true] {
        let dir = TempDir::new(if pick_max { "bridge-max" } else { "bridge-min" });
        let mut store = store(&dir);
        for path in BRIDGED {
            let node = schema.find(path).unwrap();
            let value = match node.kind.as_ref().unwrap() {
                Kind::Int { min, max, .. } => json!(if pick_max { *max } else { *min }),
                Kind::Float { min, max, .. } | Kind::Percent { min, max, .. } => {
                    json!(if pick_max { *max } else { *min })
                }
                Kind::Range => json!({"min": if pick_max { 1.0 } else { 0.0 }, "max": 1.0}),
                _ => continue,
            };
            store.set(path, value).unwrap();
        }
        for descriptor in &descriptors {
            let mut settings = BotSettings::default();
            settings.active_module = descriptor.id.clone();
            bridge::apply(&store, &mut settings);
            descriptor
                .prepare_settings(settings.module_config_for(&descriptor.id).settings)
                .unwrap_or_else(|error| panic!("{} (max={pick_max}): {error}", descriptor.id));
        }
    }
}

#[test]
fn apply_only_touches_the_active_module() {
    let dir = TempDir::new("bridge-active");
    let mut store = store(&dir);
    store.set("collect.radius", json!(777)).unwrap();
    let mut settings = BotSettings::default();
    settings
        .modules
        .insert("npc_killer".into(), json!({"working_map": 24}));
    settings.active_module = "kill_collect".into();
    bridge::apply(&store, &mut settings);
    assert_eq!(
        settings.modules["kill_collect"]["collect_radius"],
        json!(777)
    );
    assert_eq!(
        settings.modules["npc_killer"],
        json!({"working_map": 24}),
        "other modules untouched"
    );
}

#[test]
fn pending_module_reports_a_tree_initiated_switch_without_applying_it() {
    let dir = TempDir::new("bridge-switch");
    let mut store = store(&dir);
    let mut settings = BotSettings::default();
    settings.active_module = "kill_collect".into();
    store
        .set("general.current_module", json!("npc_killer"))
        .unwrap();
    assert_eq!(
        bridge::pending_module(&store, &settings).as_deref(),
        Some("npc_killer")
    );
    bridge::apply(&store, &mut settings);
    assert_eq!(settings.active_module, "kill_collect");
    store
        .set("general.current_module", json!("kill_collect"))
        .unwrap();
    assert_eq!(bridge::pending_module(&store, &settings), None);
}

#[test]
fn apply_translates_units_and_sentinels() {
    let dir = TempDir::new("bridge-units");
    let mut store = store(&dir);
    store
        .set(
            "general.safety.repair_hp_range",
            json!({"min": 0.25, "max": 0.8}),
        )
        .unwrap();
    store.set("general.safety.max_deaths", json!(-1)).unwrap();
    store
        .set("general.offensive", json!({"config": 2, "formation": "3"}))
        .unwrap();
    store.set("loot.sab.percent", json!(0.65)).unwrap();
    store.set("miscellaneous.refresh_time", json!(0)).unwrap();
    store.set("pet.module_id", json!(5)).unwrap();
    store
        .set("bot_settings.api_config.browser_api", json!("packet"))
        .unwrap();
    store.set("xenomit.engine.tick_ms", json!(0)).unwrap();
    store
        .set(
            "loot.npc_infos",
            json!({"Streuner": {"kill": false}, "Lordakia": {"kill": true}}),
        )
        .unwrap();

    let mut settings = BotSettings::default();
    bridge::apply(&store, &mut settings);
    let payload = settings.module_config_for("kill_collect").settings;

    assert_eq!(payload["safety"]["min_health_percent"], json!(25.0));
    assert_eq!(payload["safety"]["max_health_percent"], json!(80.0));
    assert_eq!((settings.min_health, settings.max_health), (25, 80));
    assert_eq!(
        payload["safety"]["max_deaths"],
        json!(0),
        "unlimited deaths -> 0"
    );
    assert_eq!(payload["offensive_config"], json!(2));
    assert_eq!(settings.offensive_config, 2);
    assert_eq!(payload["sab_shield_percent"], json!(65.0));
    assert_eq!(settings.behaviors["refresh"]["enabled"], json!(false));
    assert_eq!(payload["pet_gear"], json!(5));
    assert_eq!(
        settings.client,
        xenomit_bot_runtime::settings::ClientKind::Packet
    );
    assert_eq!(
        payload["tick_ms"],
        json!(150),
        "0 keeps the backend default"
    );
    assert_eq!(settings.npcs.get("Streuner"), Some(&false));
    assert_eq!(settings.npcs.get("Lordakia"), Some(&true));
}

#[test]
fn import_seeds_the_tree_from_existing_settings_and_round_trips() {
    let mut settings = BotSettings::default();
    settings.min_health = 33;
    settings.max_health = 88;
    settings.max_deaths = 4;
    settings.offensive_config = 2;
    settings.revive_wait_seconds = 12;
    settings.feature_flags.insert("pet_active".into(), false);
    settings.npcs.insert("Streuner".into(), false);
    settings.boxes.insert("BONUS_BOX".into(), true);
    settings.behaviors = json!({"refresh": {"enabled": true, "interval_minutes": 90}});

    let dir = TempDir::new("bridge-import");
    let mut store = store(&dir);
    bridge::import(&settings, &mut store, None);
    assert_eq!(
        store.get("general.safety.repair_hp_range"),
        json!({"min": 0.33, "max": 0.88})
    );
    assert_eq!(store.get("general.safety.max_deaths"), json!(4));
    assert_eq!(store.get("general.offensive")["config"], json!(2));
    assert_eq!(store.get("general.safety.wait_before_revive"), json!(12));
    assert_eq!(store.get("pet.enabled"), json!(false));
    assert_eq!(store.get("miscellaneous.refresh_time"), json!(90));
    assert_eq!(
        store.get("loot.npc_infos")["Streuner"]["kill"],
        json!(false)
    );
    assert_eq!(
        store.get("collect.box_infos")["BONUS_BOX"]["collect"],
        json!(true)
    );

    let mut round_trip = BotSettings::default();
    bridge::apply(&store, &mut round_trip);
    assert_eq!(round_trip.min_health, 33);
    assert_eq!(round_trip.max_health, 88);
    assert_eq!(round_trip.max_deaths, 4);
    assert_eq!(round_trip.offensive_config, 2);
    assert_eq!(round_trip.npcs, settings.npcs);
    assert_eq!(round_trip.boxes, settings.boxes);
    assert_eq!(
        round_trip.behaviors["refresh"]["interval_minutes"],
        json!(90)
    );
}

// Documentation generator -------------------------------------------------------

fn kind_name(kind: &Kind) -> String {
    match kind {
        Kind::Bool => "boolean".into(),
        Kind::Int {
            min, max, disabled, ..
        } => format!(
            "integer {min}..{max}{}",
            disabled
                .as_ref()
                .map(|d| format!(" (off = {})", d.value))
                .unwrap_or_default()
        ),
        Kind::Float { min, max, .. } => format!("number {min}..{max}"),
        Kind::Percent { .. } => "percentage".into(),
        Kind::Text => "text".into(),
        Kind::Char => "key".into(),
        Kind::Choice { multi: false, .. } => "dropdown".into(),
        Kind::Choice { multi: true, .. } => "multi-select".into(),
        Kind::Dynamic(Source::Modules) => "module".into(),
        Kind::Dynamic(Source::Maps) => "map".into(),
        Kind::Range => "percent range".into(),
        Kind::Ship => "ship config".into(),
        Kind::Tag(_) => "player tag".into(),
        Kind::Color { .. } => "colour".into(),
        Kind::Condition => "condition".into(),
        Kind::Table(_) => "table".into(),
        Kind::Json => "JSON".into(),
    }
}

/// `cargo test regenerate_docs -- --ignored` rewrites the generated docs.
#[test]
#[ignore]
fn regenerate_docs() {
    let schema = schema();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs");

    let mut gaps_doc = String::from(
        "# Settings gaps: what the runtime still needs\n\n\
         The settings tree (`src/bot/config_tree/`) ports DarkBot's full option list. Options marked\n\
         **LIVE** have a working consumer in the shell, runtime, engine, or client.\n\
         **PARTIAL** options are consumed when their required observations or operations are available.\n\
         **PLANNED** options are shown and persisted per profile, but have no working consumer yet.\n\n\
         Each gap names the layer that must change. The id (`SHIP-FORMATION`, ...) is what the schema\n\
         references; the GUI shows the same text when you click an option's status chip.\n\
         This file is generated: `cargo test regenerate_docs -- --ignored`. A test fails if an id is\n\
         missing here.\n\n",
    );
    for layer in [
        gaps::Layer::ClientApi,
        gaps::Layer::Engine,
        gaps::Layer::Runtime,
        gaps::Layer::Shell,
    ] {
        let in_layer: Vec<_> = GAPS
            .iter()
            .filter(|gap| gap.layers.contains(&layer))
            .collect();
        if in_layer.is_empty() {
            continue;
        }
        gaps_doc.push_str(&format!("## {}\n\n", layer.label()));
        for gap in in_layer {
            let other: Vec<&str> = gap
                .layers
                .iter()
                .filter(|l| **l != layer)
                .map(|l| l.label())
                .collect();
            gaps_doc.push_str(&format!("### `{}` — {}\n\n", gap.id, gap.title));
            if !other.is_empty() {
                gaps_doc.push_str(&format!("Also touches: {}.\n\n", other.join(", ")));
            }
            gaps_doc.push_str(&format!("{}\n\nAffects:\n", gap.needs));
            for leaf in schema.leaves() {
                if leaf.support.is_some_and(|s| s.gap == gap.id) {
                    let status = match leaf.support.unwrap().status {
                        Status::Partial => "partial",
                        _ => "planned",
                    };
                    gaps_doc.push_str(&format!("- `{}` — {} ({status})\n", leaf.path, leaf.label));
                }
            }
            gaps_doc.push('\n');
        }
    }
    std::fs::write(root.join("SETTINGS_GAPS.md"), gaps_doc).unwrap();

    let mut options = String::from(
        "# Settings option reference\n\nGenerated by `cargo test regenerate_docs -- --ignored`.\n\n\
         | Path | Label | Kind | Level | Status | Runtime target / gap |\n|---|---|---|---|---|---|\n",
    );
    for leaf in schema.leaves() {
        let support = leaf.support.unwrap();
        let (status, detail) = match support.status {
            Status::Live => ("live", support.target.to_owned()),
            Status::Partial => ("partial", format!("{} / `{}`", support.target, support.gap)),
            Status::Gap => ("planned", format!("`{}`", support.gap)),
        };
        options.push_str(&format!(
            "| `{}` | {} | {} | {} | {status} | {detail} |\n",
            leaf.path,
            leaf.label.replace('|', "/"),
            kind_name(leaf.kind.as_ref().unwrap()),
            leaf.level.label(),
        ));
    }
    std::fs::write(root.join("SETTINGS_OPTIONS.md"), options).unwrap();
}

#[test]
fn import_prefers_the_module_payload_and_fills_engine_defaults() {
    let descriptors = PlannerRuntime::default().modules().unwrap();
    let kill_collect = descriptors.iter().find(|d| d.id == "kill_collect").unwrap();

    let mut settings = BotSettings::default();
    settings.active_module = "kill_collect".into();
    // Global flag says on; the module's own payload says off, and wins.
    settings.feature_flags.insert("pet_active".into(), true);
    settings.modules.insert(
        "kill_collect".into(),
        json!({"features": {"pet_active": false}, "safety": {"min_health_percent": 22.0}, "working_map": 8}),
    );

    let dir = TempDir::new("bridge-precedence");
    let mut store = store(&dir);
    bridge::import(&settings, &mut store, Some(kill_collect));
    assert_eq!(store.get("pet.enabled"), json!(false));
    assert_eq!(
        store.get("general.safety.repair_hp_range")["min"],
        json!(0.22)
    );
    assert_eq!(store.get("general.working_map"), json!(8));
    // Untouched keys take the engine default (500), not DarkBot's (400).
    assert_eq!(store.get("collect.radius"), json!(500));
    assert_eq!(store.get("loot.sab.percent"), json!(0.5));
}

#[test]
fn explicit_tick_override_is_imported_and_zero_leaves_the_payload_alone() {
    let mut settings = BotSettings::default();
    settings
        .modules
        .insert("kill_collect".into(), json!({"tick_ms": 333}));
    let dir = TempDir::new("bridge-tick");
    let mut store = store(&dir);
    bridge::import(&settings, &mut store, None);
    assert_eq!(store.get("xenomit.engine.tick_ms"), json!(333));
    store.set("xenomit.engine.tick_ms", json!(0)).unwrap();
    bridge::apply(&store, &mut settings);
    assert_eq!(
        settings.modules["kill_collect"]["tick_ms"],
        json!(333),
        "0 means untouched"
    );
}
