//! Map interaction: click/drag navigation and zone drawing/editing.

use super::*;

impl BotView {
    pub(super) fn map_world_point(
        &self,
        map_width: f32,
        map_height: f32,
        position: gpui_kit::Point<gpui_kit::Pixels>,
    ) -> (f64, f64) {
        let center = self
            .live_frame
            .hero
            .as_ref()
            .map(|hero| (hero.x, hero.y))
            .unwrap_or((map_width / 2.0, map_height / 2.0));
        let zoom = self
            .settings
            .config_tree
            .pointer("/bot_settings/map_display/map_zoom")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(1.0)
            .clamp(0.1, 1.0) as f32;
        let Some(bounds) = self.map_canvas_bounds.get() else {
            return (f64::from(center.0), f64::from(center.1));
        };
        live_map::screen_to_world_zoom(bounds, map_width, map_height, position, center, zoom)
    }

    pub(super) fn snap_zone_point(
        &self,
        x: f64,
        y: f64,
        map_width: f32,
        map_height: f32,
    ) -> (f64, f64) {
        let divisions = self
            .settings
            .config_tree
            .pointer("/bot_settings/other/zone_resolution")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(30)
            .clamp(1, 300) as f64;
        let step_x = f64::from(map_width.max(1.0)) / divisions;
        let step_y = f64::from(map_height.max(1.0)) / divisions;
        (
            ((x / step_x).round() * step_x).clamp(0.0, f64::from(map_width)),
            ((y / step_y).round() * step_y).clamp(0.0, f64::from(map_height)),
        )
    }

    /// Continue an in-progress map drag: while the left button stays held,
    /// retarget navigation at `MAP_NAV_DRAG_INTERVAL`. Window-level listeners
    /// registered during the canvas paint call this even when the pointer
    /// has left the canvas, so the target keeps clamping to the map edge.
    pub(super) fn drag_map_target(
        &mut self,
        map_width: f32,
        map_height: f32,
        event: &gpui_kit::MouseMoveEvent,
        cx: &mut Context<Self>,
    ) {
        if !event.dragging() {
            self.map_dragging = false;
            return;
        }
        if !self.map_dragging || self.map_nav_at.elapsed() < MAP_NAV_DRAG_INTERVAL {
            return;
        }
        let (world_x, world_y) = self.map_world_point(map_width, map_height, event.position);
        self.map_nav_at = Instant::now();
        self.navigate_to_map_point(world_x, world_y, cx);
    }

    pub(super) fn drag_zone_target(
        &mut self,
        map_width: f32,
        map_height: f32,
        event: &gpui_kit::MouseMoveEvent,
        cx: &mut Context<Self>,
    ) {
        if !event.dragging() {
            return;
        }
        let Some((map_id, start_x, start_y)) = self.zone_drag_start else {
            return;
        };
        let (end_x, end_y) = self.map_world_point(map_width, map_height, event.position);
        let (end_x, end_y) = self.snap_zone_point(end_x, end_y, map_width, map_height);
        self.zone_draft = Some(MapZone {
            name: "New zone".into(),
            x: start_x.min(end_x),
            y: start_y.min(end_y),
            width: (end_x - start_x).abs(),
            height: (end_y - start_y).abs(),
            enabled: true,
            kind: MapZoneKind::Preferred,
        });
        let _ = map_id;
        cx.notify();
    }

    pub(super) fn finish_zone_drawing(&mut self, cx: &mut Context<Self>) {
        let Some((map_id, _, _)) = self.zone_drag_start.take() else {
            return;
        };
        if let Some(mut zone) = self.zone_draft.take() {
            if zone.width >= 150.0 && zone.height >= 150.0 {
                let zones = self.settings.map_zones.entry(map_id).or_default();
                zone.name = format!("Zone {}", zones.len() + 1);
                zones.push(zone);
                self.activity = format!("Added a work zone on map {map_id}.");
                self.save_settings();
            } else {
                self.activity = "The zone is too small; drag a larger rectangle.".into();
            }
        }
        self.zone_hovered = None;
        cx.notify();
    }

    pub(super) fn remove_map_zone(&mut self, map_id: i32, index: usize, cx: &mut Context<Self>) {
        if let Some(zones) = self.settings.map_zones.get_mut(&map_id) {
            if index < zones.len() {
                zones.remove(index);
            }
            if zones.is_empty() {
                self.settings.map_zones.remove(&map_id);
            }
            self.save_settings();
        }
        self.zone_hovered = None;
        cx.notify();
    }

    pub(super) fn toggle_map_zone(&mut self, map_id: i32, index: usize, cx: &mut Context<Self>) {
        if let Some(zone) = self
            .settings
            .map_zones
            .get_mut(&map_id)
            .and_then(|zones| zones.get_mut(index))
        {
            zone.enabled = !zone.enabled;
            self.save_settings();
        }
        self.zone_hovered = None;
        cx.notify();
    }

    pub(super) fn toggle_map_zone_kind(
        &mut self,
        map_id: i32,
        index: usize,
        cx: &mut Context<Self>,
    ) {
        if let Some(zone) = self
            .settings
            .map_zones
            .get_mut(&map_id)
            .and_then(|zones| zones.get_mut(index))
        {
            zone.kind = match zone.kind {
                MapZoneKind::Preferred => MapZoneKind::Avoided,
                MapZoneKind::Avoided => MapZoneKind::Preferred,
            };
            self.save_settings();
        }
        self.zone_hovered = None;
        cx.notify();
    }

    pub(super) fn navigate_to_map_point(&mut self, x: f64, y: f64, cx: &mut Context<Self>) {
        let Some(bridge) = self.bridge.clone() else {
            self.activity = "Connect the game client before navigating the map.".into();
            cx.notify();
            return;
        };
        let x = x.clamp(0.0, f64::from(i32::MAX)) as i32;
        let y = y.clamp(0.0, f64::from(i32::MAX)) as i32;
        let call = FunctionCall::new(
            "net.bigpoint.darkorbit.managers.ScreenManager.getInstance().eventManager",
            "moveHeroToCordinates",
        )
        .with_arguments(vec![json!(x), json!(y)])
        .in_frame();
        let generation = self.connection_generation;
        let map_id = self.live_frame.map.as_ref().map_or(-1, |m| m.id);
        let requested_at = Instant::now();
        self.map_click_target = Some((generation, map_id, x, y, requested_at));
        self.activity = format!("Moving to map position {x}, {y}.");
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    bridge
                        .call_function(&call)
                        .map_err(|error| error.to_string())
                })
                .await;
            let _ = this.update(cx, |view, cx| {
                if view.connection_generation != generation {
                    return;
                }
                if !view
                    .map_click_target
                    .as_ref()
                    .is_some_and(|t| t.4 == requested_at)
                {
                    return;
                }
                if result.is_err()
                    && view
                        .map_click_target
                        .as_ref()
                        .is_some_and(|t| t.2 == x && t.3 == y)
                {
                    view.map_click_target = None;
                }
                view.activity = match result {
                    Ok(_) => format!("Navigation target set to {x}, {y}."),
                    Err(error) => format!("Could not navigate to map position: {error}"),
                };
                cx.notify();
            });
        })
        .detach();
    }
}
