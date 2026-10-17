<!-- xenomit-tooling-guidelines:managed -->

# Xenomit agent guidelines

These rules apply to AI agents working in Xenomit repositories. Repo-local instructions (for example `AGENTS.md`, `CLAUDE.md`, `CONTRIBUTING.md`, or supplementary `AGENTS.local.md` / `CLAUDE.local.md` in the project root) may add requirements; when they conflict on project specifics, follow the repo-local file for that repository.

If **`AGENTS.local.md`** or **`CLAUDE.local.md`** exists in a target repository root, agents must read it as **repository-specific supplementary guidance** alongside the synced root `AGENTS.md` and `CLAUDE.md`.

## Role

- Act as an **orchestrator**: break work into clear steps, use the right tools and subagents for each step, and keep the user informed of progress and blockers.
- Prefer small, reviewable changes with a clear purpose.

## Preserve user work

## Settings tree

The General tab is a Legacy-style settings tree (`src/bot/config_tree/`, docs in
`docs/SETTINGS_TREE.md`): a typed schema ported from Legacy's `Config.java`, profile files
(`config.json`, `configs/<name>.json`) beside `modules.json`, and `bridge.rs`, which pushes the
options the runtime understands into `BotSettings`. Options the runtime cannot honour yet are
listed with the needed client API / engine / runtime changes in `docs/SETTINGS_GAPS.md`
(regenerate with `cargo test regenerate_docs -- --ignored`). The old per-module pages live under
the Modules tab.

## Behavior runtime

## Before editing

- Read **repository-specific** agent and contributor instructions in the repo you are changing before modifying code or config, including `AGENTS.local.md` and `CLAUDE.local.md` when present.
- Match existing conventions (naming, structure, tooling) in that repository.

## GitHub

- Use the **`gh` CLI** for GitHub operations (issues, pull requests, checks, releases) when working in this environment.
- Before merging, **verify the relevant checks** (CI, required reviews, branch protection) for that pull request or branch.
- Do not weaken branch protections or bypass policy without explicit owner approval.

## Dependencies across repositories

- **Pin cross-repository dependencies** to versioned **GitHub Releases or git tags**, not sibling directories, local filesystem paths, or unpinned `main`/`master` references—unless the consuming repo documents an exception.
- **Same-repository workspace paths** (multiple packages or apps inside one repo) are allowed when that repo's own docs prescribe them.

## Delegation

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
- The window header (`render_header`) is Legacy's single row: ☰ menu, ⚙
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
- `live_map/` follows Legacy's drawers (`.dbref`): no grid, oval entity
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

- In-process Flash (`bot_settings.api_config.flash_in_process`, or
  `XENOMIT_FLASH_INPROC=1|0` to override; Windows only) loads
  `xenomit_flash.dll` through `xenomit_game_api::start_flash_inproc` instead of
  spawning the exe, the way Legacy hosts InProcessPlayer. There is no child process
  or pipe: `flash_host` holds the `FlashInProcClient`, `game_pid` is this
  process, and host log lines reach client.log through `set_flash_log_sink`.
  Window show/hide goes through `set_game_window_visible` (host `set_window`),
  backend switches call `set_backend` on the same client, and stop, reload and
  close call `shutdown()` before anything starts a new host (one per process).
  The DLL is found via `XENOMIT_FLASH_DLL`, beside the bot exe, or the sibling
  `xenomit-flash-client/build`. A Flash crash ends the bot; the exe remains
  the isolated default. `performance.max_fps` caps Flash in both modes.
- Building against unpublished sibling branches (for example game-api
  `inprocess-inproc`): `cargo build --config scripts/local-siblings.toml`. Do not
  commit the Cargo.lock changes that produces.
- Flash starts visible until a loaded hero/map is ready, then the controller hides
  it. Never-shown windows can stall genuine Flash/Flex startup. Runtime override:
  XENOMIT_FLASH_OCX_PATH. Ruffle retains its independent startup policy.
