//! SERMO - the village's voice. A tagged corpus of authored lines,
//! and the engine that picks from it.
//!
//! Named by Brett (2026-08-10), Latin for speech and conversation, in
//! the house that already holds Divus Factus, Ordo and Opificium. It
//! stays in this tree on purpose: the engine below is small, but the
//! soul of it - the corpus in the game's own voice, and lines keyed to
//! the game's own facts (memories, faith, trades, regard) - IS the
//! game, and does not transplant.
//!
//! Every word a villager says or thinks is picked from a hand-written,
//! hand-tagged corpus (`assets/voice/`), chosen by the tags of the moment
//! — who is speaking, what they hold by, what the moment is — and scored
//! for specificity and freshness. Picking is instant and free, which is
//! the load-bearing fact: nothing in the simulation ever waits on words,
//! gates on words, or spends anything to have them.
//!
//! An LLM teller lived here once, composing lines at runtime; it was
//! retired for this book. Its economies — compose only for watched heads,
//! show only composed lines — outlived it for a while and kept whole
//! conversations silent, and they are gone too. What remains of that era
//! is the authoring toolchain: [`admissible`] and [`speaks_only_of`] gate
//! the corpus batches, the want-list (`voice-wanted.txt`) records the
//! moments that went without words, and `divus-factus --voice` is the
//! bench the lines are read at.
//!
//! Nothing picked is ever saved: the chronicle stores the structured
//! event, never the prose, and a world stays rebuildable from its seed.
//!
//! `DIVUS_FACTUS_TELLER=0` silences the village, for capture runs.

use std::collections::HashMap;

use bevy::prelude::*;

pub mod bench;
mod corpus;
pub mod vault;
#[cfg(feature = "living-voice")]
pub mod vivarium;

/// The living voice, absent - see the `living-voice` feature in `Cargo.toml`.
///
/// A stub rather than a scattering of `#[cfg]`s at every call site: the Tongue
/// asks the same questions either way and gets `None`, which is exactly what
/// it gets when the voice is present and has not met this moment yet. The
/// optimizer removes the whole path, and a release build has no HTTP client
/// linked into it at all.
#[cfg(not(feature = "living-voice"))]
pub mod vivarium {
    pub struct Vivarium;

    impl Vivarium {
        pub fn awake() -> Option<Vivarium> {
            None
        }

        pub fn ask(
            &mut self,
            _speaker: u64,
            _tags: &[&str],
            _slots: &[(&str, &str)],
            _heard: Option<&str>,
        ) -> Option<String> {
            None
        }

        pub fn take_ready(&mut self) -> Vec<ReadyLine> {
            Vec::new()
        }
    }

    pub struct ReadyLine {
        pub speaker: u64,
        pub register: String,
        pub text: String,
    }

    pub fn probe() {}
}
use crate::villager::work::Vocation;
use crate::witness::{DivineEventKind, Whom};

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
    "wireless",
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
}

/// One villager's telling of one act: everything that changes which line
/// the corpus answers with, and nothing that does not. It was once a cache
/// key for composed lines; the corpus needs no cache, so it is now just
/// the shape of the ask.
#[derive(Clone, Debug)]
pub struct Retelling {
    pub kind: DivineEventKind,
    pub hand: Hand,
    pub voice: Option<Vocation>,
    pub faith: FaithBand,
    /// Who it happened to, in the teller's own terms — fills the `{whom}`
    /// slot, so a line about "Feitreh, your brother" is never put in the
    /// mouth of someone he is nothing to.
    pub whom: Option<Whom>,
    /// How many times they have told this before. A worn story is flatter.
    pub told: u32,
    /// What the act happened to - person, beast or thing - so a line about
    /// somebody flying is never told about a berry bush.
    pub of: crate::witness::SubjectClass,
}

