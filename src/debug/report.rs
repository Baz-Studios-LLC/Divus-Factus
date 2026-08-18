//! The run report: a page about the run that just happened, written as it
//! happens.
//!
//! Brett: "We should have the game dump a log file to a md or txt in the log
//! folder so when I run you can read that after I test it." Which is the right
//! instrument for the way this project actually works — he plays, I read — and
//! it replaces the worst part of that loop: a person reading five numbers off a
//! HUD and typing them into a message.
//!
//! `logs/run.md`, and the run before it kept as `logs/run-previous.md`. Written
//! CONTINUOUSLY and flushed, so a force-quit or a freeze still leaves the
//! evidence, and so it can be read live while the game is still up.
//!
//! What makes it worth more than the log it replaces: THE SWITCHES ARE IN IT.
//! Every framerate row carries the configuration it was taken under, and a
//! flipped switch starts a new section. An A/B stops being a thing anybody has
//! to remember doing — the file simply has two blocks in it, and the difference
//! between them is the answer.
//!
//! Read the warning on [`report_frames`] before believing any single number
//! here. This machine's frame time drifts several milliseconds between batches;
//! two adjacent sections of one run are worth far more than two runs.

use bevy::diagnostic::DiagnosticsStore;
use bevy::prelude::*;
use std::io::Write;
use std::sync::Mutex;
use std::time::Instant;

use crate::terrain::LoadedChunks;

/// The open page. `None` until the first write, and after any failure to open
/// it — a report that cannot be written is not worth a crash or a log line
/// every five seconds.
static PAGE: Mutex<Option<Page>> = Mutex::new(None);

struct Page {
    to: std::fs::File,
    /// Whether a table is open and its header already written. Anything that is
    /// not a row closes it, and the next row writes a fresh header — a table
    /// with prose in the middle of it is not a table.
    row_open: bool,
    /// The last configuration written as a heading, so only CHANGES speak.
    setting: String,
    /// When the per-system table was last copied in. See [`note_systems`].
    systems_at: Option<Instant>,
}

/// Where the report goes. The same root the want-list uses, so both land beside
/// the game rather than beside whatever launched it.
fn at() -> std::path::PathBuf {
    let root = std::env::var("BEVY_ASSET_ROOT").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(root).join("logs")
}

/// Starts a fresh page, keeping the last one.
///
/// One line of header, and then the dials — because the single most expensive
/// mistake made with these instruments was reading "no slow frames" off a
/// stopwatch that was switched off. A report that does not say which
/// instruments were live is a report that can be believed wrongly.
pub fn open_the_report(seed: Res<crate::WorldSeed>) {
    let dir = at();
    let _ = std::fs::create_dir_all(&dir);
    let page = dir.join("run.md");
    let _ = std::fs::rename(&page, dir.join("run-previous.md"));

    let Ok(mut to) = std::fs::File::create(&page) else {
        return;
    };
    // The shell knows the date and this does not, so the launcher hands it
    // over. Unstamped is not worth pulling a calendar in for.
    let when = std::env::var("DIVUS_FACTUS_RUN_STAMP").unwrap_or_else(|_| "unstamped".to_string());
    let dial = |name: &str| {
        if std::env::var(name).is_ok() {
            "on"
        } else {
            "off"
        }
    };
    let _ = write!(
        to,
        "# The run of {when}\n\n\
         - world seed **{}** — `DIVUS_FACTUS_SEED={}` returns here\n\
         - build: {}, features: {}\n\
         - per-system stopwatch: **{}** (`DIVUS_FACTUS_TIMINGS=1`)\n\
         - passes forbidden outright: {}\n\n\
         Every row is one five-second window. A heading means a switch moved; \
         compare the blocks, not the rows.\n",
        seed.0,
        seed.0,
        if cfg!(debug_assertions) { "dev" } else { "release" },
        if cfg!(feature = "living-voice") {
            "living-voice"
        } else {
            "none"
        },
        dial("DIVUS_FACTUS_TIMINGS"),
        match std::env::var("DIVUS_FACTUS_NO") {
            Ok(off) if !off.is_empty() => off,
            _ => "none".to_string(),
        },
    );
    let _ = to.flush();
    *held() = Some(Page {
        to,
        row_open: false,
        setting: String::new(),
        systems_at: None,
    });
}

