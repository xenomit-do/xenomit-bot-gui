//! Remaining client/runtime limitations for settings-tree options.
//! Every `Support::gap`/`partial` in `settings.rs` names one of these ids;
//! `docs/SETTINGS_GAPS.md` is generated from this table.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Layer {
    /// `xenomit-game-api` and the clients (Ruffle, Flash, packet).
    ClientApi,
    /// `xenomit-behavior-engine`.
    Engine,
    /// `xenomit-bot-runtime` (worker, settings, host services).
    Runtime,
    /// This crate's GPUI shell.
    Shell,
}

impl Layer {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Layer::ClientApi => "Client API",
            Layer::Engine => "Behavior engine",
            Layer::Runtime => "Runtime",
            Layer::Shell => "GUI shell",
        }
    }
}

pub(crate) struct Gap {
    pub id: &'static str,
    pub title: &'static str,
    pub layers: &'static [Layer],
    /// The addition needed before the options can become `Live`.
    pub needs: &'static str,
}

use Layer::{ClientApi, Engine, Runtime, Shell};

pub(crate) const GAPS: &[Gap] = &[
    Gap {
        id: "SHIP-FORMATION",
        title: "Per-mode drone formation",
        layers: &[Engine, ClientApi],
        needs: "The planner reads the selected formation for each mode and resolves it through the observed slot bar. Flash and packet clients do not yet report the slot key-to-item mapping, so formation changes are skipped safely when that mapping is unavailable.",
    },
    Gap {
        id: "HOTKEYS",
        title: "Slot-bar hotkey resolution",
        layers: &[ClientApi, Runtime],
        needs: "The engine resolves one-character and named keys against `ClientFeatureState.slot_bar` and validates item availability before acting. The checked-in Flash and packet adapters still return no slot-key mapping, so key-based actions remain unavailable on those clients.",
    },
    Gap {
        id: "REVIVE-LOCATION",
        title: "Revive at portal or spot",
        layers: &[ClientApi, Engine],
        needs: "The safety planner selects the requested location only when it is observed, then falls back to an offered free base repair. The Flash adapter currently exposes only the free base option and the packet protocol does not identify destinations; unrecognized offers remain `Unknown` and are never guessed to be base repairs.",
    },
    Gap {
        id: "INSTANT-REPAIR",
        title: "Instant repair items",
        layers: &[ClientApi, Engine],
        needs: "The safety manager uses the configured threshold and a typed inventory observation that includes the exact item id. Neither checked-in client adapter currently reports that repair inventory, so the action is skipped until both count and item identity are observed.",
    },
    Gap {
        id: "SHIP-ABILITY",
        title: "Ship ability by hotkey and distance",
        layers: &[ClientApi, Engine],
        needs: "The planner resolves the configured ability key and checks readiness, cooldown, and the configured safety distance. The adapters do not yet publish slot mappings or per-ship ability readiness/range, so these actions remain unavailable until observations arrive.",
    },
    Gap {
        id: "ROAMING",
        title: "Roaming policy",
        layers: &[Engine, Runtime],
        needs: "Waypoint retention and sequential/random selection are implemented. The remaining `enemy_cbs_invisible` rule needs a reliable hero-cloak observation and station ownership/visibility facts; it is not inferred from missing entity data.",
    },
    Gap {
        id: "NPC-TABLE",
        title: "Per-NPC radius, priority, ammo, formation and flags",
        layers: &[Engine],
        needs: "Per-NPC radius, priority, allow/deny, key, formation, and flags are consumed by the planner. Key and formation actions depend on the slot-bar mapping described in HOTKEYS; absent mappings safely skip those actions.",
    },
    Gap {
        id: "CONDITIONS",
        title: "condition expression language",
        layers: &[Engine, Runtime],
        needs: "The editor parses supported condition expressions into the engine's typed representation and reports unsupported expressions. The supported operator/value subset is intentionally bounded; adding another condition expression requires a typed evaluator and validation case.",
    },
    Gap {
        id: "GROUP",
        title: "Group management",
        layers: &[ClientApi, Engine],
        needs: "The planner applies tag rules and typed group commands when client state is observed. Flash and packet clients do not yet provide complete member/invite/open-state observations or all group operations, so unsupported commands return unavailable.",
    },
    Gap {
        id: "CAPTCHA-BACKPAGE",
        title: "Backpage captcha solving",
        layers: &[Runtime],
        needs: "Backpage captcha detection routes the user to the existing manual session-ID login flow. Automatic solving is intentionally out of scope; the option cannot claim a successful solve.",
    },
    Gap {
        id: "STATS-PAUSED",
        title: "Statistics while paused and honor-loss filtering",
        layers: &[Runtime],
        needs: "Statistics continue updating while paused. `honor_lost_exact` still needs an authoritative reward/event classification that distinguishes kill rewards from unrelated honor loss.",
    },
    Gap {
        id: "DRONE-REPAIR",
        title: "Drone repair threshold",
        layers: &[ClientApi, Engine],
        needs: "The engine issues the typed repair command only when observed drone damage crosses the threshold. Neither adapter currently reports damaged-drone inventory, and the Flash command bridge rejects drone repair until its manager path is known.",
    },
    Gap {
        id: "AVOIDANCE",
        title: "Mine, CBS and radiation avoidance toggles",
        layers: &[ClientApi, Engine],
        needs: "The route planner filters typed Mine/CBS/Radiation shapes independently and always preserves no-access zones. The Flash bridge currently emits no mine, station-area, or radiation shapes; add authoritative geometry observations before these toggles can affect routes.",
    },
    Gap {
        id: "AUTO-REFINE",
        title: "Automatic ore refining",
        layers: &[Engine],
        needs: "The economy rule uses observed ore quantities and the typed refine command. A client must confirm refinery availability and accepted recipe quantities; unsupported or missing refinery state leaves the command unavailable.",
    },
    Gap {
        id: "PET-GEARS",
        title: "Pet gear list from the pet",
        layers: &[ClientApi],
        needs: "The settings picker filters to the observed equipped gear list when available, and the engine skips unobserved gear choices. Without a live pet feature observation the picker falls back to its supported gear set and marks the choice unavailable.",
    },
    Gap {
        id: "SHELL-PREFS",
        title: "Window and interface preferences",
        layers: &[Shell],
        needs: "Window bounds, exit prompts, title, panel layout, and the supported English locale are consumed by the shell. `disable_master_password` has no corresponding master-password feature in this application and remains inert until that feature exists.",
    },
    Gap {
        id: "API-LAUNCH",
        title: "Game client launch options",
        layers: &[Runtime, ClientApi],
        needs: "The checked-in Flash launcher consumes visibility, language, 2D/3D, size, quality, and hardware acceleration through WMode. Changes to launch settings restart the active Flash/Ruffle client and preserve the running behavior state. Proxy configuration, stuck-cache recovery, and URL replacement patterns still have no launcher/client consumer. Ruffle host internals are out of scope.",
    },
    Gap {
        id: "MAP-DISPLAY",
        title: "Live map display options",
        layers: &[Shell],
        needs: "The live map consumes display toggles, obstacle and entity colors, zone fill/outline/hover colors, trail length, zoom, repaint delay, marker shape, click control, and custom image/opacity. Remaining gaps are font names, in-game background sampling, and safety-editor-only colors that have no matching editor in this application.",
    },
    Gap {
        id: "PERFORMANCE",
        title: "Render and tick pacing",
        layers: &[Runtime, Shell],
        needs: "The runtime applies `min_tick` as a floor over the selected backend/module interval. `always_enabled`, `disable_render`, and `max_fps` still need a checked-in client/runtime consumer. Ruffle host internals are out of scope.",
    },
    Gap {
        id: "EXTRA-ACTIONS",
        title: "Extra actions (condition -> action table)",
        layers: &[Engine, Runtime],
        needs: "The runtime accepts the shared typed condition representation and limits action rows to supported game commands and host actions. The editor reports parser/action validation errors; unsupported condition syntax or commands remain disabled.",
    },
    Gap {
        id: "CLIENT-OPTIONS",
        title: "Client choice applies on next launch",
        layers: &[Runtime],
        needs: "The selected client is used for the next session and settings changes request the stop/relaunch path. Client relaunch still depends on a valid launch configuration and a client process that the host can replace.",
    },
];

pub(crate) fn find(id: &str) -> Option<&'static Gap> {
    GAPS.iter().find(|gap| gap.id == id)
}
