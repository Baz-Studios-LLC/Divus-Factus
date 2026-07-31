//! The teller: a villager's own words for what they saw.
//!
//! Every rumour in the game is a hand-written phrasing picked from
//! [`crate::witness::DivineEventKind::rumors`]. Those stay, forever, as what
//! the game says when nothing else is available. This module is an optional
//! layer *over* them: a small language model, running on the player's own
//! machine, asked to put the same event into the mouth of this particular
//! person — a doubting fisher who only heard about it speaks differently from
//! a devout mason who watched it happen.
//!
//! Three rules hold this in its place, and all three are load-bearing:
//!
//! 1. **Nothing waits on it.** The simulation never blocks. A line that has
//!    not come back yet is simply a written line instead.
//! 2. **Nothing depends on it.** With the dial off — the default — this module
//!    inserts no resource, opens no socket, and spawns no thread. Every test
//!    and every soak run behaves exactly as it did before it existed.
//! 3. **Nothing generated is ever saved.** The chronicle keeps storing the
//!    structured event, never the prose. A world remains rebuildable from its
//!    seed, which is the property the whole save format rests on.
//!
//! On by default while the game is in development, so every run exercises it;
//! `DIVUS_FACTUS_TELLER=0` turns it off. Nothing is required to be installed —
//! with no model listening it gives up after a few attempts, says so once, and
//! the village goes on speaking its written lines. The model is spoken to over
//! a plain JSON POST to localhost: no crate for it, because `serde_json` is
//! already here and the request is forty lines.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bevy::prelude::*;

use crate::villager::work::Vocation;
use crate::witness::DivineEventKind;

/// How many requests may be outstanding at once.
///
/// The cap, not the queue, is what keeps this honest: at eight times speed the
/// gossip mill fires constantly, and a queue that fills faster than it drains
/// would have people describing events three days stale. Past this, the
/// written phrasings answer and the request is simply not made.
const MAX_INFLIGHT: usize = 2;

/// How many distinct lines to keep per shape of telling before reusing them.
const LINES_PER_KEY: usize = 3;

/// How many failed asks before the teller stops asking altogether.
///
/// The case this exists for is the ordinary one: no model installed. Rather
/// than attempt a doomed connection for the rest of the session, it tries a
/// couple of dozen times, says so once, and leaves the village to its written
/// lines. A few attempts rather than one, because a model still loading its
/// weights refuses connections too and deserves a second chance.
const GIVE_UP_AFTER: u32 = 24;

/// The model asked by default: small and quick on purpose. A retelling is one
/// short sentence under hard constraints — the smallest instruct model that
/// can follow "one sentence, plain words" does the job, leaves the frame
/// budget alone, and answers fast enough to keep up with the mill.
const DEFAULT_MODEL: &str = "llama3.2:1b";

/// Where the local model listens. Localhost only, always: villager chatter is
/// not worth a network round trip or a question about where the words went.
const DEFAULT_ENDPOINT: &str = "127.0.0.1:11434";

/// The longest a retelling may run before it stops sounding like speech.
const MAX_WORDS: usize = 16;

/// Words no villager in this world has. A model reaching for any of them has
/// slipped out of the setting, and the written line is better.
const ANACHRONISMS: &[&str] = &[
    "phone",
    "car",
    "computer",
    "technology",
    "police",
    "electricity",
    "camera",
    "video",
    "internet",
    "okay",
    "guys",
    "dude",
];

/// How a teller came by their story.
///
/// The axis that matters most: a villager who *saw* the god drown someone and
/// one who heard about it thirdhand should not tell the same tale, and the
/// simulation already tracks the difference in [`crate::witness::Witnessed`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Hand {
    /// They were there.
    Witnessed,
    /// Someone who was there told them.
    Heard,
    /// It reached them through several mouths, and nobody they know saw it.
    Distant,
}

impl Hand {
    fn word(self) -> &'static str {
        match self {
            Hand::Witnessed => "saw it happen with their own eyes",
            Hand::Heard => "was told of it by someone who saw it",
            Hand::Distant => "only knows it as a story passed along",
        }
    }
}