/// One villager's inward moment, as the tags of it: who is thinking, what
/// they hold by, what the body is saying, and — when it is an answer to
/// someone — what they just heard.
#[derive(Clone, Debug)]
pub struct Musing {
    /// Whose thought this is; the answer comes back keyed on it.
    pub who: Entity,
    pub voice: Option<Vocation>,
    pub faith: FaithBand,
    /// What the body is saying: "hungry", "worn out". Empty when it is quiet.
    pub body: Vec<&'static str>,
    /// What was just said to them, when this is a reply rather than an idle
    /// thought. A reply waits for the conversation's beat instead of showing
    /// as a thought.
    pub heard: Option<String>,
    /// What the telling was ABOUT, structurally - "event:smote",
    /// "topic:food" - so the first answer to a smiting and the first
    /// answer to a birth stop drawing from one generic pool. The words
    /// in `heard` are for quoting; this is for understanding.
    pub about: Option<String>,
    /// Whether this is truly VOICED — a scream, a cry for help. An idle
    /// musing is a thought and shows as one: people who talk to the wind
    /// unsettle their neighbors.
    pub aloud: bool,
}

impl Musing {
    /// Whether this is an answer to someone rather than a private moment.
    fn is_reply(&self) -> bool {
        self.heard.is_some()
    }
}

