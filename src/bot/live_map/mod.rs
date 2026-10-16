//! Live map painting helpers: the world-to-screen transform, per-kind entity
//! styling, and the hero trail ring buffer. The transform, diplomacy, and
//! trail rules are pure functions so they can be unit-tested without a
//! running game client.
//!
//! The visuals replicate DarkBot's map (see `.dbref` ColorScheme/drawers):
//! a large centered map name — `Loading` while the client is between maps —
//! portal rings labeled with their target map, a left-edge statistics
//! column, hero configuration + health/shield bars at the bottom, and the
//! `going`-color destination line.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use gpui_kit::{
    App, BorderStyle, Bounds, Hsla, PathBuilder, Pixels, Point, ShapedLine, SharedString,
    TextAlign, TextRun, Window, fill, font, outline, point, px, rgb, size,
};
use xenomit_bot_runtime::settings::MapZone;
use xenomit_game_api::live::{LiveEntity, LiveFrame, LiveKind};
use xenomit_game_api::model::SafePlace;
use xenomit_game_api::star_map;
use xenomit_game_api::{PortalInfo, StarSystem};

mod geometry;
mod hud;
mod markers;
mod naming;
mod overlay;
mod paint;
mod palette;
#[cfg(test)]
mod tests;
mod trail;

pub use geometry::*;
pub use hud::*;
pub use markers::*;
pub use naming::*;
pub use overlay::*;
use paint::*;
use palette::*;
pub use trail::*;
