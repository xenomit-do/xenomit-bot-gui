use super::*;

/// Trail points are sampled on this cadence — the tail is time-based, so
/// its length tracks seconds, not travelled distance.
pub const TRAIL_SAMPLE: Duration = Duration::from_millis(250);

/// Trail points expire after this long.
pub const TRAIL_TTL: Duration = Duration::from_secs(5);

/// A jump larger than this is a teleport or portal; the trail resets.
pub const TRAIL_MAX_JUMP: f32 = 2_000.0;

/// Hero positions sampled from live frames on a fixed cadence; every point
/// expires `TRAIL_TTL` after it was recorded, so the tail stays short.
#[derive(Clone, Debug)]
pub struct HeroTrail {
    points: VecDeque<(f32, f32, Instant)>,
    map_id: Option<i32>,
    last_sample: Option<Instant>,
    ttl: Duration,
}

impl Default for HeroTrail {
    fn default() -> Self {
        Self {
            points: VecDeque::new(),
            map_id: None,
            last_sample: None,
            ttl: TRAIL_TTL,
        }
    }
}

impl HeroTrail {
    pub fn set_ttl(&mut self, ttl: Duration) {
        self.ttl = ttl;
        if ttl.is_zero() {
            self.clear();
        }
    }
    /// Record the newest hero position. Returns true when a point was added.
    /// Points are taken at most once per `TRAIL_SAMPLE` and expire after
    /// `TRAIL_TTL`. Resets on map changes and on jumps beyond
    /// `TRAIL_MAX_JUMP`; frames without a map are ignored without clearing
    /// the existing trail.
    pub fn update(&mut self, map_id: Option<i32>, x: f32, y: f32, now: Instant) -> bool {
        let Some(map_id) = map_id else {
            return false;
        };
        if self.map_id != Some(map_id) {
            self.points.clear();
            self.map_id = Some(map_id);
            self.last_sample = None;
        }
        if let Some(&(last_x, last_y, _)) = self.points.back() {
            if (x - last_x).hypot(y - last_y) > TRAIL_MAX_JUMP {
                self.points.clear();
                self.last_sample = None;
            }
        }
        if self.ttl.is_zero() {
            self.clear();
            return false;
        }
        let due = self
            .last_sample
            .is_none_or(|sampled| now.saturating_duration_since(sampled) >= TRAIL_SAMPLE);
        if due {
            self.points.push_back((x, y, now));
            self.last_sample = Some(now);
        }
        while self
            .points
            .front()
            .is_some_and(|(_, _, recorded)| now.saturating_duration_since(*recorded) > self.ttl)
        {
            self.points.pop_front();
        }
        due
    }

    pub fn clear(&mut self) {
        self.points.clear();
        self.map_id = None;
        self.last_sample = None;
    }

    pub fn points(&self) -> impl Iterator<Item = (f32, f32)> + '_ {
        self.points.iter().map(|&(x, y, _)| (x, y))
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.points.len()
    }
}

// --- text painting -----------------------------------------------------