/// How sure of the god the teller is.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum FaithBand {
    Doubting,
    Wavering,
    Sure,
}

impl FaithBand {
    /// Banded rather than exact, because the cache is keyed on it: a hundred
    /// slightly different trust values are the same voice.
    pub fn of(trust: f32) -> FaithBand {
        if trust < 0.25 {
            FaithBand::Doubting
        } else if trust < 0.6 {
            FaithBand::Wavering
        } else {
            FaithBand::Sure
        }
    }

    fn word(self) -> &'static str {
        match self {
            FaithBand::Doubting => "doubts there is any god in it",
            FaithBand::Wavering => "is not sure what to believe",
            FaithBand::Sure => "is certain it was the god",
        }
    }
}

/// The shape of a telling: everything that should change the words, and
/// nothing that should not.
///
/// This is the cache key, and keying on a *shape* rather than on a person is
/// what makes the whole thing affordable. A village of twenty produces a few
/// dozen shapes over a long session, not thousands of requests — while the
/// lines still differ by who is speaking and how they came by the story.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct TellingKey {
    pub kind: DivineEventKind,
    pub hand: Hand,
    pub voice: Option<Vocation>,
    pub faith: FaithBand,
}

/// One villager's telling of one act, as fields rather than prose.
#[derive(Clone, Debug)]
pub struct Retelling {
    pub key: TellingKey,
    /// 0 bolts at anything, 1 walks toward it.
    pub boldness: f32,
    /// How many times they have told this before. A worn story is flatter.
    pub told: u32,
}

impl Retelling {
    pub fn new(
        kind: DivineEventKind,
        hand: Hand,
        voice: Option<Vocation>,
        trust: f32,
        boldness: f32,
        told: u32,
    ) -> Retelling {
        Retelling {
            key: TellingKey {
                kind,
                hand,
                voice,
                faith: FaithBand::of(trust),
            },
            boldness,
            told,
        }
    }

    /// Reads the hand from what a witness actually holds.
    pub fn hand_of(witnessed: &crate::witness::Witnessed) -> Hand {
        if witnessed.total > 0 {
            Hand::Witnessed
        } else if witnessed.secondhand > 2 {
            Hand::Distant
        } else {
            Hand::Heard
        }
    }
}

/// The villagers' borrowed tongue: lines that have come back, and the thread
/// that fetches more.
///
/// Absent entirely unless the dial is set, which is what makes this module
/// free when it is off.
#[derive(Resource)]
pub struct Tongue {
    ready: HashMap<TellingKey, Vec<String>>,
    /// Which shapes have been asked about, so the same request is not made
    /// again every time two people meet.
    asked: HashSet<TellingKey>,
    /// Rotated so a shape with several lines does not always give the first.
    turn: usize,
    inflight: Arc<AtomicUsize>,
    ask: Sender<Retelling>,
    heard: Mutex<Receiver<(TellingKey, Option<String>)>>,
    /// Failed asks so far, and whether the teller has already given up.
    misses: u32,
    quiet: bool,
}

impl Tongue {
    /// A line for this telling, if one has come back.
    ///
    /// `None` means not yet, not available, or over the cap — and every caller
    /// answers that with a written phrasing. There is no failure mode here
    /// that the game notices.
    pub fn line(&mut self, of: &Retelling) -> Option<String> {
        self.collect();
        if let Some(lines) = self.ready.get(&of.key)
            && !lines.is_empty()
        {
            self.turn = self.turn.wrapping_add(1);
            let picked = lines[self.turn % lines.len()].clone();
            // Keep asking until this shape has a few phrasings, so a village
            // does not settle into saying one thing.
            if lines.len() < LINES_PER_KEY {
                self.request(of);
            }
            return Some(picked);
        }
        self.request(of);
        None
    }

