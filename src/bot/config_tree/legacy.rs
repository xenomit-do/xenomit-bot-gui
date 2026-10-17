//! The settings tree itself: a port of Legacy's `Config.java` (every group,
//! option, default, bound and visibility level), followed by a `xenomit`
//! group holding the runtime options Legacy has no equivalent for.
//!
//! Each option declares its runtime support (`live`/`partial`/`gap`). The
//! `bridge` module implements the live ones; the gap ids are documented in
//! `gaps.rs` and `docs/SETTINGS_GAPS.md`.
use super::schema::*;
use Level::{Advanced, Developer, Intermediate};
use serde_json::{Value, json};

pub(crate) fn schema() -> Schema {
    Schema::new(vec![
        general(),
        collect(),
        loot(),
        pet(),
        group(),
        miscellaneous(),
        bot_settings(),
        extra(),
        xenomit(),
    ])
}

fn general() -> Node {
    grp(
        "general",
        vec![
            opt(
                "current_module",
                Kind::Dynamic(Source::Modules),
                json!("kill_collect"),
            )
            .sup(live("settings.active_module")),
            opt("working_map", Kind::Dynamic(Source::Maps), Value::Null)
                .sup(live("module working_map")),
            opt("offensive", Kind::Ship, ship(1, "8"))
                .sup(partial("offensive_config", "SHIP-FORMATION")),
            opt("roam", Kind::Ship, ship(1, "9")).sup(partial("roam_config", "SHIP-FORMATION")),
            opt("run", Kind::Ship, ship(2, "9")).sup(partial("run_config", "SHIP-FORMATION")),
            grp(
                "safety",
                vec![
                    opt("repair_hp_range", Kind::Range, range(0.4, 0.95))
                        .sup(live("safety.min/max_health_percent")),
                    opt("repair_hp_no_npc", pct(), json!(0.5))
                        .sup(live("safety.roaming_repair_health_percent")),
                    opt("repair_to_shield", pct(), json!(1.0))
                        .sup(live("safety.repair_shield_percent")),
                    opt("repair", Kind::Ship, ship(1, "9"))
                        .sup(partial("repair_config", "SHIP-FORMATION")),
                    opt("max_deaths", int_off(1, 9999, 1, -1, 10), json!(10))
                        .sup(live("safety.max_deaths")),
                    opt(
                        "revive",
                        choice(&[("Base", "base"), ("Portal", "portal"), ("Spot", "spot")]),
                        json!("base"),
                    )
                    .i18n("config.general.safety.revive_location")
                    .sup(partial("settings.revive_location", "REVIVE-LOCATION")),
                    opt("wait_before_revive", int(5, 60, 10), json!(5))
                        .lvl(Intermediate)
                        .sup(live("safety.revive_wait_seconds")),
                    opt("wait_after_revive", int(3, 900, 10), json!(90))
                        .lvl(Intermediate)
                        .sup(live("safety.revive_wait_after_seconds")),
                    opt("instant_repair", int_off(0, 99999, 50, 0, 500), json!(0))
                        .lvl(Intermediate)
                        .sup(partial("settings.instant_repair_above", "INSTANT-REPAIR")),
                ],
            ),
            grp(
                "running",
                vec![
                    opt("run_from_enemies", Kind::Bool, json!(true))
                        .sup(live("safety.run_from_enemy")),
                    opt("enemies_tag", Kind::Tag(TagDefault::None), Value::Null)
                        .sup(live("safety.enemies_tag/player_tags")),
                    opt("remember_enemies_for", int(0, 86400, 300), json!(300))
                        .lvl(Intermediate)
                        .sup(live("safety.remember_enemies_for_seconds")),
                    opt("run_from_enemies_sight", Kind::Bool, json!(false))
                        .sup(live("safety.run_in_sight")),
                    opt("stop_running_no_sight", Kind::Bool, json!(true))
                        .sup(live("safety.stop_running_no_sight")),
                    opt("max_sight_distance", int(500, 20000, 500), json!(4000))
                        .lvl(Intermediate)
                        .sup(live("safety.sight_distance")),
                    opt("ship_ability", Kind::Char, Value::Null)
                        .sup(partial("observed ability readiness", "SHIP-ABILITY")),
                    opt("ship_ability_min", int(0, 20000, 500), json!(1500))
                        .lvl(Intermediate)
                        .sup(partial("safety ship ability distance", "SHIP-ABILITY")),
                    opt("run_furthest_port", int(0, 20000, 500), json!(1500))
                        .lvl(Intermediate)
                        .sup(live("safety.run_port_distance")),
                ],
            ),
            grp(
                "roaming",
                vec![
                    opt("keep", Kind::Bool, json!(true)).sup(live("movement.roaming.keep")),
                    opt("sequential", Kind::Bool, json!(false))
                        .sup(live("movement.roaming.sequential")),
                    opt("only_kill_preferred", Kind::Bool, json!(false))
                        .sup(live("only_kill_preferred")),
                    opt("enemy_cbs_invisible", Kind::Bool, json!(false)).sup(gap("ROAMING")),
                ],
            )
            .lvl(Intermediate),
        ],
    )
}

