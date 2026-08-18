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
const DEFAULT_MODEL: &str = "gpt-5-mini";
const MAX_PENDING: usize = 12;
const AWKWARD_WORK_PHRASES: &[&str] = &["cutter", "my cutting", "my cuttings"];
const SYSTEM_WORDS: &[&str] = &["morale", "wavering", "muse", "trait"];

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
}

/// Descriptive simulation truth for the writer. Unlike `tags`, these facts
/// explain a person or a scene but are not part of Sermo's runtime vocabulary.
/// They must all come from ECS state, including every proper name.
#[derive(Clone, Debug, Default, Serialize)]
pub struct SocialTruth {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker_name: Option<String>,
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
    answer: std::result::Result<Candidate, String>,
}

/// A completed line waiting to rejoin the main-thread presentation flow.
/// Network work never touches ECS directly.
pub struct ReadyLine {
    pub speaker: u64,
    pub register: String,
    pub text: String,
}

/// One development-only asynchronous model connection.
pub struct Vivarium {
    jobs: Sender<Job>,
    results: Mutex<Receiver<Result>>,
    cache: HashMap<String, Candidate>,
    ready: Vec<ReadyLine>,
    pending: HashSet<String>,
    exhausted: HashSet<String>,
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
            log,
        })
    }

    /// Returns a validated cached candidate, or queues an equivalent request
    /// once. The caller can always fall back to the corpus immediately.
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
                Ok(candidate) => {
                    self.cache.insert(result.key.clone(), candidate.clone());
                    self.ready.push(ReadyLine {
                        speaker: result.moment.speaker,
                        register: result.moment.register.clone(),
                        text: candidate.text.clone(),
                    });
                    self.record(
                        "candidate",
                        &result.key,
                        &result.moment,
                        Some(&candidate),
                        None,
                    );
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
        let answer = request_candidate(&client, &key, &model, &job.moment)
            .and_then(|candidate| validate(&job.moment, candidate));
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
) -> std::result::Result<Candidate, String> {
    let prompt = format!(
        "Write one Sermo utterance for this truth packet:\n{}",
        serde_json::to_string_pretty(moment).map_err(|error| error.to_string())?
    );
    let body = json!({
        "model": model,
        "store": false,
        "text": {
            "verbosity": "low",
            "format": {
                "type": "json_schema",
                "name": "sermo_utterance",
                "strict": true,
                "schema": {
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
        "input": [
            {
                "role": "developer",
                "content": [{
                    "type": "input_text",
                    "text": "You write candidate dialogue for a medieval village simulation. Return only the required JSON. The truth packet is complete: never introduce a person, object, event, motive, relationship, place, weather, time of day, or condition it does not contain. `world_facts` and `topic_facts` are the plain meanings of engine state; never repeat an engine tag or label in the line. When present, `conversation_intent` says what this beat is trying to do. Fulfill it naturally, without describing the intent. Ordinary American English, sentence case, one or two short sentences, eighteen words maximum. Concrete everyday speech; no poetry, archaic diction, modern slang, narration, stage direction, or explanation. Thoughts are first-person private thoughts, never third-person narration or stat readouts. Generated names are private context: do not use them in the returned text. Use 'the god' if the god is genuinely relevant; the game will render the current name. Only the `prayer` register may address or ask the god directly. `chat:*`, `reply`, `tell`, and `muse` are never prayers and must not address the god. A reply reacts directly to the quoted speech if present. Use ordinary work words: foresters speak of woods, trees, timber, and felling; miners of quarry, rock, stone, and veins; farmers of fields, crops, soil, and harvest; builders of houses, walls, timber, and roofs; hunters of woods, trails, and game; fishers of rivers, shores, and nets. Never call a person a cutter or call their work 'my cutting'. Never say morale, wavering, muse, trait, or the raw event labels Delivered, Uprooted, Perished, Flourished, Beckoned, or DoubtSown. `tags` must be a nonempty subset of the supplied tags and include the register. `grounding` lists only supplied tags or fact fields that shaped the sentence."
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
    serde_json::from_value(parsed)
        .map_err(|error| format!("structured response did not match Sermo's shape: {error}"))
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
    if moment.register != "prayer" && addresses_god(&lower, moment) {
        return Err("non-prayer candidate addressed the god".to_string());
    }
    if moment.register == "muse" && is_third_person_thought(&lower, moment) {
        return Err("thought was written as third-person narration".to_string());
    }
    if uses_private_name(&lower, moment) {
        return Err("candidate exposed a run-specific name".to_string());
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

fn uses_private_name(lower: &str, moment: &Moment) -> bool {
    [
        moment.facts.speaker_name.as_deref(),
        moment.facts.listener_name.as_deref(),
        moment.slots.get("whom").map(String::as_str),
    ]
    .into_iter()
    .flatten()
    .any(|name| lower.contains(&name.to_ascii_lowercase()))
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
/// The key, from the environment and from nowhere else.
///
/// The fork this came from also read a file beside the executable, and that
/// file is exactly the kind of thing that ends up in a commit or a release
/// build. Here there is one source, it lives on the machine that runs the
/// game, and a build with no key simply speaks from the corpus.
fn load_key() -> Option<String> {
    std::env::var(KEY_VAR)
        .ok()
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty())
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