fn held() -> std::sync::MutexGuard<'static, Option<Page>> {
    PAGE.lock().unwrap_or_else(|held| held.into_inner())
}

/// The table's columns, in one place so the header and the rows cannot part.
const COLUMNS: [&str; 11] = [
    "clock", "fps", "avg ms", "worst", "zoom", "shown", "standing", "owed", "chunks", "casters",
    "folk",
];

impl Page {
    /// Writes prose, closing any open table first.
    fn prose(&mut self, text: &str) {
        self.row_open = false;
        let _ = write!(self.to, "\n{text}\n");
        let _ = self.to.flush();
    }

    fn row(&mut self, cells: &[String]) {
        if !self.row_open {
            let _ = write!(
                self.to,
                "\n| {} |\n|{}|\n",
                COLUMNS.join(" | "),
                COLUMNS
                    .iter()
                    .map(|_| " --- ")
                    .collect::<Vec<_>>()
                    .join("|")
            );
            self.row_open = true;
        }
        let _ = write!(self.to, "| {} |\n", cells.join(" | "));
        let _ = self.to.flush();
    }
}

/// The per-system table, copied in from the stopwatch.
///
/// THIRTY SECONDS APART AT MOST, and only the rows that cost something. The
/// stopwatch says its piece every two seconds, and all of it would bury the
/// framerate rows this file exists for — three hundred blocks in a ten minute
/// run, which is a file nobody reads.
pub fn note_systems(over: f64, frames: u32, split: Option<(f64, f64)>, rows: &[(String, f64)]) {
    let mut page = held();
    let Some(page) = page.as_mut() else {
        return;
    };
    let now = Instant::now();
    if page
        .systems_at
        .is_some_and(|last| now.duration_since(last).as_secs_f32() < 30.0)
    {
        return;
    }
    page.systems_at = Some(now);

    let mut said = format!("where the frame went, over {over:.1}s and {frames} frames:\n\n```\n");
    if let Some((frame, main)) = split {
        said.push_str(&format!(
            "  {frame:>7.2}ms  THE FRAME  = {main:.2}ms main world + {:.2}ms renderer\n",
            (frame - main).max(0.0)
        ));
    }
    for (name, each) in rows.iter().take(8) {
        said.push_str(&format!("  {each:>7.2}ms/frame  {name}\n"));
    }
    said.push_str("```");
    page.prose(&said);
}

