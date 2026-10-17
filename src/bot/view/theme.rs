//! Palette, shared widgets and header icons.

use super::*;

pub(crate) const BG: u32 = 0x3c3f41;

pub(crate) const TOP: u32 = 0x45494a;

pub(crate) const PANEL: u32 = 0x4a4e50;

pub(crate) const PANEL_DARK: u32 = 0x35383a;

pub(crate) const PANEL_HOVER: u32 = 0x56595c;

pub(crate) const LINE: u32 = 0x5f6366;

pub(crate) const LINE_SOFT: u32 = 0x505456;

pub(crate) const TEXT: u32 = 0xececec;

pub(crate) const MUTED: u32 = 0xb4b8bc;

pub(crate) const FAINT: u32 = 0x8f9396;

pub(crate) const BLUE: u32 = 0x5b8fd9;

pub(crate) const BLUE_HOVER: u32 = 0x6c9fe4;

pub(crate) const BLUE_TINT: u32 = 0x4a6fa5;

pub(crate) const GREEN: u32 = 0x3da56d;

pub(crate) const GREEN_HOVER: u32 = 0x4ab57d;

pub(crate) const GREEN_TINT: u32 = 0x3f6e52;

pub(crate) const RED: u32 = 0xc75b5b;

pub(crate) const RED_HOVER: u32 = 0xd66c6c;

pub(crate) const RED_TINT: u32 = 0x6b474b;

pub(crate) const AMBER: u32 = 0xcf9f4f;

pub(crate) const MAP_CHIP: u32 = 0x2f3b44;

pub(crate) const MAP_LINE: u32 = 0x4a5a66;

pub(crate) fn label(text: &str, color: u32, size: f32) -> gpui_kit::Div {
    div()
        .text_size(px(size.max(12.0)))
        .text_color(rgb(color))
        .child(text.to_string())
}

pub(crate) fn legend_swatch(name: &str, color: u32) -> gpui_kit::Div {
    div()
        .flex()
        .items_center()
        .gap(px(5.0))
        .child(div().size(px(7.0)).rounded_full().bg(rgb(color)))
        .child(label(name, MUTED, 11.0))
}

