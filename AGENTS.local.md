# xenomit-bot-gui notes

GPUI desktop controller for `xenomit-client`. Module metadata comes from the
behavior engine; settings are persisted in `modules.json`. Start runs
`PlannerRuntime` (in the sibling `xenomit-bot-runtime` crate),
which feeds `GameSnapshot`s from `xenomit-game-api` into
`xenomit-behavior-engine` and executes the resulting `ActionIntent`s through
`GameClient::call_function`.

## Layout

Phase 0 split the crate in two (orchestrator `docs/02-headless-bot.md` §2.1):

- `xenomit-bot-gui` (this repo) is the thin GPUI shell. `src/bot/view/` holds
  `BotView` (state struct, `Render`, `Drop` in `mod.rs`) with behavior split
  into `impl BotView` blocks per concern: `theme` (palette/widgets), `chrome`
  (titlebar/header/menu), `login`, `launch` (child-process launch, log
  capture), `client`, `lifecycle`, `live` (frame monitor), `run_control`
  (start/stop behavior), `remote_panel`, `catalog`, `pages`/`account`
  (settings UI), `stats_page`, `map`/`map_zones`, `window` (pin/tray/settings
  window), `platform` (Win32 helpers), `settings`. Submodules do
  `use super::*;` and expose methods as `pub(super)`.
  `src/bot/live_map/` (palette, geometry, markers, naming, hud, trail, paint,
  overlay + tests), `src/bot/module_settings/` (mod, maps, render, fields,
  edit + tests) and `src/bot/mod.rs` (window bootstrap). Nothing
  planner-adjacent stays here.
- `xenomit-bot-runtime` (git dependency, **no gpui**) owns
  `behavior.rs` (`PlannerRuntime`, worker, host actions, Backpage services),
  `statistics.rs`, `panel.rs`, `packet_client.rs`, `star_catalog.rs`, plus
  new `settings.rs` (`BotSettings`/`ClientKind`/`FlashBackend`/`SettingsStore`)
  and `session.rs` (`SessionProvider`/`BackpageSessionProvider`).

Four seams keep the runtime front-end-agnostic; the GUI uses the behavior-
preserving defaults and `botd` can swap them later:

- `HostActionHandler` ← `HostActionQueue` (process-global queue drained by
  `take_host_actions()` on the UI tick; `SelectTarget` executes in-runtime,
  `RequestRefresh` stays an internal signal).
- `StatSink` ← `FileStatSink` (same JSONL journals).
- `SettingsStore` ← `FileSettingsStore` (same `%APPDATA%\XenomitBot\modules.json`).
- `SessionProvider` ← `BackpageSessionProvider` (password or pasted dosid).

## Behavior runtime

- Base's `refresh` rule defaults to enabled with `interval_minutes = 60`.
  It emits `RequestRefresh`, which the worker checks before ticking the module
  via `ModuleRunner::interruption_status`. Overdue requests keep normal work
  running until permission is `Ready`; gates defer until exit, and durable
  sequences/account operations defer until completion. Approval parks the
  worker until the UI reloads and resumes; completed workflows use
  `RefreshIdle` to avoid replaying them. Custom modules default to `Deferred`.

- Built-in contributions belong to independently switchable `base`, `combat`,
  `gg`, `event`, `economy` and `utility` plugins. Global `plugins` selections
  persist in modules.json, are locked during a run, and filter both objective
  factories and active/stopped general rules. Disabled settings are retained.
  `PlannerRuntime` discovers version 1 third-party JSON manifests under
  `%APPDATA%\XenomitBot\plugins` at startup; restart to reload files. See the
  engine's `PLUGINS.md` for sequence/planner/rule formats, namespace and
  dependency validation, and `API_GAPS.md` / `api-gaps.json` for missing APIs.

- General rules are independent of the objective module. Global settings live
  in `behaviors` in modules.json and are injected after per-module settings;
  the retained Behaviors page does not change `active_module`. The engine's
  `BehaviorRegistry` creates retained rules; higher-priority temporary takeovers
  suspend module ticks and resume the same instance afterward. Item selectors
  can supplement defense. `BehaviorRuntime::tick_stopped` runs on the status
  monitor's background executor and honors each rule's `tick_stopped` option.
