use super::*;
#[test]
fn obstacle_outlines_preserve_world_geometry() {
    use xenomit_game_api::{Bounds, ObstacleShape, Point};
    let circle = super::obstacle_outline(&ObstacleShape::Circle {
        center: Point { x: 100.0, y: 200.0 },
        radius: 50.0,
    });
    assert_eq!(circle.len(), 4);
    assert_eq!(circle[0], (50.0, 150.0));
    assert_eq!(circle[2], (150.0, 250.0));
    assert_eq!(
        super::obstacle_outline(&ObstacleShape::Rectangle {
            bounds: Bounds {
                x: 1.0,
                y: 2.0,
                width: 3.0,
                height: 4.0
            }
        }),
        vec![(1.0, 2.0), (4.0, 2.0), (4.0, 6.0), (1.0, 6.0)]
    );
    assert!(
        super::obstacle_outline(&ObstacleShape::Circle {
            center: Point {
                x: f64::NAN,
                y: 0.0
            },
            radius: 1.0
        })
        .is_empty()
    );
}

#[test]
fn obstacle_colors_follow_observed_geometry_kind() {
    use xenomit_game_api::ObstacleKind;

    assert_eq!(super::obstacle_color_key(ObstacleKind::NoAccess), "safety");
    assert_eq!(super::obstacle_color_key(ObstacleKind::Mine), "mines");
    assert_eq!(super::obstacle_color_key(ObstacleKind::Cbs), "bases");
    assert_eq!(
        super::obstacle_color_key(ObstacleKind::Radiation),
        "radiation"
    );
}
use super::*;
use xenomit_game_api::Point as ApiPoint;
use xenomit_game_api::live::LiveMap;
use xenomit_game_api::navigation::{MapDescriptor, MapLink, PortalInfo, StarSystem};

fn canvas() -> Bounds<Pixels> {
    Bounds::new(point(px(10.0), px(20.0)), size(px(420.0), px(262.0)))
}

fn ship(faction: i32, clan: i32, clan_diplomacy: i32) -> LiveEntity {
    LiveEntity {
        kind: LiveKind::Player,
        faction,
        clan,
        clan_diplomacy,
        ..LiveEntity::default()
    }
}

fn frame_with_map(id: i32) -> LiveFrame {
    LiveFrame {
        captured_unix_ms: 1_000_000,
        map: Some(LiveMap {
            id,
            width: 21_000.0,
            height: 13_100.0,
        }),
        ..LiveFrame::default()
    }
}

#[test]
fn world_to_screen_stretches_each_axis() {
    let bounds = canvas();
    // 21000x13100 world on a 420x262 canvas => 50px/1000u x, 20px/1000u y.
    let origin = world_to_screen(bounds, 21_000.0, 13_100.0, 0.0, 0.0);
    assert_eq!(origin, point(px(10.0), px(20.0)));
    let far = world_to_screen(bounds, 21_000.0, 13_100.0, 21_000.0, 13_100.0);
    assert_eq!(far, point(px(430.0), px(282.0)));
    let mid = world_to_screen(bounds, 21_000.0, 13_100.0, 10_500.0, 6_550.0);
    assert!((f32::from(mid.x) - 220.0).abs() < 0.01);
    assert!((f32::from(mid.y) - 151.0).abs() < 0.01);
}

#[test]
fn screen_to_world_inverts_and_clamps() {
    let bounds = canvas();
    let (x, y) = screen_to_world(bounds, 21_000.0, 13_100.0, point(px(220.0), px(151.0)));
    assert!((x - 10_500.0).abs() < 1.0);
    assert!((y - 6_550.0).abs() < 1.0);
    // Clicks outside the canvas clamp to the map edge.
    let (x, y) = screen_to_world(bounds, 21_000.0, 13_100.0, point(px(-50.0), px(9_999.0)));
    assert_eq!((x, y), (0.0, 13_100.0));
}

#[test]
fn enemy_and_ally_classification() {
    // Different nonzero faction -> enemy; war diplomacy -> enemy.
    assert!(is_enemy_ship(&ship(2, 0, 0), 1));
    assert!(is_enemy_ship(&ship(1, 0, 3), 1));
    // Same faction or factionless ships are not enemies.
    assert!(!is_enemy_ship(&ship(1, 0, 0), 1));
    assert!(!is_enemy_ship(&ship(0, 0, 0), 1));
    // Ally = same faction or same nonzero clan; unknown hero never allies.
    assert!(is_ally_ship(&ship(1, 0, 0), 1, 9));
    assert!(is_ally_ship(&ship(2, 7, 0), 1, 7));
    assert!(!is_ally_ship(&ship(2, 0, 0), 1, 7));
    assert!(!is_ally_ship(&ship(0, 0, 0), -1, -1));
    assert!(!is_ally_ship(&ship(0, 5, 0), -1, -1));
}

