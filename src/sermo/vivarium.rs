//! THE LIVING VOICE: villagers speaking lines nobody wrote.
//!
//! Ported from Sermo Vivarium, the sister fork built to try this - Brett:
//! "It uses ChatGPT for the NPC voices and records them. I want you to study
//! this feature and incorporate it into the main game."
//!
//! Every speech opportunity becomes a compact TRUTH PACKET: the register, the
//! simulation tags in play, the slots that may be filled, a few plain facts
//! about the speaker and whoever they are talking to. The model may speak
//! only from that packet - omission is a prohibition, so a fact the game did
//! not send is a fact the villager cannot know. One background worker does
//! the asking; the Bevy thread NEVER waits on the network. A situation nobody
//! has a line for yet is simply quiet until one arrives, and from then on
//! every equivalent moment uses it.
//!
//! Every request, every accepted line, every rejected one and every reuse is
//! appended to a local JSONL file. That file is the point as much as the
//! speech is: it is the raw material for promoting good lines into the
//! authored corpus, where they cost nothing and work offline.
//!
//! THE KEY IS NEVER IN THIS REPOSITORY. It is read from `OPENAI_API_KEY` at
//! runtime, it is never logged, and without it this module does nothing and
//! the authored corpus speaks as it always has - Brett: "build it so the key
//! is gitignored and not supplied, or even better so it just uses an env var
//! on my laptop or something."

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs::OpenOptions,
    io::Write,
    path::PathBuf,
    sync::{
        Mutex,
        mpsc::{self, Receiver, Sender, TryRecvError},
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use bevy::log::{info, warn};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const KEY_VAR: &str = "OPENAI_API_KEY";
const MODEL_VAR: &str = "SERMO_LIVING_MODEL";
const LOG_VAR: &str = "SERMO_LIVING_LOG";
/// GPT-5.6 Luna: built for cost-sensitive, high-volume work, which is exactly
/// what a million lines of dialogue is. Brett: "Its way smarter and cheaper."
///
/// `SERMO_LIVING_MODEL` overrides it without a rebuild, which is how two
/// models get compared on the same moments before a spending decision that
/// will be lived with for the whole corpus.
const DEFAULT_MODEL: &str = "gpt-5.6-luna";
const MAX_PENDING: usize = 12;

/// HOW MANY LINES ONE CALL ASKS FOR. Brett's idea, and the best one of the
/// night: "What if we had it generate 20 instead of 1? Only one is shown in
/// game, but the other 19 are saved."
///
/// The economics are far more lopsided than twenty-to-one. The expensive part
/// of a call is the PROMPT - the whole voice specification and the truth packet
/// - sent every time to get eighteen words back. Twenty lines in one call sends
/// that prompt ONCE. Input cost stays flat and output rises twentyfold from a
/// tiny base, which on Luna's pricing is closer to a tenfold cut per line than
/// a wash.
///
/// And the variety is better, not merely cheaper. Twenty separate calls each
/// reach for the most obvious phrasing in isolation; one call producing twenty
/// sees all twenty at once and can deliberately differentiate them. That is
/// the repetition problem attacked at its source rather than patched with
/// `already_said` afterwards.
///
/// UP TO twenty, never exactly twenty - see the prompt. Demanding a count
/// invites padding, and a padded batch fills the vault with the rewordings the
/// whole exercise exists to avoid.
const A_BATCH: usize = 20;

/// The batch size actually asked for, `SERMO_BATCH` overriding the default.
///
/// A KNOB BECAUSE THE RIGHT NUMBER IS EMPIRICAL. Brett: "20 is just a random
/// number. Should we do more, less?" - and the answer is not in the pricing,
/// it is in how many genuinely distinct thoughts one narrow moment supports.
/// A forester telling firsthand about goblins may have ten real things to say;
/// asking for fifty buys forty rewordings, which is the exact pollution this
/// system exists to avoid.
///
/// It is a CEILING and not a quota - the prompt says stop early rather than
/// pad - so a generous number is fairly safe, and the log prints how many came
/// back against how many were asked for. Sweep it and read that line.
fn a_batch() -> usize {
    let set = ASKED_FOR.load(std::sync::atomic::Ordering::Relaxed);
    if set > 0 {
        return set;
    }
    std::env::var("SERMO_BATCH")
        .ok()
        .and_then(|n| n.parse::<usize>().ok())
        .filter(|n| (1..=200).contains(n))
        .unwrap_or(A_BATCH)
}

/// How many the settings screen is asking for. Zero means nobody has said.
///
/// AN ATOMIC, not a resource, because this is read on the WORKER THREAD when a
/// request is built and a worker has no ECS to ask. Brett wanted the number
/// adjustable while playing - "Maybe a setting in sermo settings to set that in
/// game?" - and a plain integer shared across a thread boundary is the whole
/// mechanism that needs.
pub static ASKED_FOR: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// How many lines a call asks for right now, for the settings to display.
pub fn batch_size() -> usize {
    a_batch()
}

/// Sets how many lines a call asks for.
pub fn ask_for(many: usize) {
    ASKED_FOR.store(many.clamp(1, 200), std::sync::atomic::Ordering::Relaxed);
}
const AWKWARD_WORK_PHRASES: &[&str] = &["cutter", "my cutting", "my cuttings"];
const SYSTEM_WORDS: &[&str] = &["morale", "wavering", "muse", "trait"];

/// THINGS THIS WORLD DOES NOT HAVE.
///
/// Brett, on a line about telling the mayor: "the town just started and there
/// is no town hall or mayor." There is no mayor in this game at all, and never
/// has been - the model reached for one because a village with no facts
/// attached to it is a village it has to imagine, and what it imagines is the
/// stock medieval one: mayors, lords, markets, taxes, coin.
///
/// This is the failure that matters most - Brett: "the entire simulation and
/// realism falls apart if they start making stuff up" - and it is the one
/// nothing else catches. An invented office is not a leaked name, not a system
/// word, not an engine label. It is a plausible sentence about a thing that
/// does not exist, which is exactly the shape of lie that does the damage.
///
/// A DENYLIST IS A PATCH, not the cure. The cure is a truth packet full enough
/// that there is no vacuum to fill - see `Dossier`. This is the floor under
/// it: whatever else goes wrong, nobody mentions a king.
///
/// KEPT SHORT, AND ONLY THINGS STRUCTURALLY ABSENT. The first version of this
/// listed "mayor" - and Brett: "When a town hall is raised they do elect a
/// mayor." There IS one. A denylist that bans real things is worse than none,
/// because it silences true lines and nobody notices which.
///
/// So: no feudal hierarchy above the village, and no money economy. Those are
/// absent by DESIGN rather than by not being built yet, which is the only
/// thing that makes a word safe to ban. Anything a village might plausibly
/// grow - livestock, carts, traders, a mayor - stays off this list, because
/// the day it is added the ban becomes the bug.
const NO_SUCH_THING: &[&str] = &[
    "lord", "king", "queen", "prince", "baron", "knight", "castle", "bishop", "abbot", "sheriff",
    "tax", "taxes", "rent", "coin",
];

/// The exact facts one candidate is allowed to speak from. This is deliberately
/// much smaller than an ECS dump: omission means the model is not allowed to
/// invent the fact.
#[derive(Clone, Debug, Serialize)]
struct Moment {
    speaker: u64,
    register: String,
    tags: Vec<String>,
    slots: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    world_facts: Vec<String>,
    #[serde(skip_serializing_if = "SocialTruth::is_empty")]
    facts: SocialTruth,
    #[serde(skip_serializing_if = "Option::is_none")]
    heard: Option<String>,
    /// LINES ALREADY WRITTEN FOR THIS SHAPE OF MOMENT, so the model can avoid
    /// them.
    ///
    /// Brett's run produced three near-identical wolf lines and two prayers
    /// that both began "steady my heart" - because nothing told the model what
    /// it had already said. Variety is the entire reason this system exists:
    /// "the same event may have a lot of different lines. We want to make sure
    /// that the user doesn't see repeated lines as much as possible."
    ///
    /// Rejecting duplicates afterwards would only waste the call. Sending them
    /// is what the model's context window is FOR - Brett: "we do have a
    /// massive context on the model we are using, we can pass whatever we need
    /// to."
    #[serde(skip_serializing_if = "Vec::is_empty")]
    already_said: Vec<String>,
}

/// Descriptive simulation truth for the writer. Unlike `tags`, these facts
/// explain a person or a scene but are not part of Sermo's runtime vocabulary.
/// They must all come from ECS state, including every proper name.
#[derive(Clone, Debug, Default, Serialize)]
pub struct SocialTruth {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker_name: Option<String>,
    /// "she", "he" or "they" - so a pronoun is CHOSEN rather than guessed.
    ///
    /// Nothing told the model this before, so every pronoun it wrote was a
    /// coin flip, and slotting a name froze the wrong one into a line that
    /// would then be replayed forever. Brett saw it coming: "should we add
    /// gender to that in case the lines use pronouns?"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker_is: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub traits: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub morale: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub listener_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relationship_to_listener: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relationship_cause: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settlement_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation_intent: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub topic_facts: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub recent_memories: Vec<String>,
}

impl SocialTruth {
    fn is_empty(&self) -> bool {
        self.speaker_name.is_none()
            && self.traits.is_empty()
            && self.morale.is_none()
            && self.listener_name.is_none()
            && self.relationship_to_listener.is_none()
            && self.relationship_cause.is_none()
            && self.settlement_name.is_none()
            && self.conversation_intent.is_none()
            && self.topic_facts.is_empty()
            && self.recent_memories.is_empty()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Candidate {
    text: String,
    tags: Vec<String>,
    grounding: Vec<String>,
}

/// What one call comes back with: a batch, of which one is spoken and the rest
/// are kept. See [`A_BATCH`].
#[derive(Clone, Debug, Deserialize)]
struct Batch {
    lines: Vec<Candidate>,
}

impl Moment {
    fn new(
        speaker: u64,
        tags: &[&str],
        slots: &[(&str, &str)],
        heard: Option<&str>,
        facts: SocialTruth,
    ) -> Moment {
        let mut tags: Vec<String> = tags.iter().map(|tag| (*tag).to_string()).collect();
        tags.sort_unstable();
        tags.dedup();
        let register = if tags.iter().any(|tag| tag == "prayer") {
            "prayer".to_string()
        } else {
            tags.iter()
                .find(|tag| {
                    matches!(
                        tag.as_str(),
                        "muse"
                            | "tell"
                            | "reply"
                            | "yell"
                            | "prayer"
                            | "chat:open"
                            | "chat:reply"
                            | "chat:followup"
                            | "chat:end"
                    )
                })
                .cloned()
                .unwrap_or_else(|| "speech".to_string())
        };
        let world_facts = world_facts(&tags);
        Moment {
            already_said: Vec::new(),
            speaker,
            register,
            tags,
            slots: slots
                .iter()
                .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
                .collect(),
            world_facts,
            facts,
            // Quoted speech belongs in a reply prompt, but cap it so one unusual
            // line cannot turn a cheap lab request into a giant transcript.
            heard: heard.map(|line| line.chars().take(180).collect()),
        }
    }

    fn key(&self) -> String {
        // A village needs a small reusable pool, not one paid API request per
        // entity. Identity is still recorded in the transcript, but only the
        // speakable truth determines cache reuse.
        let bytes = serde_json::to_vec(&json!({
            "register": self.register,
            "tags": self.tags,
            "slots": self.slots,
            "world_facts": self.world_facts,
            "heard": self.heard,
            "facts": self.facts,
        }))
        .unwrap_or_default();
        let mut hash: u64 = 0xcbf29ce484222325;
        for byte in bytes {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        format!("{hash:016x}")
    }
}

/// Translate engine identifiers into the only event language the writer may
/// use. Raw enum words such as `Delivered` and `Uprooted` are implementation
/// details, not things a villager would ever say.
fn world_facts(tags: &[String]) -> Vec<String> {
    tags.iter()
        .filter_map(|tag| match tag.as_str() {
            "event:lifted" => Some("Someone was lifted into the air."),
            "event:thrown" => Some("Someone was hurled across the ground."),
            "event:setdown" => Some("Someone was set down gently."),
            "event:impact" => Some("Someone struck the ground hard."),
            "event:provided" => Some("Food was provided to hungry people."),
            "event:smote" => Some("Lightning was called down."),
            "event:uprooted" => Some("A tree was torn out of the ground."),
            "event:mended" => Some("An injured person was made whole."),
            "event:quaked" => Some("The ground shook."),
            "event:perished" => Some("One of the village died."),
            "event:delivered" => Some("A child was born safely."),
            "event:flourished" => Some("The fields yielded an unusually good harvest."),
            "event:mauled" => Some("A wolf attacked someone from the village."),
            "event:rained" => Some("Rain came when it was called."),
            "event:beckoned" => Some("A pillar of light stood in the world."),
            "event:fell" => Some("A stone fell from a clear sky."),
            "event:doubtsown" => Some("A shadow of doubt passed through the village."),
            "topic:miner" => Some("The conversation concerns quarry or stone work."),
            "topic:forester" => Some("The conversation concerns work in the woods."),
            "topic:builder" => Some("The conversation concerns building work."),
            "topic:hunter" => Some("The conversation concerns hunting."),
            "topic:roof" => Some("The conversation concerns a roof that needs attention."),
            _ => None,
        })
        .map(str::to_string)
        .collect()
}

struct Job {
    key: String,
    moment: Moment,
}

struct Result {
    key: String,
    moment: Moment,
    /// How many the model offered, and which of them survived the gate.
    answer: std::result::Result<(usize, Vec<Candidate>), String>,
}

/// A completed line waiting to rejoin the main-thread presentation flow.
/// Network work never touches ECS directly.
pub struct ReadyLine {
    pub speaker: u64,
    pub register: String,
    pub text: String,
    /// THE TAGS THE LINE IS FILED UNDER, so it can be written into the vault
    /// and found again. The model returns which of the moment's tags it
    /// actually spoke from, and `validate` has already checked that they are
    /// a subset of what was offered and that the register is among them - so
    /// these are the tags a future moment must carry to be answered by this
    /// sentence.
    ///
    /// Without them a generated line is a sentence with no address: sayable
    /// once, and unfindable ever after. Brett: "it talks for them and
    /// automatically writes the lines to the data base with tags and
    /// everything."
    pub tags: Vec<String>,
}

/// One development-only asynchronous model connection.
pub struct Vivarium {
    jobs: Sender<Job>,
    results: Mutex<Receiver<Result>>,
    cache: HashMap<String, Candidate>,
    ready: Vec<ReadyLine>,
    pending: HashSet<String>,
    exhausted: HashSet<String>,
    /// Every line already written for each shape of moment, so the next
    /// request can be told not to write them again. See `Moment::already_said`.
    said_before: HashMap<String, Vec<String>>,
    log: PathBuf,
}

impl Vivarium {
    /// Starts only when deliberately requested. A normal run of the fork is
    /// still the authored game, and a missing key does not turn into a panic.
    /// Wakes the worker if there is a key to speak with.
    ///
    /// No enable flag any more: the fork gated this on `SERMO_VIVARIUM=1`
    /// because it was a laboratory, and here it is a setting somebody flips
    /// mid-run. So the thread exists from startup wherever a key does, and
    /// whether it is CONSULTED is the switch's business. A machine with no
    /// key simply never has one, and the corpus speaks.
    pub fn awake() -> Option<Vivarium> {
        let Some(key) = load_key() else {
            return None;
        };
        let model = std::env::var(MODEL_VAR).unwrap_or_else(|_| DEFAULT_MODEL.to_string());
        let log = std::env::var(LOG_VAR)
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("logs/sermo-living.jsonl"));
        if let Some(parent) = log.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let (jobs, work) = mpsc::channel::<Job>();
        let (done, results) = mpsc::channel::<Result>();
        thread::Builder::new()
            .name("sermo-vivarium".to_string())
            .spawn(move || work_loop(work, done, key, model))
            .expect("Sermo Vivarium worker thread");

        info!(
            "the living voice is awake; every line it writes is recorded at {}",
            log.display()
        );
        Some(Vivarium {
            jobs,
            results: Mutex::new(results),
            cache: HashMap::new(),
            ready: Vec::new(),
            pending: HashSet::new(),
            exhausted: HashSet::new(),
            said_before: HashMap::new(),
            log,
        })
    }

    /// Returns a validated cached candidate, or queues an equivalent request
    /// once. The caller can always fall back to the corpus immediately.
    /// ALWAYS A NEW LINE, never one this run has already said.
    ///
    /// Brett: "the lines in chatgpt mode shouldnt be reused since its whole
    /// purpose is to generate fresh context." The cache is what put the same
    /// sentence in two mouths ten feet apart - one moment key, one answer,
    /// served twice. Reuse belongs to the VAULT, which is a corpus and wants
    /// one line to serve many moments; the factory's whole job is to make
    /// something that was not there before.
    ///
    /// So a cached answer is SPENT here: taken, returned once, and dropped, so
    /// the next equivalent moment asks again. What was said is not lost - it
    /// went into the vault the moment it arrived.
    /// As [`Vivarium::ask_afresh`], with what the world knows about the
    /// speaker - see `Tongue::what_is_true_of`.
    pub fn ask_afresh_of(
        &mut self,
        speaker: u64,
        tags: &[&str],
        slots: &[(&str, &str)],
        heard: Option<&str>,
        facts: SocialTruth,
    ) -> Option<String> {
        self.drain();
        let mut moment = Moment::new(speaker, tags, slots, heard, facts);
        let key = moment.key();
        // What this shape of moment has already been given. Capped: a hundred
        // is plenty to steer away from and far short of anything the context
        // would notice.
        moment.already_said = self
            .said_before
            .get(&key)
            .map(|said| said.iter().rev().take(100).cloned().collect())
            .unwrap_or_default();
        if let Some(candidate) = self.cache.remove(&key) {
            // Asked again straight away, so the next moment of this shape has
            // a fresh line waiting rather than starting from silence.
            self.pending.remove(&key);
            let _ = self.jobs.send(Job {
                key,
                moment: moment.clone(),
            });
            return Some(candidate.text);
        }
        if self.pending.len() >= MAX_PENDING {
            return None;
        }
        if self.pending.insert(key.clone()) {
            self.record("requested", &key, &moment, None, None);
            if self.jobs.send(Job { key, moment }).is_err() {
                warn!("the living voice's worker stopped");
            }
        }
        None
    }

    pub fn ask(
        &mut self,
        speaker: u64,
        tags: &[&str],
        slots: &[(&str, &str)],
        heard: Option<&str>,
    ) -> Option<String> {
        self.ask_with_truth(speaker, tags, slots, heard, SocialTruth::default())
    }

    pub fn ask_with_truth(
        &mut self,
        speaker: u64,
        tags: &[&str],
        slots: &[(&str, &str)],
        heard: Option<&str>,
        facts: SocialTruth,
    ) -> Option<String> {
        self.drain();
        let moment = Moment::new(speaker, tags, slots, heard, facts);
        let key = moment.key();
        if let Some(candidate) = self.cache.get(&key).cloned() {
            self.record("used", &key, &moment, Some(&candidate), None);
            return Some(candidate.text);
        }
        if self.exhausted.contains(&key) || self.pending.len() >= MAX_PENDING {
            return None;
        }
        if self.pending.insert(key.clone()) {
            self.record("requested", &key, &moment, None, None);
            if self.jobs.send(Job { key, moment }).is_err() {
                warn!("Sermo Vivarium worker stopped; returning to the authored corpus");
            }
        }
        None
    }

    fn drain(&mut self) {
        let arrived = {
            let receiver = self.results.lock().expect("Vivarium result receiver");
            let mut arrived = Vec::new();
            loop {
                match receiver.try_recv() {
                    Ok(result) => arrived.push(result),
                    Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
                }
            }
            arrived
        };
        for result in arrived {
            self.pending.remove(&result.key);
            match result.answer {
                Ok((asked, kept)) if !kept.is_empty() => {
                    info!(
                        "the living voice wrote {} lines for one moment, of {asked} offered",
                        kept.len()
                    );
                    // ONE IS SPOKEN, ALL ARE KEPT. Brett: "Only one is shown in
                    // game, but the other 19 are saved."
                    //
                    // Every one is remembered against this shape of moment, so
                    // the next request for it is told not to write any of them
                    // again - which is what stops the second batch being a
                    // rewording of the first.
                    let remembered = self.said_before.entry(result.key.clone()).or_default();
                    for line in &kept {
                        remembered.push(line.text.clone());
                    }
                    // The first goes to whoever asked; the rest go straight to
                    // the vault by way of `take_ready`, which is what puts a
                    // line on disk.
                    for line in &kept {
                        self.ready.push(ReadyLine {
                            speaker: result.moment.speaker,
                            register: result.moment.register.clone(),
                            text: line.text.clone(),
                            tags: line.tags.clone(),
                        });
                        self.record("candidate", &result.key, &result.moment, Some(line), None);
                    }
                    // The cache holds one for the moment that asked. The rest
                    // are already written down and will be found by the vault.
                    if let Some(first) = kept.into_iter().next() {
                        self.cache.insert(result.key.clone(), first);
                    }
                }
                Ok((asked, _)) => {
                    // Every line in the batch broke a rule. Worth saying out
                    // loud: it means the prompt and the gate disagree, and the
                    // JSONL holds each reason.
                    let why = format!("all {asked} lines in the batch were refused");
                    self.exhausted.insert(result.key.clone());
                    self.record("rejected", &result.key, &result.moment, None, Some(&why));
                }
                Err(error) => {
                    self.exhausted.insert(result.key.clone());
                    self.record("rejected", &result.key, &result.moment, None, Some(&error));
                }
            }
        }
    }

    /// Collect background answers on the game thread. A candidate is delivered
    /// once even when the original conversational beat has already passed.
    pub fn take_ready(&mut self) -> Vec<ReadyLine> {
        self.drain();
        std::mem::take(&mut self.ready)
    }

    fn record(
        &self,
        state: &str,
        key: &str,
        moment: &Moment,
        candidate: Option<&Candidate>,
        error: Option<&str>,
    ) {
        let row = json!({
            "at": now_secs(),
            "state": state,
            "key": key,
            "moment": moment,
            "candidate": candidate,
            "error": error,
        });
        let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&self.log) else {
            return;
        };
        let _ = writeln!(file, "{row}");
    }
}

/// A deliberate one-request smoke test for the local development setup. It is
/// never run by the game itself; `cargo run -- --vivarium-probe` is the call.
pub fn probe() {
    let Some(mut vivarium) = Vivarium::awake() else {
        eprintln!(
            "Vivarium is not enabled. Set SERMO_VIVARIUM=1 and provide OPENAI_API_KEY or OpenAI-API-Key.txt."
        );
        return;
    };
    let tags = ["muse", "hungry", "wavering", "housed"];
    let started = std::time::Instant::now();
    loop {
        if let Some(line) = vivarium.ask(0, &tags, &[], None) {
            println!("Vivarium candidate: {line}");
            return;
        }
        if started.elapsed() > Duration::from_secs(45) {
            eprintln!("The living voice did not answer in time. See logs/sermo-living.jsonl.");
            return;
        }
        thread::sleep(Duration::from_millis(150));
    }
}

fn work_loop(work: Receiver<Job>, done: Sender<Result>, key: String, model: String) {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(35))
        .build()
        .expect("Vivarium HTTP client");
    for job in work {
        // EVERY LINE IN THE BATCH IS JUDGED ON ITS OWN. A batch of twenty is
        // twenty chances to break a rule, and one bad line must not cost the
        // other nineteen - so the gate runs per line and the survivors come
        // back together. Rejections are free and are recorded with their
        // reasons, which is how the prompt gets better.
        let answer = request_candidate(&client, &key, &model, &job.moment).map(|batch| {
            let asked = batch.len();
            let kept: Vec<Candidate> = batch
                .into_iter()
                .filter_map(|line| validate(&job.moment, line).ok())
                .collect();
            (asked, kept)
        });
        let _ = done.send(Result {
            key: job.key,
            moment: job.moment,
            answer,
        });
    }
}

fn request_candidate(
    client: &reqwest::blocking::Client,
    key: &str,
    model: &str,
    moment: &Moment,
) -> std::result::Result<Vec<Candidate>, String> {
    let prompt = format!(
        "Write up to {} genuinely different Sermo utterances for this truth packet:\n{}",
        a_batch(),
        serde_json::to_string_pretty(moment).map_err(|error| error.to_string())?
    );
    let body = json!({
        "model": model,
        // NOT STORED SERVER-SIDE. Every line is already written to our own
        // JSONL, which is the record that matters and the one that feeds the
        // corpus.
        "store": false,
        // NO REASONING, and this is the single biggest lever on what a
        // million lines cost. Reasoning tokens bill as OUTPUT tokens, and a
        // villager saying eighteen words does not need a chain of thought -
        // left at the default the thinking would cost several times the
        // sentence. `SERMO_LIVING_EFFORT` raises it if the lines come back
        // worse; that is a judgement to make by reading them, not by
        // guessing here.
        "reasoning": {
            "effort": std::env::var("SERMO_LIVING_EFFORT")
                .unwrap_or_else(|_| "none".to_string())
        },
        "text": {
            "verbosity": "low",
            "format": {
                "type": "json_schema",
                "name": "sermo_utterance",
                "strict": true,
                // A BATCH, not a line. See `A_BATCH`.
                "schema": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "lines": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "additionalProperties": false,
                                "properties": {
                                    "text": { "type": "string" },
                                    "tags": {
                                        "type": "array",
                                        "items": { "type": "string" }
                                    },
                                    "grounding": {
                                        "type": "array",
                                        "items": { "type": "string" }
                                    }
                                },
                                "required": ["text", "tags", "grounding"]
                            }
                        }
                    },
                    "required": ["lines"]
                }
            }
        },
        "input": [
            {
                "role": "developer",
                "content": [{
                    "type": "input_text",
                    "text": "You write candidate dialogue for a medieval village simulation. Return only the required JSON. The truth packet is complete: never introduce a person, object, event, motive, relationship, place, weather, time of day, or condition it does not contain. This is a village of ten to forty people with no lord, no king, no coin and no market; it has a longhouse, huts, a shrine, a storehouse, fields, a mine, a dock and a tavern, and it elects a mayor only once a town hall stands. Never mention a rank, trade, building or custom the packet has not named. `speaker_is` gives the speaker's pronoun - use it and never guess. One tag says how the speaker knows: `saw` means THEY witnessed it themselves, `heard` means somebody told them, and `distant` means it has been round the village several times. Write from the one you are given: with `heard` do not claim to have seen it, and with `saw` do not attribute it to somebody else. `world_facts` and `topic_facts` are the plain meanings of engine state; never repeat an engine tag or label in the line. When present, `conversation_intent` says what this beat is trying to do. Fulfill it naturally, without describing the intent. Ordinary American English, sentence case, one or two short sentences, eighteen words maximum. Concrete everyday speech; no poetry, archaic diction, modern slang, narration, stage direction, or explanation. Thoughts are first-person private thoughts, never third-person narration or stat readouts. Generated names are private context: do not use them in the returned text. Use 'the god' if the god is genuinely relevant; the game will render the current name. Only the `prayer` register may address or ask the god directly. `chat:*`, `reply`, `tell`, and `muse` are never prayers and must not address the god. A reply reacts directly to the quoted speech if present. Use ordinary work words: foresters speak of woods, trees, timber, and felling; miners of quarry, rock, stone, and veins; farmers of fields, crops, soil, and harvest; builders of houses, walls, timber, and roofs; hunters of woods, trails, and game; fishers of rivers, shores, and nets. Never call a person a cutter or call their work 'my cutting'. Never say morale, wavering, muse, trait, or the raw event labels Delivered, Uprooted, Perished, Flourished, Beckoned, or DoubtSown. `tags` must be a nonempty subset of the supplied tags and include the register. `grounding` lists only supplied tags or fact fields that shaped the sentence. Return UP TO 20 lines for this one moment, in `lines`. They must be genuinely different from each other - different thoughts, not one thought reworded twenty ways: vary what the speaker notices, what they feel about it, and what they intend to do. Stop early rather than pad; twelve good lines are worth more than twenty with eight rewordings among them. Every line must independently obey every rule here. `already_said` is every line already written for this exact situation: write nothing that repeats any of them. Vary what the speaker notices and what they do about it, not merely the adjectives. Do not open a prayer with the god's name every time."
                }]
            },
            {
                "role": "user",
                "content": [{ "type": "input_text", "text": prompt }]
            }
        ]
    });
    let response = client
        .post("https://api.openai.com/v1/responses")
        .bearer_auth(key)
        .json(&body)
        .send()
        .map_err(|error| format!("request failed: {error}"))?
        .error_for_status()
        .map_err(|error| format!("API rejected request: {error}"))?
        .json::<Value>()
        .map_err(|error| format!("unreadable API response: {error}"))?;
    let text =
        response_text(&response).ok_or_else(|| "API response had no output text".to_string())?;
    let parsed: Value = serde_json::from_str(&text)
        .map_err(|error| format!("structured response was not JSON: {error}"))?;
    let batch: Batch = serde_json::from_value(parsed)
        .map_err(|error| format!("structured response did not match Sermo's shape: {error}"))?;
    if batch.lines.is_empty() {
        return Err("the batch came back empty".to_string());
    }
    Ok(batch.lines)
}