fn collect() -> Node {
    let box_table = table(
        vec![
            Column {
                desc: String::new(),
                ..column("collect", Kind::Bool, json!(false))
            },
            column("wait", int(0, 600_000, 100), json!(0)),
            column("priority", int(0, 99, 1), json!(0)),
        ],
        true,
        Some("boxes"),
    );
    grp(
        "collect",
        vec![
            opt("stay_away_from_enemies", Kind::Bool, json!(false))
                .sup(live("stay_away_from_enemies")),
            opt("auto_cloack", Kind::Bool, json!(false)).sup(live("auto_cloak")),
            opt("auto_cloack_key", Kind::Char, Value::Null)
                .sup(partial("observed slot bar", "HOTKEYS")),
            opt("radius", int(0, 10000, 50), json!(400)).sup(live("collect_radius")),
            opt("ignore_contested_boxes", Kind::Bool, json!(true))
                .sup(live("ignore_contested_boxes")),
            opt("box_infos", box_table, json!({}))
                .sup(live("per-box collection, waits, and priorities")),
        ],
    )
}

fn npc_flag_options() -> Kind {
    const FLAGS: [&str; 12] = [
        "no_circle",
        "ignore_ownership",
        "ignore_attacked",
        "ignore_boxes",
        "aggressive_follow",
        "passive",
        "attack_second",
        "leech_only",
        "no_sab",
        "use_rsb",
        "pet_locator",
        "opposite_config",
    ];
    Kind::Choice {
        options: FLAGS
            .iter()
            .map(|flag| {
                let label = translate(&format!("config.loot.npc_table.extra.{flag}"))
                    .map(str::to_owned)
                    .unwrap_or_else(|| humanize(flag));
                (label, json!(flag.to_ascii_uppercase()))
            })
            .collect(),
        multi: true,
    }
}

