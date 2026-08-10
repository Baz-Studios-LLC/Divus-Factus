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
    /// When this frame's main-world work began.
    opened: Mutex<Option<Instant>>,
    /// Main-world seconds, whole-frame seconds, and frames, since the report.
    whole: Mutex<(f64, f64, u32)>,
    /// This ONE frame's watched systems, cleared at the end of every frame.
    this_frame: Mutex<HashMap<&'static str, f64>>,
    /// When a slow frame was last reported, so a sustained bad patch says so
    /// once every couple of seconds rather than a dozen times a second.
    last_cried: Mutex<Option<Instant>>,
    /// Spans opened by name and not yet closed. See [`open_span`].
    spans: Mutex<HashMap<&'static str, Instant>>,
}

impl Default for Timings {
    fn default() -> Self {
        Timings {
            on: std::env::var("DIVUS_FACTUS_TIMINGS").is_ok(),
            spent: Mutex::new(HashMap::new()),
            since: Mutex::new(None),
            opened: Mutex::new(None),
            whole: Mutex::new((0.0, 0.0, 0)),
            this_frame: Mutex::new(HashMap::new()),
            last_cried: Mutex::new(None),
            spans: Mutex::new(HashMap::new()),
        }
    }
}

/// A main-world frame worse than this is worth a line of its own.
///
/// Well over the sixteen-and-two-thirds a sixty-hertz frame is allowed and well
/// under the eighty this is hunting, so an ordinary frame never trips it and a
/// bad one always does.
const A_SLOW_FRAME: f64 = 0.025;

/// Opens the frame's own stopwatch, first thing.
fn open_the_frame(timings: Res<Timings>) {
    if !timings.on {
        return;
    }
    if let Ok(mut opened) = timings.opened.lock() {
        *opened = Some(Instant::now());
    }
}

/// Closes it, last thing, and banks both halves of the split.
///
/// The whole point of the pair. Every watch below can only ever exonerate a
/// system that carries one, and a handful do — so "the systems add up to half a
/// millisecond" was a fact about those systems and not about the game. A frame
/// of ninety milliseconds with half a millisecond accounted for is a
/// measurement that has not begun.
///
/// This splits the frame in two before any of it is attributed: what the MAIN
/// WORLD spent — every system in every schedule, watched or not — and what is
/// left, which is the renderer extracting, queueing, preparing, drawing and
/// presenting. One number says which half of the engine to go and look in, and
/// that is the question every frame-time hunt in this project has had to answer
/// first, the hard way, each time.
fn close_the_frame(timings: Res<Timings>, time: Res<Time<Real>>) {
    if !timings.on {
        return;
    }
    let Ok(opened) = timings.opened.lock() else {
        return;
    };
    let Some(from) = *opened else {
        return;
    };
    let main = from.elapsed().as_secs_f64();
    if let Ok(mut whole) = timings.whole.lock() {
        whole.0 += main;
        whole.1 += time.delta_secs_f64();
        whole.2 += 1;
    }

    // This frame's own watched systems, taken and cleared either way.
    let mut named: Vec<(&'static str, f64)> = timings
        .this_frame
        .lock()
        .map(|mut frame| {
            let taken = frame.iter().map(|(name, spent)| (*name, *spent)).collect();
            frame.clear();
            taken
        })
        .unwrap_or_default();

    // A bad frame says so AT THE TIME. The two-second averages below cannot
    // see this: the trouble comes in bursts of a few seconds, and a burst
    // averaged with the good frames either side of it reads as a mild
    // wobble. Brett had it exactly - "sometimes it dips", "the fps is all
    // over the place" - and an average is the one instrument that cannot
    // answer a complaint phrased that way.
    if main < A_SLOW_FRAME {
        return;
    }
    let now = Instant::now();
    let Ok(mut cried) = timings.last_cried.lock() else {
        return;
    };
    if cried.is_some_and(|last| now.duration_since(last).as_secs_f32() < 2.0) {
        return;
    }
    *cried = Some(now);

    named.sort_by(|a, b| b.1.total_cmp(&a.1));
    let watched: f64 = named.iter().map(|(_, spent)| spent).sum();
    let told = named
        .iter()
        .take(4)
        .map(|(name, spent)| format!("{name} {:.1}", spent * 1000.0))
        .collect::<Vec<_>>()
        .join(", ");
    // The number that matters is the LAST one. If a slow frame is nearly all
    // "unwatched", the culprit is a system with no stopwatch on it and the net
    // has to be thrown wider; if it is small, the name is already in the list.
    info!(
        "a slow frame: {:.1}ms in the main world - {told} - and {:.1}ms unwatched",
        main * 1000.0,
        (main - watched).max(0.0) * 1000.0,
    );
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
        let took = from.elapsed().as_secs_f64();
        if let Ok(mut spent) = self.timings.spent.lock() {
            let entry = spent.entry(self.name).or_insert((0.0, 0));
            entry.0 += took;
            entry.1 += 1;
        }
        // And into this frame's own tally, which `close_the_frame` reads and
        // empties. A system that runs twice in a frame adds to itself.
        if let Ok(mut frame) = self.timings.this_frame.lock() {
            *frame.entry(self.name).or_insert(0.0) += took;
        }
    }
}