fn response_text(response: &Value) -> Option<String> {
    response
        .get("output")?
        .as_array()?
        .iter()
        .find(|item| item.get("type").and_then(Value::as_str) == Some("message"))?
        .get("content")?
        .as_array()?
        .iter()
        .find(|item| item.get("type").and_then(Value::as_str) == Some("output_text"))?
        .get("text")?
        .as_str()
        .map(ToOwned::to_owned)
}

fn validate(moment: &Moment, mut candidate: Candidate) -> std::result::Result<Candidate, String> {
    // THE NAMES BECOME THEIR SLOTS FIRST, before anything judges the line -
    // see `slot_the_names`. A sentence that named the person it was about is
    // not a bad sentence, it is an unfinished one, and finishing it is a
    // substitution the moment already has the answer to.
    candidate.text = slot_the_names(&candidate.text, moment);
    let tidy = crate::sermo::tidy(&candidate.text);
    if !crate::sermo::admissible(&tidy) {
        return Err("candidate failed Sermo's local voice gate".to_string());
    }
    if !tidy.chars().next().is_some_and(char::is_uppercase) {
        return Err("candidate did not begin with sentence case".to_string());
    }
    if !tidy.ends_with(['.', '!', '?']) {
        return Err("candidate did not finish as a sentence".to_string());
    }
    let lower = tidy.to_ascii_lowercase();
    if AWKWARD_WORK_PHRASES
        .iter()
        .any(|phrase| lower.contains(phrase))
    {
        return Err("candidate used an unnatural work phrase".to_string());
    }
    if SYSTEM_WORDS.iter().any(|word| lower.contains(word)) {
        return Err("candidate spoke a simulation word aloud".to_string());
    }
    if raw_event_label(moment).is_some_and(|word| lower.contains(word)) {
        return Err("candidate repeated a raw event label".to_string());
    }
    // Word boundaries, or "king" would take "asking" and "cart" would take
    // "cartwheel" - a denylist that eats innocent words teaches nobody
    // anything except to switch it off.
    if let Some(invented) = NO_SUCH_THING
        .iter()
        .find(|word| says_the_word(&lower, word))
    {
        return Err(format!(
            "candidate invented a {invented}, which this world has none of"
        ));
    }
    if moment.register != "prayer" && addresses_god(&lower, moment) {
        return Err("non-prayer candidate addressed the god".to_string());
    }
    if moment.register == "muse" && is_third_person_thought(&lower, moment) {
        return Err("thought was written as third-person narration".to_string());
    }
    if let Some(leftover) = uses_private_name(&lower, moment) {
        return Err(format!(
            "candidate named {leftover}, which has no slot to stand in for it"
        ));
    }
    candidate.tags.sort_unstable();
    candidate.tags.dedup();
    if candidate.tags.is_empty() {
        return Err("candidate carried no Sermo tags".to_string());
    }
    if candidate.tags.iter().any(|tag| !moment.tags.contains(tag)) {
        return Err("candidate invented a Sermo tag".to_string());
    }
    if !candidate.tags.iter().any(|tag| tag == &moment.register) {
        return Err("candidate omitted the moment's speech register".to_string());
    }
    candidate.text = tidy;
    Ok(candidate)
}

