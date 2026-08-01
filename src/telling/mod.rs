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
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
#[allow(deprecated)]
use llama_cpp_2::model::Special;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaModel};
use llama_cpp_2::sampling::LlamaSampler;

use crate::villager::traits::Bearing;
use crate::villager::work::Vocation;
use crate::witness::{DivineEventKind, Whom};

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
const MAX_WORDS: usize = 18;

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
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct TellingKey {
    pub kind: DivineEventKind,
    pub hand: Hand,
    pub voice: Option<Vocation>,
    pub faith: FaithBand,
    /// Which way this person's manner bends their words. Part of the key and
    /// not merely the prompt: a shape that mentions a manner but does not key
    /// on one would hand a gloomy villager a line composed for a cheerful
    /// one, which is worse than handing them a generic line.
    pub bearing: Bearing,
    /// Who it happened to, in the teller's own terms. In the key for the same
    /// reason the manner is: a line composed about "Feitreh, your brother" put
    /// in the mouth of someone he is nothing to would be the model telling the
    /// player something false about the world. The key holding a name is what
    /// stops a cached specific from ever crossing to the wrong teller.
    pub whom: Option<Whom>,
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

/// One villager's inward moment: everything true about them right now,
/// assembled on the main thread where the components live, shipped to the
/// worker as plain data.
///
/// Unlike a [`Retelling`] this is not cached by shape — a thought is this
/// person, this place, this moment, and serving it to anyone else would be
/// the lie the whole design exists to prevent. What makes that affordable is
/// that thoughts have no listener and therefore no deadline: they are asked
/// for ahead of need, by [`Regard::Close`](crate::attention::Regard), for the
/// handful of people the god is actually watching.
#[derive(Clone, Debug)]
pub struct Musing {
    /// Whose thought this is; the answer comes back keyed on it.
    pub who: Entity,
    pub voice: Option<Vocation>,
    pub bearing: Bearing,
    pub faith: FaithBand,
    /// What the body is saying: "hungry", "worn out". Empty when it is quiet.
    pub body: Vec<&'static str>,
    /// The settlement's now, from [`crate::now::WorldNow`].
    pub place: Vec<String>,
    /// The one thing pressing on them, chosen by the sim.
    pub mind: String,
    /// What was just said to them, when this is a reply rather than an idle
    /// thought. Changes the instruction from inward to answering, and the
    /// words come back spoken rather than mused.
    pub heard: Option<String>,
    /// Whether this is truly VOICED — a scream, a cry for help. An idle
    /// musing is a thought and shows as one: people who talk to the wind
    /// unsettle their neighbours.
    pub aloud: bool,
    /// Whether these words are addressed to the god — a prayer wears its
    /// own colour, because it is the one channel aimed at the player.
    pub prayer: bool,
    /// Every proper noun the thought is allowed to contain: their own name,
    /// their place, their people. The truth gate holds the words to this.
    pub known: Vec<String>,
}

impl Musing {
    /// Whether this is an answer to someone rather than a private moment.
    fn is_reply(&self) -> bool {
        self.heard.is_some()
    }
}

impl Retelling {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        kind: DivineEventKind,
        hand: Hand,
        voice: Option<Vocation>,
        trust: f32,
        bearing: Bearing,
        whom: Option<Whom>,
        boldness: f32,
        told: u32,
    ) -> Retelling {
        Retelling {
            key: TellingKey {
                kind,
                hand,
                voice,
                faith: FaithBand::of(trust),
                bearing,
                whom,
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
/// What the game asks the worker for.
enum Ask {
    Retell(Retelling),
    Muse(Box<Musing>),
    /// Put down one voice and take up another, mid-session. The settings
    /// page put down its picker, but the switch stays strung: the cloud
    /// voices to come will pull on it again.
    #[allow(dead_code)]
    Switch(std::path::PathBuf),
}

/// What comes back, keyed the way it was asked.
enum Answer {
    Told(TellingKey, Option<String>),
    /// The bool says whether it was a reply, so an answer composed for a
    /// conversation is never shown as a stray idle thought. The inner bool
    /// carries `aloud` — a cry keeps its voice all the way to the bubble.
    Mused(Entity, bool, Option<(String, bool, bool)>),
    /// The switch happened (the new voice's name), or it did not (None) and
    /// the old voice carries on.
    Switched(Option<String>),
}

#[derive(Resource)]
pub struct Tongue {
    ready: HashMap<TellingKey, Vec<String>>,
    /// Which shapes have been asked about, so the same request is not made
    /// again every time two people meet.
    asked: HashSet<TellingKey>,
    /// Thoughts that have come back, waiting to be shown.
    mused: HashMap<Entity, (String, bool, bool)>,
    /// Replies that have come back, waiting for the conversation's beat.
    replies: HashMap<Entity, String>,
    /// Whose words are being composed right now, and of which kind — a
    /// person can have an idle thought and a reply in flight at once.
    musing: HashSet<(Entity, bool)>,
    /// Rotated so a shape with several lines does not always give the first.
    turn: usize,
    inflight: Arc<AtomicUsize>,
    ask: Sender<Ask>,
    heard: Mutex<Receiver<Answer>>,
    /// Failed asks so far, and whether the teller has already given up.
    misses: u32,
    quiet: bool,
    /// The file the voice was loaded from, as shown in the settings page.
    current: String,
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
        if !self.asked.insert(of.key.clone()) {
            return;
        }
        self.inflight.fetch_add(1, Ordering::Relaxed);
        if self.ask.send(Ask::Retell(of.clone())).is_err() {
            self.inflight.fetch_sub(1, Ordering::Relaxed);
        }
    }

    /// Asks for someone's thought, ahead of needing it.
    ///
    /// Refused quietly when the queue is full or their last thought has not
    /// come back yet — a thought that never gets composed simply never
    /// appears, and the written murmur carries on.
    pub fn muse(&mut self, of: Musing) {
        if self.quiet {
            return;
        }
        if self.inflight.load(Ordering::Relaxed) >= MAX_INFLIGHT {
            return;
        }
        // One of each kind per head at a time, composing or waiting unshown.
        let waiting = if of.is_reply() {
            self.replies.contains_key(&of.who)
        } else {
            self.mused.contains_key(&of.who)
        };
        if waiting || !self.musing.insert((of.who, of.is_reply())) {
            return;
        }
        self.inflight.fetch_add(1, Ordering::Relaxed);
        if self.ask.send(Ask::Muse(Box::new(of))).is_err() {
            self.inflight.fetch_sub(1, Ordering::Relaxed);
        }
    }

    /// The composed thought waiting for this person, if any. Taking it makes
    /// room for their next.
    pub fn take_musing(&mut self, who: Entity) -> Option<(String, bool, bool)> {
        self.collect();
        self.mused.remove(&who)
    }

    /// The composed reply waiting for this person, if any.
    pub fn take_reply(&mut self, who: Entity) -> Option<String> {
        self.collect();
        self.replies.remove(&who)
    }

    /// Everyone whose thought has come back and not yet been shown.
    pub fn mused_heads(&mut self) -> Vec<Entity> {
        self.collect();
        self.mused.keys().copied().collect()
    }

    /// The model the teller speaks with, by file name.
    #[allow(dead_code)] // waits, with switch_to, for the voice picker's return
    pub fn speaking_with(&mut self) -> String {
        self.collect();
        self.current.clone()
    }

    /// Puts down the current voice and takes up the given weights.
    ///
    /// The swap happens on the worker thread — the loading seconds never
    /// touch a frame — and the choice is written down so the same voice
    /// answers on the next launch. Every cached line is dropped: they were
    /// the OLD voice's words, and serving them from the new one would make
    /// the switch look like it did nothing.
    #[allow(dead_code)] // waits for the voice picker's return
    pub fn switch_to(&mut self, weights: std::path::PathBuf) {
        if let (Some(dir), Some(name)) = (model_dir(), weights.file_name()) {
            let _ = std::fs::write(dir.join("chosen"), name.to_string_lossy().as_bytes());
        }
        self.ready.clear();
        self.asked.clear();
        self.mused.clear();
        self.replies.clear();
        self.musing.clear();
        self.quiet = false;
        self.misses = 0;
        self.current = format!(
            "{} (loading)",
            weights.file_name().unwrap_or_default().to_string_lossy()
        );
        let _ = self.ask.send(Ask::Switch(weights));
    }

    /// Takes in whatever the thread has finished.
    fn collect(&mut self) {
        // A `Receiver` is `Send` but not `Sync`, and a Bevy resource must be
        // both. The lock is never contended — only this method touches it.
        let Ok(heard) = self.heard.lock() else {
            return;
        };
        let mut arrived: Vec<Answer> = Vec::new();
        while let Ok(answer) = heard.try_recv() {
            arrived.push(answer);
        }
        drop(heard);
        for answer in arrived {
            let line = match answer {
                Answer::Told(key, line) => {
                    // Cleared either way: a shape that failed must be askable
                    // again, or a model that starts up late is never reached.
                    self.asked.remove(&key);
                    if let Some(line) = line {
                        let lines = self.ready.entry(key).or_default();
                        if !lines.iter().any(|held| held == &line) {
                            lines.push(line.clone());
                        }
                        Some(line)
                    } else {
                        None
                    }
                }
                Answer::Switched(name) => {
                    if let Some(name) = name {
                        info!("the teller now speaks with {name}");
                        self.current = name;
                    } else {
                        self.current = self.current.replace(" (loading)", " (failed)");
                    }
                    continue;
                }
                Answer::Mused(who, reply, line) => {
                    self.musing.remove(&(who, reply));
                    if let Some((line, aloud, prayer)) = line {
                        if reply {
                            self.replies.insert(who, line.clone());
                        } else {
                            self.mused.insert(who, (line.clone(), aloud, prayer));
                        }
                        Some(line)
                    } else {
                        None
                    }
                }
            };
            match line {
                Some(_) => self.misses = 0,
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

/// The weights to speak with, chosen from whatever is on disk. (The
/// tokenizer lives inside the GGUF itself since the llama.cpp move.)
///
/// Discovered rather than named, so a player who wants a better voice only has
/// to drop a larger `.gguf` in the folder — the biggest file wins, on the
/// reasoning that nobody puts one there by accident. That is the whole of the
/// "bring your own bigger model" feature, and it costs nothing.
fn find_model() -> Option<std::path::PathBuf> {
    // A remembered choice outranks size: the settings page writes one when
    // the player switches, and it must survive a restart or the switch was
    // a lie. A choice whose file has since been deleted falls through.
    let dir = model_dir()?;
    let chosen = std::fs::read_to_string(dir.join("chosen"))
        .ok()
        .map(|name| dir.join(name.trim()))
        .filter(|path| path.is_file());
    chosen.or_else(|| list_models().into_iter().next())
}

/// Every model on disk, largest first — the order the settings page shows.
pub fn list_models() -> Vec<std::path::PathBuf> {
    let Some(dir) = model_dir() else {
        return Vec::new();
    };
    let mut weights: Vec<(u64, std::path::PathBuf)> = std::fs::read_dir(&dir)
        .ok()
        .into_iter()
        .flatten()
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
    weights.into_iter().map(|(_, path)| path).collect()
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
        let Some(weights) = find_model() else {
            // No model on disk: the village keeps to its written lines, and
            // nothing about the game is worse than it was.
            return;
        };
        let weights_name = weights
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        info!("the teller found {weights_name}");

        let (ask, requests) = channel::<Ask>();
        let (answers, heard) = channel::<Answer>();
        let inflight = Arc::new(AtomicUsize::new(0));

        // Every ask is answered, success or not: silence would leave the
        // shape marked as asked and never answered.
        let refuse = |ask: Ask| match ask {
            Ask::Retell(of) => Answer::Told(of.key, None),
            Ask::Muse(of) => Answer::Mused(of.who, of.is_reply(), None),
            Ask::Switch(..) => Answer::Switched(None),
        };

        // One plain thread owning the model. No async runtime: this is a queue
        // of one short generation at a time, and the game never waits on it.
        // Loading happens HERE rather than on the main thread, so a second and
        // a half of weight-reading never shows up as a stutter.
        let worker_inflight = Arc::clone(&inflight);
        std::thread::Builder::new()
            .name("teller".into())
            .stack_size(8 * 1024 * 1024)
            .spawn(move || {
                // Background QoS: llama.cpp's worker pool inherits the QoS of
                // the thread that calls decode, so demoting THIS thread
                // demotes every core the teller touches — the frame is always
                // scheduled first, and the last trace of the pre-bubble hitch
                // goes with it. A line arrives a beat later; nothing waits.
                #[cfg(target_os = "macos")]
                unsafe {
                    libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_UTILITY, 0);
                }
                let mut voice = match Voice::load(&weights) {
                    Ok(voice) => voice,
                    Err(e) => {
                        warn!("the teller could not read its model: {e}");
                        // Drain and refuse, so callers stop asking.
                        while let Ok(asked) = requests.recv() {
                            worker_inflight.fetch_sub(1, Ordering::Relaxed);
                            if answers.send(refuse(asked)).is_err() {
                                return;
                            }
                        }
                        return;
                    }
                };
                info!("the teller has its voice");
                // Varied per draw, or every villager says one thing.
                let mut draws: u64 = 0;
                loop {
                    let mut ctx = match voice.context() {
                        Ok(ctx) => ctx,
                        Err(e) => {
                            warn!("the teller could not open a context: {e}");
                            while let Ok(asked) = requests.recv() {
                                worker_inflight.fetch_sub(1, Ordering::Relaxed);
                                if answers.send(refuse(asked)).is_err() {
                                    return;
                                }
                            }
                            return;
                        }
                    };
                    let mut take_up: Option<std::path::PathBuf> = None;
                    while let Ok(asked) = requests.recv() {
                        let answer = match asked {
                            Ask::Retell(of) => {
                                draws += 1;
                                let line = voice.retell(&mut ctx, &of, draws);
                                worker_inflight.fetch_sub(1, Ordering::Relaxed);
                                Answer::Told(of.key, line)
                            }
                            Ask::Muse(of) => {
                                draws += 1;
                                let line = voice
                                    .muse(&mut ctx, &of, draws)
                                    .map(|line| (line, of.aloud, of.prayer));
                                worker_inflight.fetch_sub(1, Ordering::Relaxed);
                                Answer::Mused(of.who, of.is_reply(), line)
                            }
                            // Not counted against inflight: not a line. The
                            // context borrows the voice, so the swap happens
                            // outside this inner loop, after the drop.
                            Ask::Switch(weights) => {
                                take_up = Some(weights);
                                break;
                            }
                        };
                        if answers.send(answer).is_err() {
                            return;
                        }
                    }
                    drop(ctx);
                    let Some(weights) = take_up else {
                        return;
                    };
                    let answer = match Voice::load(&weights) {
                        Ok(fresh) => {
                            voice = fresh;
                            Answer::Switched(Some(
                                weights
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .to_string(),
                            ))
                        }
                        Err(e) => {
                            warn!("the teller could not take up {weights:?}: {e}");
                            Answer::Switched(None)
                        }
                    };
                    if answers.send(answer).is_err() {
                        return;
                    }
                }
            })
            .expect("spawning the teller thread");

        app.insert_resource(Tongue {
            ready: HashMap::new(),
            asked: HashSet::new(),
            mused: HashMap::new(),
            replies: HashMap::new(),
            musing: HashSet::new(),
            turn: 0,
            inflight,
            ask,
            heard: Mutex::new(heard),
            misses: 0,
            quiet: false,
            current: weights_name,
        });
    }
}

/// The loaded model, and everything needed to speak with it.
///
/// Lives only on the teller thread. llama.cpp underneath: the tokenizer comes
/// out of the GGUF itself, and generation runs on CPU cores the game barely
/// uses — the GPU is the renderer's alone, which is what ended the hitch.
struct Voice {
    backend: LlamaBackend,
    model: LlamaModel,
}

impl Voice {
    fn load(weights: &std::path::Path) -> Result<Voice, String> {
        // CPU, always: the renderer has sole call on the GPU, and a model's
        // prompt prefill is one indivisible dispatch that showed up as a
        // hitch before every bubble. On llama.cpp's kernels the CPU is fast
        // enough that nothing misses it.
        let backend = LlamaBackend::init().map_err(|e| e.to_string())?;
        let params = LlamaModelParams::default();
        let model =
            LlamaModel::load_from_file(&backend, weights, &params).map_err(|e| e.to_string())?;
        Ok(Voice { backend, model })
    }

    /// One long-lived context: the KV cache is a large allocation, and
    /// rebuilding it per line churned the VM hard enough to stall the frame
    /// from the kernel's side — the hitch's last address. Cleared between
    /// lines instead.
    fn context(&self) -> Result<llama_cpp_2::context::LlamaContext<'_>, String> {
        self.model
            .new_context(
                &self.backend,
                LlamaContextParams::default()
                    .with_n_ctx(std::num::NonZeroU32::new(1280))
                    .with_n_threads(2)
                    .with_n_threads_batch(2),
            )
            .map_err(|e| e.to_string())
    }

    /// One villager's line, or `None` if what came back is unfit to say.
    fn retell(
        &self,
        ctx: &mut llama_cpp_2::context::LlamaContext<'_>,
        of: &Retelling,
        seed: u64,
    ) -> Option<String> {
        let prompt = chatml(&describe(of));
        let raw = self.generate(ctx, &prompt, seed).ok()?;
        let line = raw.lines().next()?;
        if !admissible(line) {
            return None;
        }
        // The truth gate: the only name this telling may drop is the one the
        // simulation put in it. A line that reaches for any other has invented
        // someone, and an invented person on screen poisons the premise that
        // the village is real — including the shots' own example name, if the
        // model parrots it back for the wrong subject.
        let known: Vec<&str> = of.key.whom.iter().map(|w| w.name.as_str()).collect();
        if !speaks_only_of(line, &known) {
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
            || shots()
                .iter()
                .any(|(_, said)| said.eq_ignore_ascii_case(&line))
        {
            return None;
        }
        Some(line)
    }

    /// One villager's thought, or `None` if what came back is unfit to think.
    fn muse(
        &self,
        ctx: &mut llama_cpp_2::context::LlamaContext<'_>,
        of: &Musing,
        seed: u64,
    ) -> Option<String> {
        let prompt = muse_chatml(of.is_reply(), &describe_musing(of));
        let raw = self.generate(ctx, &prompt, seed).ok()?;
        let line = raw.lines().next()?;
        if !admissible(line) {
            return None;
        }
        // The same truth gate as speech: a thought may name this person's own
        // world — themself, their place, their people — and nobody else.
        let known: Vec<&str> = of.known.iter().map(String::as_str).collect();
        if !speaks_only_of(line, &known) {
            return None;
        }
        let line = tidy(line);
        // A worked example handed straight back is not this person's thought —
        // and a reply that merely echoes what it was answering is not a reply.
        let shots = if of.is_reply() {
            reply_shots()
        } else {
            muse_shots()
        };
        if shots
            .iter()
            .any(|(_, said)| said.eq_ignore_ascii_case(&line))
        {
            return None;
        }
        if of
            .heard
            .as_ref()
            .is_some_and(|heard| heard.eq_ignore_ascii_case(&line))
        {
            return None;
        }
        Some(line)
    }

    #[allow(deprecated)] // token_to_str's Special: fine until the crate's replacement lands
    fn generate(
        &self,
        ctx: &mut llama_cpp_2::context::LlamaContext<'_>,
        prompt: &str,
        seed: u64,
    ) -> Result<String, String> {
        // The context lives as long as the voice; only its MEMORY is wiped
        // between lines. The prompts share no prefix worth keeping once the
        // dossier is in them, and a clean slate is the simplest correctness.
        ctx.clear_kv_cache();

        // ChatML's <|im_start|> markers are special tokens; the parser must
        // be told to read them as such or they tokenize as punctuation soup.
        let tokens = self
            .model
            .str_to_token(prompt, AddBos::Never)
            .map_err(|e| e.to_string())?;

        // The prompt is fed in SMALL SIPS with a breath between each. Not for
        // the arithmetic — for the MEMORY BUS. This machine's GPU and CPU
        // drink from the same unified memory, and prefilling a whole prompt
        // in one burst streams the model's entire weights through the
        // controller the renderer is feeding from: no thread priority can
        // protect the frame from that. Chunked, with pauses, the same work
        // happens as a hum instead of a spike. (llama.cpp masks offset
        // batches correctly, so this is safe here where it was not in the
        // old engine.) Costs ~100ms per line; nothing waits on a line.
        let mut batch = LlamaBatch::new(1280, 1);
        let last = tokens.len() - 1;
        for (chunk_index, chunk) in tokens.chunks(48).enumerate() {
            batch.clear();
            let start = chunk_index * 48;
            for (i, token) in chunk.iter().enumerate() {
                let position = start + i;
                batch
                    .add(*token, position as i32, &[0], position == last)
                    .map_err(|e| e.to_string())?;
            }
            ctx.decode(&mut batch).map_err(|e| e.to_string())?;
            std::thread::sleep(std::time::Duration::from_millis(8));
        }

        // Warm enough to differ between tellers, cool enough to obey.
        let mut sampler = LlamaSampler::chain_simple([
            LlamaSampler::top_p(0.9, 1),
            LlamaSampler::temp(0.8),
            LlamaSampler::dist(seed as u32),
        ]);

        let mut out = String::new();
        let mut position = tokens.len() as i32;
        for _ in 0..MAX_TOKENS {
            let token = sampler.sample(&ctx, batch.n_tokens() - 1);
            sampler.accept(token);
            // The GGUF knows its own turn-enders; <|im_end|> is one of them.
            if self.model.is_eog_token(token) {
                break;
            }
            let piece = self
                .model
                .token_to_str(token, Special::Tokenize)
                .unwrap_or_default();
            out.push_str(&piece);
            // One sentence: a newline ends the turn whatever the model thinks.
            if out.contains('\n') {
                break;
            }
            batch.clear();
            batch
                .add(token, position, &[0], true)
                .map_err(|e| e.to_string())?;
            ctx.decode(&mut batch).map_err(|e| e.to_string())?;
            position += 1;
        }
        Ok(out)
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
     complete spoken sentence of eight to sixteen words, with a subject — the \
     way an ordinary person actually talks, never a clipped fragment. Plain \
     everyday speech — no poetry, no riddles, no proverbs. Never repeat the \
     details back. Never name your \
     trade, your belief, your manner or your nature — let them show in HOW you \
     say it. Never explain or narrate. No quotation marks, no modern words. \
     Say 'the god', never a name."
}

/// The teller's own circumstances, as fields rather than prose.
fn describe(of: &Retelling) -> String {
    let mut lines = vec![
        format!("what happened: {}", of.key.kind.describe()),
        format!("how you know: {}", of.key.hand.word()),
        format!("your belief: {}", of.key.faith.word()),
    ];
    // The one specific in the telling: who it befell, by name and by what
    // they are to the teller. This is what turns "saw someone hurled across
    // the ground" into a story about Feitreh, your brother.
    if let Some(whom) = &of.key.whom {
        lines.push(format!("who it happened to: {}", whom.phrase()));
    }
    if let Some(voice) = of.key.voice {
        lines.push(format!("your trade: {}", voice.describe()));
    }
    if let Some(manner) = of.key.bearing.word() {
        lines.push(format!("your manner: {manner}"));
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
    let example = |kind: DivineEventKind,
                   hand: Hand,
                   trust: f32,
                   voice: Vocation,
                   bearing: Bearing,
                   which: usize| {
        let said = kind.rumors()[which % kind.rumors().len()].to_string();
        (
            Retelling::new(kind, hand, Some(voice), trust, bearing, None, 0.5, 0),
            said,
        )
    };
    // Three examples, three different manners — including one Plain, so the
    // model sees that the manner line is sometimes simply absent rather than
    // learning to expect it and inventing one when it is missing.
    let mut shots = vec![
        example(
            DivineEventKind::Lifted,
            Hand::Witnessed,
            0.8,
            Vocation::Gatherer,
            Bearing::Plain,
            1,
        ),
        example(
            DivineEventKind::Smote,
            Hand::Heard,
            0.4,
            Vocation::Mason,
            Bearing::Terse,
            2,
        ),
        example(
            DivineEventKind::Provided,
            Hand::Distant,
            0.1,
            Vocation::Hunter,
            Bearing::Bleak,
            2,
        ),
    ];
    // One example that NAMES its subject, with an answer written for it —
    // the only shot not drawn from the rumour corpus, because the corpus
    // predates subjects and never names one. Without this, a model handed
    // "who it happened to: Feitreh, your brother" treats the name as one
    // more field to ignore; shown once how a teller uses it, it uses it.
    // The example name is made up, which is safe on both sides: it appears
    // in the prompt (never checked), and if the model parrots it back for a
    // different subject, the truth gate refuses the line.
    shots.push((
        Retelling::new(
            DivineEventKind::Thrown,
            Hand::Witnessed,
            Some(Vocation::Farmer),
            0.6,
            Bearing::Plain,
            Some(Whom {
                name: "Sathei".into(),
                tie: "your neighbour".into(),
            }),
            0.5,
            0,
        ),
        "it threw Sathei across the square like a sack of grain".into(),
    ));
    shots
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

/// What the model is told about thinking, as against telling.
///
/// A thought has no audience, and the instruction leans on that: inward,
/// unperformed, half a sentence is fine. Everything else — the register, the
/// bans, the god's name — matches the speaking prompt, because it is the same
/// world in the same mouth.
fn muse_system_prompt() -> &'static str {
    "You are a villager in a pre-industrial village. Give ONLY the thought \
     passing through your head right now: one complete sentence of eight to \
     sixteen words, with a subject, the way an ordinary person grumbles or \
     wonders to themself — never a clipped fragment. Plain everyday speech — \
     no poetry, no riddles, no proverbs. Never repeat the details back. Never name your trade, your \
     belief, your manner or your nature — let them show in HOW you think. No \
     quotation marks, no modern words. Say 'the god', never a name."
}

/// What the model is told about answering a neighbour.
///
/// The third and last register: not a story performed, not a thought kept in,
/// but the short word back across a fence. The reply must react to what was
/// said rather than restate it — small models love to echo, and the ban does
/// more work here than anywhere.
fn reply_system_prompt() -> &'static str {
    "You are a villager in a pre-industrial village. A neighbour has just told \
     you something; answer ONLY with the words you would say back: one \
     complete spoken sentence with a subject, eight to sixteen words, the way \
     an ordinary person actually talks — never a clipped fragment. React to \
     it — believe it, doubt it, fear it, want more of it — never repeat it \
     back. Plain everyday speech, no poetry, no proverbs. \
     Never name your trade, your belief, your manner or your nature. No \
     quotation marks, no modern words. Say 'the god', never a name."
}

/// A musing's circumstances, as fields rather than prose.
///
/// The place lines come first — a thought starts from where you are standing —
/// and the pressing thing comes last, nearest the answer, which is where a
/// small model's attention actually lands.
fn describe_musing(of: &Musing) -> String {
    let mut lines = of.place.clone();
    if let Some(voice) = of.voice {
        lines.push(format!("your trade: {}", voice.describe()));
    }
    if let Some(manner) = of.bearing.word() {
        lines.push(format!("your manner: {manner}"));
    }
    lines.push(format!("your belief: {}", of.faith.word()));
    if !of.body.is_empty() {
        lines.push(format!("your body: {}", of.body.join(", and ")));
    }
    if let Some(heard) = &of.heard {
        lines.push(format!("they just told you: {heard}"));
    }
    lines.push(format!("on your mind: {}", of.mind));
    lines.join("\n")
}

/// Worked examples for thinking, anchored to the small-talk corpus the same
/// way the retelling shots anchor to the rumours: the answers are lines a
/// person wrote for this world, so the model learns this register rather
/// than inventing one.
fn muse_shots() -> Vec<(Musing, &'static str)> {
    let place = |name: &str, time: &str, village: &str| {
        vec![
            format!("where you live: the village of {name}"),
            format!("the time: {time}"),
            format!("the village: {village}"),
        ]
    };
    vec![
        (
            Musing {
                who: Entity::PLACEHOLDER,
                voice: Some(Vocation::Farmer),
                bearing: Bearing::Bleak,
                faith: FaithBand::Wavering,
                body: vec!["hungry"],
                place: place("Harrowfen", "autumn, rain falling", "the larder runs thin"),
                mind: "the empty larder".into(),
                heard: None,
                aloud: false,
                prayer: false,
                known: vec!["Harrowfen".into()],
            },
            // Hand-written whole sentences, no longer lifted from the corpus:
            // the corpus lines are clipped fragments by design, and the model
            // copies its examples far harder than its instructions — shown
            // fragments, the whole village spoke in them.
            "I cannot remember when I last ate a proper meal",
        ),
        (
            Musing {
                who: Entity::PLACEHOLDER,
                voice: Some(Vocation::Mason),
                bearing: Bearing::Plain,
                faith: FaithBand::Doubting,
                body: vec![],
                place: place(
                    "Harrowfen",
                    "autumn, the sky grey and low",
                    "the larder holds, for now",
                ),
                mind: "no roof of your own yet".into(),
                heard: None,
                aloud: false,
                prayer: false,
                known: vec!["Harrowfen".into()],
            },
            "I want a roof of my own before the cold comes",
        ),
        (
            Musing {
                who: Entity::PLACEHOLDER,
                voice: Some(Vocation::Fisher),
                bearing: Bearing::Bright,
                faith: FaithBand::Sure,
                body: vec![],
                place: place(
                    "Harrowfen",
                    "summer, the sky clear",
                    "the stores stand full",
                ),
                mind: "a fine day at the water".into(),
                heard: None,
                aloud: false,
                prayer: false,
                known: vec!["Harrowfen".into()],
            },
            "the water was kind to me today, and that is enough",
        ),
    ]
}

/// Worked examples for answering, whose answers are the game's own written
/// replies — the sceptic's, the believer's, the frightened one's.
fn reply_shots() -> Vec<(Musing, &'static str)> {
    let place = vec![
        "where you live: the village of Harrowfen".to_string(),
        "the time: autumn, the sky grey and low".to_string(),
        "the village: the larder holds, for now".to_string(),
    ];
    let heard = |voice, bearing, faith, told: &str, mind: &str| Musing {
        who: Entity::PLACEHOLDER,
        voice: Some(voice),
        bearing,
        faith,
        body: vec![],
        place: place.clone(),
        mind: mind.into(),
        heard: Some(told.into()),
        aloud: true,
        prayer: false,
        known: vec!["Harrowfen".into()],
    };
    vec![
        (
            heard(
                Vocation::Mason,
                Bearing::Plain,
                FaithBand::Doubting,
                "one bolt, out of a sky with no storm in it",
                "whether to believe a word of it",
            ),
            // The doubter's written reply.
            "I will believe that when I see it myself",
        ),
        (
            heard(
                Vocation::Gatherer,
                Bearing::Terse,
                FaithBand::Sure,
                "the larder was empty and then it was not, they say",
                "whether to believe a word of it",
            ),
            "so the stories people tell are true after all",
        ),
        (
            heard(
                Vocation::Fisher,
                Bearing::Plain,
                FaithBand::Wavering,
                "he just rose, feet kicking at nothing",
                "you stood there and saw it happen too",
            ),
            // The fellow witness's written reply.
            "that is no story — I stood right there beside you",
        ),
    ]
}

/// ChatML for a musing or a reply, mirroring [`chatml`].
fn muse_chatml(reply: bool, user: &str) -> String {
    let (system, shots) = if reply {
        (reply_system_prompt(), reply_shots())
    } else {
        (muse_system_prompt(), muse_shots())
    };
    let mut out = format!("<|im_start|>system\n{system}<|im_end|>\n");
    for (fields, said) in shots {
        out.push_str(&format!(
            "<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n{said}<|im_end|>\n",
            describe_musing(&fields)
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

/// Whether a line speaks only of people the teller actually knows of.
///
/// The truth gate. A model asked about "Feitreh, your neighbour" will now and
/// then reach for a name of its own — a cousin Marcus, a village elder nobody
/// has ever heard of — and a single invented person on screen poisons the
/// whole premise that the village is real. So the rule is structural rather
/// than hopeful: every capitalised word in the line must be a name the
/// simulation gave it. Anything else is a miss, exactly as if no model had
/// answered.
///
/// There is deliberately NO exemption for the first word. The whole register —
/// every written rumour, every worked example — runs lowercase, so the model
/// imitating its examples starts lowercase too, and a line that opens with a
/// capital is already drifting. Exempting it as sentence case would be the one
/// door left open ("Marcus saw it too" walks straight through), and the cost
/// of keeping it shut is only that an occasional honestly-capitalised line
/// falls back to a written one. The speaking I, which is legitimately capital
/// anywhere, is the sole exception.
pub fn speaks_only_of(line: &str, known: &[&str]) -> bool {
    for word in line.split_whitespace() {
        let word = word.trim_matches(|c: char| !c.is_alphanumeric());
        let Some(first) = word.chars().next() else {
            continue;
        };
        if !first.is_uppercase() {
            continue;
        }
        if word == "I" || word.starts_with("I'") {
            continue;
        }
        if !known.contains(&word) {
            return false;
        }
    }
    true
}

/// Tidies an admissible line into the shape the bubbles expect.
pub fn tidy(line: &str) -> String {
    let mut out = line.trim().trim_matches('"').trim().to_string();
    // The bubbles run lower-case; a capital reads as a caption. The pronoun
    // I is the one exception — it is capital anywhere, including first, and
    // "i wish I had someone" was the bug that proved it.
    let leading_i =
        out == "I" || out.starts_with("I ") || out.starts_with("I'") || out.starts_with("I,");
    if !leading_i
        && let Some(first) = out.chars().next()
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
        // The speaking I keeps its capital even at the head of the line.
        assert_eq!(
            tidy("I wish I had someone to come home to."),
            "I wish I had someone to come home to"
        );
        assert_eq!(
            tidy("I'll not walk that field again"),
            "I'll not walk that field again"
        );
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
            Bearing::Plain,
            None,
            0.4,
            0,
        );
        let b = Retelling::new(
            DivineEventKind::Smote,
            Hand::Witnessed,
            Some(Vocation::Fisher),
            0.95,
            Bearing::Plain,
            None,
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
            Bearing::Plain,
            None,
            0.4,
            0,
        );
        assert_ne!(a.key, heard.key);

        // And so is the grain of the person. The prompt tells the model about
        // their manner, so the cache MUST divide on it — a shape that mentions
        // a manner without keying on one would hand a gloomy villager a line
        // composed for a cheerful one.
        let gloomy = Retelling::new(
            DivineEventKind::Smote,
            Hand::Witnessed,
            Some(Vocation::Fisher),
            0.8,
            Bearing::Bleak,
            None,
            0.4,
            0,
        );
        assert_ne!(a.key, gloomy.key, "the manner must divide the cache");
    }

    #[test]
    fn no_invented_name_survives_the_truth_gate() {
        // The gate that makes invention structurally impossible rather than
        // unlikely: every capitalised word must be a name the simulation gave.
        let known = ["Feitreh"];
        // The subject may be named; nobody else may.
        assert!(speaks_only_of("they took Feitreh right up", &known));
        assert!(!speaks_only_of("Marcus saw it too, ask him", &known));
        assert!(!speaks_only_of(
            "the elder Tobias warned us of this",
            &known
        ));
        // With no subject, no name at all may appear — including the few-shot
        // example's own, parroted back for the wrong telling.
        assert!(!speaks_only_of(
            "it flung Sathei across the square like a sack of grain",
            &[]
        ));
        // No sentence-case exemption, even at the head of the line — that
        // door is exactly where an invented name walks through. The village's
        // register is lowercase, so an honest line rarely pays this, and one
        // that does only falls back to a written phrasing.
        assert!(!speaks_only_of("The sky split open, I swear it", &[]));
        assert!(speaks_only_of("the sky split open, I swear it", &[]));
        // The speaking I is the one legitimate capital.
        assert!(speaks_only_of("I'll not walk that field again", &[]));
        assert!(speaks_only_of("I saw it, I did", &[]));
        // Punctuation does not smuggle a name past the gate.
        assert!(!speaks_only_of("it was Marcus, I tell you", &known));
        // But the same known name wrapped in punctuation still passes.
        assert!(speaks_only_of("poor Feitreh, poor soul", &known));
    }

    #[test]
    fn the_subject_reaches_the_prompt_and_divides_the_cache() {
        let with = |whom: Option<Whom>| {
            Retelling::new(
                DivineEventKind::Thrown,
                Hand::Witnessed,
                Some(Vocation::Fisher),
                0.8,
                Bearing::Plain,
                whom,
                0.4,
                0,
            )
        };
        let brother = with(Some(Whom {
            name: "Feitreh".into(),
            tie: "your brother".into(),
        }));
        assert!(
            describe(&brother).contains("who it happened to: Feitreh, your brother"),
            "the prompt must carry the subject: {}",
            describe(&brother),
        );
        // No subject, no line — not a line saying there is no subject.
        assert!(!describe(&with(None)).contains("who it happened to"));

        // The same act befalling a stranger is a different telling. Without
        // this split, a line composed about a brother would be served to
        // someone he is nothing to.
        let stranger = with(Some(Whom {
            name: "Feitreh".into(),
            tie: "your neighbour".into(),
        }));
        assert_ne!(brother.key, stranger.key);
        assert_ne!(brother.key, with(None).key);
    }

    #[test]
    fn a_manner_reaches_the_prompt_and_a_plain_one_says_nothing() {
        let of = |bearing| {
            describe(&Retelling::new(
                DivineEventKind::Smote,
                Hand::Witnessed,
                Some(Vocation::Fisher),
                0.8,
                bearing,
                None,
                0.4,
                0,
            ))
        };
        assert!(of(Bearing::Bleak).contains("your manner: expects the worst"));
        assert!(of(Bearing::Terse).contains("your manner:"));
        // A manner that bends nothing is left out entirely rather than spending
        // a line of the prompt telling the model to disregard something.
        assert!(
            !of(Bearing::Plain).contains("your manner"),
            "an unremarkable manner should not reach the prompt at all",
        );
    }

    #[test]
    fn every_manner_the_traits_can_produce_is_one_the_teller_knows() {
        use crate::villager::traits::{Trait, Traits};
        // The mapping is the seam between two modules, and a trait added on one
        // side without a manner on the other would silently fall through to
        // Plain. Every speech-bending trait must land somewhere real.
        for (trait_, expected) in [
            (Trait::Quiet, Bearing::Terse),
            (Trait::Gloomy, Bearing::Bleak),
            (Trait::Cheerful, Bearing::Bright),
            (Trait::Diligent, Bearing::Plain),
        ] {
            assert_eq!(Traits(vec![trait_]).bearing(), expected, "{trait_:?}");
        }
        // Brevity governs the delivery of whatever else is true.
        assert_eq!(
            Traits(vec![Trait::Cheerful, Trait::Quiet]).bearing(),
            Bearing::Terse,
            "a quiet cheerful person is short about the good news",
        );
        assert_eq!(Traits(Vec::new()).bearing(), Bearing::Plain);
    }

    #[test]
    fn the_prompt_carries_the_fields_and_never_a_gods_name() {
        let of = Retelling::new(
            DivineEventKind::Uprooted,
            Hand::Heard,
            Some(Vocation::Mason),
            0.1,
            Bearing::Plain,
            None,
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
            Bearing::Plain,
            None,
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