/// Opens a named span at this point in the schedule; the [`close_span`]
/// of the same name banks the wall-clock between the two into the report,
/// as if the span were one watched system. The pair bracket ground a
/// `watch` inside one system cannot cover: a whole chain of systems, or
/// the engine's own tail of the frame. A span is wall time between two
/// scheduling points - systems from elsewhere may run inside it.
pub fn open_span(name: &'static str) -> impl Fn(Res<Timings>) {
    move |timings: Res<Timings>| {
        if !timings.on {
            return;
        }
        if let Ok(mut spans) = timings.spans.lock() {
            spans.insert(name, Instant::now());
        }
    }
}

/// The other end of [`open_span`].
pub fn close_span(name: &'static str) -> impl Fn(Res<Timings>) {
    move |timings: Res<Timings>| {
        if !timings.on {
            return;
        }
        let Some(from) = timings
            .spans
            .lock()
            .ok()
            .and_then(|mut spans| spans.remove(name))
        else {
            return;
        };
        let took = from.elapsed().as_secs_f64();
        if let Ok(mut spent) = timings.spent.lock() {
            let entry = spent.entry(name).or_insert((0.0, 0));
            entry.0 += took;
            entry.1 += 1;
        }
        // Spans stay OUT of the per-frame tally on purpose. A span is a
        // wall-clock window that may hold other systems' work, so counting
        // it beside the watches double-books the frame: the slow-frame
        // line once read "0.0ms unwatched" while a hundred unwatched
        // milliseconds hid inside overlapping spans. The 2s report tells
        // spans from `spent`; the burst line tells only true watches, and
        // its unwatched column stays honest.
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
    let frames = rows
        .iter()
        .map(|(_, _, hits)| *hits)
        .max()
        .unwrap_or(1)
        .max(1);
    info!(
        "where the frames went, over {:.1}s and {frames} frames:",
        over
    );

    // The split first, because it says which half of the engine the rest of
    // this list is even relevant to. See `close_the_frame`.
    let mut whole = timings.whole.lock().unwrap();
    let (main, frame, counted) = *whole;
    *whole = (0.0, 0.0, 0);
    drop(whole);
    if counted > 0 {
        let each = counted as f64;
        let main = main * 1000.0 / each;
        let frame = frame * 1000.0 / each;
        info!(
            "  {frame:>7.2}ms  THE FRAME  = {main:.2}ms main world (every system) \
             + {:.2}ms renderer and present",
            (frame - main).max(0.0),
        );
    }

    // Two dozen rows, not ten: the chain-bracket spans ride at the top of
    // any bad window, and a cap of ten once hid the guilty system's own
    // watch beneath eight brackets - a hundred milliseconds sat at row
    // eleven while the hunt blamed ghosts.
    for (name, total, hits) in rows.iter().take(24) {
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
        // Chained on purpose: the frame has to be closed before it is
        // reported, or the split reads a frame's worth of nothing.
        // NOTHING of the stopwatch's may live in the engine's schedules
        // beyond the frame clock that always has. The tail was measured
        // once with probes threaded between Bevy's PostUpdate sets, and
        // once more with a single edge-free system merely REGISTERED
        // there - and either way the registration re-sorted the
        // neighbourhood where visibility and shadow-caster lists are
        // built, and settled a bistable race: the world's shadows came
        // up flickering, or stuck off, depending on the sort. Spans
        // bracket OUR systems in OUR schedules; the engine's tail is
        // main-world minus Update's spans, derived, not probed.
        app.init_resource::<Timings>()
            .add_systems(First, open_the_frame)
            .add_systems(Last, (close_the_frame, report_timings).chain());
    }
}