- Safe stop/Stop + close request the gate-aware rule. Host actions execute
  disconnect, process cleanup, authenticated reload/resume, profile switches,
  and persisted player tags. Urgent quest acceptance needs `game.urgent_quest`;
  calendar claiming needs `backpage.calendar`. Missing services reject enabled
  actions. See the engine's `GENERAL_BEHAVIORS.md` for upstream revisions,
  rule settings, and backend observation requirements.

- Register modules and service adapters before constructing
  `PlannerRuntime::with_registry`. The registry is immutable after attachment.
  `BehaviorRuntime::modules()` is queried once on a background executor at
  startup, concurrently with login/game loading. `ModuleSettingsPage` builds
  retained inputs/subscriptions once for each descriptor; switching modules
  reuses them. Per-module values live under `modules.<id>` with legacy settings
  providing migration defaults. Start validates values and required services.
- Modules cannot change during starting/running/stopping. `ModuleRunner`
  retains one module instance, rejects ID switches, and updates settings in
  place only when that module permits it. `TaskSequence` supports cooperative
  delays, result-dependent operations, timeouts and explicit idempotent retries.
  Nongame operations tick without a snapshot while the bridge loads; game
  operations wait for readiness. Stop unparks the worker and cancels services.
  See the engine's `CUSTOM_MODULES.md` and `examples/sequenced_event.rs`.

