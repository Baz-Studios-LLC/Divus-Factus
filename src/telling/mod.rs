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
//! The model runs IN THIS PROCESS. There is no daemon to install, no server to
//! reach, nothing for a player to do but launch the game — which is the whole
//! requirement. Weights are not compiled in: they sit beside the saves, the
//! launcher fetches them once, and their absence costs nothing because the
//! written lines are always there.
//!
//! `DIVUS_FACTUS_TELLER=0` turns it off.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use candle_core::quantized::gguf_file;
use candle_core::{Device, Tensor};
use candle_transformers::generation::LogitsProcessor;
use candle_transformers::models::quantized_qwen2::ModelWeights;
use tokenizers::Tokenizer;

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

/// The most tokens one line may take before it stops being a line.
///
/// A villager says a mouthful, not a paragraph, and a runaway generation is
/// wasted time on a thread the game is waiting for nothing from.
const MAX_TOKENS: usize = 48;

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

/// Where the weights live: beside the game's own saves, NOT inside the
/// installed game folder — which the launcher wipes and re-unpacks on every
/// update. Fetched once, kept forever.
///
/// The launcher writes here by mirroring this same convention; macOS hands a
/// bundle to LaunchServices and drops the environment on the way, so there is
/// no channel to be told a path. If this moves, the launcher's `support_path`
/// moves with it.
pub fn model_dir() -> Option<std::path::PathBuf> {
    let home = std::env::var("HOME").ok();
    if cfg!(target_os = "macos") {
        Some(
            std::path::PathBuf::from(home?).join("Library/Application Support/Divus Factus/models"),
        )
    } else if cfg!(target_os = "windows") {
        Some(
            std::path::PathBuf::from(std::env::var("APPDATA").ok()?)
                .join("Divus Factus")
                .join("models"),
        )
    } else {
        Some(std::path::PathBuf::from(home?).join(".local/share/divus-factus/models"))
    }
}

/// The weights and tokenizer to speak with, chosen from whatever is on disk.
///
/// Discovered rather than named, so a player who wants a better voice only has
/// to drop a larger `.gguf` in the folder — the biggest file wins, on the
/// reasoning that nobody puts one there by accident. That is the whole of the
/// "bring your own bigger model" feature, and it costs nothing.
fn find_model() -> Option<(std::path::PathBuf, std::path::PathBuf)> {
    let dir = model_dir()?;
    let mut weights: Vec<(u64, std::path::PathBuf)> = std::fs::read_dir(&dir)
        .ok()?
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            if path.extension().is_some_and(|e| e == "gguf") {
                let size = path.metadata().ok()?.len();
                Some((size, path))
            } else {
                None
            }
        })
        .collect();
    weights.sort_by_key(|(size, _)| std::cmp::Reverse(*size));
    let (_, model) = weights.into_iter().next()?;

    // A tokenizer named for this model, or the only one in the folder.
    let stem = model.file_stem()?.to_string_lossy().to_string();
    let paired = dir.join(format!("{stem}-tokenizer.json"));
    let tokenizer = if paired.exists() {
        paired
    } else {
        std::fs::read_dir(&dir)
            .ok()?
            .filter_map(|entry| {
                let path = entry.ok()?.path();
                let name = path.file_name()?.to_string_lossy().to_string();
                name.ends_with("tokenizer.json").then_some(path)
            })
            .next()?
    };
    Some((model, tokenizer))
}

/// Installs the teller. Silent and free when there are no weights to read.
pub struct TellingPlugin;

