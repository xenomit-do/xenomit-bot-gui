# Settings gaps: what the runtime still needs

The settings tree (`src/bot/config_tree/`) ports DarkBot's full option list. Options marked
**LIVE** have a working consumer in the shell, runtime, engine, or client.
**PARTIAL** options are consumed when their required observations or operations are available.
**PLANNED** options are shown and persisted per profile, but have no working consumer yet.

Each gap names the layer that must change. The id (`SHIP-FORMATION`, ...) is what the schema
references; the GUI shows the same text when you click an option's status chip.
This file is generated: `cargo test regenerate_docs -- --ignored`. A test fails if an id is
missing here.

## Client API

### `SHIP-FORMATION` — Per-mode drone formation

Also touches: Behavior engine.

The planner reads the selected formation for each mode and resolves it through the observed slot bar. Flash and packet clients do not yet report the slot key-to-item mapping, so formation changes are skipped safely when that mapping is unavailable.

Affects:
- `general.offensive` — Offensive config (partial)
- `general.roam` — Roam config (partial)
- `general.run` — Run config (partial)
- `general.safety.repair` — Repair config (partial)

### `HOTKEYS` — Slot-bar hotkey resolution

Also touches: Runtime.

The engine resolves one-character and named keys against `ClientFeatureState.slot_bar` and validates item availability before acting. The checked-in Flash and packet adapters still return no slot-key mapping, so key-based actions remain unavailable on those clients.

Affects:
- `collect.auto_cloack_key` — Auto cloack key (partial)
- `loot.sab.key` — Key (partial)
- `loot.rsb.key` — Key (partial)
- `loot.ammo_key` — Ammo key (partial)

### `REVIVE-LOCATION` — Revive at portal or spot

Also touches: Behavior engine.

The safety planner selects the requested location only when it is observed, then falls back to an offered free base repair. The Flash adapter currently exposes only the free base option and the packet protocol does not identify destinations; unrecognized offers remain `Unknown` and are never guessed to be base repairs.

Affects:
- `general.safety.revive` — Revive location (partial)

### `INSTANT-REPAIR` — Instant repair items

Also touches: Behavior engine.

The safety manager uses the configured threshold and a typed inventory observation that includes the exact item id. Neither checked-in client adapter currently reports that repair inventory, so the action is skipped until both count and item identity are observed.

Affects:
- `general.safety.instant_repair` — Use instant repairs when above (partial)

### `SHIP-ABILITY` — Ship ability by hotkey and distance

Also touches: Behavior engine.

The planner resolves the configured ability key and checks readiness, cooldown, and the configured safety distance. The adapters do not yet publish slot mappings or per-ship ability readiness/range, so these actions remain unavailable until observations arrive.

Affects:
- `general.running.ship_ability` — Ship ability (partial)
- `general.running.ship_ability_min` — Ship ability min distance (partial)
- `loot.ship_ability` — Offensive ability key (partial)
- `loot.ship_ability_min` — Offensive ability min health (partial)

### `GROUP` — Group management

Also touches: Behavior engine.

The planner applies tag rules and typed group commands when client state is observed. Flash and packet clients do not yet provide complete member/invite/open-state observations or all group operations, so unsupported commands return unavailable.

Affects:
- `group.accept_invites` — Accept invites (partial)
- `group.open_invites` — Open invites (Leader) (partial)
- `group.block_invites` — Block invites (partial)
- `group.leave_no_whitelisted` — Leave if no whitelisted remain (partial)
- `group.kick_no_invited` — Kick if not invited players (partial)

### `DRONE-REPAIR` — Drone repair threshold

Also touches: Behavior engine.

The engine issues the typed repair command only when observed drone damage crosses the threshold. Neither adapter currently reports damaged-drone inventory, and the Flash command bridge rejects drone repair until its manager path is known.

Affects:
- `miscellaneous.drone_repair_percentage` — Repair Drone Percentage (partial)

### `AVOIDANCE` — Mine, CBS and radiation avoidance toggles

Also touches: Behavior engine.

The route planner filters typed Mine/CBS/Radiation shapes independently and always preserves no-access zones. The Flash bridge currently emits no mine, station-area, or radiation shapes; add authoritative geometry observations before these toggles can affect routes.

