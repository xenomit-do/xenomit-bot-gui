use super::*;

/// World position -> point inside `bounds`; the map is stretched to fill
/// the canvas independently in x and y (matching DarkOrbit's own map).
pub fn world_to_screen(
    bounds: Bounds<Pixels>,
    map_width: f32,
    map_height: f32,
    x: f32,
    y: f32,
) -> Point<Pixels> {
    let scale_x = f64::from(bounds.size.width) / f64::from(map_width.max(1.0));
    let scale_y = f64::from(bounds.size.height) / f64::from(map_height.max(1.0));
    point(
        bounds.origin.x + px((f64::from(x) * scale_x) as f32),
        bounds.origin.y + px((f64::from(y) * scale_y) as f32),
    )
}

/// Inverse of [`world_to_screen`] for click-to-move; the result is clamped
/// to the map rectangle.
pub fn screen_to_world(
    bounds: Bounds<Pixels>,
    map_width: f32,
    map_height: f32,
    position: Point<Pixels>,
) -> (f64, f64) {
    let width = f64::from(bounds.size.width).max(1.0);
    let height = f64::from(bounds.size.height).max(1.0);
    let x = (f64::from(position.x) - f64::from(bounds.origin.x)) / width * f64::from(map_width);
    let y = (f64::from(position.y) - f64::from(bounds.origin.y)) / height * f64::from(map_height);
    (
        x.clamp(0.0, f64::from(map_width.max(0.0))),
        y.clamp(0.0, f64::from(map_height.max(0.0))),
    )
}

/// Zoom around a world-space camera center. The settings value follows the
/// DarkBot percentage convention: `1.0` shows the full map and smaller values
/// zoom in while keeping the camera centered on the hero.
pub fn world_to_screen_zoom(
    bounds: Bounds<Pixels>,
    map_width: f32,
    map_height: f32,
    x: f32,
    y: f32,
    center: (f32, f32),
    zoom: f32,
) -> Point<Pixels> {
    let zoom = zoom.clamp(0.1, 1.0);
    world_to_screen(
        bounds,
        map_width,
        map_height,
        center.0 + (x - center.0) / zoom,
        center.1 + (y - center.1) / zoom,
    )
}

pub fn screen_to_world_zoom(
    bounds: Bounds<Pixels>,
    map_width: f32,
    map_height: f32,
    point: Point<Pixels>,
    center: (f32, f32),
    zoom: f32,
) -> (f64, f64) {
    let zoom = zoom.clamp(0.1, 1.0);
    let width = f64::from(bounds.size.width).max(1.0);
    let height = f64::from(bounds.size.height).max(1.0);
    let x = (f64::from(point.x) - f64::from(bounds.origin.x)) / width * f64::from(map_width);
    let y = (f64::from(point.y) - f64::from(bounds.origin.y)) / height * f64::from(map_height);
    (
        (f64::from(center.0) + (x - f64::from(center.0)) * f64::from(zoom))
            .clamp(0.0, f64::from(map_width.max(0.0))),
        (f64::from(center.1) + (y - f64::from(center.1)) * f64::from(zoom))
            .clamp(0.0, f64::from(map_height.max(0.0))),
    )
}

/// Shared shape conversion for drawing and geometry validation tests. Inverted
/// zones draw their boundary only: their interior is playable.
pub fn obstacle_outline(shape: &xenomit_game_api::ObstacleShape) -> Vec<(f64, f64)> {
    use xenomit_game_api::ObstacleShape;
    let points: Vec<_> = match shape {
        ObstacleShape::Circle { center, radius } if radius.is_finite() && *radius >= 0.0 => vec![
            (center.x - radius, center.y - radius),
            (center.x + radius, center.y - radius),
            (center.x + radius, center.y + radius),
            (center.x - radius, center.y + radius),
        ],
        ObstacleShape::Rectangle { bounds: b } if b.width > 0.0 && b.height > 0.0 => vec![
            (b.x, b.y),
            (b.x + b.width, b.y),
            (b.x + b.width, b.y + b.height),
            (b.x, b.y + b.height),
        ],
        ObstacleShape::Polygon { points } => points.iter().map(|p| (p.x, p.y)).collect(),
        _ => vec![],
    };
    if points.iter().any(|(x, y)| !x.is_finite() || !y.is_finite()) {
        vec![]
    } else {
        points
    }
}
