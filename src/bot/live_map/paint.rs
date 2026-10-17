use super::*;

/// Shape one Consolas run — the font family matches the reference implementation's map fonts.
pub(super) fn shape_text(
    window: &mut Window,
    text: &str,
    font_size: f32,
    color: u32,
) -> ShapedLine {
    shape_text_in(window, "Consolas", text, font_size, color)
}

pub(super) fn shape_text_in(
    window: &mut Window,
    family: &'static str,
    text: &str,
    font_size: f32,
    color: u32,
) -> ShapedLine {
    window.text_system().shape_line(
        SharedString::from(text.to_owned()),
        px(font_size),
        &[TextRun {
            len: text.len(),
            font: font(family),
            color: rgb(color).into(),
            ..Default::default()
        }],
        None,
    )
}

/// Paint a shaped line with its glyph baseline at `(x, baseline_y)`.
/// `paint_line` pads `origin.y` by `(line_height - ascent - descent)/2`
/// before the ascent, so invert that to land the baseline exactly.
pub(super) fn paint_line_at(
    window: &mut Window,
    cx: &mut App,
    line: &ShapedLine,
    x: f32,
    baseline_y: f32,
) {
    let line_height = f32::from(line.font_size) * 1.2;
    let top = baseline_y - (line_height + f32::from(line.ascent) - f32::from(line.descent)) / 2.0;
    let _ = line.paint(
        point(px(x), px(top)),
        px(line_height),
        TextAlign::Left,
        None,
        window,
        cx,
    );
}

/// Centered text with its baseline at `baseline_y`.
pub(super) fn paint_centered(
    window: &mut Window,
    cx: &mut App,
    text: &str,
    center_x: f32,
    baseline_y: f32,
    font_size: f32,
    color: u32,
) {
    paint_centered_in(
        window, cx, "Consolas", text, center_x, baseline_y, font_size, color,
    );
}

#[allow(clippy::too_many_arguments)]
pub(super) fn paint_centered_in(
    window: &mut Window,
    cx: &mut App,
    family: &'static str,
    text: &str,
    center_x: f32,
    baseline_y: f32,
    font_size: f32,
    color: u32,
) {
    let line = shape_text_in(window, family, text, font_size, color);
    paint_line_at(
        window,
        cx,
        &line,
        center_x - f32::from(line.width()) / 2.0,
        baseline_y,
    );
}

/// Right-aligned text whose right edge sits at `right_x`.
pub(super) fn paint_right(
    window: &mut Window,
    cx: &mut App,
    text: &str,
    right_x: f32,
    baseline_y: f32,
    font_size: f32,
    color: u32,
) {
    let line = shape_text(window, text, font_size, color);
    paint_line_at(
        window,
        cx,
        &line,
        right_x - f32::from(line.width()),
        baseline_y,
    );
}

pub(super) fn tinted(color: u32, alpha: f32) -> Hsla {
    rgb(color).alpha(alpha).into()
}

/// `drawOvalCentered`: an anti-aliased circle/disc of `diameter` px.
pub(super) fn paint_oval(
    window: &mut Window,
    center: Point<Pixels>,
    diameter: f32,
    color: Hsla,
    filled: bool,
) {
    let bounds = Bounds::new(
        point(
            px(f32::from(center.x) - diameter / 2.0),
            px(f32::from(center.y) - diameter / 2.0),
        ),
        size(px(diameter), px(diameter)),
    );
    let quad = if filled {
        fill(bounds, color)
    } else {
        outline(bounds, color, BorderStyle::Solid)
    };
    window.paint_quad(quad.corner_radii(px(diameter / 2.0)));
}

pub(super) fn paint_marker(
    window: &mut Window,
    center: Point<Pixels>,
    diameter: f32,
    color: Hsla,
    filled: bool,
    round: bool,
) {
    if round {
        paint_oval(window, center, diameter, color, filled);
        return;
    }
    let bounds = Bounds::new(
        point(
            px(f32::from(center.x) - diameter / 2.0),
            px(f32::from(center.y) - diameter / 2.0),
        ),
        size(px(diameter), px(diameter)),
    );
    window.paint_quad(if filled {
        fill(bounds, color)
    } else {
        outline(bounds, color, BorderStyle::Solid).border_widths(px(1.0))
    });
}