#[test]
fn marker_colors_follow_kind_and_diplomacy() {
    let (_, color) = marker_style(&ship(2, 0, 0), 1, 0);
    assert_eq!(color, COLOR_ENEMY);
    let (_, color) = marker_style(&ship(1, 0, 0), 1, 0);
    assert_eq!(color, COLOR_ALLY);
    let mut ore = LiveEntity {
        kind: LiveKind::Ore,
        has_type_id: true,
        type_id: PALLADIUM_TYPE_ID,
        ..LiveEntity::default()
    };
    assert_eq!(marker_style(&ore, 1, 0).1, COLOR_PALLADIUM);
    ore.type_id = 0;
    assert_eq!(marker_style(&ore, 1, 0).1, COLOR_ORE);
    assert_eq!(entity_label(&ore).as_deref(), Some("Prometium"));
    ore.type_id = PALLADIUM_TYPE_ID;
    assert_eq!(entity_label(&ore).as_deref(), Some("Palladium"));
    ore.has_type_id = false;
    assert_eq!(entity_label(&ore), None);
}

#[test]
fn map_status_detects_loading_and_stale_frames() {
    let active = frame_with_map(11);
    let now = 1_000_500;
    assert_eq!(map_status(&active, now), MapStatus::Active);
    // Fresh frame, no map -> the client's loading screen.
    let loading = LiveFrame {
        captured_unix_ms: 1_000_000,
        map: None,
        ..LiveFrame::default()
    };
    assert_eq!(map_status(&loading, now), MapStatus::Loading);
    // Same frame past its freshness window -> disconnected, not loading.
    assert_eq!(map_status(&loading, now + 5_000), MapStatus::Unavailable);
    // Never received a frame at all.
    assert_eq!(
        map_status(&LiveFrame::default(), 1_000_000_000),
        MapStatus::Unavailable
    );
}

#[test]
fn map_title_uses_catalog_static_then_reported() {
    // Live star-system catalog wins.
    let mut frame = frame_with_map(51);
    frame.navigation.star_system = StarSystem {
        maps: vec![MapDescriptor {
            id: 51,
            name: "GG α".into(),
            min_level: 0,
        }],
        links: Vec::new(),
    };
    assert_eq!(map_title(&frame, None).as_deref(), Some("GG α"));
    // The static catalog covers every known map without a live catalog.
    let frame = frame_with_map(11);
    assert_eq!(map_title(&frame, None).as_deref(), Some("3-3"));
    // A reported name covers maps outside both catalogs.
    let frame = frame_with_map(999);
    assert_eq!(
        map_title(&frame, Some("New Event")).as_deref(),
        Some("New Event")
    );
    // The last resort is the bare id.
    let frame = frame_with_map(999);
    assert_eq!(map_title(&frame, None).as_deref(), Some("MAP 999"));
}

#[test]
fn map_title_appends_jump_destination() {
    let mut frame = frame_with_map(1);
    frame.navigation.portals = vec![PortalInfo {
        id: 7,
        type_id: 0,
        position: ApiPoint { x: 0.0, y: 0.0 },
        target_map: Some(2),
        jumping: true,
    }];
    assert_eq!(map_title(&frame, None).as_deref(), Some("1-1→1-2"));
}

#[test]
fn portal_label_resolves_navigation_and_static_targets() {
    let mut frame = frame_with_map(1);
    // Native payload resolves by portal id.
    frame.navigation.portals = vec![PortalInfo {
        id: 42,
        type_id: 0,
        position: ApiPoint { x: 0.0, y: 0.0 },
        target_map: Some(11),
        jumping: false,
    }];
    let portal = LiveEntity {
        id: 42,
        kind: LiveKind::Portal,
        x: 100.0,
        y: 100.0,
        ..LiveEntity::default()
    };
    assert_eq!(portal_label(&frame, &portal).as_deref(), Some("3-3"));
    // Unknown to the navigation payload -> static position table.
    let portal = LiveEntity {
        id: 1,
        kind: LiveKind::Portal,
        x: 18500.0,
        y: 11500.0,
        ..LiveEntity::default()
    };
    assert_eq!(portal_label(&frame, &portal).as_deref(), Some("1-2"));
    // A gate type resolves through the gate table.
    let portal = LiveEntity {
        id: 2,
        kind: LiveKind::Portal,
        x: 500.0,
        y: 500.0,
        has_type_id: true,
        type_id: 2,
        ..LiveEntity::default()
    };
    assert_eq!(portal_label(&frame, &portal).as_deref(), Some("GG α"));
    // Nothing known -> no label, matching DarkBot.
    let portal = LiveEntity {
        id: 3,
        kind: LiveKind::Portal,
        x: 5_000.0,
        y: 5_000.0,
        ..LiveEntity::default()
    };
    assert_eq!(portal_label(&frame, &portal), None);
    // No map -> loading screen, nothing labeled.
    let loading = LiveFrame {
        captured_unix_ms: 1,
        ..LiveFrame::default()
    };
    assert_eq!(portal_label(&loading, &portal), None);
}