Affects:
- `miscellaneous.avoid_mines` — Avoid mines (partial)
- `miscellaneous.avoid_cbs` — Avoid CBS (partial)
- `miscellaneous.avoid_radiation` — Avoid radiation (partial)

### `PET-GEARS` — Pet gear list from the pet

The settings picker filters to the observed equipped gear list when available, and the engine skips unobserved gear choices. Without a live pet feature observation the picker falls back to its supported gear set and marks the choice unavailable.

Affects:
- `pet.module_id` — PET module (partial)

### `API-LAUNCH` — Game client launch options

Also touches: Runtime.

The checked-in Flash launcher consumes visibility, language, 2D/3D, size, quality, and hardware acceleration through WMode. Changes to launch settings restart the active Flash/Ruffle client and preserve the running behavior state. Proxy configuration, stuck-cache recovery, and URL replacement patterns still have no launcher/client consumer. Ruffle host internals are out of scope.

Affects:
- `bot_settings.api_config.enforce_hw_accel` — Enforce hardware acceleration (partial)
- `bot_settings.api_config.use_proxy` — Use proxy (must on win7) (planned)
- `bot_settings.api_config.clear_cache_on_stuck` — Clear cache on loading stuck (planned)
- `bot_settings.api_config.block_patterns` — Block patterns (planned)

## Behavior engine

### `SHIP-FORMATION` — Per-mode drone formation

Also touches: Client API.

The planner reads the selected formation for each mode and resolves it through the observed slot bar. Flash and packet clients do not yet report the slot key-to-item mapping, so formation changes are skipped safely when that mapping is unavailable.

Affects:
- `general.offensive` — Offensive config (partial)
- `general.roam` — Roam config (partial)
- `general.run` — Run config (partial)
- `general.safety.repair` — Repair config (partial)

### `REVIVE-LOCATION` — Revive at portal or spot

Also touches: Client API.

The safety planner selects the requested location only when it is observed, then falls back to an offered free base repair. The Flash adapter currently exposes only the free base option and the packet protocol does not identify destinations; unrecognized offers remain `Unknown` and are never guessed to be base repairs.

Affects:
- `general.safety.revive` — Revive location (partial)

### `INSTANT-REPAIR` — Instant repair items

Also touches: Client API.

The safety manager uses the configured threshold and a typed inventory observation that includes the exact item id. Neither checked-in client adapter currently reports that repair inventory, so the action is skipped until both count and item identity are observed.

Affects:
- `general.safety.instant_repair` — Use instant repairs when above (partial)

### `SHIP-ABILITY` — Ship ability by hotkey and distance

Also touches: Client API.

The planner resolves the configured ability key and checks readiness, cooldown, and the configured safety distance. The adapters do not yet publish slot mappings or per-ship ability readiness/range, so these actions remain unavailable until observations arrive.

Affects:
- `general.running.ship_ability` — Ship ability (partial)
- `general.running.ship_ability_min` — Ship ability min distance (partial)
- `loot.ship_ability` — Offensive ability key (partial)
- `loot.ship_ability_min` — Offensive ability min health (partial)

### `ROAMING` — Roaming policy

Also touches: Runtime.

Waypoint retention and sequential/random selection are implemented. The remaining `enemy_cbs_invisible` rule needs a reliable hero-cloak observation and station ownership/visibility facts; it is not inferred from missing entity data.

Affects:
- `general.roaming.enemy_cbs_invisible` — Ignore CBS status when invisible (planned)

### `NPC-TABLE` — Per-NPC radius, priority, ammo, formation and flags

Per-NPC radius, priority, allow/deny, key, formation, and flags are consumed by the planner. Key and formation actions depend on the slot-bar mapping described in HOTKEYS; absent mappings safely skip those actions.

Affects:
- `loot.npc_infos` — Npc infos (partial)

### `CONDITIONS` — DarkBot condition language

Also touches: Runtime.

The editor parses supported DarkBot conditions into the engine's typed representation and reports unsupported expressions. The supported operator/value subset is intentionally bounded; adding another DarkBot condition requires a typed evaluator and validation case.

Affects:
- `loot.sab.condition` — Other conditions (partial)

### `GROUP` — Group management

Also touches: Client API.

The planner applies tag rules and typed group commands when client state is observed. Flash and packet clients do not yet provide complete member/invite/open-state observations or all group operations, so unsupported commands return unavailable.

