//! Settings pages, sidebar and the main content router.

use super::*;

impl BotView {
    pub(super) fn select_settings_category(&mut self, category: &str, cx: &mut Context<Self>) {
        self.settings_category = category.to_owned();
        cx.notify();
    }

    pub(super) fn settings_category_destination(&self) -> String {
        let find_module = |query: &str| {
            self.module_descriptors
                .iter()
                .find(|descriptor| {
                    descriptor.id.to_ascii_lowercase().contains(query)
                        || descriptor.name.to_ascii_lowercase().contains(query)
                })
                .map(|descriptor| descriptor.id.clone())
        };
        match self.settings_category.as_str() {
            "General" => self.settings.active_module.clone(),
            "Collect" => find_module("collect").unwrap_or_else(|| "__targets".into()),
            "NPC killer" => find_module("npc").unwrap_or_else(|| "__targets".into()),
            "PET" => find_module("pet").unwrap_or_else(|| "__behaviors".into()),
            "Group" => find_module("group").unwrap_or_else(|| "__behaviors".into()),
            "Miscellaneous" => "__behaviors".into(),
            "Plugins" => "__plugins".into(),
            _ => String::new(),
        }
    }

    /// the reference implementation's compact settings category rail.
    pub(super) fn render_settings_sidebar(
        &self,
        cx: &mut Context<Self>,
        tabs: bool,
        button_height: f32,
        button_size: f32,
    ) -> impl IntoElement {
        const CATEGORIES: [&str; 8] = [
            "General",
            "Collect",
            "NPC killer",
            "PET",
            "Group",
            "Miscellaneous",
            "Bot settings",
            "Plugins",
        ];

        let mut sidebar = div()
            .id("settings-category-rail")
            .flex_shrink_0()
            .flex()
            .gap(px(1.0))
            .bg(rgb(PANEL_DARK));
        if tabs {
            sidebar = sidebar
                .w_full()
                .h(px(button_height + 8.0))
                .items_center()
                .px(px(8.0))
                .border_b_1()
                .border_color(rgb(LINE));
        } else {
            sidebar = sidebar
                .w(px(138.0))
                .h_full()
                .flex_col()
                .py(px(4.0))
                .border_r_1()
                .border_color(rgb(LINE));
        }
        for title in CATEGORIES {
            let active = self.settings_category == title;
            let title_owned = title.to_owned();
            let mut item = div()
                .id(format!(
                    "settings-category-{}",
                    title.to_ascii_lowercase().replace(' ', "-")
                ))
                .flex_shrink_0()
                .flex()
                .items_center()
                .text_size(px(9.5 + button_size * 0.5))
                .text_color(rgb(if active { TEXT } else { MUTED }))
                .bg(rgb(if active { BLUE_TINT } else { PANEL_DARK }))
                .hover(move |style| style.bg(rgb(if active { BLUE_TINT } else { PANEL_HOVER })))
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.select_settings_category(&title_owned, cx);
                }))
                .child(title);
            if tabs {
                item = item.h(px(button_height)).px(px(5.0 + button_size));
            } else {
                item = item.w_full().h(px(button_height)).px(px(9.0));
            }
            sidebar = sidebar.child(item);
        }
        sidebar.when(self.catalog_loading, |sidebar| {
            sidebar.child(
                div()
                    .px(px(8.0))
                    .py(px(5.0))
                    .text_size(px(10.0))
                    .text_color(rgb(FAINT))
                    .child("Loading modules…"),
            )
        })
    }

    pub(super) fn render_players(&self) -> impl IntoElement {
        let Some(snapshot) = &self.snapshot else {
            return div()
                .id("players-scroll")
                .flex_1()
                .h_full()
                .overflow_y_scrollbar()
                .p(px(18.0))
                .flex()
                .flex_col()
                .child(label("Connect the game client and refresh its snapshot to list visible players and entities.", MUTED, 13.0));
        };
        let mut rows = div().flex().flex_col().gap(px(4.0)).mt(px(12.0));
        for entity in snapshot.entities.iter().take(30) {
            let pair = |cur: Option<f64>, max: Option<f64>| match (cur, max) {
                (Some(cur), Some(max)) => Some(format!(
                    "{}/{}",
                    live_map::format_amount(cur),
                    live_map::format_amount(max)
                )),
                (Some(cur), None) => Some(live_map::format_amount(cur)),
                _ => None,
            };
            let vitals = [
                pair(entity.health, entity.max_health).map(|hp| format!("hp {hp}")),
                pair(entity.shield, entity.max_shield).map(|shield| format!("shd {shield}")),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join("  ·  ");
            let right = if vitals.is_empty() {
                format!("{:.0}, {:.0}", entity.position.x, entity.position.y)
            } else {
                format!(
                    "{}  ·  {:.0}, {:.0}",
                    vitals, entity.position.x, entity.position.y
                )
            };
            rows = rows.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(12.0))
                    .py(px(7.0))
                    .rounded_md()
                    .bg(rgb(PANEL))
                    .border_1()
                    .border_color(rgb(LINE_SOFT))
                    .child(label(
                        &format!("{}  ·  {}", entity.name, entity.class_name),
                        TEXT,
                        12.0,
                    ))
                    .child(
                        div()
                            .font_family("Consolas")
                            .text_size(px(11.5))
                            .text_color(rgb(MUTED))
                            .child(right),
                    ),
            );
        }
        div()
            .id("players-scroll")
            .flex_1()
            .h_full()
            .overflow_y_scrollbar()
            .p(px(18.0))
            .flex()
            .flex_col()
            .child(section(&format!(
                "VISIBLE ENTITIES  ·  {}",
                snapshot.entities.len()
            )))
            .child(rows)
    }

    pub(super) fn render_config(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_1()
            .h_full()
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .gap(px(6.0))
            .overflow_y_scrollbar()
            .bg(rgb(BG))
            .child(self.render_account_panel(cx))
            .child(self.render_bridge_danger_zone(cx))
    }

    pub(super) fn render_targets_page(&self) -> impl IntoElement {
        let page = self.module_pages.get("__targets").cloned();
        div()
            .flex_1()
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .gap(px(8.0))
            .p(px(18.0))
            .bg(rgb(BG))
            .child(section("SHARED TARGET LISTS"))
            .child(label(
                "These lists are shared by every objective. New NPCs, boxes and resources appear on first encounter; switch entries off to ignore them. Extra flags are reserved for the next definitions.",
                MUTED,
                12.0,
            ))
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.0))
                    .when_some(page, |this, page| this.child(page)),
            )
    }

    pub(super) fn render_design_page(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let current_map = self
            .live_frame
            .map
            .as_ref()
            .map(|map| map.id)
            .filter(|map| *map != -1)
            .or_else(|| {
                self.snapshot
                    .as_ref()
                    .and_then(|snapshot| snapshot.map.as_ref())
                    .and_then(|map| map.id)
                    .filter(|map| *map != -1)
            })
            .or_else(|| {
                self.snapshot
                    .as_ref()
                    .and_then(|snapshot| snapshot.navigation.map_id)
                    .filter(|map| *map != -1)
            });
        let zones = current_map
            .and_then(|map| self.settings.map_zones.get(&map))
            .cloned()
            .unwrap_or_default();
        let mut zone_rows = div().flex().flex_col().gap(px(4.0));
        if zones.is_empty() {
            zone_rows = zone_rows.child(label(
                "No zones on this map yet. Use Draw square zone on the map, then drag an area.",
                FAINT,
                12.0,
            ));
        }
        for (index, zone) in zones.iter().enumerate() {
            let map_id = current_map.unwrap_or_default();
            let is_preferred = zone.kind == MapZoneKind::Preferred;
            zone_rows = zone_rows.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(10.0))
                    .px(px(10.0))
                    .py(px(6.0))
                    .bg(rgb(PANEL))
                    .border_1()
                    .border_color(rgb(LINE_SOFT))
                    .child(div().flex_1().child(label(
                        &format!(
                            "{}  ·  x {:.0}, y {:.0}  ·  {:.0} × {:.0}",
                            zone.name, zone.x, zone.y, zone.width, zone.height
                        ),
                        if zone.enabled {
                            if is_preferred { 0x76d69a } else { 0xf07878 }
                        } else {
                            MUTED
                        },
                        12.0,
                    )))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .child(
                                button(
                                    format!("zone-kind-{map_id}-{index}"),
                                    if is_preferred { "Avoided" } else { "Preferred" },
                                    if is_preferred {
                                        Btn::Ghost
                                    } else {
                                        Btn::Selected
                                    },
                                )
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        this.toggle_map_zone_kind(map_id, index, cx)
                                    },
                                )),
                            )
                            .child(
                                button(
                                    format!("toggle-zone-{map_id}-{index}"),
                                    if zone.enabled { "Enabled" } else { "Paused" },
                                    if zone.enabled {
                                        Btn::Selected
                                    } else {
                                        Btn::Ghost
                                    },
                                )
                                .on_click(cx.listener(
                                    move |this, _, _, cx| this.toggle_map_zone(map_id, index, cx),
                                )),
                            )
                            .child(
                                button(
                                    format!("delete-zone-{map_id}-{index}"),
                                    "Remove",
                                    Btn::Ghost,
                                )
                                .on_click(cx.listener(
                                    move |this, _, _, cx| this.remove_map_zone(map_id, index, cx),
                                )),
                            ),
                    ),
            );
        }
        div()
            .flex_1()
            .h_full()
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .gap(px(8.0))
            .p(px(14.0))
            .bg(rgb(BG))
            .child(div().flex().items_center().justify_between()
                .child(section("MAP ZONES"))
                .child(label(&format!("{} zones  ·  green preferred  ·  red avoided", zones.len()), MUTED, 11.0)))
            .child(label(
                "Draw areas on the current map. Preferred zones attract activity; avoided zones are excluded from targeting and navigation for every module.",
                MUTED,
                12.0,
            ))
            .child(div().flex_1().min_h(px(0.0)).child(self.render_map(cx, false, true)))
            .child(div().flex().gap(px(10.0)).items_center().child(zone_rows))
    }

    pub(super) fn render_main_content(&self, cx: &mut Context<Self>) -> AnyElement {
        self.render_map(cx, true, false).into_any_element()
    }

    /// Tabs plus the active tab's page, shown in the settings window.
    pub(super) fn render_settings_body(&self, cx: &mut Context<Self>) -> AnyElement {
        let settings_body: AnyElement = if self.top_tab == "statistics" {
            self.render_statistics(cx).into_any_element()
        } else if self.top_tab == "players" {
            self.render_players().into_any_element()
        } else if self.top_tab == "config" {
            self.render_config(cx).into_any_element()
        } else if self.top_tab == "map_zones" {
            self.render_design_page(cx).into_any_element()
        } else if self.top_tab == "targets" {
            self.render_targets_page().into_any_element()
        } else if self.top_tab == "modules" {
            self.render_general_page(cx).into_any_element()
        } else {
            div()
                .flex_1()
                .w_full()
                .min_h(px(0.0))
                .child(self.config_page.clone())
                .into_any_element()
        };

        div()
            .flex_1()
            .h_full()
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .bg(rgb(BG))
            .child(
                div()
                    .h(px(34.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(8.0))
                    .bg(rgb(TOP))
                    .border_b_1()
                    .border_color(rgb(LINE))
                    .child(self.render_top_tabs(cx))
                    .child(
                        button("close-settings", "×  Close", Btn::Ghost)
                            .h(px(26.0))
                            .on_click(
                                cx.listener(|this, _, _, cx| this.toggle_settings_window(cx)),
                            ),
                    ),
            )
            .child(settings_body)
            .into_any_element()
    }

    pub(super) fn render_general_page(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let category = self.settings_category.as_str();
        let destination = self.settings_category_destination();
        let (subtitle, title) = match category {
            "General" => ("Choose an objective or shared settings", "SETTINGS PAGE"),
            "Collect" => ("Configure collection behavior and targets", "COLLECT"),
            "NPC killer" => ("Configure NPC combat and target selection", "NPC KILLER"),
            "PET" => ("Configure PET related behavior", "PET"),
            "Group" => ("Configure group behavior", "GROUP"),
            "Miscellaneous" => ("Shared behavior settings", "MISCELLANEOUS"),
            "Bot settings" => ("Client and controller preferences", "BOT SETTINGS"),
            "Plugins" => ("Enable or disable behavior plugins", "PLUGINS"),
            _ => ("Choose a settings category", "SETTINGS PAGE"),
        };

        let mut heading = div()
            .h(px(58.0))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(12.0))
            .px(px(12.0))
            .bg(rgb(0x3d4143))
            .border_b_1()
            .border_color(rgb(LINE_SOFT))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(3.0))
                    .child(label(title, MUTED, 10.5))
                    .when(
                        self.settings
                            .config_tree
                            .pointer("/bot_settings/other/show_instructions")
                            .and_then(serde_json::Value::as_bool)
                            .unwrap_or(true),
                        |heading| heading.child(label(subtitle, FAINT, 11.0)),
                    ),
            );
        if category == "General" {
            let mut module_picker = div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .flex_shrink_0()
                .child(label("Module", TEXT, 12.0));
            if let Some(selector) = &self.settings_module_selector {
                module_picker = module_picker.child(
                    Select::new(selector)
                        .xsmall()
                        .w(px(250.0))
                        .h(px(25.0))
                        .px(px(7.0))
                        .bg(rgb(PANEL_DARK))
                        .border_1()
                        .border_color(rgb(LINE))
                        .rounded(px(1.0))
                        .text_size(px(11.5))
                        .text_color(rgb(TEXT)),
                );
            } else {
                module_picker = module_picker.child(label(
                    if self.catalog_loading {
                        "Loading modules…"
                    } else {
                        "No modules available"
                    },
                    FAINT,
                    11.0,
                ));
            }
            heading = heading.child(module_picker);
        }

        let mut page_content = div()
            .id("settings-page-content")
            .flex_1()
            .w_full()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .overflow_hidden();
        if category == "Bot settings" {
            page_content = page_content.child(self.render_config(cx));
        } else if let Some(page) = self.module_pages.get(&destination) {
            page_content = page_content.child(page.clone());
        } else if let Some(error) = &self.catalog_error {
            page_content = page_content.child(
                div()
                    .flex_1()
                    .p(px(12.0))
                    .child(label(error, 0xf0b9b9, 12.0)),
            );
        } else if self.catalog_loading {
            page_content = page_content.child(div().flex_1().p(px(12.0)).child(label(
                "Loading settings…",
                MUTED,
                12.0,
            )));
        } else {
            page_content = page_content.child(div().flex_1().p(px(12.0)).child(label(
                "No settings are available in this category.",
                MUTED,
                12.0,
            )));
        }

        let tabs = self
            .settings
            .config_tree
            .pointer("/bot_settings/bot_gui/config_tree_tabs")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true);
        let button_size = self
            .settings
            .config_tree
            .pointer("/bot_settings/bot_gui/button_size")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(4.0)
            .clamp(1.0, 20.0) as f32;
        let button_height = 20.0 + button_size * 1.75;
        let sidebar = self.render_settings_sidebar(cx, tabs, button_height, button_size);
        let pane = div()
            .id("settings-page-pane")
            .flex_1()
            .h_full()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(rgb(PANEL))
            .child(heading)
            .child(page_content);
        let mut root = div()
            .flex_1()
            .w_full()
            .h_full()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .flex()
            .bg(rgb(PANEL));
        if tabs {
            root = root.flex_col().child(sidebar).child(pane);
        } else {
            root = root.flex_row().child(sidebar).child(pane);
        }
        root
    }
}
