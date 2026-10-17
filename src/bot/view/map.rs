//! Live map rendering and map-page helpers.

use super::*;

/// Dragging on the live map issues a navigation call at most this often.
pub(super) const MAP_NAV_DRAG_INTERVAL: Duration = Duration::from_millis(100);

/// the reference implementation's `BoosterProxy.Booster.toSimpleString`: remaining time, boost
/// percentage and category, colored by category. `age` is how long ago the
/// booster was observed, so the timer keeps counting down between polls.
pub(super) fn booster_line(
    booster: &xenomit_game_api::BoosterState,
    age: f64,
) -> Option<(String, u32)> {
    let left = booster.seconds_left.map(|seconds| seconds - age);
    if left.is_some_and(|left| left <= 0.0) {
        return None;
    }
    let time = left.map_or_else(String::new, |left| {
        let secs = left as u64;
        let text = if secs >= 86_400 {
            format!("{}d", secs / 86_400)
        } else if secs >= 3_600 {
            format!("{}h", secs / 3_600)
        } else if secs >= 60 {
            format!("{}m", secs / 60)
        } else {
            format!("{secs}s")
        };
        format!("{text} ")
    });
    let name = booster.category.to_uppercase();
    let color = match name.as_str() {
        n if n.contains("DAMAGE") || n.contains("DMG") => 0xef5350,
        n if n.contains("SHIELD") => 0x29b6f6,
        n if n.contains("REPAIR") => 0xffa726,
        n if n.contains("HONOR") || n.contains("HON") => 0xffd54f,
        n if n.contains("EXP") || n.contains("XP") => 0xab47bc,
        _ => 0x4caf50,
    };
    Some((format!("{time}{:.0}% {name}", booster.percent), color))
}

/// the reference implementation's `Time.toString`: `mm:ss` under an hour, `hh:mm:ss` beyond.
pub(super) fn format_duration(ms: u64) -> String {
    let secs = ms / 1000;
    if secs >= 3600 {
        format!(
            "{:02}:{:02}:{:02}",
            secs / 3600,
            (secs / 60) % 60,
            secs % 60
        )
    } else {
        format!("{:02}:{:02}", secs / 60, secs % 60)
    }
}

fn color_value(value: &serde_json::Value) -> Option<(u32, Option<f32>)> {
    let text = value.as_str()?.trim().trim_start_matches('#');
    if !matches!(text.len(), 6 | 8) {
        return None;
    }
    let rgb = u32::from_str_radix(&text[..6], 16).ok()?;
    let alpha = (text.len() == 8)
        .then(|| {
            u8::from_str_radix(&text[6..], 16)
                .ok()
                .map(|a| f32::from(a) / 255.0)
        })
        .flatten();
    Some((rgb, alpha))
}

fn collect_map_colors(
    value: &serde_json::Value,
    prefix: &str,
    colors: &mut BTreeMap<String, u32>,
    alphas: &mut BTreeMap<String, f32>,
) {
    let Some(object) = value.as_object() else {
        return;
    };
    for (key, child) in object {
        let path = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        if let Some((rgb, alpha)) = color_value(child) {
            colors.insert(path.clone(), rgb);
            if let Some(alpha) = alpha {
                alphas.insert(path, alpha);
            }
        } else {
            collect_map_colors(child, &path, colors, alphas);
        }
    }
}

