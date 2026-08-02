//! The voice corpus: every line a villager can say, authored by hand and
//! chosen by circumstance.
//!
//! This replaces a 1.5B-parameter model that wrote "I wonder where the
//! next meal will come from" in a hundred barely different ways while
//! costing twenty milliseconds a frame. A line here was written by
//! somebody, for a moment: it carries TAGS, and it is eligible only when
//! every tag it wears is true of the moment it would be spoken into.
//! Among the eligible, specificity wins - the line written for a roofless
//! devout fisher in a storm beats the line written for anybody - and
//! freshness is enforced rather than hoped for: every hearing is counted,
//! the counts live in the save, and the selector reaches for what this
//! world has heard least. A line marked `once` is retired the first time
//! it is ever said.

use std::collections::HashMap;

use crate::rng::Rng;

/// One authored line.
#[derive(serde::Deserialize, Clone)]
pub struct Line {
    /// The words, with `{name}`, `{whom}`, `{god}`, `{place}`, `{spouse}`
    /// slots. Using a slot silently requires the moment to supply it.
    pub t: String,
    /// Every tag must hold for the line to be eligible.
    pub tags: Vec<String>,
    /// A thumb on the scale among equally specific rivals.
    #[serde(default = "one")]
    pub w: f32,
    /// Said at most once per world, ever - for lines too distinctive to
    /// repeat without the seams showing.
    #[serde(default)]
    pub once: bool,
}

fn one() -> f32 {
    1.0
}

/// The corpus, plus the world's memory of what has been said in it.
#[derive(Default)]
pub struct Corpus {
    lines: Vec<Line>,
    /// How often each line (by stable id) has been heard in THIS world.
    /// Ids hash the words themselves, so a save survives the corpus being
    /// reordered, grown, or trimmed between releases.
    heard: HashMap<u64, u32>,
    /// The last dozen lines out of each speaker's mouth, so nobody
    /// repeats themselves in an evening even when the eligible pool is
    /// thin. Speakers are keyed by entity bits.
    recent: HashMap<u64, Vec<u64>>,
    /// Moments the corpus failed: nothing eligible, or nothing deeper
    /// than filler for a moment that deserved better. Tallied by the
    /// moment's own tags, and written out as the authoring want-list -
    /// the corpus grows toward what the world actually produces.
    wanting: HashMap<String, u32>,
}