/// Whether the line says this word, as a word rather than as a fragment of
/// one. "Asking" is not a king and "carter" is not a cart.
fn says_the_word(lower: &str, word: &str) -> bool {
    let edge = |c: Option<char>| c.is_none_or(|c| !c.is_alphanumeric() && c != '\'');
    let mut from = 0;
    while let Some(at) = lower[from..].find(word) {
        let at = from + at;
        let before = lower[..at].chars().next_back();
        let after = lower[at + word.len()..].chars().next();
        // A trailing "s" is the same word: mayors, lords, taxes.
        let after_s = if after == Some('s') {
            lower[at + word.len() + 1..].chars().next()
        } else {
            after
        };
        if edge(before) && (edge(after) || edge(after_s)) {
            return true;
        }
        from = at + word.len();
    }
    false
}

fn raw_event_label(moment: &Moment) -> Option<&'static str> {
    moment.tags.iter().find_map(|tag| match tag.as_str() {
        "event:delivered" => Some("delivered"),
        "event:uprooted" => Some("uprooted"),
        "event:perished" => Some("perished"),
        "event:flourished" => Some("flourished"),
        "event:beckoned" => Some("beckoned"),
        "event:doubtsown" => Some("doubtsown"),
        _ => None,
    })
}

fn addresses_god(lower: &str, moment: &Moment) -> bool {
    let direct = moment
        .slots
        .get("god")
        .map(|god| format!("{},", god.to_ascii_lowercase()));
    lower.starts_with("the god,") || direct.is_some_and(|god| lower.starts_with(&god))
}

