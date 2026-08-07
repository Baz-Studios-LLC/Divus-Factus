//! What each system actually costs, said out loud.
//!
//! `DIVUS_FACTUS_TIMINGS=1` prints a few lines every couple of seconds naming
//! the systems that spent the frame, worst first.
//!
//! This exists because the alternative does not work. Wall-clock averages over
//! a whole frame can only be bisected - turn a thing off, measure, turn it back
//! on - and on a machine doing anything else the spread between two runs of the
//! SAME build is wider than the difference being looked for. Chasing the title
//! screen's stutter that way ruled out patch building, chunk streaming and the
//! drift rate itself, all wrongly, before an alternated A/B showed the numbers
//! were measuring the machine rather than the code.
//!
//! A system that reports its own time cannot be argued with in that way. The
//! cost is one `Instant::now()` at each end, which is tens of nanoseconds
//! against systems that take milliseconds, and nothing at all when the dial is
//! off - the guard checks one bool.

use bevy::prelude::*;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

/// Where the frame went, by system name.
#[derive(Resource)]
pub struct Timings {
    on: bool,
    /// Name to (total seconds, times entered) since the last report.
    spent: Mutex<HashMap<&'static str, (f64, u32)>>,
    since: Mutex<Option<Instant>>,
}

impl Default for Timings {
    fn default() -> Self {
        Timings {
            on: std::env::var("DIVUS_FACTUS_TIMINGS").is_ok(),
            spent: Mutex::new(HashMap::new()),
            since: Mutex::new(None),
        }
    }
}

impl Timings {
    /// Times a system for as long as the returned guard lives.
    ///
    /// Takes `&self` rather than `&mut self` so a system can hold it as a plain
    /// `Res` alongside everything else it wants - asking for `ResMut` would
    /// make every instrumented system conflict with every other and force the
    /// whole schedule to run one at a time, which would change the very thing
    /// being measured.
    pub fn watch(&self, name: &'static str) -> Watch<'_> {
        Watch {
            timings: self,
            name,
            from: self.on.then(Instant::now),
        }
    }
}

pub struct Watch<'a> {
    timings: &'a Timings,
    name: &'static str,
    from: Option<Instant>,
}

impl Drop for Watch<'_> {
    fn drop(&mut self) {
        let Some(from) = self.from else {
            return;
        };
        let Ok(mut spent) = self.timings.spent.lock() else {
            return;
        };
        let entry = spent.entry(self.name).or_insert((0.0, 0));
        entry.0 += from.elapsed().as_secs_f64();
        entry.1 += 1;
    }
}

/// Says where the frames went, every couple of seconds.
pub fn report_timings(timings: Res<Timings>) {
    if !timings.on {
        return;
    }
    let now = Instant::now();
    let mut since = timings.since.lock().unwrap();
    let start = *since.get_or_insert(now);
    if now.duration_since(start).as_secs_f32() < 2.0 {
        return;
    }
    *since = Some(now);

    let mut spent = timings.spent.lock().unwrap();
    let over = now.duration_since(start).as_secs_f64();
    let mut rows: Vec<(&'static str, f64, u32)> = spent
        .iter()
        .map(|(name, (total, hits))| (*name, *total, *hits))
        .collect();
    spent.clear();
    drop(spent);

    rows.sort_by(|a, b| b.1.total_cmp(&a.1));
    // Per FRAME, not per call: a system that runs three times a frame and a
    // system that runs once are both answering "how much of the frame did you
    // take", and that is the only question here.
    let frames = rows.iter().map(|(_, _, hits)| *hits).max().unwrap_or(1).max(1);
    info!("where the frames went, over {:.1}s and {frames} frames:", over);
    for (name, total, hits) in rows.iter().take(10) {
        info!(
            "  {:>7.2}ms/frame  {name}  ({hits} calls)",
            total * 1000.0 / frames as f64,
        );
    }
}

/// Carried by both the game and the planet bench, so the systems they share can
/// ask for it without caring which they are running in.
pub struct TimingsPlugin;

impl Plugin for TimingsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Timings>()
            .add_systems(Last, report_timings);
    }
}