    /// Queues a request, if there is room for one and anything is answering.
    fn request(&mut self, of: &Retelling) {
        if self.quiet {
            return;
        }
        if self.inflight.load(Ordering::Relaxed) >= MAX_INFLIGHT {
            return;
        }
        let fresh = self.ready.get(&of.key).map_or(0, |lines| lines.len());
        if fresh >= LINES_PER_KEY {
            return;
        }
        // One outstanding request per shape at a time.
        if !self.asked.insert(of.key) {
            return;
        }
        self.inflight.fetch_add(1, Ordering::Relaxed);
        if self.ask.send(of.clone()).is_err() {
            self.inflight.fetch_sub(1, Ordering::Relaxed);
        }
    }

    /// Takes in whatever the thread has finished.
    fn collect(&mut self) {
        // A `Receiver` is `Send` but not `Sync`, and a Bevy resource must be
        // both. The lock is never contended — only this method touches it.
        let Ok(heard) = self.heard.lock() else {
            return;
        };
        let mut arrived: Vec<(TellingKey, Option<String>)> = Vec::new();
        loop {
            match heard.try_recv() {
                Ok(answer) => arrived.push(answer),
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
            }
        }
        drop(heard);
        for (key, line) in arrived {
            // Cleared either way: a shape that failed must be askable again,
            // or a model that starts up late would never be reached.
            self.asked.remove(&key);
            match line {
                Some(line) => {
                    self.misses = 0;
                    let lines = self.ready.entry(key).or_default();
                    if !lines.iter().any(|held| held == &line) {
                        lines.push(line);
                    }
                }
                None => {
                    self.misses += 1;
                    if self.misses >= GIVE_UP_AFTER && !self.quiet {
                        self.quiet = true;
                        info!(
                            "no teller answered after {GIVE_UP_AFTER} asks; \
                             the village keeps to its own words"
                        );
                    }
                }
            }
        }
    }
}

/// Installs the teller, but only if the player asked for it.
pub struct TellingPlugin;

impl Plugin for TellingPlugin {
    fn build(&self, app: &mut App) {
        // On unless explicitly silenced. Costs nothing when no model answers.
        if std::env::var("DIVUS_FACTUS_TELLER").is_ok_and(|dial| dial == "0") {
            return;
        }
        let model = std::env::var("DIVUS_FACTUS_TELLER_MODEL")
            .unwrap_or_else(|_| DEFAULT_MODEL.to_string());
        let endpoint = std::env::var("DIVUS_FACTUS_TELLER_AT")
            .unwrap_or_else(|_| DEFAULT_ENDPOINT.to_string());
        info!("the teller is listening: {model} at {endpoint}");

        let (ask, requests) = channel::<Retelling>();
        let (answers, heard) = channel::<(TellingKey, Option<String>)>();
        let inflight = Arc::new(AtomicUsize::new(0));

        // One plain thread, owning the whole conversation with the model. No
        // async runtime: this codebase has two dependencies and does not need
        // a third to send one HTTP request at a time.
        let worker_inflight = Arc::clone(&inflight);
        std::thread::Builder::new()
            .name("teller".into())
            .spawn(move || {
                while let Ok(of) = requests.recv() {
                    let line = ask_the_model(&endpoint, &model, &of);
                    worker_inflight.fetch_sub(1, Ordering::Relaxed);
                    // Both outcomes are reported. Silence would leave the
                    // shape marked as asked and never answered.
                    if answers.send((of.key, line)).is_err() {
                        break;
                    }
                }
            })
            .expect("spawning the teller thread");

        app.insert_resource(Tongue {
            ready: HashMap::new(),
            asked: HashSet::new(),
            turn: 0,
            inflight,
            ask,
            heard: Mutex::new(heard),
            misses: 0,
            quiet: false,
        });
    }
}

/// What the model is told about its part, once.
fn system_prompt() -> &'static str {
    // "the god" rather than a name, always: the people name their own god in
    // their own tongue, and the caller substitutes it afterwards. A model
    // inventing a name would quietly break that.
    "You are one villager in a pre-industrial village, telling a neighbour \
     about something strange you know of. Reply with ONE short sentence of \
     plain spoken words, first person, under fourteen words. Never explain, \
     never narrate, never use quotation marks. Always say 'the god' and never \
     invent a name for it. No modern words."
}