/// Frame telemetry, to the report and — when its dial is set — to the log.
///
/// READ THIS BEFORE TRUSTING A NUMBER FROM HERE. This machine's frame time
/// drifts between BATCHES of runs by three milliseconds and sometimes six —
/// larger than most of the effects worth hunting. It has been caught twice:
/// once as a run that held a flat 16.7ms across four windows and then measured
/// 22.6ms three times running from a byte-identical command line, and once as a
/// four-way comparison where every configuration in the first pass came out at
/// 27.3ms and every one in the second at 24.4ms — the pass, not the setting.
///
/// So a single run proves nothing and two runs in sequence prove nothing. The
/// only measurement that survives is A and B ALTERNATED inside one batch, read
/// as the average gap between adjacent pairs, three alternations minimum. Every
/// number written into this codebase as a measured fact was taken that way; the
/// ones that were not are how two hours went into chasing a cost that turned out
/// to be the GPU's clock speed.
///
/// Which is why the switches are written beside the numbers now: alternating
/// inside one run is the only cheap way to do this, and the file records it
/// without anybody having to keep notes.
pub(crate) fn report_frames(
    time: Res<Time<Real>>,
    real: Res<Time<Real>>,
    mut window: Local<(f32, u32, f32)>,
    // The suspects, counted alongside the frame times - a drop into the
    // teens is only diagnosable if the same row says what the world
    // was doing at the time.
    rigs: Query<&crate::camera::CameraRig>,
    chunks: Option<Res<LoadedChunks>>,
    lit: Query<(), (With<Mesh3d>, Without<bevy::light::NotShadowCaster>)>,
    folk: Query<(), With<crate::villager::Villager>>,
    // Shown against standing, because those two are a different question from
    // all of the above. A patch that is resident but hidden costs memory; one
    // that is SHOWN costs the frame, and the gap between the two is the whole
    // subject of the limb cull.
    detail: Res<crate::globe::PlanetDetail>,
    view: crate::title::TheViewSeen,
) {
    let ms = time.delta_secs() * 1000.0;
    window.0 += ms;
    window.1 += 1;
    window.2 = window.2.max(ms);
    if window.0 < 5000.0 {
        return;
    }
    let over = std::mem::replace(&mut *window, (0.0, 0, 0.0));
    let avg = over.0 / over.1 as f32;
    let distance = rigs.iter().next().map_or(0.0, |rig| rig.distance);
    let standing = chunks.as_ref().map_or(0, |c| c.count());

    if std::env::var("DIVUS_FACTUS_FRAMES").is_ok() {
        info!(
            "frames: avg {avg:.1}ms, worst {:.1}ms over {} frames | zoom {distance:.0} \
             chunks {standing} casters {} patches {}/{} owed {} folk {}",
            over.2,
            over.1,
            lit.iter().count(),
            detail.shown,
            detail.built,
            detail.owed,
            folk.iter().count(),
        );
    }

    let mut page = held();
    let Some(page) = page.as_mut() else {
        return;
    };
    // A switch moved: a new section, named by what is off now. THE HEADING IS
    // THE EXPERIMENT - everything under it was measured that way.
    let setting = view.what_is_off();
    if setting != page.setting {
        let up = real.elapsed_secs() as u32;
        page.prose(&format!(
            "### {}:{:02} — {}",
            up / 60,
            up % 60,
            if setting.is_empty() {
                "everything on".to_string()
            } else {
                format!("off: {setting}")
            }
        ));
        page.setting = setting;
    }
    let up = real.elapsed_secs() as u32;
    page.row(&[
        format!("{}:{:02}", up / 60, up % 60),
        format!("{:.0}", 1000.0 / avg.max(0.001)),
        format!("{avg:.1}"),
        format!("{:.1}", over.2),
        format!("{distance:.0}"),
        detail.shown.to_string(),
        detail.built.to_string(),
        detail.owed.to_string(),
        standing.to_string(),
        lit.iter().count().to_string(),
        folk.iter().count().to_string(),
    ]);
}

/// A closing line, so a report that ends can be told from one that was killed.
pub(crate) fn close_the_report(
    ending: MessageReader<AppExit>,
    real: Res<Time<Real>>,
    fps: Res<DiagnosticsStore>,
) {
    if ending.is_empty() {
        return;
    }
    let up = real.elapsed_secs() as u32;
    let last = fps
        .get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.value())
        .unwrap_or(0.0);
    let mut page = held();
    if let Some(page) = page.as_mut() {
        page.prose(&format!(
            "---\n\nThe run ended after {}:{:02}, last reading {last:.0} fps.",
            up / 60,
            up % 60
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A table interrupted by prose starts a fresh table.
    ///
    /// The only rule in this file with a way to be wrong: markdown wants a
    /// table's rows contiguous, and this page deliberately writes headings and
    /// system dumps between them. Without the reopened header the second half of
    /// every section renders as literal pipes and the file is unreadable exactly
    /// when it matters - after a switch moved.
    #[test]
    fn prose_between_rows_starts_a_new_table() {
        let at = std::env::temp_dir().join(format!("divus-report-{}.md", std::process::id()));
        let mut page = Page {
            to: std::fs::File::create(&at).expect("a temp file"),
            row_open: false,
            setting: String::new(),
            systems_at: None,
        };
        let row = |n: &str| vec![n.to_string(); COLUMNS.len()];
        page.row(&row("one"));
        page.prose("### 0:05 — off: the fog");
        page.row(&row("two"));
        page.row(&row("three"));

        let written = std::fs::read_to_string(&at).expect("read back");
        let _ = std::fs::remove_file(&at);
        assert_eq!(
            written.matches("| clock |").count(),
            2,
            "the header did not come back after the heading:\n{written}"
        );
        // And every row is a row: a cell count that drifts from the header is
        // the other way this file goes quietly unreadable.
        for line in written.lines().filter(|l| l.starts_with("| one")) {
            assert_eq!(line.matches('|').count(), COLUMNS.len() + 1);
        }
    }
}