impl Retelling {
    pub fn new(
        kind: DivineEventKind,
        hand: Hand,
        voice: Option<Vocation>,
        trust: f32,
        whom: Option<Whom>,
        told: u32,
        of: crate::witness::SubjectClass,
    ) -> Retelling {
        Retelling {
            kind,
            hand,
            voice,
            faith: FaithBand::of(trust),
            whom,
            told,
            of,
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

#[derive(Resource)]
/// The village's whole voice: the corpus, and the words waiting to be
/// shown.
pub struct Tongue {
    voice: corpus::Corpus,
    dice: crate::rng::Rng,
    /// What the village calls the god, filled into every `{god}` a line
    /// carries.
    ///
    /// Held here rather than passed in at each of the dozen call sites,
    /// because it is one fact about the world and none of those callers
    /// have an opinion about it. "The god" until the player names
    /// themselves - and `tidy` sentence-cases the result, so the fallback
    /// reads right at the head of a sentence as well as inside one.
    god: String,
    /// Thoughts picked and waiting to be shown: the words, and whether
    /// they are voiced aloud.
    mused: HashMap<Entity, (String, bool)>,
    /// Replies picked and waiting for the conversation's beat.
    replies: HashMap<Entity, String>,
    /// THE LIVING VOICE, when there is a key for it and the player has asked
    /// for it. Present but silent otherwise: the worker exists from startup
    /// and is simply not consulted, which keeps turning it on a switch rather
    /// than a restart.
    living: Option<vivarium::Vivarium>,
    /// Whether that voice is the one speaking. Mirrored here from the
    /// settings switch so the pick sites do not each need the resource.
    living_speaks: bool,
}

/// Carries the settings switch into the Tongue, and takes in whatever the
/// living voice finished writing since the last tick.
fn the_living_voice_answers(chosen: Res<LivingVoice>, mut tongue: ResMut<Tongue>) {
    tongue.hear_the_living(chosen.0);
}

/// Whether generated speech is answering instead of the corpus.
///
/// A RESOURCE and a settings switch rather than an environment variable,
/// because it is a thing to try mid-run and hear the difference - Brett: "add
/// a toggle to the Sermo tab in settings to use prewritten voices or ChatGPT
/// voices."
#[derive(Resource, Default)]
pub struct LivingVoice(pub bool);

impl Tongue {
    /// Asks the living voice, if it is awake and the player has chosen it.
    ///
    /// `None` means either that the voice is off - in which case the corpus
    /// answers exactly as it always has - or that this is a situation it has
    /// not been asked about before, which it now will be. A brand-new moment
    /// is QUIET rather than delayed: nothing here waits on a network, and the
    /// next equivalent moment will have the line.
    /// Whether there is a key to speak with at all.
    ///
    /// False on every machine that has not set `OPENAI_API_KEY`, which is
    /// every machine but a developer's. The settings page asks this so the
    /// switch can say WHY it will not move, rather than moving and doing
    /// nothing - Brett: "Does the game gracefully tell the user if they are
    /// missing a key that they cant use the feature and prevent them from
    /// activating it?"
    pub fn has_a_living_voice(&self) -> bool {
        self.living.is_some()
    }

    /// Whether generated speech is what a villager would answer with right
    /// now: the switch is on AND there is a key behind it.
    ///
    /// Read by the dev panel, which is the one place that has to tell the
    /// difference between "off" and "no key" while a game is running.
    pub fn living_speaks(&self) -> bool {
        self.living_speaks && self.living.is_some()
    }

    /// Turns the living voice on or off, and takes in whatever it finished
    /// writing since the last tick.
    ///
    /// THE WORDS CATCH UP. A moment the voice had never met went by in
    /// silence while it was being written, and when it arrives the person who
    /// was quiet says it - held as a musing, aloud unless it was a thought.
    /// Without this the FIRST of every kind of moment is lost for good and
    /// only the second one ever speaks, which on a rare beat means never.
    pub fn hear_the_living(&mut self, chosen: bool) {
        self.living_speaks = chosen;
        let Some(living) = self.living.as_mut() else {
            return;
        };
        let caught_up = living.take_ready();
        if !chosen {
            // Still drained, or a switch flipped off and on again would spill
            // a backlog of lines into whoever happened to be standing there.
            return;
        }
        for line in caught_up {
            let Some(who) = Entity::try_from_bits(line.speaker) else {
                continue;
            };
            let aloud = line.register != "muse";
            self.mused.entry(who).or_insert((line.text, aloud));
        }
    }

    /// A line for this telling, if the corpus has one. `None` falls back
    /// to the written phrasing at every caller, same as always.
    pub fn line(&mut self, of: &Retelling) -> Option<String> {
        let kind = format!("event:{:?}", of.kind).to_lowercase();
        let hand = match of.hand {
            Hand::Witnessed => "saw",
            Hand::Heard => "heard",
            Hand::Distant => "distant",
        };
        let mut tags = vec![
            "tell",
            kind.as_str(),
            hand,
            faith_tag(of.faith),
            of.of.tag(),
        ];
        let trade = of.voice.map(trade_tag);
        tags.extend(trade);
        if of.told > 2 {
            // `retold`, and not `worn`: a story worn thin from repetition and
            // a body worn out from work are unrelated things, and `worn` sat
            // one keystroke from the `worn out` that `speech.rs` emits for a
            // tired villager. Five lines were written against one of them and
            // thirteen against the other, and nothing warned either author.
            tags.push("retold");
        }
        let whom = of.whom.as_ref().map(|w| w.name.clone());
        let mut slots: Vec<(&str, &str)> = vec![("god", self.god.as_str())];
        if let Some(whom) = whom.as_deref() {
            slots.push(("whom", whom));
        }
        // The living voice first when it is the one speaking, and the corpus
        // whenever it has nothing - which is every moment it has not been
        // asked about yet, and every moment at all when it is switched off.
        let generated = self
            .living_speaks
            .then(|| {
                self.living
                    .as_mut()
                    .and_then(|v| v.ask(0, &tags, &slots, None))
            })
            .flatten();
        generated
            .or_else(|| self.voice.pick(0, &tags, &slots, &mut self.dice))
            .map(|said| tidy(&said))
    }

    /// A line for one beat of a conversation: the teller's followup, the
    /// listener's close, whatever role the exchange has reached.
    ///
    /// Unlike [`Tongue::line`] this carries no memory and moves no
    /// knowledge - the story changed hands on the opener, and everything
    /// after it is interpretation. That split is what keeps a four-beat
    /// conversation from propagating a rumor four times.
    #[allow(dead_code)]
    pub fn turn(
        &mut self,
        who: Entity,
        role: &'static str,
        about: &str,
        faith: FaithBand,
        voice: Option<Vocation>,
        whom: Option<&str>,
    ) -> Option<String> {
        self.turn_about(who, role, &[about], false, faith, voice, whom)
    }

    /// As [`Tongue::turn`], but saying whether the subject is something
    /// one of them WITNESSED. The witness voice - "I know what I saw" -
    /// has no business in a conversation about the weather.
    #[allow(clippy::too_many_arguments)]
    pub fn turn_about(
        &mut self,
        who: Entity,
        role: &'static str,
        about: &[&str],
        told: bool,
        faith: FaithBand,
        voice: Option<Vocation>,
        whom: Option<&str>,
    ) -> Option<String> {
        // A list, because one subject can need two words for it: a
        // quarrel is BOTH `quarrel` and the charge it is over, and a line
        // written for the charge alone would be said as pleasantly as the
        // weather.
        let mut tags = vec![role, faith_tag(faith)];
        tags.extend(about.iter().copied());
        if told {
            tags.push("told");
        }
        tags.extend(voice.map(trade_tag));
        let slots: Vec<(&str, &str)> = std::iter::once(("god", self.god.as_str()))
            .chain(whom.map(|whom| ("whom", whom)))
            .collect();
        let generated = self
            .living_speaks
            .then(|| {
                self.living
                    .as_mut()
                    .and_then(|v| v.ask(who.to_bits(), &tags, &slots, None))
            })
            .flatten();
        generated
            .or_else(|| {
                self.voice
                    .pick(who.to_bits(), &tags, &slots, &mut self.dice)
            })
            .map(|said| tidy(&said))
    }

    /// Picks someone's thought (or their reply) on the spot, and holds it
    /// for its showing - a thought until regard finds it, a reply until
    /// the conversation's beat.
    pub fn muse(&mut self, of: Musing) {
        let waiting = if of.is_reply() {
            self.replies.contains_key(&of.who)
        } else {
            self.mused.contains_key(&of.who)
        };
        if waiting {
            return;
        }
        let mut tags = vec![
            if of.is_reply() { "reply" } else { "muse" },
            faith_tag(of.faith),
        ];
        tags.extend(of.voice.map(trade_tag));
        tags.extend(of.body.iter().copied());
        // The structured topic joins the moment's truth: a reply line
        // tagged for the event wins by specificity, and the generic
        // replies remain the floor a thin pool falls back to.
        if let Some(about) = of.about.as_deref() {
            tags.push(about);
        }
        let Some(said) = self
            .voice
            .pick(of.who.to_bits(), &tags, &[], &mut self.dice)
            .map(|said| tidy(&said))
        else {
            return;
        };
        if of.is_reply() {
            self.replies.insert(of.who, said);
        } else {
            self.mused.insert(of.who, (said, of.aloud));
        }
    }

    /// Picks a prayer's words, for the moment knees touch ground.
    ///
    /// Unlike `muse`, this is not gated on being watched and holds nothing
    /// in the mused map: the words go onto the PRAYER itself, where the
    /// codex board reads them even if no eye ever does and the pink bubble
    /// replays them for every fresh look. The watched-head economy belonged
    /// to the retired teller, which paid real compute per line; the corpus
    /// picks for nothing.
    pub fn pray(
        &mut self,
        who: Entity,
        body: &[&str],
        faith: FaithBand,
        whom: Option<&str>,
    ) -> Option<String> {
        let mut tags = vec!["muse", faith_tag(faith)];
        tags.extend(body.iter().copied());
        tags.push("prayer");
        let slots: Vec<(&str, &str)> = std::iter::once(("god", self.god.as_str()))
            .chain(whom.map(|whom| ("whom", whom)))
            .collect();
        // The register wall: whatever the pool's condition, a prayer never
        // borrows smalltalk. Stale beats absurd.
        let generated = self
            .living_speaks
            .then(|| {
                self.living
                    .as_mut()
                    .and_then(|v| v.ask(who.to_bits(), &tags, &slots, None))
            })
            .flatten();
        generated
            .or_else(|| {
                self.voice
                    .pick_within(who.to_bits(), &tags, &slots, &["prayer"], &mut self.dice)
            })
            .map(|said| tidy(&said))
    }

    /// A shout, with nobody in particular to shout at.
    ///
    /// Every other line in the game is aimed at someone or is a thought
    /// kept inside. A cry for help is neither: it is speech with no
    /// listener, thrown at whoever happens to be in earshot, and it is
    /// the one thing a person being savaged is certain to do. `why` names
    /// the trouble — "wolf" for teeth in the leg — so the pool can answer
    /// the actual emergency rather than the speaker's mood.
    ///
    /// Never composed ahead: a scream that arrives a beat late is not a
    /// scream.
    pub fn cry(
        &mut self,
        who: Entity,
        why: &'static str,
        faith: FaithBand,
        voice: Option<Vocation>,
    ) {
        let mut tags = vec!["yell", why, faith_tag(faith)];
        tags.extend(voice.map(trade_tag));
        if let Some(said) = self
            .voice
            .pick(who.to_bits(), &tags, &[], &mut self.dice)
            .map(|said| tidy(&said))
        {
            // Straight into the mouth, over anything they were musing:
            // nobody finishes a thought about their boots while a wolf
            // has hold of them.
            self.mused.insert(who, (said, true));
        }
    }

    /// The thought waiting for this person, if any: the words, and whether
    /// they are voiced aloud.
    pub fn take_musing(&mut self, who: Entity) -> Option<(String, bool)> {
        self.mused.remove(&who)
    }

    /// The reply waiting for this person, if any.
    pub fn take_reply(&mut self, who: Entity) -> Option<String> {
        self.replies.remove(&who)
    }

    /// Everyone whose thought waits unshown.
    pub fn mused_heads(&mut self) -> Vec<Entity> {
        self.mused.keys().copied().collect()
    }

    /// Writes the authoring want-list beside the game.
    pub fn note_wanting(&self) {
        self.voice.write_wanting();
    }

    /// How many lines the village can speak - the workbench readout.
    pub fn lines(&self) -> usize {
        self.voice.len()
    }
}

/// The faith bands and trades as corpus tags.
fn faith_tag(band: FaithBand) -> &'static str {
    match band {
        FaithBand::Sure => "devout",
        FaithBand::Wavering => "wavering",
        FaithBand::Doubting => "doubting",
    }
}

fn trade_tag(voice: Vocation) -> &'static str {
    match voice {
        Vocation::Gatherer => "trade:gatherer",
        Vocation::Fisher => "trade:fisher",
        Vocation::Hunter => "trade:hunter",
        Vocation::Miner => "trade:miner",
        Vocation::Forester => "trade:forester",
        Vocation::Builder => "trade:builder",
        Vocation::Farmer => "trade:farmer",
        Vocation::Cook => "trade:cook",
        Vocation::Healer => "trade:healer",
        Vocation::Priest => "trade:priest",
        Vocation::Explorer => "trade:explorer",
        Vocation::Guard => "trade:guard",
    }
}

/// Flushes the authoring want-list every little while. A minute is
/// nothing to a file this small, and it means a crash loses at most a
/// minute of assignments - the want-list is the whole reason silent
/// failures are worth anything.
fn flush_the_want_list(time: Res<Time>, mut since_last: Local<f32>, tongue: Option<Res<Tongue>>) {
    *since_last += time.delta_secs();
    if *since_last < 60.0 {
        return;
    }
    *since_last = 0.0;
    if let Some(tongue) = tongue {
        tongue.note_wanting();
    }
}

pub struct SermoPlugin;

impl Plugin for SermoPlugin {
    fn build(&self, app: &mut App) {
        // The corpus reads in a blink and answers on the spot: no worker
        // thread, no weights, no model folder. DIVUS_FACTUS_TELLER=0
        // still silences the village for capture runs.
        if std::env::var("DIVUS_FACTUS_TELLER").is_ok_and(|dial| dial == "0") {
            return;
        }
        let voice = corpus::Corpus::load();
        info!("the village speaks from the corpus - {} lines", voice.len());
        // The worker wakes wherever there is a key, and stays quiet until
        // somebody asks for it in the settings. On a machine with no
        // `OPENAI_API_KEY` there is simply no living voice and the switch
        // says as much.
        // `DIVUS_FACTUS_SERMO_PROBE=1` asks the API one question and prints
        // what came back, which is how a key gets checked without playing a
        // game to find out.
        if std::env::var("DIVUS_FACTUS_SERMO_PROBE").is_ok() {
            vivarium::probe();
        }
        let living = vivarium::Vivarium::awake();
        if living.is_some() {
            info!("the living voice has a key and is waiting to be asked for");
        } else {
            info!("no OPENAI_API_KEY: the village speaks from the corpus only");
        }
        app.insert_resource(LivingVoice(false));
        app.insert_resource(Tongue {
            voice,
            dice: crate::rng::Rng::new(0x1e11),
            mused: HashMap::new(),
            replies: HashMap::new(),
            god: "the god".to_string(),
            living,
            living_speaks: false,
        });
        app.add_systems(
            Update,
            (
                flush_the_want_list,
                the_village_learns_the_name,
                the_living_voice_answers,
            ),
        );
    }
}

#[allow(dead_code)] // the corpus batches will be audited with this
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
/// The truth gate. A model asked about "Feitreh, your neighbor" will now and
/// then reach for a name of its own — a cousin Marcus, a village elder nobody
/// has ever heard of — and a single invented person on screen poisons the
/// whole premise that the village is real. So the rule is structural rather
/// than hopeful: every capitalised word in the line must be a name the
/// simulation gave it. Anything else is a miss, exactly as if no model had
/// answered.
///
/// There is deliberately NO exemption for the first word. The whole register —
/// every written rumor, every worked example — runs lowercase, so the model
/// imitating its examples starts lowercase too, and a line that opens with a
/// capital is already drifting. Exempting it as sentence case would be the one
/// door left open ("Marcus saw it too" walks straight through), and the cost
/// of keeping it shut is only that an occasional honestly-capitalised line
/// falls back to a written one. The speaking I, which is legitimately capital
/// anywhere, is the sole exception.
#[allow(dead_code)] // ditto: the truth gate for authored lines
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

/// Tidies an admissible line into the shape the bubbles expect:
/// sentence case, and a full stop at the end.
///
/// This used to do the OPPOSITE — lowercase the opener, pop the final
/// stop — because bubbles once read capitals as captions. Brett's
/// preference is now written down (Notes From ChatGPT, 2026-08-10): he
/// reads lowercase openers as sloppiness, not style. Sentence starts
/// are capitalized (interior ones too, after . ! ?), terminal
/// punctuation is guaranteed, and a line already authored correctly
/// passes through untouched — so the older lowercase corpus is
/// repaired at presentation while new lines are written properly in
/// the JSON.
pub fn tidy(line: &str) -> String {
    let trimmed = line.trim().trim_matches('"').trim();
    let mut out = String::with_capacity(trimmed.len() + 1);
    let mut at_sentence_start = true;
    for ch in trimmed.chars() {
        if ch.is_alphabetic() {
            if at_sentence_start {
                out.extend(ch.to_uppercase());
            } else {
                out.push(ch);
            }
            at_sentence_start = false;
        } else {
            out.push(ch);
            if matches!(ch, '.' | '!' | '?') {
                at_sentence_start = true;
            } else if !ch.is_whitespace() {
                at_sentence_start = false;
            }
        }
    }
    if !out.is_empty() && !matches!(out.chars().last(), Some('.' | '!' | '?' | '…')) {
        out.push('.');
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
        // Plain English, proper grammar - Brett's word, for everything:
        // sentence case at every sentence start, a full stop at the end.
        // Correctly authored lines pass through untouched; the older
        // lowercase corpus is repaired on its way to the screen.
        assert_eq!(
            tidy("Something lifted him clean off."),
            "Something lifted him clean off."
        );
        assert_eq!(tidy("  \"he just rose\"  "), "He just rose.");
        // Interior sentences are sentences too.
        assert_eq!(
            tidy("swept the whole floor today. it's a small thing."),
            "Swept the whole floor today. It's a small thing."
        );
        assert_eq!(
            tidy("I'll not walk that field again"),
            "I'll not walk that field again."
        );
        // A question or a cry keeps its mark.
        assert_eq!(tidy("was it the god?"), "Was it the god?");
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
        let rumored = Witnessed {
            secondhand: 5,
            ..Default::default()
        };
        assert_eq!(Retelling::hand_of(&rumored), Hand::Distant);
    }
}

#[cfg(test)]
mod corpus_wiring_tests {
    use super::*;