/// The teller's own circumstances, as fields.
fn describe(of: &Retelling) -> String {
    let mut lines = vec![
        format!("what happened: {}", of.key.kind.describe()),
        format!("how you know: {}", of.key.hand.word()),
        format!("your belief: {}", of.key.faith.word()),
    ];
    if let Some(voice) = of.key.voice {
        lines.push(format!("your trade: {}", voice.describe()));
    }
    lines.push(
        match of.boldness {
            b if b < 0.35 => "your nature: easily frightened",
            b if b > 0.7 => "your nature: hard to rattle",
            _ => "your nature: steady enough",
        }
        .to_string(),
    );
    if of.told > 2 {
        lines.push("you have told this many times already".to_string());
    }
    lines.join("\n")
}

/// Whether a line the model produced is fit to put in a villager's mouth.
///
/// Nothing that comes back is trusted. A malformed answer is indistinguishable
/// from the model being switched off, which is the point: there is exactly one
/// fallback path and it is always available.
pub fn admissible(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.len() > 140 {
        return false;
    }
    if trimmed.contains('\n') || trimmed.contains('"') {
        return false;
    }
    // Digits read as a stat block, not as speech.
    if trimmed.chars().any(|c| c.is_ascii_digit()) {
        return false;
    }
    if trimmed.split_whitespace().count() > MAX_WORDS {
        return false;
    }
    let lower = trimmed.to_ascii_lowercase();
    if ANACHRONISMS.iter().any(|bad| lower.contains(bad)) {
        return false;
    }
    true
}

/// Tidies an admissible line into the shape the bubble expects.
pub fn tidy(line: &str) -> String {
    let mut out = line.trim().trim_matches('"').trim().to_string();
    // The bubbles are lower-case throughout; a capital reads as a title.
    if let Some(first) = out.chars().next()
        && first.is_uppercase()
    {
        out = first.to_lowercase().collect::<String>() + &out[first.len_utf8()..];
    }
    // One sentence: drop a trailing full stop, keep a question or a cry.
    if out.ends_with('.') {
        out.pop();
    }
    out
}

/// Asks the local model for one line. Runs on the teller thread only.
fn ask_the_model(endpoint: &str, model: &str, of: &Retelling) -> Option<String> {
    let payload = serde_json::json!({
        "model": model,
        "system": system_prompt(),
        "prompt": describe(of),
        "stream": false,
        "options": {
            // Warm enough to vary between tellers, cool enough to obey.
            "temperature": 0.9,
            "num_predict": 40,
        },
    });
    let raw = post_json(endpoint, "/api/generate", &payload)?;
    let parsed: serde_json::Value = serde_json::from_slice(&raw).ok()?;
    let line = parsed.get("response")?.as_str()?;
    let line = line.lines().next()?;
    if !admissible(line) {
        return None;
    }
    Some(tidy(line))
}