/// Small-caps panel heading.
pub(crate) fn section(text: &str) -> gpui_kit::Div {
    div()
        .text_size(px(12.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(MUTED))
        .child(text.to_string())
}

/// Raised card surface used by panels and list items.
pub(crate) fn card() -> gpui_kit::Div {
    div()
        .bg(rgb(PANEL))
        .rounded_lg()
        .border_1()
        .border_color(rgb(LINE_SOFT))
}

/// Button fill hierarchy for `button`.
#[derive(Clone, Copy)]
pub(crate) enum Btn {
    /// Accent blue — the primary forward action.
    Primary,
    /// Panel fill — secondary actions.
    Ghost,
    /// Inset fill — low-emphasis secondary actions.
    Subtle,
    /// Green — start/confirm.
    Success,
    /// Red — stop/disconnect.
    Danger,
    /// Accent tint — the selected item in a list or tab strip.
    Selected,
}

pub(crate) fn button(
    id: impl Into<ElementId>,
    text: impl Into<SharedString>,
    kind: Btn,
) -> Stateful<gpui_kit::Div> {
    let (bg, hover, fg) = match kind {
        Btn::Primary => (BLUE, BLUE_HOVER, 0xffffff),
        Btn::Ghost => (PANEL, PANEL_HOVER, TEXT),
        Btn::Subtle => (PANEL_DARK, PANEL_HOVER, TEXT),
        Btn::Success => (GREEN, GREEN_HOVER, 0xffffff),
        Btn::Danger => (RED, RED_HOVER, 0xffffff),
        Btn::Selected => (BLUE_TINT, 0x355070, TEXT),
    };
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .px(px(14.0))
        .py(px(7.0))
        .rounded_md()
        .bg(rgb(bg))
        .hover(move |s| s.bg(rgb(hover)))
        .cursor_pointer()
        .text_size(px(13.0))
        .font_weight(FontWeight::MEDIUM)
        .text_color(rgb(fg))
        .child(text.into())
}

// Segoe MDL2 Assets code points for the reference style header.
pub(crate) const ICON_MENU: &str = "\u{E700}";

pub(crate) const ICON_SETTINGS: &str = "\u{E713}";

pub(crate) const ICON_PLAY: &str = "\u{E768}";

pub(crate) const ICON_PAUSE: &str = "\u{E769}";

pub(crate) const ICON_HOME: &str = "\u{E80F}";

pub(crate) const ICON_VIEW: &str = "\u{E7B3}";

pub(crate) const ICON_PIN: &str = "\u{E718}";

/// Square icon button for the header row; `selected` is the lighter
/// highlight the reference implementation gives an active toggle.
pub(crate) fn header_icon(
    id: &'static str,
    glyph: &'static str,
    selected: bool,
    enabled: bool,
) -> Stateful<gpui_kit::Div> {
    div()
        .id(id)
        .size(px(30.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded_sm()
        .font_family(if glyph.is_ascii() {
            "Segoe UI"
        } else {
            "Segoe MDL2 Assets"
        })
        .text_size(px(15.0))
        .text_color(rgb(if enabled { 0xe6e6e6 } else { FAINT }))
        .when(selected, |this| this.bg(rgb(0x4a4f57)))
        .when(enabled, |this| {
            this.cursor_pointer()
                .hover(|style| style.bg(rgb(PANEL_HOVER)))
        })
        .child(glyph)
}

#[derive(Clone, Copy)]
pub(crate) enum ReadoutIcon {
    Heart,
    Signal,
    Cpu,
    Memory,
}

/// One header readout: a small green icon followed by its value.
pub(crate) fn readout(icon: ReadoutIcon, value: &str) -> gpui_kit::Div {
    let glyph = match icon {
        ReadoutIcon::Heart => div()
            .w(px(12.0))
            .text_size(px(10.0))
            .text_color(rgb(GREEN))
            .child("♥"),
        ReadoutIcon::Signal => div()
            .w(px(12.0))
            .h(px(9.0))
            .flex()
            .items_end()
            .gap(px(1.0))
            .child(div().w(px(2.0)).h(px(3.0)).bg(rgb(GREEN)))
            .child(div().w(px(2.0)).h(px(6.0)).bg(rgb(GREEN)))
            .child(div().w(px(2.0)).h(px(9.0)).bg(rgb(GREEN))),
        ReadoutIcon::Cpu => div().w(px(12.0)).flex().justify_center().child(
            div()
                .w(px(6.0))
                .h(px(9.0))
                .border_1()
                .border_color(rgb(GREEN))
                .bg(rgb(GREEN_TINT)),
        ),
        ReadoutIcon::Memory => div().w(px(12.0)).flex().justify_center().child(
            div()
                .w(px(12.0))
                .h(px(6.0))
                .border_1()
                .border_color(rgb(GREEN))
                .bg(rgb(GREEN_TINT)),
        ),
    };
    div().flex().items_center().gap(px(3.0)).child(glyph).child(
        div()
            .min_w(px(26.0))
            .text_size(px(10.0))
            .text_color(rgb(0xbbbbbb))
            .child(value.to_owned()),
    )
}

/// Windows-style caption button for the custom titlebar. The glyph inherits
/// `text_color`, so `.hover` recolors it; `close` uses the red danger hover.
pub(crate) fn caption_button(
    id: &'static str,
    area: WindowControlArea,
    glyph: &'static str,
    close: bool,
) -> Stateful<gpui_kit::Div> {
    div()
        .id(id)
        .w(px(46.0))
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(12.0))
        .text_color(rgb(MUTED))
        .hover(move |s| {
            if close {
                s.bg(rgb(0xe81123)).text_color(rgb(0xffffff))
            } else {
                s.bg(rgb(PANEL_HOVER)).text_color(rgb(TEXT))
            }
        })
        .window_control_area(area)
        .child(glyph)
}

/// Connection/health indicator: a colored dot inside a bordered chip.
pub(crate) fn status_pill(text: &str, dot: u32) -> gpui_kit::Div {
    div()
        .flex()
        .items_center()
        .gap(px(7.0))
        .pl(px(10.0))
        .pr(px(12.0))
        .py(px(6.0))
        .rounded_full()
        .bg(rgb(PANEL_DARK))
        .border_1()
        .border_color(rgb(LINE))
        .child(div().size(px(7.0)).rounded_full().bg(rgb(dot)))
        .child(label(text, TEXT, 12.0))
}

pub(crate) fn status_row(title: &str, value: &str, color: u32) -> gpui_kit::Div {
    div()
        .flex()
        .justify_between()
        .items_center()
        .child(label(title, MUTED, 12.0))
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(color))
                .child(value.to_string()),
        )
}