Affects:
- `group.accept_invites` — Accept invites (partial)
- `group.open_invites` — Open invites (Leader) (partial)
- `group.block_invites` — Block invites (partial)
- `group.leave_no_whitelisted` — Leave if no whitelisted remain (partial)
- `group.kick_no_invited` — Kick if not invited players (partial)

### `DRONE-REPAIR` — Drone repair threshold

Also touches: Client API.

The engine issues the typed repair command only when observed drone damage crosses the threshold. Neither adapter currently reports damaged-drone inventory, and the Flash command bridge rejects drone repair until its manager path is known.

Affects:
- `miscellaneous.drone_repair_percentage` — Repair Drone Percentage (partial)

### `AVOIDANCE` — Mine, CBS and radiation avoidance toggles

Also touches: Client API.

The route planner filters typed Mine/CBS/Radiation shapes independently and always preserves no-access zones. The Flash bridge currently emits no mine, station-area, or radiation shapes; add authoritative geometry observations before these toggles can affect routes.

Affects:
- `miscellaneous.avoid_mines` — Avoid mines (partial)
- `miscellaneous.avoid_cbs` — Avoid CBS (partial)
- `miscellaneous.avoid_radiation` — Avoid radiation (partial)

### `AUTO-REFINE` — Automatic ore refining

The economy rule uses observed ore quantities and the typed refine command. A client must confirm refinery availability and accepted recipe quantities; unsupported or missing refinery state leaves the command unavailable.

Affects:
- `miscellaneous.auto_refine` — Auto refine (API-specific) (partial)

### `EXTRA-ACTIONS` — Extra actions (condition -> action table)

Also touches: Runtime.

The runtime accepts the shared typed condition representation and limits action rows to supported game commands and host actions. The editor reports parser/action validation errors; unsupported condition syntax or commands remain disabled.

Affects:
- `extra.condition` — Condition (partial)
- `extra.action_infos` — Action infos (partial)

## Runtime

### `HOTKEYS` — Slot-bar hotkey resolution

Also touches: Client API.

The engine resolves one-character and named keys against `ClientFeatureState.slot_bar` and validates item availability before acting. The checked-in Flash and packet adapters still return no slot-key mapping, so key-based actions remain unavailable on those clients.

Affects:
- `collect.auto_cloack_key` — Auto cloack key (partial)
- `loot.sab.key` — Key (partial)
- `loot.rsb.key` — Key (partial)
- `loot.ammo_key` — Ammo key (partial)

### `ROAMING` — Roaming policy

Also touches: Behavior engine.

Waypoint retention and sequential/random selection are implemented. The remaining `enemy_cbs_invisible` rule needs a reliable hero-cloak observation and station ownership/visibility facts; it is not inferred from missing entity data.

Affects:
- `general.roaming.enemy_cbs_invisible` — Ignore CBS status when invisible (planned)

### `CONDITIONS` — DarkBot condition language

Also touches: Behavior engine.

The editor parses supported DarkBot conditions into the engine's typed representation and reports unsupported expressions. The supported operator/value subset is intentionally bounded; adding another DarkBot condition requires a typed evaluator and validation case.

Affects:
- `loot.sab.condition` — Other conditions (partial)

### `CAPTCHA-BACKPAGE` — Backpage captcha solving

Backpage captcha detection routes the user to the existing manual session-ID login flow. Automatic solving is intentionally out of scope; the option cannot claim a successful solve.

Affects:
- `miscellaneous.solve_backpage_captcha` — Solve Captcha for Backpage (partial)
- `bot_settings.other.always_show_captcha` — Always show captcha window (partial)

### `STATS-PAUSED` — Statistics while paused and honor-loss filtering

Statistics continue updating while paused. `honor_lost_exact` still needs an authoritative reward/event classification that distinguishes kill rewards from unrelated honor loss.

Affects:
- `miscellaneous.honor_lost_exact` — Prevent pausing on random honor lost (planned)

### `API-LAUNCH` — Game client launch options

Also touches: Client API.

The checked-in Flash launcher consumes visibility, language, 2D/3D, size, quality, and hardware acceleration through WMode. Changes to launch settings restart the active Flash/Ruffle client and preserve the running behavior state. Proxy configuration, stuck-cache recovery, and URL replacement patterns still have no launcher/client consumer. Ruffle host internals are out of scope.

