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

/// Picks one option from every `{this|or this|or this}` in a line.
///
/// One authored shape becomes as many utterances as its choices
/// multiply out to - "the {good bushes|near patches} are {picked
/// over|nearly bare}" is four - which is the cheapest way there is to
/// buy variety without writing four lines.
///
/// Deliberately NOT a grammar. The unit an author approves is still a
/// whole sentence: swapping two nouns cannot change what a line means,
/// where a rule that assembles clauses can, and does, and only shows you
/// on the day a farmer says something eerie about the sky.
///
/// A brace with no `|` in it is a slot - `{whom}` - and is left alone.
fn choose_among(line: &str, rng: &mut Rng) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(open) = rest.find('{') {
        let Some(close) = rest[open..].find('}').map(|end| open + end) else {
            break;
        };
        let inside = &rest[open + 1..close];
        out.push_str(&rest[..open]);
        if inside.contains('|') {
            let options: Vec<&str> = inside.split('|').collect();
            out.push_str(options[rng.next_u32() as usize % options.len()]);
        } else {
            // A slot. Left whole for the caller to fill.
            out.push_str(&rest[open..=close]);
        }
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    out
}

/// How many utterances one line can make. Used by the voice bench, and
/// by the test below that keeps the two functions honest with each other.
fn ways_to_say(line: &str) -> usize {
    let mut ways = 1;
    let mut rest = line;
    while let Some(open) = rest.find('{') {
        let Some(close) = rest[open..].find('}').map(|end| open + end) else {
            break;
        };
        let inside = &rest[open + 1..close];
        if inside.contains('|') {
            ways *= inside.split('|').count();
        }
        rest = &rest[close + 1..];
    }
    ways
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
        for dir in crate::carried::roads("assets/voice") {
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
        self.pick_within(speaker, context, slots, &[], rng)
    }

    /// Like [`pick`](Self::pick), but only lines wearing every tag in
    /// `must` may answer — the register wall. A worn pool may repeat
    /// itself, but it may never borrow from the wrong register: Tiwa's
    /// thirteenth food prayer wore out all four hungry-prayer lines and
    /// the picker reached for smalltalk — "quiet today. I don't mind
    /// quiet", said on her knees. Brett: "the text doesnt match the
    /// prayer at all." A stale prayer beats a fluent non sequitur.
    pub fn pick_within(
        &mut self,
        speaker: u64,
        context: &[&str],
        slots: &[(&str, &str)],
        must: &[&str],
        rng: &mut Rng,
    ) -> Option<String> {
        let said = self.recent.entry(speaker).or_default().clone();
        let mut best: Option<(f32, &Line, u64)> = None;
        for line in &self.lines {
            if !line.tags.iter().all(|tag| context.contains(&tag.as_str())) {
                continue;
            }
            if !must
                .iter()
                .all(|need| line.tags.iter().any(|tag| tag == need))
            {
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
        let mut words = choose_among(&line.t, rng);
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

    /// How many different utterances the corpus can actually produce,
    /// counting every way each line's alternations can fall. The line
    /// count undersells a corpus that uses them.
    pub fn utterances(&self) -> usize {
        self.lines.iter().map(|line| ways_to_say(&line.t)).sum()
    }

    /// Writes the want-list where the maker will trip over it: every
    /// moment the corpus had nothing worthy to say, worst offenders
    /// first, ready to be turned into lines. Truthful accounting beats a
    /// tidy file, so it rewrites whole each time.
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
    /// Every line, for the voice bench's coverage audit.
    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

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

    /// Every tag in the corpus must be one the game actually speaks. A
    /// line wearing a tag the moments never carry can NEVER fire - it
    /// just rots silently in the file - and the likeliest way to write
    /// one is a second author (Brett drafts lines with ChatGPT) inventing
    /// or misspelling a tag. New tags are welcome; they are added HERE in
    /// the same change that teaches the game to carry them.
    #[test]
    fn every_tag_is_one_the_game_speaks() {
        const SPOKEN: &[&str] = &[
            // registers
            "muse",
            "tell",
            "reply",
            "yell",
            "prayer",
            // conversation beats
            "chat:open",
            "chat:followup",
            "chat:reply",
            "chat:end",
            "told",
            // the hand axis and wear
            "saw",
            "heard",
            "distant",
            "retold",
            // faith bands
            "devout",
            "wavering",
            "doubting",
            // moments and states
            "devotion",
            "grudge",
            "housed",
            "hungry",
            "hurt",
            "married",
            "night",
            "road",
            "roof",
            "roofless",
            "storm",
            "wolf",
            "worn out",
            // what the act happened to
            "of:person",
            "of:beast",
            "of:thing",
            // events
            "event:delivered",
            "event:flourished",
            "event:impact",
            "event:lifted",
            "event:mauled",
            "event:mended",
            "event:perished",
            "event:provided",
            "event:quaked",
            "event:setdown",
            "event:smote",
            "event:thrown",
            "event:uprooted",
            // trades, in both voices
            "trade:builder",
            "trade:cook",
            "trade:explorer",
            "trade:farmer",
            "trade:fisher",
            "trade:forester",
            "trade:gatherer",
            "trade:guard",
            "trade:healer",
            "trade:hunter",
            "trade:miner",
            "trade:priest",
            "topic:builder",
            "topic:cook",
            "topic:explorer",
            "topic:farmer",
            "topic:fisher",
            "topic:food",
            "topic:forester",
            "topic:gatherer",
            "topic:guard",
            "topic:healer",
            "topic:hunter",
            "topic:miner",
            "topic:priest",
            "topic:roof",
            "topic:weather",
            // What a quarrel is about. Never rolled for: a grievance is
            // named by the simulation or there is no argument to have.
            "quarrel",
            "over:hunger",
            "over:roof",
            "over:idleness",
            "over:grudge",
            "over:faith",
            // Which side of the charge a mouth is on. A quarrel has two
            // ends and they are not interchangeable: without these the
            // corpus could put "you ate while I went hungry" in the
            // mouth of the one who ate.
            "aggrieved",
            "advantaged",
        ];
        let voice = Corpus::load();
        for line in voice.lines() {
            for tag in &line.tags {
                assert!(
                    SPOKEN.contains(&tag.as_str()),
                    "the corpus wears a tag the game never speaks: {tag:?} on {:?}",
                    line.t
                );
            }
        }
    }

    /// Every British spelling this game refuses, and what it says instead.
    ///
    /// THE FORBIDDEN HALF IS BUILT, NOT WRITTEN. Spelled out in full, this
    /// table would contain the very words it forbids - and the day the whole
    /// game was swept from British to American, the sweep did exactly that:
    /// every pair collapsed to ("gray", "gray"), and the gate began failing
    /// the corpus for using the spelling it exists to require. Assembled from
    /// stems, a sweep has nothing to catch hold of.
    fn british_forms() -> Vec<(String, String)> {
        //
        // This table used to spell both halves of every pair out in full -
        // which meant it contained the very words it forbids, and the day the
        // whole game was swept from British to American the sweep corrupted
        // the gate along with everything else: every pair collapsed to
        // ("gray", "gray"), and the test began failing the corpus for using
        // the spelling it was there to require.
        //
        // Assembled from stems instead. A sweep has nothing to catch hold of,
        // and the assertion below refuses to run at all if a pair ever does
        // collapse.
        let ou = |stem: &str| (format!("{stem}our"), format!("{stem}or"));
        let ise = |stem: &str| (format!("{stem}ise"), format!("{stem}ize"));
        let ised = |stem: &str| (format!("{stem}ised"), format!("{stem}ized"));
        let mut drift: Vec<(String, String)> = vec![
            ou("neighb"),
            ou("col"),
            ou("hon"),
            ou("fav"),
            ou("lab"),
            ou("harb"),
            ou("rum"),
            ou("hum"),
            ou("behavi"),
            ise("real"),
            ised("real"),
            ise("recogn"),
            ised("recogn"),
            ise("apolog"),
            ised("apolog"),
            ise("organ"),
            ("marve".to_string() + "llous", "marvelous".to_string()),
            ("trave".to_string() + "lled", "traveled".to_string()),
            ("trave".to_string() + "lling", "traveling".to_string()),
            ("practi".to_string() + "se", "practice".to_string()),
            ("defen".to_string() + "ce", "defense".to_string()),
            ("offen".to_string() + "ce", "offense".to_string()),
            ("gr".to_string() + "ey", "gray".to_string()),
            ("plou".to_string() + "gh", "plow".to_string()),
            ("cent".to_string() + "re", "center".to_string()),
            ("met".to_string() + "re", "meter".to_string()),
        ];
        // Not spelling pairs but preferences, and the village keeps one:
        // "afterward" and "toward" are the American forms. Their British
        // twins had crept across eighteen files without a single American
        // one anywhere to argue with them, which is what a hand-written list
        // is for.
        drift.push(("afterward".to_string() + "s", "afterward".to_string()));
        drift.push(("toward".to_string() + "s", "toward".to_string()));
        for (theirs, ours) in &drift {
            assert_ne!(
                theirs, ours,
                "a pair in this table has collapsed - something swept the \
                 gate along with the corpus, and it would now forbid the \
                 spelling it exists to require",
            );
        }
        drift
    }

    /// The village speaks one English, and it is AMERICAN.
    ///
    /// It was British for most of this game's life, on the reasoning that
    /// the game's own labels were - a chronicle reading "nursed a neighbor
    /// back to health" beside a bubble saying "neighbor" is a seam the
    /// player can see. The seam was real; the choice was not, and nobody
    /// ever decided it. Brett: "I am not even sure how the britishg rule
    /// started, its been bugging me so i figured now is a good time to fix
    /// it." So the whole game moved - the corpus, the labels, the code's own
    /// prose and its identifiers - and this gate turned round with it.
    ///
    /// The seam is still what it guards. One English, whichever one.
    #[test]
    fn the_village_speaks_one_english() {
        let drift_pairs = british_forms();
        let voice = Corpus::load();
        for line in voice.lines() {
            let said = line.t.to_lowercase();
            for (theirs, ours) in &drift_pairs {
                // Whole words only: "colorless" is not a word this game
                // uses, but "harbor" inside "harboring" would be, and a
                // substring test would also condemn "honorary".
                let found = said
                    .split(|c: char| !c.is_alphabetic())
                    .any(|w| w == theirs.as_str());
                assert!(
                    !found,
                    "the corpus drifts into another English: {theirs:?} should be {ours:?} in {:?}",
                    line.t
                );
            }
        }
    }

    /// AND THE GAME'S OWN WORDS, not only the villagers'.
    ///
    /// The corpus gate above has guarded the lines people SAY for a long
    /// time, and nothing guarded anything else - so the labels, the notices,
    /// the chronicle and every comment in the source were free to drift, and
    /// did. The seam this is all about is between a bubble and a label, so
    /// gating one side of it was always half a job.
    ///
    /// Walks the source itself. That is unusual for a test and right here:
    /// the rule is about the words this game is written in, and the source is
    /// where they are. It also covers the identifiers, because half the code
    /// saying `colour` while the other half says `color` is the same seam one
    /// level down.
    #[test]
    fn the_game_speaks_one_english() {
        let drift = british_forms();
        let mut wrong: Vec<String> = Vec::new();
        let mut walk = vec![std::path::PathBuf::from("src")];
        while let Some(dir) = walk.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk.push(path);
                    continue;
                }
                if path.extension().is_none_or(|e| e != "rs") {
                    continue;
                }
                // This file, and only this file, may name the words it
                // forbids: a gate that cannot explain itself in its own
                // comments is a gate nobody will understand well enough to
                // keep. Every other file has no reason to write them.
                if path.ends_with("sermo/corpus.rs") {
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(&path) else {
                    continue;
                };
                let said = text.to_lowercase();
                for (theirs, ours) in &drift {
                    // Substring, not whole words: `water_colour` and
                    // `coloured` are both this game writing an English it
                    // does not speak, and neither is a word on its own.
                    if said.contains(theirs.as_str()) {
                        wrong.push(format!("{}: {theirs:?} should be {ours:?}", path.display()));
                    }
                }
            }
        }
        assert!(
            wrong.is_empty(),
            "the game drifts into another English:\n{}",
            wrong.join("\n"),
        );
    }

    /// The register wall between subjects, pinned against the real corpus.
    /// The bug this guards: a berry bush moved by the hand once came out
    /// of witnesses' mouths as "I saw somebody lifted clean into the air"
    /// - Brett: "they say they saw food fly up in the air when I never
    /// did that." A thing's telling must never borrow a person's words.
    #[test]
    fn a_things_story_is_never_told_about_a_person() {
        let mut voice = Corpus::load();
        let mut rng = crate::rng::Rng::new(7);
        for hand in ["saw", "heard", "distant"] {
            for kind in ["event:lifted", "event:thrown", "event:setdown"] {
                for _ in 0..20 {
                    let Some(line) = voice.pick(
                        1,
                        &["tell", kind, hand, "devout", "of:thing"],
                        &[],
                        &mut rng,
                    ) else {
                        continue;
                    };
                    for word in ["somebody", "people", " man ", "someone", "{whom}"] {
                        assert!(
                            !line.contains(word),
                            "a thing's {kind} telling spoke of a person: {line}"
                        );
                    }
                }
            }
        }
    }

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

    /// The register wall: a worn pool repeats itself before it borrows
    /// from the wrong register. Tiwa's thirteenth food prayer wore out
    /// every hungry-prayer line and came out as "quiet today. I don't
    /// mind quiet" — smalltalk, said on her knees.
    #[test]
    fn a_worn_register_repeats_rather_than_borrowing() {
        let mut voice = corpus(&[
            ("quiet today. I don't mind quiet", &["muse"], false),
            (
                "please, anything from the sky",
                &["muse", "prayer", "hungry"],
                false,
            ),
        ]);
        let mut rng = crate::rng::Rng::new(4);
        let moment = ["muse", "devout", "hungry", "prayer"];
        // Well past worn: the prayer line has gone out over and over, and
        // the fresh smalltalk line would win on freshness — but it may not
        // cross the wall.
        for _ in 0..12 {
            let said = voice
                .pick_within(7, &moment, &[], &["prayer"], &mut rng)
                .expect("a worn prayer still speaks");
            assert_eq!(
                said, "please, anything from the sky",
                "a prayer never borrows smalltalk, however worn its pool",
            );
        }
    }
}
