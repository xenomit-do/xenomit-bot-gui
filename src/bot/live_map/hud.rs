use std::collections::{BTreeMap, BTreeSet};

/// Text and bar data layered over the map, assembled by the view so this
/// module stays free of statistics/settings types.
#[derive(Clone, Debug, Default)]
pub struct MapHud {
    /// Large centered text — the map name, or `Loading`; empty skips it.
    pub title: String,
    /// `RUNNING`/`WAITING` plus session time, drawn under the title.
    pub status: String,
    /// Left-edge statistics column (`cre/h`, `uri/h`, `exp/h`, `hon/h`,
    /// `cargo`, `death`).
    pub stats: Vec<String>,
    /// Hero configuration label (`1C`/`2C`) at the bottom-left.
    pub config: String,
    /// Hero name + health/shield bar at the bottom-left.
    pub hero: Option<BarHud>,
    /// Locked-target bar at the bottom-right, `Some` while a ship is
    /// selected.
    pub target: Option<BarHud>,
    /// Entity id of the locked target; its marker is painted filled.
    pub target_id: Option<i32>,
    /// Top-left `<reloads> - <since last>/<interval>` line; empty skips it
    /// and the refresh icon.
    pub refresh_info: String,
    /// Module status lines under the refresh counter (top-left).
    pub module_status: Vec<String>,
    /// Top-right text after `SID: `; empty skips it.
    pub sid: String,
    /// Right-edge booster lines: text and color, in display order.
    pub boosters: Vec<(String, u32)>,
    /// Reference style display switches (`USERNAMES`, `ZONES`, and so on).
    pub display_flags: BTreeSet<String>,
    /// RGB values read from `bot_settings.map_display.cs`.
    pub colors: BTreeMap<String, u32>,
    /// Alpha values from the optional eight-digit color suffix.
    pub color_alpha: BTreeMap<String, f32>,
    /// The camera follows the hero when zoomed in.
    pub camera_center: (f32, f32),
    pub zoom: f32,
    pub round_entities: bool,
    /// Group/member summary for the `GROUP_AREA` overlay.
    pub group_lines: Vec<String>,
    /// Index of the zone under the pointer while the zone editor is open.
    pub hovered_zone: Option<usize>,
    /// Live map diagnostics shown only when developer details are enabled.
    pub developer_details: Vec<String>,
    /// Custom background-image opacity, 0..1.
    pub background_opacity: f32,
}

impl MapHud {
    pub fn enabled(&self, name: &str) -> bool {
        self.display_flags.contains(name)
    }

    pub fn color(&self, name: &str, fallback: u32) -> u32 {
        self.colors.get(name).copied().unwrap_or(fallback)
    }

    pub fn alpha(&self, name: &str, fallback: f32) -> f32 {
        self.color_alpha.get(name).copied().unwrap_or(fallback)
    }
}

/// One reference style name + health/shield bar pair.
#[derive(Clone, Debug)]
pub struct BarHud {
    pub name: String,
    pub details: String,
    /// Enemy-style red name vs the hero's neutral one.
    pub enemy: bool,
    pub hp: f32,
    pub max_hp: f32,
    /// Whether `hp` already includes the nanohull pool.
    pub hp_includes_nano: bool,
    pub nano: f32,
    pub nano_max: f32,
    pub shield: f32,
    pub max_shield: f32,
    pub show_numbers: bool,
    pub health_color: u32,
    pub health_background: u32,
    pub nano_color: u32,
    pub shield_color: u32,
    pub shield_background: u32,
    pub text_color: u32,
}

/// `###,###,###` — the reference implementation's stat format, space-grouped like its UI.
pub fn format_amount(value: f64) -> String {
    let negative = value < 0.0;
    let digits = value.abs().round() as u64;
    let mut grouped = String::new();
    for (index, digit) in digits.to_string().chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            grouped.push(' ');
        }
        grouped.push(digit);
    }
    let mut text: String = grouped.chars().rev().collect();
    if negative {
        text.insert(0, '-');
    }
    text
}