fn loot() -> Node {
    let npc_table = table(
        vec![
            column(
                "radius",
                Kind::Float {
                    min: 0.0,
                    max: 5000.0,
                    step: 10.0,
                },
                json!(560.0),
            ),
            column("priority", int(0, 99, 1), json!(0)),
            column("kill", Kind::Bool, json!(true)),
            column("attack_key", Kind::Char, Value::Null),
            column("attack_formation", Kind::Char, Value::Null),
            column("extra", npc_flag_options(), json!([])),
        ],
        true,
        Some("npcs"),
    );
    grp(
        "loot",
        vec![
            grp(
                "sab",
                vec![
                    opt("enabled", Kind::Bool, json!(false)).sup(live("sab_enabled")),
                    opt("key", Kind::Char, json!("2")).sup(partial("observed slot bar", "HOTKEYS")),
                    opt("percent", pct(), json!(0.8)).sup(live("sab_shield_percent")),
                    opt("npc_amount", int(500, 1_000_000, 1000), json!(12000))
                        .sup(live("sab_target_shield_min")),
                    opt("condition", Kind::Condition, json!(""))
                        .lvl(Advanced)
                        .sup(partial("Legacy condition subset", "CONDITIONS")),
                ],
            ),
            grp(
                "rsb",
                vec![
                    opt("enabled", Kind::Bool, json!(false)).sup(live("npc_killer.rsb_enabled")),
                    opt("key", Kind::Char, json!("3")).sup(partial("observed slot bar", "HOTKEYS")),
                ],
            ),
            opt("ammo_key", Kind::Char, json!("1")).sup(partial("observed slot bar", "HOTKEYS")),
            opt("ship_ability", Kind::Char, Value::Null)
                .lvl(Intermediate)
                .sup(partial("observed ability readiness", "SHIP-ABILITY")),
            opt(
                "ship_ability_min",
                int(50_000, 5_000_000, 50_000),
                json!(150_000),
            )
            .lvl(Intermediate)
            .sup(partial(
                "NPC health and observed ability readiness",
                "SHIP-ABILITY",
            )),
            opt("max_circle_iterations", int(0, 10, 1), json!(5))
                .lvl(Advanced)
                .sup(live("npc_killer.max_circle_iterations")),
            opt("run_config_in_circle", Kind::Bool, json!(true))
                .sup(live("npc_killer.run_config_in_circle")),
            opt("group_npcs", Kind::Bool, json!(true)).sup(live("npc_killer.group_npcs")),
            opt("npc_infos", npc_table, json!({})).sup(partial("settings.npcs", "NPC-TABLE")),
            opt("npc_distance_ignore", int(1000, 20000, 500), json!(3000))
                .lvl(Intermediate)
                .sup(live("max_npc_distance")),
        ],
    )
}

fn pet() -> Node {
    grp(
        "pet",
        vec![
            opt("enabled", Kind::Bool, json!(false)).sup(live("features.pet_active")),
            opt(
                "module_id",
                Kind::Choice {
                    options: [
                        ("Keep current", Value::Null),
                        ("Passive", json!(1)),
                        ("Guard", json!(2)),
                        ("Auto loot", json!(4)),
                        ("Resource collection", json!(5)),
                        ("Enemy locator", json!(6)),
                        ("Resource locator", json!(7)),
                        ("Ship repair", json!(11)),
                    ]
                    .into_iter()
                    .map(|(label, id)| (label.to_owned(), id))
                    .collect(),
                    multi: false,
                },
                Value::Null,
            )
            .sup(partial("pet_gear", "PET-GEARS")),
        ],
    )
}

fn group() -> Node {
    grp(
        "group",
        vec![
            opt("accept_invites", Kind::Bool, json!(false))
                .sup(partial("features.group_invites", "GROUP")),
            opt("whitelist_tag", Kind::Tag(TagDefault::All), Value::Null)
                .sup(live("group.whitelist_tag/player_tags")),
            opt("invite_tag", Kind::Tag(TagDefault::None), Value::Null)
                .sup(live("group.invite_tag/player_tags")),
            opt("open_invites", Kind::Bool, json!(false))
                .sup(partial("observed group state and client command", "GROUP")),
            opt("block_invites", Kind::Bool, json!(false))
                .sup(partial("observed group state and client command", "GROUP")),
            opt("leave_no_whitelisted", Kind::Bool, json!(false))
                .sup(partial("observed group state and client command", "GROUP")),
            opt("kick_no_invited", Kind::Bool, json!(false))
                .sup(partial("observed group state and client command", "GROUP")),
        ],
    )
}