Affects:
- `bot_settings.api_config.enforce_hw_accel` — Enforce hardware acceleration (partial)
- `bot_settings.api_config.use_proxy` — Use proxy (must on win7) (planned)
- `bot_settings.api_config.clear_cache_on_stuck` — Clear cache on loading stuck (planned)
- `bot_settings.api_config.block_patterns` — Block patterns (planned)

### `PERFORMANCE` — Render and tick pacing

Also touches: GUI shell.

The runtime applies `min_tick` as a floor over the selected backend/module interval. `always_enabled`, `disable_render`, and `max_fps` still need a checked-in client/runtime consumer. Ruffle host internals are out of scope.

Affects:
- `bot_settings.performance.always_enabled` — Always enabled (planned)
- `bot_settings.performance.disable_render` — Disable map render (3D) (planned)
- `bot_settings.performance.max_fps` — Max FPS in-game (planned)

### `EXTRA-ACTIONS` — Extra actions (condition -> action table)

Also touches: Behavior engine.

The runtime accepts the shared typed condition representation and limits action rows to supported game commands and host actions. The editor reports parser/action validation errors; unsupported condition syntax or commands remain disabled.

Affects:
- `extra.condition` — Condition (partial)
- `extra.action_infos` — Action infos (partial)

### `CLIENT-OPTIONS` — Client choice applies on next launch

The selected client is used for the next session and settings changes request the stop/relaunch path. Client relaunch still depends on a valid launch configuration and a client process that the host can replace.

Affects:
- `bot_settings.api_config.browser_api` — Browser/API, restart to apply (partial)
- `xenomit.engine.game_api_backend` — Ruffle API backend (partial)
- `xenomit.engine.flash_backend` — Flash backend (partial)

## GUI shell

### `SHELL-PREFS` — Window and interface preferences

Window bounds, exit prompts, title, panel layout, and the supported English locale are consumed by the shell. `disable_master_password` has no corresponding master-password feature in this application and remains inert until that feature exists.

Affects:
- `bot_settings.other.disable_master_password` — Disable master password (planned)

### `MAP-DISPLAY` — Live map display options

The live map consumes display toggles, obstacle and entity colors, zone fill/outline/hover colors, trail length, zoom, repaint delay, marker shape, click control, and custom image/opacity. Remaining gaps are font names, in-game background sampling, and safety-editor-only colors that have no matching editor in this application.

Affects:
- `bot_settings.map_display.toggle` — Toggle displaying (partial)
- `bot_settings.map_display.cs.space_balls` — Space balls (planned)
- `bot_settings.map_display.cs.pet_in` — Pet in (planned)
- `bot_settings.map_display.cs.ping` — Ping (planned)
- `bot_settings.map_display.cs.ping_border` — Ping border (planned)
- `bot_settings.map_display.cs.barrier` — Barrier (planned)
- `bot_settings.map_display.cs.barrier_border` — Barrier border (planned)
- `bot_settings.map_display.cs.no_cloack` — No cloack (planned)
- `bot_settings.map_display.cs.action_button` — Action button (planned)
- `bot_settings.map_display.cs.darken_back` — Darken back (planned)
- `bot_settings.map_display.cs.safety_editor.zone_highlight` — Zone highlight (planned)
- `bot_settings.map_display.cs.safety_editor.zone_selected` — Zone selected (planned)
- `bot_settings.map_display.cs.safety_editor.zone_solid` — Zone solid (planned)
- `bot_settings.map_display.cs.fonts.big` — Big (planned)
- `bot_settings.map_display.cs.fonts.mid` — Mid (planned)
- `bot_settings.map_display.cs.fonts.small` — Small (planned)
- `bot_settings.custom_background.use_game_background` — Use game background (planned)

### `PERFORMANCE` — Render and tick pacing

Also touches: Runtime.

The runtime applies `min_tick` as a floor over the selected backend/module interval. `always_enabled`, `disable_render`, and `max_fps` still need a checked-in client/runtime consumer. Ruffle host internals are out of scope.

Affects:
- `bot_settings.performance.always_enabled` — Always enabled (planned)
- `bot_settings.performance.disable_render` — Disable map render (3D) (planned)
- `bot_settings.performance.max_fps` — Max FPS in-game (planned)

