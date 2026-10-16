//! Live frame monitor and per-frame bookkeeping (deaths, trail, star catalog).

use super::*;

impl BotView {
    /// Native backend: poll the client's shared-memory frame at ~60 Hz and
    /// repaint only when the captured frame number changes; every ~500 ms the
    /// decoded frame is also shaped into a `GameSnapshot` so panels that
    /// expect the old STATE payload keep working without any pipe traffic.
    /// Legacy backend: `snapshot` blocks the player thread, so it runs on the
    /// background executor at most every 500 ms and never overlaps; the
    /// result is converted into the same `LiveFrame` representation.
    /// Both branches read `view.client_backend`/`view.bridge` on every tick
    /// so a live backend switch takes effect without respawning the monitor.
    pub(super) fn spawn_live_monitor(
        this: gpui_kit::WeakEntity<Self>,
        generation: u64,
        cx: &mut gpui_kit::AsyncApp,
    ) {
        cx.spawn(async move |cx| {
            let mut last_frame = 0u64;
            let mut snapshot_at = Instant::now();
            let legacy_in_flight = Arc::new(AtomicBool::new(false));
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                let refresh_snapshot = snapshot_at.elapsed() >= Duration::from_millis(500);
                enum Poll {
                    Stop,
                    Wait,
                    Frame(u64),
                }
                let step = this
                    .update(cx, |view, cx| {
                        if view.connection_generation != generation || !view.session_connected {
                            return Poll::Stop;
                        }
                        let Some(client) = view.bridge.clone() else {
                            return Poll::Wait;
                        };
                        if !view.uses_shared_capture() {
                            if snapshot_at.elapsed()
                                >= Duration::from_millis(
                                    if view.settings.client == ClientKind::Flash {
                                        150
                                    } else {
                                        500
                                    },
                                )
                            {
                                snapshot_at = Instant::now();
                                if !legacy_in_flight.swap(true, Ordering::SeqCst) {
                                    let in_flight = Arc::clone(&legacy_in_flight);
                                    let polled_client = client.clone();
                                    cx.spawn(async move |this, cx| {
                                        let snapshot = cx
                                            .background_executor()
                                            .spawn(async move { client.snapshot(None) })
                                            .await;
                                        in_flight.store(false, Ordering::SeqCst);
                                        let _ = this.update(cx, |view, cx| {
                                            if view.connection_generation != generation
                                                || !view.session_connected
                                                || view.uses_shared_capture()
                                                || !view.bridge.as_ref().is_some_and(|current| {
                                                    Arc::ptr_eq(current, &polled_client)
                                                })
                                            {
                                                return;
                                            }
                                            let Ok(snapshot) = snapshot else {
                                                return;
                                            };
                                            view.live_frame =
                                                LiveFrame::from_game_snapshot(&snapshot);
                                            live_map::inject_star_catalog(
                                                &mut view.live_frame,
                                                view.star_catalog.as_ref(),
                                            );
                                            view.snapshot = Some(snapshot);
                                            view.note_live_frame(cx);
                                            let map_id =
                                                view.live_frame.map.as_ref().map(|map| map.id);
                                            if let Some((x, y)) = view
                                                .live_frame
                                                .hero
                                                .as_ref()
                                                .map(|hero| (hero.x, hero.y))
                                            {
                                                view.update_hero_trail(
                                                    map_id,
                                                    x,
                                                    y,
                                                    Instant::now(),
                                                );
                                            }
                                            if view.map_repaint_due() {
                                                cx.notify();
                                            }
                                        });
                                    })
                                    .detach();
                                }
                            }
                            return Poll::Wait;
                        }
                        // Allocation-free decode into the reused frame; only a
                        // new frame number triggers a repaint.
                        if !client.read_live_frame(&mut view.live_frame)
                            || view.live_frame.frame == last_frame
                        {
                            return Poll::Wait;
                        }
                        // Native capture normally supplies the catalog itself;
                        // the fetched one covers capture gaps.
                        live_map::inject_star_catalog(
                            &mut view.live_frame,
                            view.star_catalog.as_ref(),
                        );
                        let frame = view.live_frame.frame;
                        view.note_live_frame(cx);
                        let map_id = view.live_frame.map.as_ref().map(|map| map.id);
                        if let Some((x, y)) =
                            view.live_frame.hero.as_ref().map(|hero| (hero.x, hero.y))
                        {
                            view.update_hero_trail(map_id, x, y, Instant::now());
                        }
                        if refresh_snapshot {
                            view.snapshot = Some(view.live_frame.to_game_snapshot());
                        }
                        if view.map_repaint_due() {
                            cx.notify();
                        }
                        Poll::Frame(frame)
                    })
                    .unwrap_or(Poll::Stop);
                match step {
                    Poll::Stop => break,
                    Poll::Wait => {}
                    Poll::Frame(frame) => {
                        last_frame = frame;
                        if refresh_snapshot {
                            snapshot_at = Instant::now();
                        }
                    }
                }
            }
        })
        .detach();
    }

    /// Count live frames toward `live_fps` over a rolling 1-second window,
    /// and remember each newly seen NPC species in `settings.npcs` so the
    /// killer list fills itself instead of being typed by hand.
    pub(super) fn uses_shared_capture(&self) -> bool {
        match self.settings.client {
            ClientKind::Ruffle => self.client_backend == ClientBackend::Native,
            ClientKind::Flash => self.client_flash_backend == FlashBackend::NativeMemory,
            ClientKind::Packet => false,
        }
    }

    fn update_hero_trail(&mut self, map_id: Option<i32>, x: f32, y: f32, now: Instant) {
        let seconds = self
            .settings
            .config_tree
            .pointer("/bot_settings/map_display/trail_length")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(15)
            .min(300);
        self.hero_trail.set_ttl(Duration::from_secs(seconds));
        self.hero_trail.update(map_id, x, y, now);
    }

    fn map_repaint_due(&mut self) -> bool {
        let delay = self
            .settings
            .config_tree
            .pointer("/bot_settings/map_display/refresh_delay")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(1)
            .clamp(1, 25);
        if self.last_map_repaint_at.elapsed() >= Duration::from_millis(16 * delay) {
            self.last_map_repaint_at = Instant::now();
            true
        } else {
            false
        }
    }

    pub(super) fn note_live_frame(&mut self, cx: &mut Context<Self>) {
        if let Some((generation, map_id, x, y, at)) = self.map_click_target {
            if generation != self.connection_generation
                || at.elapsed() >= Duration::from_secs(30)
                || !self.live_frame.map.as_ref().is_some_and(|m| m.id == map_id)
                || self.live_frame.hero.as_ref().is_some_and(|h| {
                    (f64::from(h.x) - f64::from(x)).hypot(f64::from(h.y) - f64::from(y)) <= 30.0
                })
            {
                self.map_click_target = None;
            }
        }
        self.live_fps_frames += 1;
        let elapsed = self.live_fps_since.elapsed();
        if elapsed >= Duration::from_secs(1) {
            if self.settings.client != ClientKind::Flash {
                self.live_fps = self.live_fps_frames as f32 / elapsed.as_secs_f32();
            }
            self.live_fps_frames = 0;
            self.live_fps_since = Instant::now();
        }
        // Count deaths on the rising edge of the killed flag — the native
        // navigation payload reports it; packet/legacy frames fall back to
        // the hero entity's hp reaching zero.
        let dead = self.live_frame.navigation.hero.killed
            || self.live_frame.entities.iter().any(|entity| {
                entity.is_hero && entity.has_hp && entity.max_hp > 0 && entity.hp <= 0
            });
        if dead && !self.hero_dead {
            self.deaths += 1;
        }
        self.hero_dead = dead;
        if self.panel.wants_frame() {
            self.panel.push_frame(&self.live_frame);
        }
        let discovered: Vec<String> = self
            .live_frame
            .entities
            .iter()
            .filter(|entity| {
                entity.is_npc
                    && !entity.name.is_empty()
                    && !self.settings.npcs.contains_key(&entity.name)
            })
            .map(|entity| entity.name.clone())
            .collect();
        if !discovered.is_empty() {
            for species in discovered {
                self.settings.npcs.insert(species, true);
            }
            self.save_settings();
        }
        let entities: Vec<_> = self
            .live_frame
            .entities
            .iter()
            .map(|entity| {
                use xenomit_game_api::{EntityCategory, live::LiveKind};
                let category = if entity.is_npc {
                    EntityCategory::Npc
                } else if entity.is_hero {
                    EntityCategory::Player
                } else {
                    match entity.kind {
                        LiveKind::Npc => EntityCategory::Npc,
                        LiveKind::Hero => EntityCategory::Player,
                        LiveKind::Player | LiveKind::Pet => EntityCategory::Ship,
                        LiveKind::Portal => EntityCategory::Portal,
                        _ => EntityCategory::Unknown,
                    }
                };
                (category, entity.name.clone())
            })
            .collect();
        for page in self.module_pages.values() {
            page.update(cx, |page, cx| {
                page.observe(&entities, cx);
                page.observe_maps(&self.live_frame.navigation.star_system.maps, cx);
                let pet_gears = self
                    .live_frame
                    .navigation
                    .features
                    .as_ref()
                    .and_then(|features| features.pet.as_ref())
                    .and_then(|pet| pet.gears.clone())
                    .or_else(|| {
                        (!self.live_frame.navigation.hero.pet_gears.is_empty())
                            .then(|| self.live_frame.navigation.hero.pet_gears.clone())
                    });
                page.observe_pet_gears(pet_gears, cx);
            });
        }
        let named_lists: Vec<(&str, String)> = self
            .live_frame
            .entities
            .iter()
            .filter_map(|entity| {
                let list = match entity.kind {
                    LiveKind::Npc => "npcs",
                    LiveKind::Box => "boxes",
                    LiveKind::Ore => "collectables",
                    _ => return None,
                };
                let name = if entity.name.trim().is_empty() {
                    entity.kind.class_name().to_owned()
                } else {
                    entity.name.clone()
                };
                Some((list, name))
            })
            .collect();
        if let Some(page) = self.module_pages.get("__targets") {
            page.update(cx, |page, cx| page.observe_named_lists(&named_lists, cx));
        }
        let maps = self.live_frame.navigation.star_system.maps.clone();
        let mut tags: Vec<String> = self.settings.player_tags.values().cloned().collect();
        tags.sort();
        tags.dedup();
        self.config_page.update(cx, |page, cx| {
            page.observe_maps(&maps, cx);
            page.observe_tags(tags, cx);
            for list in ["npcs", "boxes"] {
                let names: Vec<String> = named_lists
                    .iter()
                    .filter(|(kind, _)| *kind == list)
                    .map(|(_, name)| name.clone())
                    .collect();
                if !names.is_empty() {
                    page.observe_names(list, &names, cx);
                }
            }
        });
    }

    /// A numeric field on the hero entity: the fresh
    /// `navigation.entity_fields` block on native frames first, then the
    /// last snapshot's hero fields so legacy/packet sessions still resolve
    /// cargo and the selected target.
    pub(super) fn hero_field(&self, aliases: &[&str]) -> Option<f64> {
        if let Some(hero_id) = self.live_frame.hero.as_ref().map(|hero| hero.id) {
            if let Some(fields) = self.live_frame.navigation.entity_fields.get(&hero_id) {
                if let Some(value) = aliases.iter().find_map(|key| {
                    fields.get(*key).and_then(|value| {
                        value
                            .as_f64()
                            .or_else(|| value.as_str().and_then(|text| text.parse::<f64>().ok()))
                    })
                }) {
                    return Some(value);
                }
            }
        }
        self.snapshot
            .as_ref()
            .and_then(|snapshot| {
                snapshot.entities.iter().find(|entity| {
                    entity
                        .fields
                        .get("isHero")
                        .and_then(|value| value.as_bool())
                        .unwrap_or(false)
                })
            })
            .and_then(|hero| {
                aliases.iter().find_map(|key| {
                    hero.fields.get(*key).and_then(|value| {
                        value
                            .as_f64()
                            .or_else(|| value.as_str().and_then(|text| text.parse::<f64>().ok()))
                    })
                })
            })
    }
}