fn miscellaneous() -> Node {
    grp(
        "miscellaneous",
        vec![
            opt("refresh_after_revive", Kind::Bool, json!(false))
                .sup(live("runtime.refresh_after_revive")),
            opt("refresh_time", int(0, 720, 10), json!(60))
                .sup(live("behaviors.refresh.interval_minutes")),
            opt("pause_for", int(0, 720, 10), json!(0)).sup(live("runtime.pause_after_refresh")),
            opt("reset_refresh", Kind::Bool, json!(true)).sup(live("runtime.reset_refresh")),
            opt("solve_backpage_captcha", Kind::Bool, json!(false)).sup(partial(
                "manual session-ID captcha handoff",
                "CAPTCHA-BACKPAGE",
            )),
            opt("update_stats_while_paused", Kind::Bool, json!(true))
                .lvl(Intermediate)
                .sup(live("statistics.update_while_paused")),
            opt("drone_repair_percentage", pct(), json!(0.9))
                .lvl(Intermediate)
                .sup(partial("observed drone damage", "DRONE-REPAIR")),
            opt("honor_lost_exact", Kind::Bool, json!(true))
                .lvl(Intermediate)
                .sup(gap("STATS-PAUSED")),
            opt("log_chat", Kind::Bool, json!(false))
                .lvl(Intermediate)
                .sup(live("runtime.log_chat")),
            opt("log_deaths", Kind::Bool, json!(false))
                .lvl(Intermediate)
                .sup(live("runtime.log_deaths")),
            opt("avoid_mines", Kind::Bool, json!(true))
                .lvl(Intermediate)
                .sup(partial("typed obstacle geometry", "AVOIDANCE")),
            opt("avoid_cbs", Kind::Bool, json!(true))
                .lvl(Intermediate)
                .sup(partial("typed obstacle geometry", "AVOIDANCE")),
            opt("avoid_radiation", Kind::Bool, json!(true))
                .lvl(Intermediate)
                .sup(partial("typed obstacle geometry", "AVOIDANCE")),
            opt("username_on_title", Kind::Bool, json!(false))
                .lvl(Intermediate)
                .sup(live("window title")),
            opt("auto_refine", Kind::Bool, json!(false))
                .lvl(Advanced)
                .sup(partial(
                    "observed ore inventory and refine command",
                    "AUTO-REFINE",
                )),
        ],
    )
}

const DISPLAY_FLAGS: [(&str, &str); 13] = [
    ("USERNAMES", "usernames"),
    ("NPC_NAMES", "npc_names"),
    ("HERO_NAME", "hero_name"),
    ("HP_SHIELD_NUM", "hp_shield_num"),
    ("ZONES", "zones"),
    ("STATS_AREA", "stats_area"),
    ("GROUP_AREA", "group_area"),
    ("GROUP_NAMES", "group_names"),
    ("BOOSTER_AREA", "booster_area"),
    ("SORT_BOOSTERS", "sort_boosters"),
    ("RESOURCE_NAMES", "resource_names"),
    ("SHOW_PET", "show_pet"),
    ("SHOW_DESTINATION", "show_destination"),
];

fn display_flags() -> Kind {
    Kind::Choice {
        options: DISPLAY_FLAGS
            .iter()
            .map(|(id, key)| {
                let label = translate(&format!("config.bot_settings.map_display.toggle.{key}"))
                    .map(str::to_owned)
                    .unwrap_or_else(|| humanize(key));
                (label, json!(id))
            })
            .collect(),
        multi: true,
    }
}

/// `(key, rgb or argb hex)` of `ColorScheme`'s colour options.
const COLORS: [(&str, &str); 40] = [
    ("background", "263238"),
    ("radiation", "372638"),
    ("text", "f2f2f2"),
    ("text_dark", "bbbbbb"),
    ("going", "8f9bff"),
    ("portals", "aeaeae"),
    ("hero", "22cc22"),
    ("trail_base", "e0e0e0"),
    ("fuel", "f2f2f2"),
    ("boxes", "bbb830"),
    ("mines", "ff8000"),
    ("allies", "29b6f6"),
    ("enemies", "d50000"),
    ("npcs", "aa4040"),
    ("group_member", "ffd700"),
    ("low_relays", "00d54b"),
    ("space_balls", "00d595"),
    ("other_entities", "1647a1"),
    ("target", "7f3030"),
    ("pet", "004c8c"),
    ("pet_in", "c56000"),
    ("health", "388e3c"),
    ("nano_hull", "d0d024"),
    ("shield", "0288d1"),
    ("meteroid", "aaaaaa"),
    ("ping", "00ff0020"),
    ("ping_border", "00ff0080"),
    ("barrier", "ffffff20"),
    ("barrier_border", "ffffff80"),
    ("no_cloack", "18a0ff20"),
    ("prefer", "00ff8020"),
    ("avoid", "ff000020"),
    ("safety", "1080ff60"),
    ("bases", "00ff80"),
    ("base_spots", "00ff8020"),
    ("unknown", "7c05d1"),
    ("texts_background", "26323880"),
    ("action_button", "ffffffa0"),
    ("darken_back", "00000060"),
    ("zone_lines", "80808080"),
];