impl BotView {
    /// Map dimensions in world units: the live frame first, then the last
    /// snapshot, then the stock 21000x13100 sector while neither has
    /// reported a usable size.
    pub(super) fn map_dimensions(&self) -> (f32, f32) {
        let map_info = self
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.map.as_ref());
        let width = self
            .live_frame
            .map
            .as_ref()
            .map(|map| f64::from(map.width))
            .or_else(|| map_info.and_then(|map| map.width))
            .filter(|width| width.is_finite() && *width > 0.0)
            .unwrap_or(21_000.0);
        let height = self
            .live_frame
            .map
            .as_ref()
            .map(|map| f64::from(map.height))
            .or_else(|| map_info.and_then(|map| map.height))
            .filter(|height| height.is_finite() && *height > 0.0)
            .unwrap_or(13_100.0);
        (width as f32, height as f32)
    }

    pub(super) fn render_map(
        &self,
        cx: &mut Context<Self>,
        interactive: bool,
        design_mode: bool,
    ) -> impl IntoElement {
        let map_info = self
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.map.as_ref());
        // Map dimensions drive the world transform; prefer the live frame
        // and fall back to the last snapshot or the stock 1-1 dimensions.
        let (map_width, map_height) = self.map_dimensions();
        // Reference style overlays painted inside the canvas: the centered
        // map name ("Loading" between maps), the left statistics column,
        // hero configuration + health bars, and the locked-target bar.
        let now_ms = statistics::now_ms();
        let map_state = live_map::map_status(&self.live_frame, now_ms);
        let mut hud = live_map::MapHud {
            title: match map_state {
                live_map::MapStatus::Active => live_map::map_title(
                    &self.live_frame,
                    map_info.and_then(|map| map.name.as_deref()),
                )
                .unwrap_or_default(),
                live_map::MapStatus::Loading => "Loading".to_owned(),
                live_map::MapStatus::Unavailable => String::new(),
            },
            config: self
                .live_frame
                .navigation
                .hero
                .configuration
                .map(|config| format!("{config}C"))
                .unwrap_or_default(),
            ..Default::default()
        };
        let tree = &self.settings.config_tree;
        let map_settings = tree.pointer("/bot_settings/map_display");
        hud.display_flags = map_settings
            .and_then(|settings| settings.get("toggle"))
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .map(str::to_owned)
            .collect();
        let mut colors = BTreeMap::new();
        let mut color_alpha = BTreeMap::new();
        if let Some(color_scheme) = map_settings.and_then(|settings| settings.get("cs")) {
            collect_map_colors(color_scheme, "", &mut colors, &mut color_alpha);
        }
        hud.colors = colors;
        hud.color_alpha = color_alpha;
        hud.zoom = map_settings
            .and_then(|settings| settings.get("map_zoom"))
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(1.0)
            .clamp(0.1, 1.0) as f32;
        hud.camera_center = self
            .live_frame
            .hero
            .as_ref()
            .map(|hero| (hero.x, hero.y))
            .unwrap_or((map_width / 2.0, map_height / 2.0));
        hud.round_entities = map_settings
            .and_then(|settings| settings.get("round_entities"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        hud.background_opacity = tree
            .pointer("/bot_settings/custom_background/opacity")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.3)
            .clamp(0.0, 1.0) as f32;
        hud.group_lines = if hud.enabled("GROUP_AREA") {
            let group = &self.live_frame.navigation.group;
            if group.active == Some(true) {
                group
                    .members
                    .iter()
                    .filter(|member| {
                        member.map_id == self.live_frame.map.as_ref().map_or(-1, |m| m.id)
                    })
                    .map(|member| {
                        format!("{}{}", if member.leader { "★ " } else { "" }, member.name)
                    })
                    .collect()
            } else if group.active == Some(false) {
                vec!["Group: solo".into()]
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };
        if tree
            .pointer("/bot_settings/other/dev_stuff")
            .and_then(serde_json::Value::as_bool)
            == Some(true)
        {
            hud.developer_details = vec![
                format!("map: {:?}", self.live_frame.map.as_ref().map(|map| map.id)),
                format!(
                    "frame: {} · entities: {}",
                    self.live_frame.frame,
                    self.live_frame.entities.len()
                ),
                format!(
                    "obstacles: {} · backend: {}",
                    self.live_frame.navigation.obstacles.len(),
                    self.settings
                        .client
                        .backend_label(self.client_backend, self.client_flash_backend)
                ),
            ];
        }
        if let Some(shared) = &self.statistics {
            if let Ok(stats) = shared.lock() {
                let rates = stats.rates(now_ms);
                hud.status = format!(
                    "{} {}",
                    if self.behavior_running {
                        "RUNNING"
                    } else {
                        "WAITING"
                    },
                    format_duration(now_ms.saturating_sub(stats.started)),
                );
                let cargo = self.hero_field(&["cargo"]);
                let cargo_max = self.hero_field(&["maxCargo"]);
                let max_deaths = if self.settings.max_deaths == 0 {
                    "∞".to_owned()
                } else {
                    self.settings.max_deaths.to_string()
                };
                hud.stats = vec![
                    format!("cre/h {}", live_map::format_amount(f64::from(rates[0]))),
                    format!("uri/h {}", live_map::format_amount(f64::from(rates[1]))),
                    format!("exp/h {}", live_map::format_amount(f64::from(rates[3]))),
                    format!("hon/h {}", live_map::format_amount(f64::from(rates[2]))),
                    match (cargo, cargo_max) {
                        (Some(current), Some(max)) => format!(
                            "cargo {}/{}",
                            live_map::format_amount(current),
                            live_map::format_amount(max)
                        ),
                        (Some(current), None) => {
                            format!("cargo {}", live_map::format_amount(current))
                        }
                        _ => "cargo -/-".to_owned(),
                    },
                    format!("death {}/{}", self.deaths, max_deaths),
                ];
            }
        }
        // `InfosDrawer.drawInfos`: `<reloads> - <since last>/<interval>`,
        // the module status lines and the SID status.
        let refresh_minutes = self
            .settings
            .behaviors
            .pointer("/refresh/interval_minutes")
            .and_then(|value| value.as_u64())
            .unwrap_or(60);
        hud.refresh_info = format!(
            "{} - {}/{}",
            self.reload_count,
            if self.behavior_running {
                format_duration(self.last_reload.elapsed().as_millis() as u64)
            } else {
                "00".to_owned()
            },
            format_duration(refresh_minutes * 60_000),
        );
        if self.behavior_running && !self.activity.is_empty() {
            hud.module_status = self.activity.lines().map(str::to_owned).collect();
        }
        let boosters_age = self.boosters_at.elapsed().as_secs_f64();
        hud.boosters = self
            .boosters
            .iter()
            .filter_map(|booster| booster_line(booster, boosters_age))
            .collect();
        if hud.enabled("SORT_BOOSTERS") {
            hud.boosters.sort_by(|left, right| left.0.cmp(&right.0));
        }
        let now_secs = now_ms / 1000;
        hud.sid =
            self.session
                .as_ref()
                .map_or_else(String::new, |session| match session.expires_at {
                    Some(end) if end <= now_secs => "EXPIRED".to_owned(),
                    Some(end) => format!(
                        "OK {}/{}",
                        format_duration(now_secs.saturating_sub(self.session_started_secs) * 1000),
                        format_duration(end.saturating_sub(self.session_started_secs) * 1000),
                    ),
                    None => "OK".to_owned(),
                });
        let hero_faction = self
            .live_frame
            .hero
            .as_ref()
            .map(|hero| hero.faction)
            .unwrap_or(-1);
        let bar_style = (
            hud.enabled("HP_SHIELD_NUM"),
            hud.color("health", 0x388e3c),
            hud.color("health", 0x26632a),
            hud.color("nano_hull", 0xd0d024),
            hud.color("shield", 0x0288d1),
            hud.color("shield", 0x015f92),
            hud.color("text", 0xf2f2f2),
        );
        let bar_of = |entity: &LiveEntity, enemy: bool| live_map::BarHud {
            name: if entity.name.trim().is_empty() {
                entity.kind.class_name().into()
            } else {
                entity.name.clone()
            },
            details: String::new(),
            enemy,
            hp: entity.hp as f32,
            max_hp: entity.max_hp as f32,
            // DarkOrbit's hp value is total durability; nanohull is the
            // yellow prefix within that total, not additional HP.
            hp_includes_nano: true,
            nano: entity.nanohull as f32,
            nano_max: entity.nanohull_max as f32,
            shield: entity.shield as f32,
            max_shield: entity.max_shield as f32,
            show_numbers: bar_style.0,
            health_color: bar_style.1,
            health_background: bar_style.2,
            nano_color: bar_style.3,
            shield_color: bar_style.4,
            shield_background: bar_style.5,
            text_color: bar_style.6,
        };
        hud.hero = self
            .live_frame
            .entities
            .iter()
            .find(|entity| entity.is_hero)
            .or_else(|| {
                let hero_id = self
                    .live_frame
                    .hero
                    .as_ref()
                    .map(|hero| hero.id)
                    .filter(|id| *id > 0)?;
                self.live_frame
                    .entities
                    .iter()
                    .find(|entity| entity.id == hero_id)
            })
            .map(|entity| bar_of(entity, false));
        hud.target_id = self
            .hero_field(&[
                "localTargetId",
                "localTargetID",
                "lockedTargetId",
                "lockedTargetID",
                // Native Flash frames sometimes lack `localTargetId`; the hero's
                // attack-trait target is the next best signal.
                "attackingTargetId",
            ])
            .or_else(|| {
                self.snapshot
                    .as_ref()?
                    .entities
                    .iter()
                    .find(|entity| {
                        entity
                            .fields
                            .get("isHero")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false)
                    })
                    .and_then(|hero| hero.fields.get("localTargetId"))
                    .and_then(|value| {
                        value
                            .as_f64()
                            .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
                    })
            })
            .map(|value| value as i32)
            .filter(|id| *id > 0);
        hud.target = hud
            .target_id
            .and_then(|id| {
                self.live_frame
                    .entities
                    .iter()
                    .find(|entity| {
                        entity.id == id
                            && matches!(
                                entity.kind,
                                LiveKind::Player | LiveKind::Npc | LiveKind::Pet | LiveKind::Hero
                            )
                    })
                    .map(|entity| bar_of(entity, live_map::is_enemy_ship(entity, hero_faction)))
                    // A live row without hp/shield data (`has_hp` unset) would
                    // draw an empty `0/0` bar; the snapshot's Flash fields may
                    // still carry the real values.
                    .filter(|bar| bar.max_hp > 0.0 || bar.max_shield > 0.0)
                    .or_else(|| {
                        let entity = self.snapshot.as_ref()?.entities.iter().find(|entity| {
                            entity
                                .id
                                .as_ref()
                                .or_else(|| entity.fields.get("id"))
                                .and_then(|value| {
                                    value
                                        .as_i64()
                                        .or_else(|| {
                                            value.as_u64().and_then(|id| i64::try_from(id).ok())
                                        })
                                        .or_else(|| {
                                            value.as_str().and_then(|text| text.parse().ok())
                                        })
                                })
                                == Some(i64::from(id))
                        })?;
                        let number = |name: &str| {
                            entity.fields.get(name).and_then(|value| {
                                value.as_f64().or_else(|| {
                                    value.as_str().and_then(|text| text.parse::<f64>().ok())
                                })
                            })
                        };
                        let enemy = entity
                            .fields
                            .get("isEnemy")
                            .and_then(|value| value.as_bool())
                            .unwrap_or(false);
                        Some(live_map::BarHud {
                            name: if entity.name.trim().is_empty() {
                                entity.class_name.clone()
                            } else {
                                entity.name.clone()
                            },
                            details: String::new(),
                            enemy,
                            hp: entity.health.unwrap_or(0.0) as f32,
                            max_hp: entity.max_health.unwrap_or(0.0) as f32,
                            hp_includes_nano: true,
                            nano: number("nanohullHP").unwrap_or(0.0) as f32,
                            nano_max: number("nanohullMaxHP").unwrap_or(0.0) as f32,
                            shield: entity.shield.unwrap_or(0.0) as f32,
                            max_shield: entity.max_shield.unwrap_or(0.0) as f32,
                            show_numbers: bar_style.0,
                            health_color: bar_style.1,
                            health_background: bar_style.2,
                            nano_color: bar_style.3,
                            shield_color: bar_style.4,
                            shield_background: bar_style.5,
                            text_color: bar_style.6,
                        })
                    })
            })
            .or_else(|| {
                // The live frame can lag the selected-target property by one
                // capture. The snapshot retains the Flash fields and can
                // still supply the bottom-right target bar.
                let id = hud.target_id?;
                let entity = self.snapshot.as_ref()?.entities.iter().find(|entity| {
                    entity
                        .id
                        .as_ref()
                        .or_else(|| entity.fields.get("id"))
                        .and_then(|value| {
                            value
                                .as_i64()
                                .or_else(|| value.as_u64().and_then(|id| i64::try_from(id).ok()))
                                .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
                        })
                        == Some(i64::from(id))
                })?;
                let number = |name: &str| {
                    entity.fields.get(name).and_then(|value| {
                        value
                            .as_f64()
                            .or_else(|| value.as_str().and_then(|text| text.parse::<f64>().ok()))
                    })
                };
                Some(live_map::BarHud {
                    name: if entity.name.trim().is_empty() {
                        entity.class_name.clone()
                    } else {
                        entity.name.clone()
                    },
                    details: String::new(),
                    enemy: entity
                        .fields
                        .get("isEnemy")
                        .and_then(|value| value.as_bool())
                        .unwrap_or(false),
                    hp: entity.health.unwrap_or(0.0) as f32,
                    max_hp: entity.max_health.unwrap_or(0.0) as f32,
                    hp_includes_nano: true,
                    nano: number("nanohullHP").unwrap_or(0.0) as f32,
                    nano_max: number("nanohullMaxHP").unwrap_or(0.0) as f32,
                    shield: entity.shield.unwrap_or(0.0) as f32,
                    max_shield: entity.max_shield.unwrap_or(0.0) as f32,
                    show_numbers: bar_style.0,
                    health_color: bar_style.1,
                    health_background: bar_style.2,
                    nano_color: bar_style.3,
                    shield_color: bar_style.4,
                    shield_background: bar_style.5,
                    text_color: bar_style.6,
                })
            })
            .map(|mut target| {
                target.details = format!(
                    "HP {} / {}  ·  Shield {} / {}  ·  ID {}",
                    live_map::format_amount(f64::from(target.hp)),
                    live_map::format_amount(f64::from(target.max_hp)),
                    live_map::format_amount(f64::from(target.shield)),
                    live_map::format_amount(f64::from(target.max_shield)),
                    hud.target_id.unwrap_or_default(),
                );
                target
            });
        let map_heading = if hud.title.is_empty() {
            "Waiting for live map data".to_owned()
        } else {
            hud.title.clone()
        };
        let player_position = self
            .live_frame
            .hero
            .as_ref()
            .map(|hero| (hero.x, hero.y))
            .or_else(|| {
                map_info.and_then(|map| map.player_position.map(|p| (p.x as f32, p.y as f32)))
            });
        let coordinate_label = player_position.map_or_else(
            || "Player position unavailable".to_owned(),
            |position| format!("SHIP  {:.0}, {:.0}", position.0, position.1),
        );
        let safe_place_count = map_info.map_or(0, |map| map.safe_places.len());
        let safe_status = map_info.map_or_else(
            || "Safe places unavailable".to_owned(),
            |map| {
                if map.in_safe_place == Some(true) {
                    "Inside a safe place".to_owned()
                } else if let Some(position) = map.player_position {
                    let nearest = map
                        .safe_places
                        .iter()
                        .map(|place| {
                            let dx = position.x - place.x;
                            let dy = position.y - place.y;
                            (place, (dx * dx + dy * dy).sqrt())
                        })
                        .min_by(|left, right| left.1.total_cmp(&right.1));
                    nearest.map_or_else(
                        || "No safe places detected".to_owned(),
                        |(place, distance)| {
                            format!(
                                "Nearest {} at {:.0}, {:.0}  ·  {:.0} units",
                                place.kind.replace('_', " "),
                                place.x,
                                place.y,
                                distance
                            )
                        },
                    )
                } else {
                    "Player position unavailable".to_owned()
                }
            },
        );
        let can_navigate = interactive
            && self.bridge.is_some()
            && (map_info.is_some() || self.live_frame.map.is_some());
        let map_id = self
            .live_frame
            .map
            .as_ref()
            .map(|map| map.id)
            .filter(|map| *map != -1)
            .or_else(|| map_info.and_then(|map| map.id).filter(|map| *map != -1))
            .or_else(|| {
                self.snapshot
                    .as_ref()
                    .and_then(|snapshot| snapshot.navigation.map_id)
                    .filter(|map| *map != -1)
            });
        hud.hovered_zone = (design_mode && self.zone_drawing)
            .then_some(map_id)
            .flatten()
            .and_then(|map_id| {
                self.zone_hovered
                    .filter(|(hovered_map, _)| *hovered_map == map_id)
                    .map(|(_, index)| index)
            });
        let can_design = design_mode && self.zone_drawing && map_id.is_some();
        let map_zones = map_id
            .and_then(|map| self.settings.map_zones.get(&map))
            .cloned()
            .unwrap_or_default();
        let zone_draft = design_mode.then(|| self.zone_draft.clone()).flatten();

        let background = hud.color("background", 0x263238);
        let mut map_canvas = div()
            .id("main-map-canvas")
            .flex_1()
            .h_full()
            .relative()
            .overflow_hidden()
            .bg(rgb(background).alpha(hud.alpha("background", 1.0)));
        if tree
            .pointer("/bot_settings/custom_background/enabled")
            .and_then(serde_json::Value::as_bool)
            == Some(true)
            && let Some(image) = tree
                .pointer("/bot_settings/custom_background/image")
                .and_then(serde_json::Value::as_str)
                .filter(|image| !image.trim().is_empty())
        {
            let path = PathBuf::from(image);
            let path = if path.is_absolute() {
                path
            } else {
                self.config_path
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new("."))
                    .join(path)
            };
            if path.is_file() {
                let opacity = hud.background_opacity;
                map_canvas = map_canvas.child(
                    gpui_kit::img(path)
                        .absolute()
                        .inset_0()
                        .size_full()
                        .opacity(opacity)
                        .object_fit(gpui_kit::ObjectFit::Cover),
                );
            }
        }
        let map_start_stop = tree
            .pointer("/bot_settings/map_display/map_start_stop")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        if can_navigate || can_design {
            map_canvas = map_canvas.on_mouse_down(
                gpui_kit::MouseButton::Left,
                cx.listener(move |this, event: &gpui_kit::MouseDownEvent, _window, cx| {
                    let (world_x, world_y) =
                        this.map_world_point(map_width, map_height, event.position);
                    if design_mode && this.zone_drawing {
                        let (world_x, world_y) =
                            this.snap_zone_point(world_x, world_y, map_width, map_height);
                        if let Some(map_id) = this
                            .live_frame
                            .map
                            .as_ref()
                            .map(|map| map.id)
                            .filter(|map| *map != -1)
                            .or_else(|| {
                                this.snapshot
                                    .as_ref()
                                    .and_then(|snapshot| snapshot.map.as_ref())
                                    .and_then(|map| map.id)
                                    .filter(|map| *map != -1)
                            })
                            .or_else(|| {
                                this.snapshot
                                    .as_ref()
                                    .and_then(|snapshot| snapshot.navigation.map_id)
                                    .filter(|map| *map != -1)
                            })
                        {
                            this.zone_drag_start = Some((map_id, world_x, world_y));
                            this.zone_draft = Some(MapZone {
                                name: "New zone".into(),
                                x: world_x,
                                y: world_y,
                                width: 0.0,
                                height: 0.0,
                                enabled: true,
                                kind: MapZoneKind::Preferred,
                            });
                            cx.notify();
                        }
                    } else if can_navigate && map_start_stop {
                        if this.behavior_running || this.behavior_starting {
                            this.stop_behavior(cx, None);
                        } else {
                            this.run_behavior(cx, None);
                        }
                    } else if can_navigate {
                        this.map_dragging = true;
                        this.map_nav_at = Instant::now();
                        this.navigate_to_map_point(world_x, world_y, cx);
                    }
                }),
            );
        }
        if can_navigate && map_start_stop && !design_mode {
            map_canvas = map_canvas.on_mouse_down(
                gpui_kit::MouseButton::Right,
                cx.listener(move |this, event: &gpui_kit::MouseDownEvent, _window, cx| {
                    let (world_x, world_y) =
                        this.map_world_point(map_width, map_height, event.position);
                    this.map_dragging = false;
                    this.navigate_to_map_point(world_x, world_y, cx);
                }),
            );
        }

        // Everything on the map is painted by a single canvas element; the
        // overlay panels below stay as regular divs on top of it.
        let mut frame = self.live_frame.clone();
        if let Some((generation, map_id, x, y, at)) = self.map_click_target {
            if generation == self.connection_generation
                && frame.map.as_ref().is_some_and(|m| m.id == map_id)
                && at.elapsed() < Duration::from_secs(30)
            {
                if let Some(hero) = frame.hero.as_mut() {
                    if (f64::from(hero.x) - f64::from(x)).hypot(f64::from(hero.y) - f64::from(y))
                        > 30.0
                    {
                        hero.dest = Some(xenomit_game_api::model::Point {
                            x: f64::from(x),
                            y: f64::from(y),
                        });
                    }
                }
            }
        }
        let safe_places = map_info
            .map(|map| map.safe_places.clone())
            .unwrap_or_default();
        let trail = self.hero_trail.clone();
        let painted_zones = map_zones.clone();
        let painted_draft = zone_draft.clone();
        let painted_bounds = Rc::clone(&self.map_canvas_bounds);
        let drag_view = cx.weak_entity();
        map_canvas = map_canvas.child(
            canvas(
                move |bounds, _window, _cx| painted_bounds.set(Some(bounds)),
                move |bounds, (), window, cx| {
                    live_map::paint_live_map(
                        window,
                        cx,
                        bounds,
                        &frame,
                        &safe_places,
                        &trail,
                        &painted_zones,
                        painted_draft.as_ref(),
                        &hud,
                    );
                    if !can_navigate && !can_design {
                        return;
                    }
                    // Per-frame window listeners keep drag retargeting alive
                    // after the pointer leaves the canvas — the same pattern
                    // scrollbars use. Cleared on the next paint, so no
                    // duplicates accumulate.
                    window.on_mouse_event({
                        let view = drag_view.clone();
                        move |event: &gpui_kit::MouseMoveEvent, phase, _window, cx| {
                            if phase != gpui_kit::DispatchPhase::Bubble {
                                return;
                            }
                            let _ = view.update(cx, |this, cx| {
                                if design_mode && this.zone_drawing {
                                    if this.zone_drag_start.is_some() {
                                        this.drag_zone_target(map_width, map_height, event, cx);
                                    } else if let Some(map_id) = map_id {
                                        let (x, y) = this.map_world_point(
                                            map_width,
                                            map_height,
                                            event.position,
                                        );
                                        let hovered = this
                                            .settings
                                            .map_zones
                                            .get(&map_id)
                                            .and_then(|zones| {
                                                zones.iter().enumerate().rev().find_map(
                                                    |(index, zone)| {
                                                        (zone.enabled
                                                            && x >= zone.x.min(zone.x + zone.width)
                                                            && x <= zone.x.max(zone.x + zone.width)
                                                            && y >= zone
                                                                .y
                                                                .min(zone.y + zone.height)
                                                            && y <= zone
                                                                .y
                                                                .max(zone.y + zone.height))
                                                        .then_some((map_id, index))
                                                    },
                                                )
                                            });
                                        if this.zone_hovered != hovered {
                                            this.zone_hovered = hovered;
                                            cx.notify();
                                        }
                                    }
                                } else {
                                    this.drag_map_target(map_width, map_height, event, cx);
                                }
                            });
                        }
                    });
                    window.on_mouse_event({
                        let view = drag_view.clone();
                        move |event: &gpui_kit::MouseUpEvent, phase, _window, cx| {
                            if phase != gpui_kit::DispatchPhase::Bubble
                                || event.button != gpui_kit::MouseButton::Left
                            {
                                return;
                            }
                            let _ = view.update(cx, |this, cx| {
                                if design_mode && this.zone_drag_start.is_some() {
                                    this.finish_zone_drawing(cx);
                                } else {
                                    this.map_dragging = false;
                                }
                            });
                        }
                    });
                },
            )
            .absolute()
            .inset_0(),
        );

        map_canvas = map_canvas.when(design_mode, |map_canvas| {
            map_canvas.child(
                div()
                    .absolute()
                    .top_4()
                    .left_4()
                    .flex()
                    .flex_col()
                    .gap(px(3.0))
                    .px(px(12.0))
                    .py(px(9.0))
                    .rounded_lg()
                    .bg(rgb(MAP_CHIP).alpha(0.88))
                    .border_1()
                    .border_color(rgb(MAP_LINE))
                    .shadow_md()
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(TEXT))
                            .child(map_heading),
                    )
                    .child(label(&coordinate_label, MUTED, 12.0))
                    .child(label(
                        if design_mode {
                            if self.zone_drawing {
                                "Drag on the map to draw a square work area"
                            } else {
                                "Work areas are saved for this map"
                            }
                        } else if can_navigate {
                            "Click or drag on the map to move"
                        } else {
                            "Connect the game to navigate the map"
                        },
                        FAINT,
                        11.0,
                    ))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(12.0))
                            .mt(px(3.0))
                            .child(legend_swatch("NPC", 0xaa4040))
                            .child(legend_swatch("Enemy", 0xd50000))
                            .child(legend_swatch("Ally", 0x29b6f6))
                            .child(legend_swatch("Box", 0xbbb830))
                            .child(legend_swatch("Ore", 0x4fc3d9)),
                    )
                    .when(design_mode, |this| {
                        this.child(
                            button(
                                "toggle-zone-drawing",
                                if self.zone_drawing {
                                    "Finish drawing"
                                } else {
                                    "＋  Draw square zone"
                                },
                                if self.zone_drawing {
                                    Btn::Selected
                                } else {
                                    Btn::Primary
                                },
                            )
                            .mt(px(5.0))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.zone_drawing = !this.zone_drawing;
                                this.zone_drag_start = None;
                                this.zone_draft = None;
                                this.zone_hovered = None;
                                cx.notify();
                            })),
                        )
                    }),
            )
        });

        map_canvas = map_canvas.when(design_mode, |map_canvas| {
            map_canvas.child(
                div()
                    .absolute()
                    .top_4()
                    .right_4()
                    .flex()
                    .flex_col()
                    .items_end()
                    .gap(px(4.0))
                    .px(px(12.0))
                    .py(px(9.0))
                    .rounded_lg()
                    .bg(rgb(MAP_CHIP).alpha(0.88))
                    .border_1()
                    .border_color(rgb(MAP_LINE))
                    .shadow_md()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .size(px(7.0))
                                    .rounded_full()
                                    .bg(rgb(if map_info.is_some() { GREEN } else { FAINT })),
                            )
                            .child(label(
                                &format!("SAFE PLACES  {safe_place_count}"),
                                TEXT,
                                12.0,
                            )),
                    )
                    .child(label(&safe_status, MUTED, 12.0))
                    .when(self.live_frame.frame > 0, |this| {
                        this.child(label(
                            &if self.settings.client == ClientKind::Flash {
                                format!(
                                    "FLASH {:.1} fps · capture {}µs · {} entities",
                                    self.live_fps,
                                    self.flash_capture_us,
                                    self.live_frame.entities.len()
                                )
                            } else if self.client_backend == ClientBackend::LegacyPipe {
                                format!(
                                    "legacy pipe · {} fps · {} entities",
                                    self.live_fps.round() as u32,
                                    self.live_frame.entities.len(),
                                )
                            } else {
                                format!(
                                    "LIVE {} fps · capture {}µs · {} entities",
                                    self.live_fps.round() as u32,
                                    self.live_frame.capture_us,
                                    self.live_frame.entities.len(),
                                )
                            },
                            MUTED,
                            11.0,
                        ))
                    })
                    .child(label(
                        if self.behavior_running {
                            "Bot running"
                        } else {
                            "Bot stopped"
                        },
                        if self.behavior_running { GREEN } else { FAINT },
                        11.0,
                    )),
            )
        });

        map_canvas
    }
}