/// A stable id for a line: FNV-1a over its words.
fn id_of(text: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in text.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

impl Corpus {
    /// Reads every `assets/voice/*.json` beside the game, merged. The
    /// same two-root dance as the baked buildings: the launcher's copy
    /// first, the workspace's for a dev run.
    pub fn load() -> Corpus {
        let mut lines: Vec<Line> = Vec::new();
        let root = std::env::var("BEVY_ASSET_ROOT").unwrap_or_else(|_| ".".to_string());
        for dir in [
            std::path::PathBuf::from(&root).join("assets/voice"),
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/voice"),
        ] {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_none_or(|e| e != "json") {
                    continue;
                }
                match std::fs::read_to_string(&path)
                    .ok()
                    .and_then(|text| serde_json::from_str::<Vec<Line>>(&text).ok())
                {
                    Some(batch) => lines.extend(batch),
                    None => bevy::log::warn!("unreadable voice file: {}", path.display()),
                }
            }
            if !lines.is_empty() {
                break;
            }
        }
        bevy::log::info!("the corpus holds {} lines", lines.len());
        Corpus {
            lines,
            ..Default::default()
        }
    }

    /// Picks the freshest, most specific line for the moment, fills its
    /// slots, and records the hearing. `context` is every tag true right
    /// now; `slots` is every datum the moment can supply.
    pub fn pick(
        &mut self,
        speaker: u64,
        context: &[&str],
        slots: &[(&str, &str)],
        rng: &mut Rng,
    ) -> Option<String> {
        let said = self.recent.entry(speaker).or_default().clone();
        let mut best: Option<(f32, &Line, u64)> = None;
        for line in &self.lines {
            if !line.tags.iter().all(|tag| context.contains(&tag.as_str())) {
                continue;
            }
            // A slot the moment cannot fill rules the line out.
            if ["{whom}", "{place}", "{spouse}", "{name}", "{god}"]
                .iter()
                .any(|slot| {
                    line.t.contains(slot)
                        && !slots.iter().any(|(key, _)| format!("{{{key}}}") == *slot)
                })
            {
                continue;
            }
            let id = id_of(&line.t);
            let heard = self.heard.get(&id).copied().unwrap_or(0);
            if line.once && heard > 0 {
                continue;
            }
            // Repeating yourself is a fault; silence is a worse one. A
            // line this speaker just said takes a penalty no fresh rival
            // ever loses to - but when the pool is thin enough that the
            // rivals ran out, it goes out again rather than not at all,
            // and the worn-pool tally below records what that cost.
            let echo = if said.contains(&id) { 100.0 } else { 0.0 };
            // Specificity first, then freshness, then the dice.
            let score = line.tags.len() as f32 * 10.0 - heard as f32 * 4.0 - echo
                + line.w * rng.range(0.0, 3.0);
            if best.as_ref().is_none_or(|(top, ..)| score > *top) {
                best = Some((score, line, id));
            }
        }
        // A pool worn thin shows as repetition before it shows as
        // anything else: a line going out for the third time means the
        // moments that reach for it outnumber the lines that answer
        // them, whatever the corpus's total size says. File it under the
        // same want-list - more lines needed HERE.
        if let Some((_, line, id)) = &best
            && self.heard.get(id).copied().unwrap_or(0) >= 2
        {
            let mut worn: Vec<&str> = line.tags.iter().map(|t| t.as_str()).collect();
            worn.sort_unstable();
            *self
                .wanting
                .entry(format!("(worn pool) {}", worn.join(" ")))
                .or_default() += 1;
        }
        // A miss, or a moment much deeper than the best line found for
        // it, is a line somebody should sit down and write.
        let depth = best
            .as_ref()
            .map(|(_, line, _)| line.tags.len())
            .unwrap_or(0);
        if depth == 0 || (context.len() >= 5 && depth <= 1) {
            let mut moment: Vec<&str> = context.to_vec();
            moment.sort_unstable();
            *self.wanting.entry(moment.join(" ")).or_default() += 1;
        }
        let (_, line, id) = best?;
        let mut words = line.t.clone();
        for (key, value) in slots {
            words = words.replace(&format!("{{{key}}}"), value);
        }
        *self.heard.entry(id).or_default() += 1;
        let ring = self.recent.entry(speaker).or_default();
        ring.push(id);
        if ring.len() > 12 {
            ring.remove(0);
        }
        Some(words)
    }

    /// Writes the want-list where the maker will trip over it: every
    /// moment the corpus had nothing worthy to say, worst offenders
    /// first, ready to be turned into lines. Truthful accounting beats a
    /// tidy file, so it rewrites whole each time.
    #[allow(dead_code)] // flushed on a slow timer once wired
    pub fn write_wanting(&self) {
        if self.wanting.is_empty() {
            return;
        }
        let root = std::env::var("BEVY_ASSET_ROOT").unwrap_or_else(|_| ".".to_string());
        let mut rows: Vec<(&String, &u32)> = self.wanting.iter().collect();
        rows.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
        let body: String = rows
            .iter()
            .map(|(moment, n)| format!("{n:5}  {moment}\n"))
            .collect();
        let _ = std::fs::write(
            std::path::PathBuf::from(root).join("voice-wanted.txt"),
            format!(
                "# Moments that went without words - write lines for these.\n# count  tags of the moment\n{body}"
            ),
        );
    }

    /// How many lines the book holds.
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// The hearing ledger, for the save file.
    #[allow(dead_code)] // the save file's, once the ledger is wired in
    pub fn export_heard(&self) -> Vec<(u64, u32)> {
        self.heard.iter().map(|(id, n)| (*id, *n)).collect()
    }

    /// Restores the ledger from a save, so `once` means once per world
    /// and not once per sitting.
    #[allow(dead_code)] // likewise
    pub fn import_heard(&mut self, heard: &[(u64, u32)]) {
        self.heard = heard.iter().copied().collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn corpus(lines: &[(&str, &[&str], bool)]) -> Corpus {
        Corpus {
            lines: lines
                .iter()
                .map(|(t, tags, once)| Line {
                    t: t.to_string(),
                    tags: tags.iter().map(|s| s.to_string()).collect(),
                    w: 1.0,
                    once: *once,
                })
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn specificity_beats_filler_and_once_means_once() {
        let mut voice = corpus(&[
            ("some weather", &["muse"], false),
            (
                "third night on the cold ground",
                &["muse", "roofless", "night"],
                false,
            ),
            (
                "the sky split for {whom}",
                &["tell", "event:lightning"],
                true,
            ),
        ]);
        let mut rng = Rng::new(7);
        // The specific line wins the moment it fits.
        let said = voice
            .pick(1, &["muse", "roofless", "night"], &[], &mut rng)
            .unwrap();
        assert_eq!(said, "third night on the cold ground");
        // A slot the moment cannot fill rules a line out entirely.
        assert!(
            voice
                .pick(1, &["tell", "event:lightning"], &[], &mut rng)
                .is_none()
        );
        // Filled, said once - and never again, even for a new speaker.
        let told = voice
            .pick(
                2,
                &["tell", "event:lightning"],
                &[("whom", "Feitreh")],
                &mut rng,
            )
            .unwrap();
        assert_eq!(told, "the sky split for Feitreh");
        assert!(
            voice
                .pick(
                    3,
                    &["tell", "event:lightning"],
                    &[("whom", "Feitreh")],
                    &mut rng
                )
                .is_none()
        );
        // The ledger round-trips through a save.
        let saved = voice.export_heard();
        let mut fresh = corpus(&[(
            "the sky split for {whom}",
            &["tell", "event:lightning"],
            true,
        )]);
        fresh.import_heard(&saved);
        assert!(
            fresh
                .pick(
                    4,
                    &["tell", "event:lightning"],
                    &[("whom", "Feitreh")],
                    &mut rng
                )
                .is_none()
        );
    }

    #[test]
    fn unanswered_moments_are_written_down() {
        let mut voice = corpus(&[("some weather", &["muse"], false)]);
        let mut rng = Rng::new(1);
        // A plain miss, and a deep moment fobbed off with filler.
        assert!(voice.pick(1, &["cry", "drowning"], &[], &mut rng).is_none());
        voice
            .pick(
                1,
                &["muse", "roofless", "storm", "widowed", "night", "devout"],
                &[],
                &mut rng,
            )
            .unwrap();
        assert_eq!(voice.wanting.len(), 2);
        assert!(voice.wanting.keys().any(|k| k == "cry drowning"));
        // And a pool leaned on three times reports itself worn.
        let mut thin = corpus(&[("again", &["muse"], false)]);
        for speaker in 0..3 {
            thin.pick(speaker, &["muse"], &[], &mut rng).unwrap();
        }
        assert!(thin.wanting.keys().any(|k| k.starts_with("(worn pool)")));
    }

    #[test]
    fn nobody_repeats_themselves_in_an_evening() {
        let mut voice = corpus(&[("a", &["muse"], false), ("b", &["muse"], false)]);
        let mut rng = Rng::new(3);
        let first = voice.pick(9, &["muse"], &[], &mut rng).unwrap();
        let second = voice.pick(9, &["muse"], &[], &mut rng).unwrap();
        assert_ne!(first, second, "the ring must hold the last line out");
        // Both said, pool exhausted: a repeat beats silence, and the
        // early corpus keeps talking while the want-list keeps score.
        assert!(voice.pick(9, &["muse"], &[], &mut rng).is_some());
    }
}
