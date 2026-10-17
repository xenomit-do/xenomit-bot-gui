use super::*;
use xenomit_game_api::ObstacleKind;

pub(super) fn obstacle_color_key(kind: ObstacleKind) -> &'static str {
    match kind {
        ObstacleKind::NoAccess => "safety",
        ObstacleKind::Mine => "mines",
        ObstacleKind::Cbs => "bases",
        ObstacleKind::Radiation => "radiation",
    }
}

pub(super) fn obstacle_color_key(kind: ObstacleKind) -> &'static str {
    match kind {
        ObstacleKind::NoAccess => "safety",
        ObstacleKind::Mine => "mines",
        ObstacleKind::Cbs => "bases",
        ObstacleKind::Radiation => "radiation",
    }
}

/// Paint one full map frame into `bounds` the way the reference implementation's `MapDrawer`
/// does: the stretched map on a flat background, portals, bases and battle
/// stations, then dynamic entities (oval markers), the hero trail/path and
/// hero, and finally the info/overlay text — refresh counter, module status,
/// SID, the centered map name and run status, the statistics and booster
/// panels, and the hero/target health bars.
pub fn paint_live_map(
    window: &mut Window,
    cx: &mut App,
    bounds: Bounds<Pixels>,
    frame: &LiveFrame,
    safe_places: &[SafePlace],
    trail: &HeroTrail,
    zones: &[MapZone],
    draft_zone: Option<&MapZone>,
    hud: &MapHud,
) {
    let (center_x, center_y) = (
        f32::from(bounds.origin.x) + f32::from(bounds.size.width) / 2.0,
        f32::from(bounds.origin.y) + f32::from(bounds.size.height) / 2.0,
    );

    if let Some(map) = &frame.map {
        let (map_width, map_height) = (map.width, map.height);
        if map_width > 0.0 && map_height > 0.0 {
            let project = |x: f32, y: f32| {
                world_to_screen_zoom(
                    bounds,
                    map_width,
                    map_height,
                    x,
                    y,
                    hud.camera_center,
                    hud.zoom,
                )
            };
            let x_scale = f32::from(bounds.size.width) / map_width;
            let y_scale = f32::from(bounds.size.height) / map_height;

            let mut paint_zone = |zone: &MapZone,
                                  color: u32,
                                  alpha: f32,
                                  border_color: u32,
                                  border_alpha: f32,
                                  border: f32| {
                if zone.width <= 0.0 || zone.height <= 0.0 {
                    return;
                }
                let top_left = project(zone.x as f32, zone.y as f32);
                let bottom_right =
                    project((zone.x + zone.width) as f32, (zone.y + zone.height) as f32);
                let (x1, y1) = (f32::from(top_left.x), f32::from(top_left.y));
                let (x2, y2) = (f32::from(bottom_right.x), f32::from(bottom_right.y));
                let zone_bounds = Bounds::new(
                    point(px(x1.min(x2)), px(y1.min(y2))),
                    size(px((x2 - x1).abs()), px((y2 - y1).abs())),
                );
                window.paint_quad(fill(zone_bounds, rgb(color).alpha(alpha)));
                window.paint_quad(
                    outline(
                        zone_bounds,
                        rgb(border_color).alpha(border_alpha),
                        BorderStyle::Solid,
                    )
                    .border_widths(px(border)),
                );
            };
            if hud.enabled("ZONES") {
                for (index, zone) in zones.iter().enumerate().filter(|(_, zone)| zone.enabled) {
                    if hud.hovered_zone == Some(index) {
                        let color = hud.color("zone_editor.hovering", 0x0096c8);
                        paint_zone(
                            zone,
                            color,
                            hud.alpha("zone_editor.hovering", 0.20),
                            color,
                            0.9,
                            2.0,
                        );
                        continue;
                    }
                    let (key, fallback) =
                        if zone.kind == xenomit_bot_runtime::settings::MapZoneKind::Avoided {
                            ("avoid", COLOR_AVOID_ZONE)
                        } else {
                            ("prefer", COLOR_ZONE)
                        };
                    paint_zone(
                        zone,
                        hud.color(key, fallback),
                        hud.alpha(key, 0.16),
                        hud.color("zone_lines", 0x808080),
                        hud.alpha("zone_lines", 0.5),
                        1.5,
                    );
                }
                if let Some(zone) = draft_zone {
                    let border = hud.color("zone_editor.selecting", COLOR_ZONE_DRAFT);
                    paint_zone(
                        zone,
                        hud.color("zone_editor.zone", COLOR_ZONE_DRAFT),
                        hud.alpha("zone_editor.zone", 0.2),
                        border,
                        hud.alpha("zone_editor.selecting", 0.9),
                        2.0,
                    );
                }
            }

            // Paint the native obstacle outlines used by the route planner.
            if frame.navigation.map_id == Some(map.id) {
                for obstacle in &frame.navigation.obstacles {
                    let points = obstacle_outline(&obstacle.shape);
                    if points.len() < 3 {
                        continue;
                    }
                    let mut border = PathBuilder::stroke(px(1.2));
                    let mut area = PathBuilder::fill();
                    let color_key = obstacle_color_key(obstacle.kind);
                    let color = hud.color(color_key, hud.color("radiation", 0xe06055));
                    let alpha = hud.alpha(
                        color_key,
                        if obstacle.kind == ObstacleKind::NoAccess {
                            0.25
                        } else {
                            0.20
                        },
                    );
                    for (index, (x, y)) in points.iter().enumerate() {
                        let point = project(*x as f32, *y as f32);
                        if index == 0 {
                            border.move_to(point);
                            area.move_to(point);
                        } else {
                            border.line_to(point);
                            area.line_to(point);
                        }
                    }
                    let first = project(points[0].0 as f32, points[0].1 as f32);
                    border.line_to(first);
                    area.line_to(first);
                    if !obstacle.inverted
                        && let Ok(path) = area.build()
                    {
                        window.paint_path(path, rgb(color).alpha(alpha));
                    }
                    if let Ok(path) = border.build() {
                        window.paint_path(
                            path,
                            rgb(hud.color("zone_lines", color))
                                .alpha(hud.alpha("zone_lines", 0.85)),
                        );
                    }
                }
            }

            let (hero_faction, hero_clan) = frame
                .hero
                .as_ref()
                .map(|hero| (hero.faction, hero.clan))
                .unwrap_or((-1, -1));
            let has_live_station = frame
                .entities
                .iter()
                .any(|entity| entity.kind == LiveKind::Station);
            let group_ids: std::collections::BTreeSet<i32> = frame
                .navigation
                .group
                .members
                .iter()
                .map(|member| member.id)
                .collect();

            // Constant entities, as `ConstantEntitiesDrawer`: base discs
            // (`BASE_SPOTS`, a translucent ellipse the size of the base's
            // world radius), portals, and battle stations.
            for place in safe_places {
                let center = project(place.x as f32, place.y as f32);
                match place.kind.as_str() {
                    "portal" => paint_oval(
                        window,
                        center,
                        12.0,
                        rgb(hud.color("portals", COLOR_PORTAL)).into(),
                        false,
                    ),
                    "base" if place.radius > 0.0 => {
                        let radius = place.radius as f32;
                        paint_ellipse(
                            window,
                            center,
                            radius * x_scale,
                            radius * y_scale,
                            tinted(
                                hud.color("base_spots", COLOR_BASE),
                                hud.alpha("base_spots", ALPHA_BASE_SPOT),
                            ),
                        );
                    }
                    "battle_station" if !has_live_station => {
                        paint_marker(
                            window,
                            center,
                            10.0,
                            rgb(hud.color("allies", COLOR_ALLY)).into(),
                            true,
                            hud.round_entities,
                        );
                        if !place.name.is_empty() {
                            paint_centered(
                                window,
                                cx,
                                &place.name,
                                f32::from(center.x),
                                f32::from(center.y) - 10.0,
                                13.0,
                                hud.color("allies", COLOR_ALLY),
                            );
                        }
                    }
                    _ => {}
                }
            }
            for entity in &frame.entities {
                if entity.kind == LiveKind::Station {
                    let center = project(entity.x, entity.y);
                    let color = if entity.max_hp == 0 && entity.hp == 0 {
                        hud.color("meteroid", COLOR_METEROID)
                    } else if is_enemy_ship(entity, hero_faction) {
                        hud.color("enemies", COLOR_ENEMY)
                    } else {
                        hud.color("allies", COLOR_ALLY)
                    };
                    paint_marker(
                        window,
                        center,
                        10.0,
                        rgb(color).into(),
                        true,
                        hud.round_entities,
                    );
                    if !entity.name.is_empty() && hud.enabled("USERNAMES") {
                        paint_centered(
                            window,
                            cx,
                            &entity.name,
                            f32::from(center.x),
                            f32::from(center.y) - 10.0,
                            13.0,
                            color,
                        );
                    }
                }
            }
            for entity in &frame.entities {
                if entity.kind != LiveKind::Portal {
                    continue;
                }
                let center = project(entity.x, entity.y);
                paint_oval(
                    window,
                    center,
                    12.0,
                    rgb(hud.color("portals", COLOR_PORTAL)).into(),
                    false,
                );
                if let Some(label) = portal_label(frame, entity) {
                    paint_centered(
                        window,
                        cx,
                        &label,
                        f32::from(center.x),
                        f32::from(center.y) - 8.0,
                        13.0,
                        hud.color("portals", COLOR_PORTAL),
                    );
                }
            }

            // Destination lines of moving ships (`SHOW_DESTINATION`).
            for entity in &frame.entities {
                if hud.enabled("SHOW_DESTINATION")
                    && matches!(
                        entity.kind,
                        LiveKind::Player | LiveKind::Npc | LiveKind::Pet
                    )
                    && let Some(dest) = entity.dest
                {
                    let mut line = PathBuilder::stroke(px(1.0));
                    line.move_to(project(entity.x, entity.y));
                    line.line_to(project(dest.x as f32, dest.y as f32));
                    if let Ok(path) = line.build() {
                        window.paint_path(path, rgb(hud.color("going", COLOR_GOING)));
                    }
                }
            }

            // Dynamic entities as ovals (`round_entities`): the size grows
            // by one when filled, then two for the oval. Ships are hollow;
            // wanted resources and mines are filled.
            let paint_entities = |window: &mut Window, cx: &mut App, kinds: &[LiveKind]| {
                for entity in &frame.entities {
                    if !kinds.contains(&entity.kind)
                        || hud.target_id == Some(entity.id)
                        || (entity.kind == LiveKind::Pet && !hud.enabled("SHOW_PET"))
                    {
                        continue;
                    }
                    let center = project(entity.x, entity.y);
                    let (size, mut color) = marker_style(entity, hero_faction, hero_clan);
                    let color_key = match entity.kind {
                        LiveKind::Box => "boxes",
                        LiveKind::Ore => "fuel",
                        LiveKind::Mine => "mines",
                        LiveKind::Npc => "npcs",
                        LiveKind::Pet => "pet",
                        LiveKind::Player if group_ids.contains(&entity.id) => "group_member",
                        LiveKind::Player if is_enemy_ship(entity, hero_faction) => "enemies",
                        LiveKind::Player => "allies",
                        LiveKind::Beacon => "low_relays",
                        LiveKind::Other => "unknown",
                        _ => "other_entities",
                    };
                    let fallback = color;
                    color = hud.color(color_key, fallback);
                    if color == COLOR_NEUTRAL {
                        color = hud.color("allies", COLOR_ALLY);
                    }
                    let filled = match entity.kind {
                        LiveKind::Mine => true,
                        LiveKind::Box | LiveKind::Ore => entity.is_collectable,
                        _ => false,
                    };
                    let diameter = size + if filled { 1.0 } else { 0.0 } + 2.0;
                    paint_marker(
                        window,
                        center,
                        diameter,
                        rgb(color).into(),
                        filled,
                        hud.round_entities,
                    );
                    let named = match entity.kind {
                        LiveKind::Npc => hud.enabled("NPC_NAMES"),
                        LiveKind::Player => {
                            hud.enabled("USERNAMES")
                                || (hud.enabled("GROUP_NAMES") && group_ids.contains(&entity.id))
                        }
                        LiveKind::Pet => hud.enabled("USERNAMES") && hud.enabled("SHOW_PET"),
                        _ => false,
                    };
                    if named && !entity.name.is_empty() {
                        paint_centered(
                            window,
                            cx,
                            &entity.name,
                            f32::from(center.x),
                            f32::from(center.y) - 6.0,
                            13.0,
                            color,
                        );
                    }
                    if hud.enabled("RESOURCE_NAMES")
                        && let Some(label) = entity_label(entity)
                    {
                        paint_centered(
                            window,
                            cx,
                            &label,
                            f32::from(center.x),
                            f32::from(center.y) - 5.0,
                            13.0,
                            color,
                        );
                    }
                }
            };
            paint_entities(window, cx, &[LiveKind::Box, LiveKind::Ore, LiveKind::Mine]);
            paint_entities(window, cx, &[LiveKind::Npc]);
            if hud.enabled("SHOW_PET") {
                paint_entities(window, cx, &[LiveKind::Pet]);
            }
            paint_entities(
                window,
                cx,
                &[LiveKind::Player, LiveKind::Other, LiveKind::Beacon],
            );

            // The locked target: a filled oval in the target color.
            if let Some(target) = hud.target_id.and_then(|id| {
                frame.entities.iter().find(|entity| {
                    entity.id == id
                        && !matches!(
                            entity.kind,
                            LiveKind::Hero | LiveKind::Portal | LiveKind::Base | LiveKind::Station
                        )
                })
            }) {
                let center = project(target.x, target.y);
                paint_marker(
                    window,
                    center,
                    7.0,
                    rgb(hud.color("target", COLOR_TARGET)).into(),
                    true,
                    hud.round_entities,
                );
            }

            // Hero trail — a per-segment alpha ramp like the reference implementation's trail.
            let trail_points: Vec<Point<Pixels>> =
                trail.points().map(|(x, y)| project(x, y)).collect();
            let segments = trail_points.len().saturating_sub(1);
            for index in 0..segments {
                let alpha = 0.1 + 0.9 * (index + 1) as f32 / segments.max(1) as f32;
                let mut line = PathBuilder::stroke(px(1.0));
                line.move_to(trail_points[index]);
                line.line_to(trail_points[index + 1]);
                if let Ok(path) = line.build() {
                    window.paint_path(path, rgb(hud.color("trail_base", COLOR_TRAIL)).alpha(alpha));
                }
            }

            // Hero destination line, then the hero oval on top.
            if let Some(hero) = &frame.hero {
                let hero_point = project(hero.x, hero.y);
                if hud.enabled("SHOW_DESTINATION")
                    && let Some(dest) = &hero.dest
                {
                    let mut line = PathBuilder::stroke(px(1.0));
                    line.move_to(hero_point);
                    line.line_to(project(dest.x as f32, dest.y as f32));
                    if let Ok(path) = line.build() {
                        window.paint_path(path, rgb(hud.color("going", COLOR_GOING)));
                    }
                }
                paint_marker(
                    window,
                    hero_point,
                    9.0,
                    rgb(hud.color("hero", COLOR_HERO)).into(),
                    true,
                    hud.round_entities,
                );
                if hud.enabled("HERO_NAME")
                    && let Some(hero_entity) = frame.entities.iter().find(|entity| entity.is_hero)
                    && !hero_entity.name.is_empty()
                {
                    paint_centered(
                        window,
                        cx,
                        &hero_entity.name,
                        f32::from(hero_point.x),
                        f32::from(hero_point.y) - 8.0,
                        13.0,
                        hud.color("text", COLOR_TEXT),
                    );
                }
            }
        }
    }

    // `InfosDrawer.drawInfos`: refresh counter + module status top-left,
    // SID status top-right, run status under the map name.
    let left = f32::from(bounds.origin.x);
    let top = f32::from(bounds.origin.y);
    let right = left + f32::from(bounds.size.width);
    if !hud.refresh_info.is_empty() {
        let icon = shape_text_in(
            window,
            "Segoe UI Symbol",
            "\u{27F3}",
            15.0,
            hud.color("text_dark", COLOR_TEXT_DARK),
        );
        paint_line_at(window, cx, &icon, left + 5.0, top + 15.0);
        let info = shape_text(
            window,
            &hud.refresh_info,
            13.0,
            hud.color("text_dark", COLOR_TEXT_DARK),
        );
        paint_line_at(window, cx, &info, left + 23.0, top + 12.0);
    }
    for (index, line) in hud.module_status.iter().enumerate() {
        let shaped = shape_text(window, line, 13.0, hud.color("text_dark", COLOR_TEXT_DARK));
        paint_line_at(
            window,
            cx,
            &shaped,
            left + 7.0,
            top + 12.0 + 14.0 * (index + 1) as f32,
        );
    }
    if !hud.sid.is_empty() {
        paint_right(
            window,
            cx,
            &format!("SID: {}", hud.sid),
            right - 5.0,
            top + 12.0,
            13.0,
            hud.color("text_dark", COLOR_TEXT_DARK),
        );
    }

    if hud.enabled("GROUP_AREA") && !hud.group_lines.is_empty() {
        for (index, line) in hud.group_lines.iter().enumerate() {
            paint_right(
                window,
                cx,
                line,
                right - 5.0,
                top + 30.0 + index as f32 * 14.0,
                12.0,
                hud.color("group_member", COLOR_ALLY),
            );
        }
    }
    for (index, line) in hud.developer_details.iter().enumerate() {
        let shaped = shape_text(window, line, 11.0, hud.color("text_dark", COLOR_TEXT_DARK));
        paint_line_at(
            window,
            cx,
            &shaped,
            left + 7.0,
            top + 56.0 + index as f32 * 13.0,
        );
    }

    // `InfosDrawer.drawMap`: the big map name; "Loading" between maps.
    if !hud.title.is_empty() {
        paint_centered(
            window,
            cx,
            &hud.title,
            center_x,
            center_y - 5.0,
            32.0,
            hud.color("text_dark", COLOR_TEXT_DARK),
        );
    }
    if !hud.status.is_empty() {
        paint_centered(
            window,
            cx,
            &hud.status,
            center_x,
            center_y + 35.0,
            13.0,
            hud.color("text_dark", COLOR_TEXT_DARK),
        );
    }

    // `OverlayDrawer`: statistics on the left, boosters on the right, both
    // on a translucent panel centered vertically.
    if hud.enabled("STATS_AREA") && !hud.stats.is_empty() {
        let lines: Vec<ShapedLine> = hud
            .stats
            .iter()
            .map(|text| shape_text(window, text, 13.0, hud.color("text", COLOR_TEXT)))
            .collect();
        let block = paint_backgrounded(
            window,
            bounds,
            &lines,
            15.0,
            false,
            hud.color("texts_background", COLOR_TEXT_BG),
            hud.alpha("texts_background", ALPHA_TEXT_BG),
        );
        for (index, line) in lines.iter().enumerate() {
            let baseline = block.top + index as f32 * block.line_height + 14.0;
            paint_line_at(window, cx, line, block.left, baseline);
        }
    }
    if hud.enabled("BOOSTER_AREA") && !hud.boosters.is_empty() {
        let lines: Vec<ShapedLine> = hud
            .boosters
            .iter()
            .map(|(text, color)| shape_text(window, text, 13.0, *color))
            .collect();
        let block = paint_backgrounded(
            window,
            bounds,
            &lines,
            15.0,
            true,
            hud.color("texts_background", COLOR_TEXT_BG),
            hud.alpha("texts_background", ALPHA_TEXT_BG),
        );
        for (index, line) in lines.iter().enumerate() {
            let baseline = block.top + index as f32 * block.line_height + 14.0;
            paint_line_at(window, cx, line, block.left, baseline);
        }
    }

    // Hero name + health/shield bar at the bottom-left, target mirrored on
    // the right — using the reference `InfosDrawer` layout.
    let bottom = top + f32::from(bounds.size.height);
    let mid_x = left + f32::from(bounds.size.width) / 2.0;
    let bar_width = f32::from(bounds.size.width) / 2.0 - 20.0;
    if let Some(hero) = &hud.hero {
        if hud.enabled("HERO_NAME") && !hero.name.is_empty() {
            paint_centered_in(
                window,
                cx,
                FONT_MID,
                &hero.name,
                left + 10.0 + bar_width / 2.0,
                bottom - 40.0,
                18.0,
                hud.color("text", COLOR_TEXT),
            );
        }
        paint_health_bar(
            window,
            cx,
            hero,
            left + 10.0,
            bottom - 34.0,
            bar_width,
            12.0,
        );
    }
    if let Some(target) = &hud.target {
        let name_color = if target.enemy {
            COLOR_ENEMY
        } else {
            COLOR_ALLY
        };
        if !target.name.is_empty() {
            paint_centered_in(
                window,
                cx,
                FONT_MID,
                &target.name,
                mid_x + 10.0 + bar_width / 2.0,
                bottom - 40.0,
                18.0,
                hud.color(if target.enemy { "enemies" } else { "allies" }, name_color),
            );
        }
        paint_health_bar(
            window,
            cx,
            target,
            mid_x + 10.0,
            bottom - 34.0,
            bar_width,
            12.0,
        );
    }
    if !hud.config.is_empty() {
        let config_line = shape_text(window, &hud.config, 13.0, hud.color("text", COLOR_TEXT));
        paint_line_at(window, cx, &config_line, left + 12.0, bottom - 12.0);
    }
}