/// A JSON POST to a local address, by hand.
///
/// Forty lines against a whole HTTP crate, in a project that hand-rolls its
/// own noise and its own random numbers. `Connection: close` lets the reply be
/// read to the end without chunk parsing.
fn post_json(endpoint: &str, path: &str, body: &serde_json::Value) -> Option<Vec<u8>> {
    let body = serde_json::to_vec(body).ok()?;
    let mut stream = TcpStream::connect(endpoint).ok()?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .ok()?;
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .ok()?;

    let head = format!(
        "POST {path} HTTP/1.1\r\n\
         Host: {endpoint}\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).ok()?;
    stream.write_all(&body).ok()?;
    stream.flush().ok()?;

    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).ok()?;
    // Split the head from the body on the blank line.
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n")? + 4;
    Some(raw[split..].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_written_lines_answer_anything_the_model_gets_wrong() {
        // Every one of these is a real way a small model fails, and every one
        // must fall back rather than reach a villager's mouth.
        assert!(!admissible(""), "empty");
        assert!(!admissible("   "), "blank");
        assert!(!admissible("he rose\nand fell"), "two lines");
        assert!(!admissible("\"he just rose\""), "quoted");
        assert!(!admissible("i saw 3 men lifted"), "digits");
        assert!(!admissible("check your phone about it"), "anachronism");
        assert!(
            !admissible("Okay so basically the god did a thing"),
            "modern"
        );
        let rambling = "and then i saw the man go up and up into the air over the \
                        rooftops and past the trees and he was shouting all the while";
        assert!(!admissible(rambling), "too long");
    }

    #[test]
    fn a_plain_spoken_line_is_admitted() {
        assert!(admissible("the air itself took hold of him, i saw it"));
        assert!(admissible("Something lifted him clean off the ground."));
        assert!(admissible("was it the god? i cannot say"));
    }

    #[test]
    fn lines_are_tidied_into_the_bubbles_own_voice() {
        // The bubbles run lower-case and without full stops; a model's
        // sentence case would read as a caption rather than speech.
        assert_eq!(
            tidy("Something lifted him clean off."),
            "something lifted him clean off"
        );
        assert_eq!(tidy("  \"he just rose\"  "), "he just rose");
        // A question or a cry keeps its mark.
        assert_eq!(tidy("was it the god?"), "was it the god?");
    }

    #[test]
    fn faith_bands_cover_the_whole_range() {
        assert_eq!(FaithBand::of(0.0), FaithBand::Doubting);
        assert_eq!(FaithBand::of(0.24), FaithBand::Doubting);
        assert_eq!(FaithBand::of(0.25), FaithBand::Wavering);
        assert_eq!(FaithBand::of(0.59), FaithBand::Wavering);
        assert_eq!(FaithBand::of(0.6), FaithBand::Sure);
        assert_eq!(FaithBand::of(1.0), FaithBand::Sure);
    }

    #[test]
    fn how_you_know_is_read_from_what_you_hold() {
        use crate::witness::Witnessed;
        let seen = Witnessed {
            total: 1,
            ..Default::default()
        };
        assert_eq!(Retelling::hand_of(&seen), Hand::Witnessed);
        let told = Witnessed {
            secondhand: 1,
            ..Default::default()
        };
        assert_eq!(Retelling::hand_of(&told), Hand::Heard);
        let rumoured = Witnessed {
            secondhand: 5,
            ..Default::default()
        };
        assert_eq!(Retelling::hand_of(&rumoured), Hand::Distant);
    }

    #[test]
    fn the_shape_of_a_telling_is_what_gets_cached() {
        // Two people of the same trade and belief who came by a story the same
        // way share a shape. This is what keeps the model from being asked
        // thousands of times: it answers per shape, not per villager.
        let a = Retelling::new(
            DivineEventKind::Smote,
            Hand::Witnessed,
            Some(Vocation::Fisher),
            0.8,
            0.4,
            0,
        );
        let b = Retelling::new(
            DivineEventKind::Smote,
            Hand::Witnessed,
            Some(Vocation::Fisher),
            0.95,
            0.9,
            7,
        );
        assert_eq!(a.key, b.key, "same shape, however the details differ");

        // Coming by it differently is a different shape, because it should
        // sound different — that is the whole mechanic.
        let heard = Retelling::new(
            DivineEventKind::Smote,
            Hand::Distant,
            Some(Vocation::Fisher),
            0.8,
            0.4,
            0,
        );
        assert_ne!(a.key, heard.key);
    }

    #[test]
    fn the_prompt_carries_the_fields_and_never_a_gods_name() {
        let of = Retelling::new(
            DivineEventKind::Uprooted,
            Hand::Heard,
            Some(Vocation::Mason),
            0.1,
            0.2,
            0,
        );
        let prompt = describe(&of);
        assert!(prompt.contains("what happened"));
        assert!(prompt.contains("how you know"));
        assert!(prompt.contains("your belief"));
        assert!(prompt.contains("your trade"));
        // The people name their own god; the model must not.
        assert!(system_prompt().contains("never invent a name"));
    }
}
