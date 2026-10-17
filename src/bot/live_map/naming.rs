use super::*;

/// How the map HUD should describe the client's state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapStatus {
    /// A fresh frame reports an active map.
    Active,
    /// A fresh frame reports no map — the client sits on a loading screen.
    Loading,
    /// No fresh frame — the client is disconnected or stopped publishing.
    Unavailable,
}

/// Loading is exactly the reference implementation's `hero.getMap().id == -1`: the shared-memory
/// header publishes `map_id = -1` between maps, which decodes to
/// `frame.map == None`. A frame past `LiveFrame`'s freshness window means no
/// client data at all, which is not the loading screen.
pub fn map_status(frame: &LiveFrame, now_unix_ms: u64) -> MapStatus {
    if !frame.is_fresh(now_unix_ms) {
        MapStatus::Unavailable
    } else if frame.map.as_ref().is_some_and(|map| map.id >= 0) {
        MapStatus::Active
    } else {
        MapStatus::Loading
    }
}

/// A map name from the live catalog — used for both the centered title and
/// portal target labels. `short` picks the bundled short name when the
/// catalog only knows the full name.
pub(super) fn resolve_map_name(frame: &LiveFrame, id: i32, short: bool) -> Option<String> {
    frame
        .navigation
        .star_system
        .map(id)
        .map(|map| map.name.trim())
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .or_else(|| {
            if short {
                star_map::map_short_name(id)
            } else {
                star_map::map_name(id)
            }
            .map(str::to_owned)
        })
}

/// The centered map title: the live star-system catalog first, then the
/// bundled static catalog, then the snapshot-reported name, then a bare
/// `MAP <id>` fallback. While a portal jump is running the reference implementation appends
/// `→<target>`, so a jumping portal contributes its destination name.
pub fn map_title(frame: &LiveFrame, reported: Option<&str>) -> Option<String> {
    let map = frame.map.as_ref()?;
    let base = resolve_map_name(frame, map.id, false)
        .or_else(|| {
            reported
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_owned)
        })
        .unwrap_or_else(|| format!("MAP {}", map.id));
    let next = frame
        .navigation
        .portals
        .iter()
        .find(|portal| portal.jumping)
        .and_then(|portal| portal.target_map)
        .and_then(|id| resolve_map_name(frame, id, true));
    Some(match next {
        Some(next) => format!("{base}→{next}"),
        None => base,
    })
}

/// The destination-map name drawn beside a portal, matching the reference implementation: the
/// navigation payload knows the jumpgate's target directly, the bundled
/// star map resolves by position/portal type otherwise. `None` when the
/// target is unknown — the reference implementation draws no label then.
pub fn portal_label(frame: &LiveFrame, portal: &LiveEntity) -> Option<String> {
    let map = frame.map.as_ref()?;
    let target = frame
        .navigation
        .portals
        .iter()
        .find(|p| p.id == portal.id)
        .and_then(|p| p.target_map)
        .or_else(|| {
            star_map::portal_target(
                map.id,
                portal.x,
                portal.y,
                if portal.has_type_id {
                    portal.type_id
                } else {
                    0
                },
            )
        });
    target.and_then(|id| resolve_map_name(frame, id, true))
}

/// Fill navigation gaps on backends without live capture (legacy pipe,
/// packet) from the CDN-fetched catalog: an empty `star_system` adopts it
/// wholesale, and portal entities gain `PortalInfo` entries whose targets
/// resolve through the catalog's jumpgate links — `portal_label` then
/// works exactly like the native path. Frames that already carry a
/// catalog/portals are untouched.
pub fn inject_star_catalog(frame: &mut LiveFrame, catalog: Option<&StarSystem>) {
    let Some(catalog) = catalog else { return };
    if frame.navigation.star_system.maps.is_empty() {
        frame.navigation.star_system = catalog.clone();
    }
    if !frame.navigation.portals.is_empty() {
        return;
    }
    let Some(map_id) = frame.map.as_ref().map(|map| map.id) else {
        return;
    };
    let system = &frame.navigation.star_system;
    let portals: Vec<PortalInfo> = frame
        .entities
        .iter()
        .filter(|entity| entity.kind == LiveKind::Portal)
        .map(|entity| PortalInfo {
            id: entity.id,
            type_id: 0,
            position: xenomit_game_api::Point {
                x: f64::from(entity.x),
                y: f64::from(entity.y),
            },
            target_map: system
                .links
                .iter()
                .find(|link| link.from == map_id && link.portal_id == entity.id)
                .map(|link| link.to),
            jumping: false,
        })
        .collect();
    frame.navigation.portals = portals;
}