fn is_third_person_thought(lower: &str, moment: &Moment) -> bool {
    let speaker = moment
        .facts
        .speaker_name
        .as_deref()
        .is_some_and(|name| lower.contains(&name.to_ascii_lowercase()));
    speaker
        || lower.starts_with("he ")
        || lower.starts_with("she ")
        || lower.contains(" wonders ")
        || lower.contains(" muses ")
        || lower.contains(" stands ")
}

/// A NAME LEFT STANDING, if any - one the moment has no slot for.
///
/// See [`slot_the_names`]: a name the moment CAN name is turned into its slot
/// and the line is kept. This is only for the ones that cannot be, which are
/// genuinely unusable: a sentence naming somebody the moment never mentioned
/// could never be true again.
fn uses_private_name(lower: &str, moment: &Moment) -> Option<String> {
    [
        moment.facts.speaker_name.as_deref(),
        moment.facts.listener_name.as_deref(),
        moment.facts.settlement_name.as_deref(),
    ]
    .into_iter()
    .flatten()
    .find(|name| lower.contains(&name.to_ascii_lowercase()))
    .map(str::to_string)
}

/// TURNS THE NAMES INTO THEIR SLOTS, so a line about Sayia becomes a line
/// about whoever the moment is about.
///
/// Brett: "We should have a system where it turns a villager's name into a tag
/// so that I can use that in a future sentence with the other villager's
/// name." Which is exactly what the authored corpus has always done - a line
/// reading `I saw {whom} go` is worth having, and the same line reading `I saw
/// Sayia go` is worth one use and then never again.
///
/// This was FORTY-ONE OF FORTY-TWO REJECTIONS in the first real run: more than
/// half of every call paid for was thrown away for writing down a name the
/// moment had itself supplied. Now the name is put back where it came from.
///
/// Longest names first, so a name that contains another (`Sayia` inside
/// `Sayiath`) cannot leave half a name behind.
fn slot_the_names(text: &str, moment: &Moment) -> String {
    // The slots the moment offered, plus the names it sent as FACTS - the
    // speaker's own and their village's. Facts and slots were separate lists
    // and only the slots were folded, which is how "That is good news for
    // Shutel" reached the vault: the village's name went out as a fact, came
    // back in the line, and had nothing to collapse into.
    let speaker = moment.facts.speaker_name.clone();
    let home = moment.facts.settlement_name.clone();
    let extra: Vec<(String, String)> = [
        speaker.map(|name| ("name".to_string(), name)),
        home.map(|name| ("place".to_string(), name)),
    ]
    .into_iter()
    .flatten()
    .filter(|(slot, _)| !moment.slots.contains_key(slot))
    .collect();
    let mut named: Vec<(&String, &String)> = moment.slots.iter().collect();
    named.extend(extra.iter().map(|(slot, name)| (slot, name)));
    named.sort_by_key(|(_, value)| std::cmp::Reverse(value.len()));
    let mut said = text.to_string();
    for (slot, name) in named {
        if name.is_empty() {
            continue;
        }
        // Case-insensitively, because a line may open on the name.
        let mut out = String::with_capacity(said.len());
        let mut rest = said.as_str();
        while let Some(at) = rest.to_ascii_lowercase().find(&name.to_ascii_lowercase()) {
            out.push_str(&rest[..at]);
            out.push_str(&format!("{{{slot}}}"));
            rest = &rest[at + name.len()..];
        }
        out.push_str(rest);
        said = out;
    }
    said
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|time| time.as_secs())
        .unwrap_or_default()
}