/// A filled ellipse with independent screen radii (the map stretches each
/// axis separately, so a world-space circle is an ellipse on screen).
pub(super) fn paint_ellipse(
    window: &mut Window,
    center: Point<Pixels>,
    rx: f32,
    ry: f32,
    color: Hsla,
) {
    const SEGMENTS: usize = 64;
    let (cx, cy) = (f32::from(center.x), f32::from(center.y));
    let mut path = PathBuilder::fill();
    for index in 0..SEGMENTS {
        let angle = index as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        let vertex = point(px(cx + rx * angle.cos()), px(cy + ry * angle.sin()));
        if index == 0 {
            path.move_to(vertex);
        } else {
            path.line_to(vertex);
        }
    }
    path.close();
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}

/// the reference implementation's `drawHealth`: a darker health background, the nanohull prefix
/// in yellow, hp in green with a centered `hp/max` readout, then — only for
/// ships that have a shield — a shield bar directly below.
#[allow(clippy::too_many_arguments)]
pub(super) fn paint_health_bar(
    window: &mut Window,
    cx: &mut App,
    bar: &BarHud,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) {
    let rect = |x: f32, y: f32, w: f32, h: f32| {
        Bounds::new(point(px(x), px(y)), size(px(w.max(0.0)), px(h)))
    };
    let nano_w = if bar.nano_max > 0.0 {
        width * (bar.nano / bar.nano_max).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let hp_w = if bar.max_hp > 0.0 {
        let remaining = if bar.hp_includes_nano {
            (bar.hp - bar.nano).max(0.0)
        } else {
            bar.hp
        };
        (width - nano_w) * (remaining / bar.max_hp).clamp(0.0, 1.0)
    } else {
        0.0
    };
    window.paint_quad(fill(rect(x, y, width, height), rgb(bar.health_background)));
    window.paint_quad(fill(
        rect(x, y, nano_w + hp_w, height),
        rgb(bar.health_color),
    ));
    if nano_w > 0.0 {
        window.paint_quad(fill(rect(x, y, nano_w, height), rgb(bar.nano_color)));
    }
    let show_amount = bar.show_numbers && height >= 8.0;
    if show_amount {
        paint_centered(
            window,
            cx,
            &format!(
                "{}/{}",
                format_amount(f64::from(bar.hp)),
                format_amount(f64::from(bar.max_hp))
            ),
            x + width / 2.0,
            y + height - 2.0,
            13.0,
            bar.text_color,
        );
    }
    if bar.max_shield > 0.0 {
        let shield_y = y + height;
        let shield_w = width * (bar.shield / bar.max_shield).clamp(0.0, 1.0);
        window.paint_quad(fill(
            rect(x, shield_y, width, height),
            rgb(bar.shield_background),
        ));
        window.paint_quad(fill(
            rect(x, shield_y, shield_w, height),
            rgb(bar.shield_color),
        ));
        if show_amount {
            paint_centered(
                window,
                cx,
                &format!(
                    "{}/{}",
                    format_amount(f64::from(bar.shield)),
                    format_amount(f64::from(bar.max_shield))
                ),
                x + width / 2.0,
                shield_y + height - 2.0,
                13.0,
                bar.text_color,
            );
        }
    }
}

/// A panel using the reference `drawBackgrounded` layout: translucent, sized to its lines and
/// vertically centered on the canvas, flush with the left or right edge.
pub(super) struct BackgroundedBlock {
    pub(super) left: f32,
    pub(super) top: f32,
    pub(super) line_height: f32,
}

pub(super) fn paint_backgrounded(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    lines: &[ShapedLine],
    line_height: f32,
    right_aligned: bool,
    background: u32,
    background_alpha: f32,
) -> BackgroundedBlock {
    let width = lines
        .iter()
        .map(|line| f32::from(line.width()))
        .fold(0.0_f32, f32::max)
        + 8.0;
    let height = lines.len() as f32 * line_height + 4.0;
    let canvas_w = f32::from(bounds.size.width);
    let canvas_h = f32::from(bounds.size.height);
    let left = f32::from(bounds.origin.x) + if right_aligned { canvas_w - width } else { 0.0 };
    let top = f32::from(bounds.origin.y) + canvas_h / 2.0 - height / 2.0;
    window.paint_quad(fill(
        Bounds::new(point(px(left), px(top)), size(px(width), px(height))),
        tinted(background, background_alpha),
    ));
    BackgroundedBlock {
        left: left + 4.0,
        top,
        line_height,
    }
}
