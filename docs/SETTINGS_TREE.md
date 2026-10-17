# Settings tree

The **General** tab is a port of Legacy's `AdvancedConfig` (see `legacy-settings-map.md` in the
workspace root for the reference analysis). The old per-module pages remain under the **Modules**
tab (module settings, behaviors, plugins).

Code: `src/bot/config_tree/`.

| File | Legacy counterpart | Role |
|---|---|---|
| `schema.rs` | `@Option`, `@Number`, `@Percentage`, `@Dropdown`, `@Table`, `@Tag`, `@Visibility`, `ValueHandler.validate` | Typed tree: `Kind` (the annotation), defaults, `Level`, translations, clamping. |
| `legacy.rs` | `Config.java` | The tree itself: every Legacy group and option with its default, bounds and level, plus a `xenomit` group. |
| `legacy_en.properties` | `strings_en.properties` (`config.*`) | Labels and descriptions, looked up by `config.<path>` / `.desc`. |
| `store.rs` | `ConfigManager` | Values, profiles (`config.json`, `configs/<name>.json`), `_old` backups, load-failure guard. |
| `filter.rs` | `TreeFilter` | Search (label, description, path) and visibility level. |
| `edit.rs` | editors | Text <-> value conversion for text-backed editors. |
| `page.rs`, `page/render.rs` | `AdvancedConfig`, `ConfigTree`, `*Editor` | The GPUI page. |
| `bridge.rs` | `ConfigEntity`, `ConfigHandler` | Pushes runtime-backed options into `BotSettings`; seeds a new profile from it. |
| `gaps.rs` | — | Catalogue of what the runtime still lacks (see `SETTINGS_GAPS.md`). |

Files are written to `%APPDATA%\XenomitBot` next to `modules.json`: `config.json` (default
profile), `configs/<name>.json`, `config_old.json` backups and `config_profile.txt` (last profile).
Only explicitly set values are written, plus a reserved `_ui` object (visibility level, last tab);
unknown keys survive a save.

## UI

- **Left**: group tree (General, Collect, NPC killer, PET, Group, Miscellaneous, Bot settings,
  Extra actions, Xenomit; nested groups expand). While searching it shows only groups with hits
  and an "All matches" entry.
- **Toolbar**: search, visibility level (Basic / Intermediate / Advanced / Developer, remembered),
  support filter (All / Working / Planned) and profile select / New / Copy / Delete.
- **Right**: the selected group's options and nested groups (collapsible, "Reset group").
  Each option shows its description, a reset arrow when it differs from the default, and a status
  chip. Clicking the chip shows the runtime target, or the gap and what it needs.
- **Editors**: checkbox, number with −/+ and optional enable checkbox (`@Number.Disabled`),
  percentage, key-capture, text, dropdown (searchable for maps), multi-select chips, percent range,
  ship config (Config 1/2 + formation key), player tag, colour with swatch, condition, JSON and
  tables (filter, add, remove, per-column editors, NPC extra-flag chips).

Beyond Legacy: status chips and filter, per-option and per-group reset, remembered level, a
`xenomit` group exposing the engine options Legacy has no equivalent for (pickup radius, ammo
and ability item ids, tick override, API backends, behavior-config override), and a generated
option reference (`SETTINGS_OPTIONS.md`).

## Runtime support

Every option declares one of:

- **live** — `bridge::apply` writes it into `BotSettings`, and from there into the active module's
  `ModuleConfig`.
- **partial** — written, but the runtime honours only part of it (named gap).
- **planned** — shown, validated and saved, but nothing reads it yet (named gap).

A test requires the set of live/partial options to equal `bridge::BRIDGED`, every gap id to exist
in `gaps.rs` and `SETTINGS_GAPS.md`, and every gap to be referenced. Further tests check that the
engine's own schemas accept the bridged defaults and the minimum and maximum of every bridged
number.

### How values flow

- Edit in the tree -> `ConfigStore::set` (clamps like Legacy) -> save -> `ConfigChanged` ->
  `BotView::apply_config_tree` -> `bridge::apply` -> `save_settings` (which also updates a running
  module through `engine.update`).
- **Only the active module's payload is written.** Other modules keep their values until they are
  activated.
- **Switching module.** Picking another module in the tree (or in the Modules tab) makes the tree
  *adopt* that module's own settings (`bridge::import`) instead of overwriting them. The switch is
  refused while a run is active.
- **Seeding.** A profile with no file is seeded once the module catalog is known: values come from
  the active module's payload with the descriptor's defaults filled in, so untouched options take
  the engine's value, not Legacy's. Nothing is written to `BotSettings` before that.
- **Edits made on the Modules tab** re-import into the tree (`sync_config_tree`), so the two never
  disagree.
- **Tables.** `loot.npc_infos` and `collect.box_infos` are the authority for `BotSettings.npcs` /
  `boxes` (their `kill` / `collect` columns). Names seen in game are added as rows
  (`observe_names`).
- Ranges wider than the engine's are clamped at the bridge (`max_deaths` 9999 -> 1000).

## Adding an option

1. Add `opt("name", kind, default)` to the right group in `legacy.rs`, with `.lvl(..)` if it is not
   Basic and `.sup(live(..) | partial(..) | gap("ID"))`.
2. Add `config.<path>` and `config.<path>.desc` to `legacy_en.properties` (or `.label()/.desc()`).
3. If live/partial: add it to `bridge::BRIDGED` plus `DIRECT`, `FLAGS` or a custom branch in
   `apply` and `import`.
4. If it needs a new gap, add an entry to `gaps.rs`.
5. `cargo test regenerate_docs -- --ignored` to refresh `SETTINGS_GAPS.md` and
   `SETTINGS_OPTIONS.md`, then `cargo test`.

## Known limits

- The GPUI page has been compiled and started, and the logic is covered by unit tests, but its
  layout has not been checked on screen (the settings window needs an authenticated session).
- Hotkeys use a key-capture box (click, press the key; Backspace/Delete clears, Esc cancels). Keys are stored as one character or a name (`F1`, `Enter`, `Up`, ...; `config_tree/keys.rs`). Modifier combinations are not captured.
- Table cells for numbers/text/keys are inputs; there is no drag-resize of columns.
- `legacy_en.properties` is copied from the Legacy reference checkout. Confirm its licence
  allows redistribution before publishing.