fn colors() -> Node {
    let mut children: Vec<Node> = COLORS
        .iter()
        .map(|(key, hex)| {
            let consumed = matches!(
                *key,
                "background"
                    | "radiation"
                    | "text"
                    | "text_dark"
                    | "going"
                    | "portals"
                    | "hero"
                    | "trail_base"
                    | "fuel"
                    | "boxes"
                    | "mines"
                    | "allies"
                    | "enemies"
                    | "npcs"
                    | "group_member"
                    | "low_relays"
                    | "other_entities"
                    | "target"
                    | "pet"
                    | "health"
                    | "nano_hull"
                    | "shield"
                    | "meteroid"
                    | "prefer"
                    | "avoid"
                    | "safety"
                    | "bases"
                    | "base_spots"
                    | "unknown"
                    | "zone_lines"
                    | "texts_background"
            );
            opt(
                key,
                Kind::Color {
                    alpha: hex.len() == 8,
                },
                json!(format!("#{hex}")),
            )
            .sup(if consumed {
                live("live map color consumer")
            } else {
                gap("MAP-DISPLAY")
            })
        })
        .collect();
    children.push(grp(
        "zone_editor",
        [
            ("hovering", "0096c8"),
            ("selecting", "0080ff"),
            ("zone", "00ff8040"),
        ]
        .iter()
        .map(|(key, hex)| {
            opt(
                key,
                Kind::Color {
                    alpha: hex.len() == 8,
                },
                json!(format!("#{hex}")),
            )
            .sup(if matches!(*key, "zone" | "hovering" | "selecting") {
                live("live map zone editor color")
            } else {
                gap("MAP-DISPLAY")
            })
        })
        .collect(),
    ));
    children.push(grp(
        "safety_editor",
        [
            ("zone_highlight", "00ff8060"),
            ("zone_selected", "00ff8080"),
            ("zone_solid", "00ff80"),
        ]
        .iter()
        .map(|(key, hex)| {
            opt(
                key,
                Kind::Color {
                    alpha: hex.len() == 8,
                },
                json!(format!("#{hex}")),
            )
            .sup(gap("MAP-DISPLAY"))
        })
        .collect(),
    ));
    children.push(grp(
        "fonts",
        ["big", "mid", "small"]
            .iter()
            .zip(["Consolas 32", "SansSerif 18", "Consolas 13"])
            .map(|(key, font)| opt(key, Kind::Text, json!(font)).sup(gap("MAP-DISPLAY")))
            .collect(),
    ));
    grp("cs", children)
        .i18n("config.colors")
        .label("Colors")
        .lvl(Advanced)
}