    /// Every line that speaks of the god names them, and an unnamed god
    /// still reads as English.
    ///
    /// Brett: "They should refer to me by name, not just 'the god'." The
    /// corpus said "the god" 289 times; it carries a `{god}` now, and the
    /// Tongue fills it. What must NOT have been swept up is the indefinite
    /// and the plural - "a god", "no god", "gods" - where a name does not
    /// fit and never did.
    #[test]
    fn the_corpus_names_the_god_and_leaves_the_others_alone() {
        let voice = corpus::Corpus::load();
        let mut definite = 0;
        for line in voice.lines() {
            assert!(
                !regex_lite_the_god(&line.t),
                "still says 'the god' rather than naming them: {}",
                line.t,
            );
            if line.t.contains("{god}") {
                definite += 1;
            }
        }
        assert!(definite > 200, "only {definite} lines name the god");

        // And the fallback reads: unnamed, a line at the head of a
        // sentence is still sentence-cased by `tidy`.
        assert_eq!(
            tidy(&"{god} provided.".replace("{god}", "the god")),
            "The god provided."
        );
    }

    /// A crude search for the definite singular, without pulling in a
    /// regex crate for one test: "the god" not followed by an 's'.
    fn regex_lite_the_god(line: &str) -> bool {
        let lower = line.to_lowercase();
        let mut from = 0;
        while let Some(at) = lower[from..].find("the god") {
            let at = from + at;
            let after = lower[at + 7..].chars().next();
            if !matches!(after, Some('s')) {
                return true;
            }
            from = at + 7;
        }
        false
    }