- NPC target selection lives in the settings UI ("NPC killer" tab → "NPCs to
  kill") as `npcs`: a `species → enabled` map that auto-populates from live
  frames the first time each NPC species is seen (`note_live_frame`) and
  persists in modules.json, so the user only unchecks species to spare.
  Entries match entity `name` and class names; an absent/empty map attacks
  every NPC, a populated-but-all-disabled map attacks none. The
  `behavior_config` blob in modules.json still overrides the whole engine
  config.
- The hero trail on the live map samples position every 250 ms and keeps
  points for 5 s (`TRAIL_SAMPLE`/`TRAIL_TTL` in `live_map/`) — persistence
  is time-based, not distance-based.
- The live map supports click-and-drag navigation: the canvas paint closure
  registers per-frame window mouse listeners (the scrollbar pattern), so a
  drag keeps retargeting — clamped to the map edge — even after the pointer
  leaves the canvas. Drag updates are throttled by `MAP_NAV_DRAG_INTERVAL`
  and gated on `map_dragging`.
- All map discs paint through `snapped_disc` (`live_map/`), which snaps
  the diameter and origin to the device-pixel grid: `paint_quad` rounds
  each bounds edge independently, so centered quads would jitter ±1 device
  pixel in size at fractional positions or display scales.
- The live map replicates DarkBot's overlay (`MapDrawer`/`OverlayDrawer`/
  `InfosDrawer` conventions): a large centered map name — "Loading" while
  the client reports no map (`map_id = -1`, `map_status()` → `Loading`) —
  portal rings labeled with the target map, the left-edge stats column
  (`cre/h`, `uri/h`, `exp/h`, `hon/h`, `cargo`, `death`), `NC` config and
  health/shield bars bottom-left, and the locked-target bar bottom-right.
  Ship usernames/NPC names paint above their markers (DarkBot's
  `USERNAMES`/`NPC_NAMES` flags), battle stations via `safe_places`.
  Names/targets resolve from `LiveFrame.navigation.star_system`/`portals`
  first (native captures the game's own `Map._staticConfig` +
  `SpaceMapConfig` XML), then the CDN-fetched catalog
  (`star_catalog::fetch` at login → `resources.xml` → `maps-config` +
  `spacemap-config` → `StarSystem::from_config_xml`, which expands the
  files' `<reference>` templates like `XMLUtils.resolveReferences`), then
  the bundled `star_map` table in `xenomit-game-api` (ported from
  DarkBot's `StarManager`) as the offline fallback. `inject_star_catalog`
  fills empty navigation on snapshot-fed frames and derives
  `PortalInfo.target_map` for portal entities from the jumpgate links.
  Deaths are counted on the `navigation.hero.killed` rising edge (plus
  hero hp → 0) in `note_live_frame`; `hero_field()` resolves
  `cargo`/`localTargetId` from live entity fields or the snapshot.

- `PlannerRuntime::start` returns after the worker thread is spawned; the GUI
  shows live worker state via `BehaviorRuntime::report()` (bridge readiness,
  entity counts, active target, roam status, last error) on the `activity`
  line — polled once per second by the status monitor in `view/`.
- When the plan is empty (no NPC/box in range) the executor roams map
  waypoints instead of idling; without map bounds it falls back to
  `stopHero`.
- Movement and collection calls use `CallTiming::InFrame` (`FINVOKE`/
  `NFINVOKE`); ship selection and notifications run immediately.
- `is_running()` reflects real worker liveness; a panicking worker no longer
  wedges `behavior_running` forever.

## Statistics

- `xenomit-bot-runtime/src/statistics.rs` stores account-scoped JSONL journals under
  `%APPDATA%\XenomitBot\statistics\<server>-<user-id>.jsonl`. The page keeps
  up to 30 days / 100,000 events in memory and shows 1h/24h/7d/30d graphs.
- Ruffle emits `[game-log]` and `[game-assets]` JSON records from the client's
  `StateReader`; rebuild `xenomit-client` for capture support. Message-object
  identity prevents duplicate polling without suppressing identical rewards.
  Live Hero totals reconcile all four assets every minute, on both backends.
- Packet mode drains decoded events through `PacketGameClient::pump_events`.
  Legacy `y`/`LM` asset logs contain signed changes and authoritative totals;
  these also update the packet world model. Localized log command keys are
  preserved. Ruffle matches the game's localized reward templates, with an
  English parser for additional reward sentences.
- Backpage `hangar_list` refreshes credits and Uridium every minute in a
  generation-scoped background task. Honor/experience reconcile from live
  game totals. Corrections and spending are excluded from gross reward/hour,
  which uses the last hour of the current connection and observed elapsed time.
- The Statistics toolbar button opens the page independently of bot Start.

## Build & test

- Native Ruffle navigation uses version 2 shared-memory snapshots. Rebuild bot
  and client together. `MovementRouter` follows obstacle-safe waypoints for
  planning/roaming/collection; missing current-map geometry pauses movement.
  `live_map/` paints the captured circle/rectangle/polygon NOA shapes.
- Working-map settings use the module schema's searchable map selector.
  Built-in modules retain `MapTraveler` and `SafetyManager`, follow the fewest
  directed portal jumps and resume after settling on the selected map.
- Module descriptors are discovered asynchronously while the game loads.
  `ModuleSettingsPage` retains inputs/selects/subscriptions for each module;
  changing module ID is rejected while a run is active. Custom modules and
  services register before attachment; see the engine's `CUSTOM_MODULES.md`.
- Typed manager commands execute through `GameClient::game_command`; report
  each successful command independently before later action failures.

- Startup shows only the sign-in page; the dashboard is rendered after
  authentication and client connection succeed. The GPUI component `Root`
  and dark theme provide readable inputs and Tab/Shift+Tab navigation.
- The window is borderless: `WindowOptions.titlebar.appears_transparent`
  (Windows) + `window_decorations: Client` (Linux). `BotView::render_titlebar`
  draws the drag strip and min/max/close caption buttons; they work via
  `window_control_area` hitboxes (HTCAPTION/HTMINBUTTON/HTMAXBUTTON/HTCLOSE)
  so the OS performs the actions natively — hover styles still update through
  `WM_NCMOUSEMOVE`. Control hitboxes must not nest a button inside a
  Drag-marked parent (first match in paint order wins).
- The window header (`render_header`) is DarkBot's single row: ☰ menu, ⚙
  settings, ▶/⏸ start-stop, ⌂ map, the centered title, then tick/ping/CPU/RAM
  readouts (`process_usage`: `GetProcessTimes` deltas +
  `K32GetProcessMemoryInfo`), show/hide client, Statistics, Pin
  (`HWND_TOPMOST` via `SetWindowPos`, HWND from `own_hwnd`, cached in
  `main_hwnd`) and the caption buttons. Icons are Segoe MDL2 code points.
  Disconnect and "↗ Backpage" live in the ☰ dropdown (`render_menu`), a
  window-level overlay with a dismiss backdrop. Refresh counter, SID and
  module status are painted on the map canvas itself (`MapHud.refresh_info`,
  `sid`, `module_status`): `reload_count` counts client reloads, the
  interval comes from `behaviors.refresh.interval_minutes`, and SID shows
  elapsed/total session lifetime. Active boosters (`MapHud.boosters`) come
  from `GameClient::feature_state().boosters`, polled every 10 s by the
  status monitor: Ruffle reads `BoosterProxy` natively, Flash through
  `DobotBridge.as` (rebuild with `build_bridge.ps1` then `build.ps1`), and
  the packet client rebuilds it from `AttributeBoosterUpdateCommand` (its
  timer is the soonest-expiring booster overall, not per category).
- `live_map/` follows DarkBot's drawers (`.dbref`): no grid, oval entity
  markers (`round_entities`), base discs of `radius` world units in
  `BASE_SPOTS` alpha, translucent `drawBackgrounded` stats/booster panels
  centered vertically, shield bar only when `max_shield > 0`.
- Settings nav is one scrollable column (`render_sidebar`, `#nav-scroll`):
  module list + the active module's setting groups, 210 px wide. Each
  `ModuleSettingsPage` keeps its own scroll via
  `settings-page-{descriptor.id}` — a caller-located scroll id would share
  one position across modules. Any scrollable that re-renders often (status
  tick = 1 s) needs `overflow_y_scrollbar` + a stable `.id()`, not
  `overflow_y_scroll`, or the position snaps back to top every frame.
- `client` selects `ruffle` or `packet` independently of `game_api_backend`.
  Packet sessions use the sibling `xenomit-packet-client` game-api adapter,
  authenticate from Backpage FlashVars, and resolve the current map server
  from `xml/maps.php`. The runtime's `packet_client` module translates controller calls and
  normalizes hero/collectable snapshots without changing the planner.
  Packet sessions have no child process or client window. The Ruffle
  backend preference is retained when switching client types.
- The Ruffle client window starts hidden: `hide_client_window_when_ready`
  polls `set_client_window_visible` until the spawned process owns a
  "Xenomit" window, then hides it (the window appears asynchronously after
  spawn, so a single call right after launch fails). The toolbar
  Show/Hide client button toggles visibility afterward.

- `cargo check --workspace` and `cargo test` run all three crates from this
  repo (the workspace patches `xenomit-game-api` and
  `xenomit-behavior-engine` from sibling checkouts).
- Client settings live in `%APPDATA%\XenomitBot\modules.json`; bridge/client
  log in `%APPDATA%\XenomitBot\client.log`.
- `game_api_backend` selects `native` (shared-memory frames, 150 ms tick) or
  `legacy_pipe` (AS3 reflection `STATE`/`INVOKE`/`FINVOKE`, 750 ms tick).
  Toggling it while a client runs reconnects the bridge to the same process
  (`apply_backend_change`); a running module is stopped first since its
  worker holds the old backend's client.


## Authentic Flash client

- `client = "flash"` selects the sibling player, independently of
  `game_api_backend`. On Windows that is `xenomit-flash-client` (Flash OCX).
  On Linux it is `xenomit-flash-client-linux` (Pepper Flash) and the bot always
  connects with `connect_flash_native`. The Linux host publishes the version 2
  live frame and serves reflected calls, `gameCommand`, features, and events
  on `/tmp/xenomit_flash_<pid>.sock`.
- Windows builds with the sibling's `build.ps1` and uses the bundled x64
  Flash.ocx at low quality / 60 FPS. The bot resolves `XENOMIT_FLASH_CLIENT_EXE`
  or the sibling build, passes authenticated bootstrap records over stdin, and
  connects with `connect_flash` or `connect_flash_native`. Linux resolves
  `xenomit-flash-client-linux/xenomit_flash_client` the same way.
  `XENOMIT_FLASH_PLUGIN_PATH` overrides the pinned Pepper plugin.
- Flash snapshots poll at 150 ms from a host cache; navigation refreshes at
  300 ms on ENTER_FRAME, inventory once per second; the static catalog and
  unchanged items are cached. Native Flash window mode is the default. Movement/collection and typed manager commands use
  FINVOKE and return actual frame-call results. Stale snapshots are rejected.
- Show/Hide client works for the Flash window too. The map reports actual Flash
  ENTER_FRAME FPS and capture cost, independently of controller polling rate.
- See the sibling README/docs/PERFORMANCE.md for tested behavior and remaining live
  operation coverage. Flash has no Ruffle shared-memory mapping/reward stream.

- Flash starts visible until a loaded hero/map is ready, then the controller hides
  it. Never-shown windows can stall genuine Flash/Flex startup. Runtime override:
  XENOMIT_FLASH_OCX_PATH. Ruffle retains its independent startup policy.