#[test]
fn fetched_catalog_fills_names_and_portal_targets() {
    // Legacy/packet frames arrive with an empty navigation payload; the
    // CDN catalog supplies map names and jumpgate targets, so labels
    // behave exactly like the native capture.
    let catalog = StarSystem {
        maps: vec![
            MapDescriptor {
                id: 1,
                name: "1-1".into(),
                min_level: 0,
            },
            MapDescriptor {
                id: 2,
                name: "1-2".into(),
                min_level: 0,
            },
        ],
        links: vec![MapLink {
            from: 1,
            to: 2,
            portal_id: 42,
        }],
    };
    let mut frame = frame_with_map(1);
    frame.entities.push(LiveEntity {
        id: 42,
        kind: LiveKind::Portal,
        x: 18_500.0,
        y: 11_500.0,
        ..LiveEntity::default()
    });
    inject_star_catalog(&mut frame, Some(&catalog));
    assert_eq!(map_title(&frame, None).as_deref(), Some("1-1"));
    assert_eq!(
        frame.navigation.portals,
        vec![PortalInfo {
            id: 42,
            type_id: 0,
            position: ApiPoint {
                x: 18_500.0,
                y: 11_500.0
            },
            target_map: Some(2),
            jumping: false,
        }]
    );
    assert_eq!(
        portal_label(&frame, &frame.entities[0].clone()).as_deref(),
        Some("1-2")
    );
    // A live navigation payload wins over the fetch.
    let mut live = frame_with_map(1);
    live.navigation.star_system = StarSystem {
        maps: vec![MapDescriptor {
            id: 1,
            name: "live".into(),
            min_level: 0,
        }],
        links: Vec::new(),
    };
    inject_star_catalog(&mut live, Some(&catalog));
    assert_eq!(map_title(&live, None).as_deref(), Some("live"));
}

#[test]
fn digit_grouping_matches_darkbot_format() {
    assert_eq!(format_amount(0.0), "0");
    assert_eq!(format_amount(551_375.0), "551 375");
    assert_eq!(format_amount(24_291.4), "24 291");
    assert_eq!(format_amount(-1_234.0), "-1 234");
}

#[test]
fn trail_samples_on_a_time_cadence() {
    let mut trail = HeroTrail::default();
    let t0 = Instant::now();
    assert!(trail.update(Some(8), 100.0, 100.0, t0));
    // Inside the sampling interval: not recorded, even after a move.
    assert!(!trail.update(Some(8), 400.0, 100.0, t0 + Duration::from_millis(50)));
    // Next interval: pushed.
    assert!(trail.update(Some(8), 400.0, 100.0, t0 + TRAIL_SAMPLE));
    assert_eq!(trail.len(), 2);
    // Off-map positions are ignored.
    assert!(!trail.update(None, 500.0, 500.0, t0 + TRAIL_SAMPLE * 2));
    assert_eq!(trail.len(), 2);
}

#[test]
fn trail_expires_old_points() {
    let mut trail = HeroTrail::default();
    let t0 = Instant::now();
    assert!(trail.update(Some(8), 0.0, 0.0, t0));
    assert!(trail.update(Some(8), 10.0, 0.0, t0 + TRAIL_SAMPLE));
    assert_eq!(trail.len(), 2);
    // A sample taken past the TTL drops every earlier point.
    assert!(trail.update(Some(8), 20.0, 0.0, t0 + TRAIL_TTL + Duration::from_secs(1),));
    assert_eq!(trail.points().collect::<Vec<_>>(), vec![(20.0, 0.0)]);
}

#[test]
fn trail_clears_on_map_change_and_teleport() {
    let mut trail = HeroTrail::default();
    let t0 = Instant::now();
    assert!(trail.update(Some(8), 0.0, 0.0, t0));
    assert!(trail.update(Some(8), 100.0, 0.0, t0 + TRAIL_SAMPLE));
    // New map: previous points are gone.
    assert!(trail.update(Some(9), 50.0, 50.0, t0 + TRAIL_SAMPLE * 2));
    assert_eq!(trail.points().collect::<Vec<_>>(), vec![(50.0, 50.0)]);
    // Teleport on the same map: cleared, then the new point is stored.
    assert!(trail.update(Some(9), 5_000.0, 5_000.0, t0 + TRAIL_SAMPLE * 3));
    assert_eq!(trail.points().collect::<Vec<_>>(), vec![(5_000.0, 5_000.0)]);
}