impl Plugin for TellingPlugin {
    fn build(&self, app: &mut App) {
        // On unless explicitly silenced — the point is that a player does
        // nothing to get this.
        if std::env::var("DIVUS_FACTUS_TELLER").is_ok_and(|dial| dial == "0") {
            return;
        }
        let Some((weights, tokenizer)) = find_model() else {
            // No model on disk: the village keeps to its written lines, and
            // nothing about the game is worse than it was.
            return;
        };
        info!(
            "the teller found {}",
            weights.file_name().unwrap_or_default().to_string_lossy()
        );

        let (ask, requests) = channel::<Retelling>();
        let (answers, heard) = channel::<(TellingKey, Option<String>)>();
        let inflight = Arc::new(AtomicUsize::new(0));

        // One plain thread owning the model. No async runtime: this is a queue
        // of one short generation at a time, and the game never waits on it.
        // Loading happens HERE rather than on the main thread, so a second and
        // a half of weight-reading never shows up as a stutter.
        let worker_inflight = Arc::clone(&inflight);
        std::thread::Builder::new()
            .name("teller".into())
            .stack_size(8 * 1024 * 1024)
            .spawn(move || {
                let mut voice = match Voice::load(&weights, &tokenizer) {
                    Ok(voice) => voice,
                    Err(e) => {
                        warn!("the teller could not read its model: {e}");
                        // Drain and refuse, so callers stop asking.
                        while let Ok(of) = requests.recv() {
                            worker_inflight.fetch_sub(1, Ordering::Relaxed);
                            if answers.send((of.key, None)).is_err() {
                                return;
                            }
                        }
                        return;
                    }
                };
                info!("the teller has its voice");
                while let Ok(of) = requests.recv() {
                    let line = voice.retell(&of);
                    worker_inflight.fetch_sub(1, Ordering::Relaxed);
                    // Both outcomes reported: silence would leave the shape
                    // marked as asked and never answered.
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

/// The loaded model, and everything needed to speak with it.
///
/// Lives only on the teller thread. Nothing here is `Send` across a system
/// boundary and nothing needs to be.
struct Voice {
    model: ModelWeights,
    tokenizer: Tokenizer,
    device: Device,
    /// Qwen closes a turn with `<|im_end|>`; without it the model talks past
    /// its own answer.
    end_of_turn: u32,
    /// Varied per call, or every villager in the world says one thing.
    draws: u64,
}

impl Voice {
    fn load(weights: &std::path::Path, tokenizer: &std::path::Path) -> Result<Voice, String> {
        // Metal where there is Metal. The renderer has first call on the GPU,
        // but a 1.5B model's share of it is small and the alternative is
        // seconds per line on CPU.
        let device = Device::new_metal(0).unwrap_or(Device::Cpu);
        let mut file = std::fs::File::open(weights).map_err(|e| e.to_string())?;
        let content = gguf_file::Content::read(&mut file).map_err(|e| e.to_string())?;
        let model =
            ModelWeights::from_gguf(content, &mut file, &device).map_err(|e| e.to_string())?;
        let tokenizer = Tokenizer::from_file(tokenizer).map_err(|e| e.to_string())?;
        let end_of_turn = tokenizer
            .token_to_id("<|im_end|>")
            .ok_or("the tokenizer has no <|im_end|>")?;
        Ok(Voice {
            model,
            tokenizer,
            device,
            end_of_turn,
            draws: 0,
        })
    }

    /// One villager's line, or `None` if what came back is unfit to say.
    fn retell(&mut self, of: &Retelling) -> Option<String> {
        self.draws += 1;
        let prompt = chatml(&describe(of));
        let raw = self.generate(&prompt, self.draws).ok()?;
        let line = raw.lines().next()?;
        if !admissible(line) {
            return None;
        }
        let line = tidy(line);
        // Shown the written lines as examples, a small model will sometimes
        // hand one straight back. Treated as a miss: a line already in the
        // corpus adds nothing the fallback would not have given for free, and
        // caching it would crowd out the ones it actually composed.
        if of
            .key
            .kind
            .rumors()
            .iter()
            .any(|written| written.eq_ignore_ascii_case(&line))
        {
            return None;
        }
        Some(line)
    }

    fn generate(&mut self, prompt: &str, seed: u64) -> Result<String, String> {
        let encoded = self
            .tokenizer
            .encode(prompt, true)
            .map_err(|e| e.to_string())?;
        let tokens = encoded.get_ids();
        // Warm enough to differ between tellers, cool enough to obey.
        let mut sampler = LogitsProcessor::new(seed, Some(0.8), Some(0.9));

        // The prompt in one pass; `forward` yields the last position's logits.
        let tensor = |ids: &[u32]| Tensor::new(ids, &self.device).map_err(|e| e.to_string());
        let input = tensor(tokens)?.unsqueeze(0).map_err(|e| e.to_string())?;
        let logits = self
            .model
            .forward(&input, 0)
            .and_then(|l| l.squeeze(0))
            .map_err(|e| e.to_string())?;
        let mut next = sampler.sample(&logits).map_err(|e| e.to_string())?;

        let mut out: Vec<u32> = Vec::new();
        for step in 0..MAX_TOKENS {
            if next == self.end_of_turn {
                break;
            }
            out.push(next);
            let input = tensor(&[next])?.unsqueeze(0).map_err(|e| e.to_string())?;
            let logits = self
                .model
                .forward(&input, tokens.len() + step)
                .and_then(|l| l.squeeze(0))
                .map_err(|e| e.to_string())?;
            next = sampler.sample(&logits).map_err(|e| e.to_string())?;
            // One sentence: a newline ends the turn whatever the model thinks.
            if let Ok(so_far) = self.tokenizer.decode(&out, true)
                && so_far.contains('\n')
            {
                break;
            }
        }
        self.tokenizer.decode(&out, true).map_err(|e| e.to_string())
    }
}

/// What the model is told about its part, once.
fn system_prompt() -> &'static str {
    // "the god" rather than a name, always: the people name their own god in
    // their own tongue and the caller substitutes it afterwards. A model
    // inventing a name would quietly break that.
    //
    // "Never repeat the details back" earns its place — without it a small
    // model reads the fields aloud: "My trade is hunts. My nature is steady."
    "You are a villager in a pre-industrial village, telling a neighbour what you \
     know of something strange. Answer ONLY with the words you would say: one \
     short mouthful, under twelve words, plain and spoken. Never repeat the \
     details back. Never mention your trade, your belief or your nature. Never \
     explain or narrate. No quotation marks, no modern words. Say 'the god', \
     never a name."
}

/// The teller's own circumstances, as fields rather than prose.
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

/// Worked examples, whose answers are the game's OWN written rumours.
///
/// This is the highest-leverage part of the whole feature. A small model
/// follows examples far better than it follows rules — showing it three did
/// more for quality than any amount of instruction, taking the share of
/// usable lines from two thirds to all of them. And because the answers are
/// read out of [`DivineEventKind::rumors`] rather than copied, the hand-written
/// corpus is no longer only the fallback: it is what teaches the model this
/// world's register. Writing more rumours now improves the generated ones too.
fn shots() -> Vec<(Retelling, String)> {
    let example = |kind: DivineEventKind, hand: Hand, trust: f32, voice: Vocation, which: usize| {
        let said = kind.rumors()[which % kind.rumors().len()].to_string();
        (Retelling::new(kind, hand, Some(voice), trust, 0.5, 0), said)
    };
    vec![
        example(
            DivineEventKind::Lifted,
            Hand::Witnessed,
            0.8,
            Vocation::Gatherer,
            1,
        ),
        example(DivineEventKind::Smote, Hand::Heard, 0.4, Vocation::Mason, 2),
        example(
            DivineEventKind::Provided,
            Hand::Distant,
            0.1,
            Vocation::Hunter,
            2,
        ),
    ]
}

/// Qwen speaks ChatML. Getting this wrong is the difference between a
/// villager's line and the model reciting its instructions back.
fn chatml(user: &str) -> String {
    let mut out = format!("<|im_start|>system\n{}<|im_end|>\n", system_prompt());
    for (fields, said) in shots() {
        out.push_str(&format!(
            "<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n{said}<|im_end|>\n",
            describe(&fields)
        ));
    }
    out.push_str(&format!(
        "<|im_start|>user\n{user}<|im_end|>\n<|im_start|>assistant\n"
    ));
    out
}

/// Whether a line the model produced is fit to put in a villager's mouth.
///
/// Nothing that comes back is trusted. A malformed answer is indistinguishable
/// from there being no model at all, which is the point: there is exactly one
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

/// Tidies an admissible line into the shape the bubbles expect.
pub fn tidy(line: &str) -> String {
    let mut out = line.trim().trim_matches('"').trim().to_string();
    // The bubbles run lower-case; a capital reads as a caption.
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
        assert!(system_prompt().contains("never a name"));
    }

    #[test]
    fn the_prompt_shows_the_model_the_villages_own_lines() {
        // The few-shot answers are read out of the game's rumour corpus, not
        // copied beside it — so rewriting a rumour rewrites the example, and
        // the hand-written lines keep teaching the register rather than
        // drifting out of step with it.
        let built = chatml(&describe(&Retelling::new(
            DivineEventKind::Smote,
            Hand::Witnessed,
            Some(Vocation::Fisher),
            0.9,
            0.5,
            0,
        )));
        for (_, said) in shots() {
            assert!(
                built.contains(&said),
                "the prompt should carry the written line {said:?}",
            );
            // And every example must itself be something we would accept.
            assert!(admissible(&said), "{said:?} would be rejected as an answer");
        }
        // ChatML, closed properly, ending on the assistant's turn to speak.
        assert!(built.starts_with("<|im_start|>system"));
        assert!(built.ends_with("<|im_start|>assistant\n"));
        assert_eq!(
            built.matches("<|im_end|>").count(),
            1 + shots().len() * 2 + 1
        );
    }
}