fn bot_settings() -> Node {
    grp(
        "bot_settings",
        vec![
            grp(
                "bot_gui",
                vec![
                    opt("locale", choice(&[("English", "en")]), json!("en"))
                        .sup(live("supported locale en")),
                    opt("confirm_exit", Kind::Bool, json!(true))
                        .lvl(Intermediate)
                        .sup(live("window close prompt")),
                    opt("save_gui_pos", Kind::Bool, json!(false))
                        .lvl(Intermediate)
                        .sup(live("saved window bounds")),
                    opt("config_tree_tabs", Kind::Bool, json!(true))
                        .lvl(Intermediate)
                        .sup(live("settings category layout")),
                    opt("button_size", int(1, 20, 1), json!(4))
                        .lvl(Advanced)
                        .sup(live("settings category button size")),
                ],
            ),
            grp(
                "api_config",
                vec![
                    opt(
                        "browser_api",
                        choice(&[
                            ("Ruffle", "ruffle"),
                            ("Flash (OCX)", "flash"),
                            ("Packet client", "packet"),
                        ]),
                        json!("ruffle"),
                    )
                    .sup(partial("settings.client", "CLIENT-OPTIONS")),
                    opt("fully_hide_api", Kind::Bool, json!(true))
                        .sup(live("Flash and client window visibility")),
                    opt("flash_in_process", Kind::Bool, json!(false))
                        .label("Run Flash inside the bot")
                        .desc(
                            "Windows Flash (OCX) only. Hosts Flash in this process through \
                             xenomit_flash.dll, like Legacy's InProcessPlayer: no pipe, so status \
                             and live frames never wait behind game actions. A Flash crash \
                             closes the bot. Changing it relaunches the client.",
                        )
                        .lvl(Advanced)
                        .sup(live("Flash host inside the bot process")),
                    opt("force_game_language", Kind::Bool, json!(false))
                        .sup(live("FlashVars.lang")),
                    opt("enforce_hw_accel", Kind::Bool, json!(true))
                        .sup(partial("Flash WMode direct; Ruffle excluded", "API-LAUNCH")),
                    opt("use_3d", Kind::Bool, json!(false)).sup(live("FlashVars.display2d")),
                    opt("use_proxy", Kind::Bool, json!(false)).sup(gap("API-LAUNCH")),
                    opt("clear_cache_on_stuck", Kind::Bool, json!(true)).sup(gap("API-LAUNCH")),
                    opt("width", int(320, 7680, 10), json!(1280))
                        .lvl(Advanced)
                        .sup(live("Flash launch width")),
                    opt("height", int(240, 4320, 10), json!(800))
                        .lvl(Advanced)
                        .sup(live("Flash launch height")),
                    opt(
                        "game_quality",
                        choice(&[("Low", "low"), ("Medium", "medium"), ("High", "high")]),
                        json!("low"),
                    )
                    .lvl(Advanced)
                    .sup(live("Flash launch quality")),
                    opt(
                        "block_patterns",
                        table(
                            vec![
                                column("regex", Kind::Text, json!("")),
                                column("filepath", Kind::Text, json!("")),
                                column("enable", Kind::Bool, json!(true)),
                            ],
                            true,
                            None,
                        ),
                        json!({}),
                    )
                    .lvl(Developer)
                    .sup(gap("API-LAUNCH")),
                ],
            )
            .lvl(Advanced),
            grp(
                "map_display",
                vec![
                    opt(
                        "toggle",
                        display_flags(),
                        json!([
                            "HERO_NAME",
                            "HP_SHIELD_NUM",
                            "ZONES",
                            "STATS_AREA",
                            "GROUP_AREA",
                            "GROUP_NAMES",
                            "BOOSTER_AREA",
                            "SHOW_PET"
                        ]),
                    )
                    .sup(partial("live map display flags", "MAP-DISPLAY")),
                    opt("trail_length", int(0, 300, 1), json!(15))
                        .lvl(Intermediate)
                        .sup(live("live map hero trail")),
                    opt("round_entities", Kind::Bool, json!(false))
                        .lvl(Intermediate)
                        .sup(live("live map markers")),
                    opt("map_zoom", pct(), json!(1.0))
                        .lvl(Intermediate)
                        .sup(live("live map zoom")),
                    opt("refresh_delay", int(1, 25, 1), json!(1))
                        .lvl(Advanced)
                        .sup(live("live map repaint interval")),
                    opt("map_start_stop", Kind::Bool, json!(false))
                        .lvl(Intermediate)
                        .sup(live("live map left-click control")),
                    colors(),
                ],
            ),
            grp(
                "custom_background",
                vec![
                    opt("enabled", Kind::Bool, json!(false)).sup(live("custom map background")),
                    opt("use_game_background", Kind::Bool, json!(false)).sup(gap("MAP-DISPLAY")),
                    opt("opacity", pct(), json!(0.3)).sup(live("custom background opacity")),
                    opt("image", Kind::Text, json!("")).sup(live("custom background image path")),
                ],
            )
            .lvl(Intermediate),
            grp(
                "performance",
                vec![
                    opt("always_enabled", Kind::Bool, json!(true)).sup(gap("PERFORMANCE")),
                    opt("disable_render", Kind::Bool, json!(false))
                        .i18n("config.bot_settings.api_config.disable_render")
                        .sup(gap("PERFORMANCE")),
                    opt("max_fps", int_off(1, 60, 5, 0, 15), json!(0))
                        .i18n("config.bot_settings.api_config.max_fps")
                        .sup(partial("Flash host FPS cap at launch", "PERFORMANCE")),
                    opt("min_tick", int_off(10, 250, 5, 15, 15), json!(15))
                        .i18n("config.bot_settings.other.min_tick")
                        .lvl(Advanced)
                        .sup(live("runtime effective_tick floor")),
                ],
            ),
            grp(
                "other",
                vec![
                    opt("show_instructions", Kind::Bool, json!(true))
                        .sup(live("settings page instruction banner")),
                    opt("disable_master_password", Kind::Bool, json!(false))
                        .sup(gap("SHELL-PREFS")),
                    opt("always_show_captcha", Kind::Bool, json!(false)).sup(partial(
                        "manual session-ID captcha handoff",
                        "CAPTCHA-BACKPAGE",
                    )),
                    opt("zone_resolution", int(10, 300, 5), json!(30))
                        .sup(live("zone editor snapping")),
                    opt("dev_stuff", Kind::Bool, json!(false))
                        .lvl(Advanced)
                        .sup(live("live map developer details")),
                ],
            )
            .lvl(Intermediate),
        ],
    )
}