/// Environment variables take precedence so a temporary key can be supplied
/// without touching the project folder. The local file is a deliberate
/// convenience for this private development fork and is ignored by Git.
/// The key: the environment first, then a key file beside the game.
///
/// The fork read a file and I took that out, because a file beside the
/// executable is how a key ends up in a commit or a release. Brett wants the
/// file back - "For now it can just read the file" - so it is back, with the
/// two things that make it safe instead of convenient:
///
/// - EVERY shape of the name is gitignored, globbed rather than listed. The
///   file here is `OENAI_API_KEY.txt`, and a rule that only knew the correct
///   spelling would have protected nothing.
/// - The key is never logged, never put in the JSONL, and never leaves this
///   function except as a bearer token.
///
/// The environment still wins, so a shell that exports one overrides whatever
/// is on disk.
fn load_key() -> Option<String> {
    if let Some(key) = std::env::var(KEY_VAR)
        .ok()
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty())
    {
        return Some(key);
    }
    let here = std::fs::read_dir(".").ok()?;
    for entry in here.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let upper = name.to_uppercase();
        if !upper.ends_with(".TXT") || !(upper.contains("API_KEY") || upper.contains("API-KEY")) {
            continue;
        }
        let Ok(read) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        let key = read.trim().to_string();
        if !key.is_empty() {
            info!("the living voice took its key from {name}");
            return Some(key);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_responses_api_text_item() {
        let response = json!({
            "output": [{
                "type": "message",
                "content": [{ "type": "output_text", "text": r#"{"text":"The roof still leaks.","tags":["muse"],"grounding":["roof"]}"# }]
            }]
        });
        assert_eq!(
            response_text(&response).as_deref(),
            Some(
                "{\"text\":\"The roof still leaks.\",\"tags\":[\"muse\"],\"grounding\":[\"roof\"]}"
            )
        );
    }

    #[test]
    fn packet_keys_are_stable_and_heard_words_matter() {
        let first = Moment::new(
            7,
            &["reply", "hungry"],
            &[],
            Some("The stores are thin."),
            SocialTruth::default(),
        );
        let same = Moment::new(
            7,
            &["hungry", "reply"],
            &[],
            Some("The stores are thin."),
            SocialTruth::default(),
        );
        let other = Moment::new(
            7,
            &["reply", "hungry"],
            &[],
            Some("The roof leaks."),
            SocialTruth::default(),
        );
        assert_eq!(first.key(), same.key());
        assert_ne!(first.key(), other.key());
    }

    #[test]
    fn rejects_unfit_model_prose() {
        let moment = Moment::new(0, &["muse", "housed"], &[], None, SocialTruth::default());
        let candidate = |text: &str, tags: &[&str]| Candidate {
            text: text.to_string(),
            tags: tags.iter().map(|tag| (*tag).to_string()).collect(),
            grounding: Vec::new(),
        };
        assert!(
            validate(
                &moment,
                candidate("The old wireless sings in the moonlight.", &["muse"])
            )
            .is_err()
        );
        assert!(validate(&moment, candidate("The roof still leaks.", &["muse"])).is_ok());
        assert!(validate(&moment, candidate("I was near my cutting.", &["muse"])).is_err());
        assert!(validate(&moment, candidate("The cutter came home late.", &["muse"])).is_err());
        assert!(validate(&moment, candidate("The roof still leaks.", &["invented"])).is_err());

        let chat = Moment::new(
            1,
            &["chat:open", "topic:miner"],
            &[("god", "Skyrsyr")],
            None,
            SocialTruth::default(),
        );
        assert!(
            validate(
                &chat,
                candidate("Skyrsyr, steady my hands at the quarry.", &["chat:open"])
            )
            .is_err()
        );

        let event = Moment::new(
            1,
            &["tell", "event:delivered"],
            &[],
            None,
            SocialTruth::default(),
        );
        assert!(
            validate(
                &event,
                candidate("I saw the child delivered safely.", &["tell"])
            )
            .is_err()
        );

        let thought = Moment::new(
            1,
            &["muse", "housed"],
            &[],
            None,
            SocialTruth {
                speaker_name: Some("Niawev".to_string()),
                ..Default::default()
            },
        );
        assert!(
            validate(
                &thought,
                candidate("Niawev wonders whether the roof will hold.", &["muse"])
            )
            .is_err()
        );
        assert!(
            validate(
                &thought,
                candidate("I hope the roof holds tonight.", &["muse"])
            )
            .is_ok()
        );
    }
}

#[cfg(test)]
mod slots {
    use super::*;

    fn moment_naming(pairs: &[(&str, &str)]) -> Moment {
        Moment {
            speaker: 1,
            register: "chat".to_string(),
            tags: vec!["chat".to_string()],
            slots: pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            world_facts: Vec::new(),
            facts: SocialTruth::default(),
            heard: None,
        }
    }

    /// A named line becomes a reusable one. This is the whole idea: forty-one
    /// of forty-two rejections in the first real run were lines thrown away
    /// for writing down a name the moment had itself supplied.
    #[test]
    fn a_name_becomes_its_slot() {
        let moment = moment_naming(&[("whom", "Sayia"), ("god", "Tugim")]);
        assert_eq!(
            slot_the_names("I saw Sayia at the well, and I thanked Tugim.", &moment),
            "I saw {whom} at the well, and I thanked {god}."
        );
    }

    /// Case does not save a name, because a line may open on one.
    #[test]
    fn a_name_at_the_head_of_a_sentence_is_caught() {
        let moment = moment_naming(&[("whom", "Sayia")]);
        assert_eq!(
            slot_the_names("Sayia says the nets are thin.", &moment),
            "{whom} says the nets are thin."
        );
    }

    /// Longest first, or a name inside another name leaves half of one behind.
    #[test]
    fn a_name_inside_a_name_does_not_leave_a_stump() {
        let moment = moment_naming(&[("whom", "Sayia"), ("name", "Sayiath")]);
        assert_eq!(
            slot_the_names("Sayiath told Sayia.", &moment),
            "{name} told {whom}."
        );
    }

    /// A name the moment has NO slot for is still fatal - there is nothing to
    /// stand in for it, so the line could never be true a second time.
    #[test]
    fn a_name_with_no_slot_is_still_refused() {
        let mut moment = moment_naming(&[]);
        moment.facts.listener_name = Some("Prorae".to_string());
        assert_eq!(
            uses_private_name("i told prorae myself.", &moment).as_deref(),
            Some("Prorae")
        );
    }
}

#[cfg(test)]
mod names_and_places {
    use super::*;

    /// A village's name is as run-specific as a person's, and must fold the
    /// same way.
    ///
    /// Pinned because it did not: the settlement went out as a FACT while only
    /// SLOTS were folded, and "That is good news for Shutel." went into the
    /// vault to be said one day in a village called something else.
    #[test]
    fn a_village_name_folds_into_its_slot() {
        let mut moment = Moment {
            speaker: 1,
            register: "chat".to_string(),
            tags: vec!["chat".to_string()],
            slots: Default::default(),
            world_facts: Vec::new(),
            facts: SocialTruth::default(),
            heard: None,
        };
        moment.facts.settlement_name = Some("Shutel".to_string());
        moment.facts.speaker_name = Some("Prorae".to_string());

        assert_eq!(
            slot_the_names("That is good news for Shutel, said Prorae.", &moment),
            "That is good news for {place}, said {name}."
        );
    }
}