    #[test]
    fn a_musing_is_answered_from_the_book() {
        let mut tongue = Tongue {
            voice: corpus::Corpus::load(),
            dice: crate::rng::Rng::new(1),
            mused: HashMap::new(),
            replies: HashMap::new(),
            god: "the god".to_string(),
            living: None,
            living_speaks: false,
        };
        let who = Entity::from_raw_u32(7).unwrap();
        tongue.muse(Musing {
            who,
            voice: None,
            faith: FaithBand::Wavering,
            body: vec!["hungry", "roofless"],
            heard: None,
            aloud: false,
            about: None,
        });
        let (line, ..) = tongue.take_musing(who).expect("the book must answer");
        assert!(!line.is_empty());
        println!("said: {line}");
    }
}

/// Tells the village what to call the god.
///
/// The name is the player's, set on the deity page and restored with a
/// save, so it can arrive or change long after the corpus is loaded. One
/// system keeps the Tongue's copy in step; until there is a name, every
/// line reads "the god" exactly as it always did.
fn the_village_learns_the_name(
    named: Option<Res<crate::villager::DivineName>>,
    mut tongue: ResMut<Tongue>,
) {
    let Some(named) = named else {
        return;
    };
    if !named.is_changed() && !tongue.god.is_empty() {
        if tongue.god == named.0 {
            return;
        }
    }
    if tongue.god != named.0 && !named.0.trim().is_empty() {
        tongue.god = named.0.clone();
    }
}