/// Legacy keeps this hidden (`/*@Option("Extra actions")*/`); it is listed
/// here at developer level so the full `Config` is reachable.
fn extra() -> Node {
    grp(
        "extra",
        vec![
            opt("condition", Kind::Condition, json!(""))
                .sup(partial("Legacy condition subset", "EXTRA-ACTIONS")),
            opt(
                "action_infos",
                table(
                    vec![
                        column("active", Kind::Bool, json!(false)),
                        column("check_every", int(0, 86_400_000, 100), json!(0)),
                        column("condition", Kind::Condition, json!("")),
                        column("cooldown", int(0, 86_400_000, 100), json!(0)),
                        column("action", Kind::Text, json!("")),
                        column("limit", int(0, 100_000, 1), json!(0)),
                    ],
                    true,
                    None,
                ),
                json!({}),
            )
            .sup(partial(
                "typed condition and action subset",
                "EXTRA-ACTIONS",
            )),
        ],
    )
    .label("Extra actions")
    .lvl(Developer)
}

/// Runtime options with no Legacy equivalent (the "if not more" part).
/// Everything here maps one-to-one onto an engine module setting.
fn xenomit() -> Node {
    fn feature(key: &str, default: bool, target: &'static str, label: &str) -> Node {
        opt(key, Kind::Bool, json!(default))
            .sup(live(target))
            .label(label)
    }
    let number = |min: f64, max: f64, step: f64| Kind::Float { min, max, step };
    grp(
        "xenomit",
        vec![
            grp(
                "safety",
                vec![
                    feature(
                        "enabled",
                        true,
                        "safety.enabled",
                        "Enable safety and repair",
                    ),
                    feature(
                        "auto_revive",
                        true,
                        "safety.auto_revive",
                        "Use offered free base repair",
                    ),
                    feature(
                        "retreat_low_health",
                        true,
                        "settings.feature_flags.retreat_low_health",
                        "Retreat on low health",
                    ),
                    opt("defensive_ability", Kind::Text, json!(""))
                        .sup(live("defensive_ability"))
                        .label("Defensive item id")
                        .desc("Item id used for defence (empty disables)."),
                ],
            ),
            grp(
                "collect",
                vec![
                    feature(
                        "collect_boxes",
                        true,
                        "features.collect_boxes",
                        "Collect boxes",
                    ),
                    feature(
                        "collect_cargo",
                        true,
                        "features.collect_cargo",
                        "Collect cargo",
                    ),
                    feature(
                        "collect_palladium",
                        false,
                        "settings.feature_flags.collect_palladium",
                        "Collect palladium",
                    ),
                    opt("pickup_radius", number(1.0, 1000.0, 10.0), json!(250.0))
                        .sup(live("pickup_radius"))
                        .label("Pickup radius"),
                    opt(
                        "max_box_distance",
                        number(0.0, 100_000.0, 100.0),
                        json!(5000.0),
                    )
                    .sup(live("max_box_distance"))
                    .label("Maximum box distance"),
                    opt("wait_ms", int(0, 60_000, 100), json!(1500))
                        .sup(live("wait_ms"))
                        .label("Pickup wait (ms)"),
                ],
            ),
            grp(
                "combat",
                vec![
                    feature(
                        "prioritize_npcs",
                        true,
                        "settings.feature_flags.prioritize_npcs",
                        "Prioritise NPCs over boxes",
                    ),
                    feature(
                        "switch_configs",
                        true,
                        "features.switch_configs",
                        "Switch ship configurations",
                    ),
                    opt("npc_radius", number(0.0, 5000.0, 10.0), json!(560.0))
                        .sup(live("npc_radius"))
                        .label("Default combat radius"),
                    opt("stationary_radius", number(0.0, 5000.0, 10.0), json!(800.0))
                        .sup(live("stationary_radius"))
                        .label("Maximum stationary target radius"),
                    opt("offensive_ability", Kind::Text, json!(""))
                        .sup(live("offensive_ability"))
                        .label("Offensive item id"),
                    opt(
                        "offensive_ability_range",
                        number(0.0, 5000.0, 10.0),
                        json!(600.0),
                    )
                    .sup(live("offensive_ability_range"))
                    .label("Offensive ability approach radius"),
                    opt("laser_ammo", Kind::Text, json!(""))
                        .sup(live("laser_ammo"))
                        .label("Laser ammunition item id"),
                    opt("rocket_ammo", Kind::Text, json!(""))
                        .sup(live("rocket_ammo"))
                        .label("Rocket ammunition item id"),
                ],
            ),
            grp(
                "pet",
                vec![
                    feature(
                        "pet_auto_loot",
                        true,
                        "settings.feature_flags.pet_auto_loot",
                        "Pet auto loot",
                    ),
                    feature(
                        "pet_resource_collector",
                        false,
                        "settings.feature_flags.pet_resource_collector",
                        "Pet resource collector",
                    ),
                    opt("locator_target", int(0, i32::MAX as i64, 1), json!(0))
                        .sup(live("pet_locator_target"))
                        .label("Locator NPC/resource type id (0 = default)"),
                ],
            ),
            grp(
                "group",
                vec![
                    feature(
                        "follow_leader",
                        false,
                        "settings.feature_flags.group_follow_leader",
                        "Follow the group leader",
                    ),
                    feature(
                        "assist",
                        false,
                        "settings.feature_flags.group_assist",
                        "Assist group members",
                    ),
                ],
            ),
            grp(
                "engine",
                vec![
                    opt("tick_ms", int_off(10, 60_000, 10, 0, 150), json!(0))
                        .lvl(Advanced)
                        .sup(live("tick_ms"))
                        .label("Tick interval override (ms)")
                        .desc("0 keeps the backend default (150 ms native, 750 ms legacy pipe)."),
                    opt(
                        "game_api_backend",
                        choice(&[
                            ("Native (shared memory)", "native"),
                            ("Legacy pipe", "legacy_pipe"),
                        ]),
                        json!("native"),
                    )
                    .lvl(Advanced)
                    .sup(partial("settings.game_api_backend", "CLIENT-OPTIONS"))
                    .label("Ruffle API backend"),
                    opt(
                        "flash_backend",
                        choice(&[
                            ("Cached state", "cached"),
                            ("Native memory", "native_memory"),
                        ]),
                        json!("cached"),
                    )
                    .lvl(Advanced)
                    .sup(partial("settings.flash_backend", "CLIENT-OPTIONS"))
                    .label("Flash backend"),
                    opt("behavior_config", Kind::Json, Value::Null)
                        .lvl(Developer)
                        .sup(live("behavior_config"))
                        .label("Behavior config override")
                        .desc("A non-null object replaces the planner settings entirely."),
                ],
            ),
        ],
    )
    .label("Xenomit")
}
