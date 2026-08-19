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
#[allow(dead_code)] // the stub exists so the call sites compile; nothing reads it
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

        pub fn ask_afresh_of(
            &mut self,
            _speaker: u64,
            _tags: &[&str],
            _slots: &[(&str, &str)],
            _heard: Option<&str>,
            _facts: SocialTruth,
            _written_before: &[String],
        ) -> Option<String> {
            None
        }

        pub fn take_ready(&mut self) -> Vec<ReadyLine> {
            Vec::new()
        }
    }

    /// The same shape as the real one, so `what_is_true_of` reads identically
    /// whether or not the living voice is compiled in. Nothing reads these.
    #[derive(Default)]
    pub struct SocialTruth {
        pub speaker_name: Option<String>,
        pub speaker_is: Option<String>,
        pub traits: Vec<String>,
        pub settlement_name: Option<String>,
        pub morale: Option<String>,
    }

    pub struct ReadyLine {
        pub speaker: u64,
        pub register: String,
        pub text: String,
        pub tags: Vec<String>,
    }

    pub fn probe() {}

    pub fn batch_size() -> usize {
        0
    }

    pub fn ask_for(_many: usize) {}
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

    /// Whether this person SAW the thing they are about to talk about.
    ///
    /// Reads the memory they are actually telling, not a tally of their life.
    /// This asked `witnessed.total > 0` - their lifetime count of everything
    /// they had ever seen - so anybody who had once watched a tree come down
    /// said "I saw" about every story they ever told, goblins included.
    /// Brett: "I am not sure that the person who said it ever actually saw a
    /// goblin." He had not.
    ///
    /// With no memory at all there is nothing to tell and nothing to claim, so
    /// the mildest reading wins.
    pub fn hand_of(witnessed: &crate::witness::Witnessed) -> Hand {
        let Some(telling) = witnessed.recent.first() else {
            return Hand::Heard;
        };
        if telling.firsthand {
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
    /// Which of the three is answering. Mirrored here from the settings so
    /// the pick sites do not each need the resource.
    voice_from: Voice,
    /// Everything the living voice has ever written down, and where it writes.
    vault: Option<vault::Vault>,
    /// How many lines it has put there this run, for the dev panel - a number
    /// that only moves while the factory is running is the quickest way to
    /// see that it IS running.
    kept: usize,
    /// WHO EVERYBODY IS, refreshed off the ECS a few times a second.
    ///
    /// The speech callers hand over an `Entity` and a bare name, which is all
    /// the authored corpus ever needed - it fills `{whom}` and picks by tag.
    /// A written line does not care who is speaking beyond their trade.
    ///
    /// A GENERATED line cares enormously. `SocialTruth` was designed for it
    /// and then never filled in: every line generated so far was written from
    /// the register, the tags, the slots and the quoted speech and NOTHING
    /// else, which is why so many of them are vague. The model was not being
    /// poetic by preference; it had nothing concrete to be specific about.
    ///
    /// Kept here rather than threaded through a dozen call sites, because the
    /// callers already pass the one thing this needs.
    dossiers: HashMap<Entity, Dossier>,
}

/// What the world knows about somebody, in the words a writer can use.
///
/// Deliberately plain English rather than engine values: the model is told
/// "a forester" and "she", never `Vocation::Forester` or `Sex::Female`. An
/// engine label in a truth packet comes back out in the line.
#[derive(Clone, Default)]
pub struct Dossier {
    pub name: String,
    /// "she", "he", or "they" - so a pronoun is chosen rather than guessed.
    /// Nothing told the model this before, so half the pronouns it has
    /// written were coin flips.
    pub pronoun: &'static str,
    pub trade: Option<&'static str>,
    pub settlement: Option<String>,
    /// How their spirits stand, as of the last refresh.
    ///
    /// Kept as the raw figure rather than the band, so the tag and the packet's
    /// words are both derived HERE and cannot drift apart.
    pub spirits: f32,
    /// Whether the god's eye was on them when this was last refreshed.
    ///
    /// Read to decide whether a line's WEAR is charged: a sentence nobody
    /// heard leaves the pool as fresh as it found it. Up to a third of a
    /// second stale, which for "was anybody watching" is close enough - the
    /// alternative is threading a camera question through four speech
    /// signatures and a dozen call sites.
    pub watched: bool,
}

/// Learns who everybody is, a few times a second.
///
/// THE TRUTH PACKET IS ONLY AS GOOD AS THIS. A generated line can only be
/// specific about facts it was given, and until this existed it was given
/// none: no name, no trade, no pronoun, no village. That is why the early
/// lines read as vague and faintly literary - vagueness is what is left when
/// a writer has nothing concrete to say.
///
/// Refreshed rather than event-driven on purpose. A villager's trade changes
/// with the muster, their village can be founded around them, and a dossier
/// that went stale would put a fisherman's words in a forester's mouth - a
/// quiet, plausible lie, which is the worst kind. Three times a second is far
/// cheaper than the speech it serves and never more than a third of a second
/// out of date.
fn the_tongue_learns_who_is_who(
    time: Res<Time>,
    mut since: Local<f32>,
    mut tongue: ResMut<Tongue>,
    folk: Query<(
        Entity,
        &crate::villager::Person,
        &crate::creature::genome::CreatureGenome,
        Option<&Vocation>,
        Option<&crate::villager::MemberOf>,
        &Transform,
        Option<&crate::villager::Morale>,
    )>,
    towns: Query<&crate::villager::Settlement>,
    eye: Option<Res<crate::attention::Attention>>,
) {
    *since += time.delta_secs();
    if *since < 0.33 {
        return;
    }
    *since = 0.0;
    tongue.dossiers.clear();
    for (who, person, genome, trade, home, at, heart) in &folk {
        let settlement = home
            .and_then(|home| towns.get(home.0).ok())
            .map(|town| town.name.clone());
        tongue.dossiers.insert(
            who,
            Dossier {
                name: person.name.clone(),
                // Plain words, never `Sex::Female`: an engine label in the
                // packet comes back out in the line.
                pronoun: match genome.sex {
                    crate::creature::genome::Sex::Female => "she",
                    crate::creature::genome::Sex::Male => "he",
                },
                trade: trade.map(|trade| trade_in_words(*trade)),
                settlement,
                // Optional, because a soul may be read before their morale
                // exists; the default is the same contented figure a villager
                // is born with, which reads as "steady".
                spirits: heart.map_or(0.8, |heart| heart.spirits),
                watched: crate::attention::regard(eye.as_deref(), at.translation).worth_saying(),
            },
        );
    }
}

/// A trade as a person would say it, not as the enum spells it.
fn trade_in_words(voice: Vocation) -> &'static str {
    use Vocation as V;
    match voice {
        V::Gatherer => "a gatherer",
        V::Fisher => "a fisher",
        V::Hunter => "a hunter",
        V::Miner => "a miner",
        V::Forester => "a forester",
        V::Builder => "a builder",
        V::Farmer => "a farmer",
        V::Cook => "a cook",
        V::Healer => "a healer",
        V::Priest => "the priest",
        V::Scholar => "a scholar",
        V::Explorer => "an explorer",
        V::Guard => "a guard",
    }
}

/// Carries the settings switch into the Tongue, and takes in whatever the
/// living voice finished writing since the last tick.
fn the_living_voice_answers(chosen: Res<Voice>, mut tongue: ResMut<Tongue>) {
    tongue.speak_with(*chosen);
}

/// WHERE THE WORDS COME FROM. Three places, and they are genuinely different
/// things rather than three settings of one thing.
///
/// Brett: "in the settings for sermo I can turn their voice to one of three
/// different settings. Authored, ChatGPT or the database."
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Voice {
    /// The hand-written corpus in `assets/voice`. Instant, free, offline, and
    /// the only one a player will ever hear.
    #[default]
    Authored,
    /// Written as the moment arrives, and WRITTEN DOWN as it goes: every line
    /// the model gives that passes the gates is put in the vault with its
    /// tags. This is the factory.
    Generated,
    /// Everything the factory has made so far, read back off disk. No key, no
    /// network, no cost - which is the whole point of having written it down.
    Vault,
}

impl Voice {
    pub fn label(self) -> &'static str {
        match self {
            Voice::Authored => "authored",
            Voice::Generated => "ChatGPT",
            Voice::Vault => "the vault",
        }
    }

    pub fn note(self) -> &'static str {
        match self {
            Voice::Authored => "the written corpus - instant, and what ships",
            Voice::Generated => "written as each moment arrives, and kept",
            Voice::Vault => "everything ChatGPT has written so far, off disk",
        }
    }
}

impl Tongue {
    /// WAS ANYBODY THERE. A line said out of sight leaves the pool as fresh as
    /// it found it — see [`Dossier::watched`] and [`corpus::Corpus::pick_within`].
    ///
    /// Anybody with no dossier counts as watched: the dossiers are a moment
    /// behind the world, and a soul this system has not met yet must cost its
    /// line rather than get it free.
    fn was_anybody_there(&self, speaker: u64) -> bool {
        Entity::try_from_bits(speaker)
            .and_then(|who| self.dossiers.get(&who))
            .is_none_or(|who| who.watched)
    }

    /// Settles what the last pick cost: charged if a soul was there to hear it,
    /// forgiven if the field was empty.
    ///
    /// EVERY pick must be settled, and that is the whole discipline of deferred
    /// wear. Three of the five places that pick went unsettled when the wear
    /// first moved out of `pick_within`, so musings and cries never wore out at
    /// all — which flattens the picker's freshness term to nothing on the two
    /// paths that carry most of the village's speech.
    fn settle(&mut self, speaker: u64) {
        if self.was_anybody_there(speaker) {
            self.voice.was_heard();
        } else {
            self.voice.was_unheard();
        }
    }

    /// ONE DOOR FOR ALL THREE VOICES, so no caller has to know which is on.
    ///
    /// Generated speech falls back to the authored corpus when it has nothing
    /// yet - a moment the model has never met is quiet until the words come
    /// back, and a written line beats silence in the meantime. The vault falls
    /// back the same way, because it starts empty and fills as ChatGPT talks.
    fn said(
        &mut self,
        speaker: u64,
        tags: &[&str],
        slots: &[(&str, &str)],
        must: &[&str],
    ) -> Option<String> {
        // THE SPEAKER'S HEART, ADDED HERE so every register carries it and no
        // caller has to remember. The packet has always described who somebody
        // IS and never how they ARE, and a line the model wrote knowing the
        // speaker was downcast may only ever be replayed to a downcast speaker
        // - which is what this tag is for. A fact sent unpinned is a lie
        // waiting for its second outing.
        //
        // Additive, and deliberately: the authored corpus wears no mood tags,
        // so nothing it holds stops answering. A line only narrows if it CHOSE
        // to wear one.
        let mut with_mood: Vec<&str> = tags.to_vec();
        let heart = Entity::try_from_bits(speaker)
            .and_then(|who| self.dossiers.get(&who))
            .map(|who| mood_tag(who.spirits));
        if let Some(heart) = heart
            && !with_mood.contains(&heart)
        {
            with_mood.push(heart);
        }
        let tags: &[&str] = &with_mood;

        // THE VILLAGE'S NAME IS A SLOT, not a fact to be quoted.
        //
        // A generated line came back reading "That is good news for Shutel."
        // Shutel is this run's settlement, so that line is false in every
        // other village - and it got in because I started SENDING the
        // settlement without giving it a slot to collapse into. A fact sent is
        // a fact that must be pinned; this is the pin.
        //
        // `{place}` has existed in the corpus all along with nothing supplying
        // it. Supplied here, one place for all three voices: the model may use
        // the name, `slot_the_names` turns it back into `{place}` on the way
        // to the vault, and authored lines may finally use it too.
        let home = self
            .dossiers
            .get(&Entity::try_from_bits(speaker).unwrap_or(Entity::PLACEHOLDER))
            .and_then(|who| who.settlement.clone());
        let mut with_place: Vec<(&str, &str)> = slots.to_vec();
        if let Some(home) = home.as_deref()
            && !with_place.iter().any(|(key, _)| *key == "place")
        {
            with_place.push(("place", home));
        }
        let slots: &[(&str, &str)] = &with_place;

        let written = |me: &mut Self| {
            let said = if must.is_empty() {
                me.voice.pick(speaker, tags, slots, &mut me.dice)
            } else {
                me.voice
                    .pick_within(speaker, tags, slots, must, &mut me.dice)
            };
            me.settle(speaker);
            said
        };
        match self.voice_from {
            Voice::Authored => written(self),
            // CHATGPT ANSWERS OR NOBODY DOES, the same as the vault.
            // Falling back to the written corpus would mean most of what
            // you heard while testing was the corpus wearing ChatGPT's
            // name, and the generated voice could not be judged at all.
            // Brett: "Same in ChatGPT mode, no fallback."
            //
            // So a moment it has not met is SILENT until the words come
            // back - and then the person says them, and every equivalent
            // moment after that is answered at once.
            // FRESH EVERY TIME. Brett: "the lines in chatgpt mode shouldnt be
            // reused since its whole purpose is to generate fresh context."
            //
            // The cache is what made two villagers ten feet apart say "I saw
            // green ones, they built something out there" in the same breath:
            // one moment key, one answer, served twice. Reuse is right for the
            // VAULT, which is a corpus and wants a line to serve many moments.
            // It is wrong for the factory, whose entire job is to produce
            // something that was not there before.
            Voice::Generated => {
                let facts = self.what_is_true_of(speaker);
                // WHAT HAS ALREADY BEEN WRITTEN FOR THIS MOMENT, off disk.
                // Taken before the factory is borrowed, because both live on
                // this struct.
                let written = self.written_before(tags, slots, must);
                self.living
                    .as_mut()
                    .and_then(|v| v.ask_afresh_of(speaker, tags, slots, None, facts, &written))
            }
            // THE VAULT ANSWERS OR NOBODY DOES. No falling back to the
            // written corpus, on purpose: an empty vault that quietly spoke
            // authored lines would sound exactly like the authored setting,
            // and there would be no way to tell whether the vault was being
            // used at all. Silence is the honest reading of "nothing written
            // for this yet", and it is also the coverage report - the moments
            // that stay quiet are precisely the ones still to generate.
            Voice::Vault => self.from_the_vault(speaker, tags, slots, must),
        }
    }

    /// Every line the vault ALREADY holds for a moment of this shape.
    ///
    /// The factory kept its own note of what it had written and lost it every
    /// time the game closed - so each session ChatGPT set about rewriting lines
    /// it had written weeks ago, was told none of it, and produced them again.
    /// A quarter of the first six hundred lines had a near-twin, and this is
    /// half of why: not that the model repeats itself within a call, but that
    /// nothing carried across calls.
    ///
    /// The vault is the durable memory - every accepted line goes into it the
    /// moment it arrives - so it is the honest answer to "what has already been
    /// said here", and it survives a restart. Asked with the same eligibility
    /// the game itself uses, so what comes back is exactly the set that would
    /// answer this moment.
    ///
    /// This is the cheaper half of the anti-duplicate work, and by a distance:
    /// the gate at the vault door THROWS AWAY a line that has already been paid
    /// for, while this stops it being written.
    fn written_before(
        &self,
        tags: &[&str],
        slots: &[(&str, &str)],
        must: &[&str],
    ) -> Vec<String> {
        self.vault
            .as_ref()
            .and_then(|vault| vault.eligible(tags, must, slots).ok())
            .map(|found| found.into_iter().map(|line| line.t).collect())
            .unwrap_or_default()
    }

    /// EVERYTHING THE WORLD KNOWS about the person about to speak.
    ///
    /// This is the fix for lines that read as vague and faintly literary. The
    /// packet carried a register, some tags and the quoted speech and nothing
    /// else, so the model had nothing concrete to be specific ABOUT - and what
    /// it does with a vacuum is fill it from its own idea of a medieval
    /// village, which is where the mayor came from. Vagueness and invention
    /// are the same symptom.
    ///
    /// Only PLAIN WORDS go in. "A forester", never `Vocation::Forester`; "she"
    /// rather than `Sex::Female`. An engine label put into a packet comes back
    /// out in a line.
    ///
    /// And only things that are TRUE of this person right now, because
    /// everything in here is something the villager is permitted to say - and
    /// anything they say has to be pinned by a tag before the line can safely
    /// be replayed. That is the real budget on how much goes in here, and it
    /// is a design budget rather than a token one.
    fn what_is_true_of(&self, speaker: u64) -> vivarium::SocialTruth {
        let Some(who) = Entity::try_from_bits(speaker) else {
            return vivarium::SocialTruth::default();
        };
        let Some(dossier) = self.dossiers.get(&who) else {
            return vivarium::SocialTruth::default();
        };
        vivarium::SocialTruth {
            speaker_name: Some(dossier.name.clone()),
            speaker_is: Some(dossier.pronoun.to_string()),
            traits: dossier
                .trade
                .map(|t| vec![t.to_string()])
                .unwrap_or_default(),
            settlement_name: dossier.settlement.clone(),
            morale: Some(mood_words(dossier.spirits).to_string()),
            ..Default::default()
        }
    }

    /// The best line the vault holds for this moment, scored the way the
    /// authored corpus scores its own.
    ///
    /// The SELECTION is SQL and the RANKING is here - see [`vault`]. Specific
    /// lines beat general ones, and the weighted dice break the tie.
    fn from_the_vault(
        &mut self,
        speaker: u64,
        tags: &[&str],
        slots: &[(&str, &str)],
        must: &[&str],
    ) -> Option<String> {
        let found = self.vault.as_ref()?.eligible(tags, must, slots).ok()?;
        // SCORED THE WAY THE CORPUS SCORES, which is the point of the whole
        // exercise: a pool of twenty lines for one moment is only worth having
        // if the pool is actually spread across. Ranking on specificity alone
        // would pick whichever wears the most tags, every time, and twenty
        // lines would sound like one.
        //
        // Specificity first, then how often the WORLD has heard it, then a
        // penalty no fresh rival loses to if this speaker just said it - and
        // the dice to break what is left.
        let mut best: Option<(f32, &vault::Candidate)> = None;
        for line in &found {
            let (heard, echoed) = self.voice.wear_of(speaker, line.id);
            let echo = if echoed { 100.0 } else { 0.0 };
            let score = line.tag_count as f32 * 10.0 - heard as f32 * 4.0 - echo
                + line.w * self.dice.range(0.0, 3.0);
            if best.as_ref().is_none_or(|(top, _)| score > *top) {
                best = Some((score, line));
            }
        }
        let (_, chosen) = best?;
        let (id, said) = (chosen.id, chosen.t.clone());
        // The vault's lines wear the same way, and for the same reason.
        let watched = Entity::try_from_bits(speaker)
            .and_then(|who| self.dossiers.get(&who))
            .is_none_or(|who| who.watched);
        if watched {
            self.voice.now_said(speaker, id);
        }
        Some(corpus::dress(&said, slots, &mut self.dice))
    }

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

    /// Which voice is actually answering, allowing for a chosen one that
    /// cannot: ChatGPT with no key behind it speaks as the corpus does.
    pub fn speaking_with(&self) -> Voice {
        match self.voice_from {
            Voice::Generated if self.living.is_none() => Voice::Authored,
            Voice::Vault if self.vault.is_none() => Voice::Authored,
            chosen => chosen,
        }
    }

    /// Chooses which voice answers, and takes in - and WRITES DOWN - whatever
    /// the living one finished since the last tick.
    ///
    /// THE WORDS CATCH UP. A moment the voice had never met went by in
    /// silence while it was being written, and when it arrives the person who
    /// was quiet says it - held as a musing, aloud unless it was a thought.
    /// Without this the FIRST of every kind of moment is lost for good and
    /// only the second one ever speaks, which on a rare beat means never.
    pub fn speak_with(&mut self, chosen: Voice) {
        self.voice_from = chosen;
        let Some(living) = self.living.as_mut() else {
            return;
        };
        let caught_up = living.take_ready();
        if chosen != Voice::Generated {
            // Still drained, or a switch flipped off and on again would spill
            // a backlog of lines into whoever happened to be standing there.
            return;
        }
        for line in caught_up {
            // WRITTEN DOWN FIRST, because a line that is only spoken is a line
            // paid for and thrown away. Brett: "it talks for them and
            // automatically writes the lines to the data base with tags and
            // everything."
            if let Some(vault) = self.vault.as_ref() {
                let kept = vault.remember(&corpus::Line {
                    t: line.text.clone(),
                    tags: line.tags.clone(),
                    w: 1.0,
                    once: false,
                });
                match kept {
                    Ok(true) => self.kept += 1,
                    Ok(false) => {}
                    Err(error) => warn!("the vault would not take a line: {error}"),
                }
            }
            // NOBODY IN PARTICULAR SAYS NOTHING. `line()` - the retelling
            // path - passes 0 as a placeholder meaning "no speaker", and 0 is
            // a perfectly valid entity: the catch-up put those lines in the
            // mouth of whoever entity 0 happened to be, typically one of the
            // first founders.
            //
            // Brett watched a man announce he had seen goblins in a world
            // that had none: "the game had just started and there were no
            // goblins and that dude said he saw one." He had not seen one.
            // The line was written for a retelling with no speaker at all,
            // and then handed to him.
            //
            // A line with no speaker is still worth having - it goes in the
            // vault, where a real moment can find it later - but it must
            // never be SAID by somebody the game did not choose.
            if line.speaker == 0 {
                continue;
            }
            let Some(who) = Entity::try_from_bits(line.speaker) else {
                continue;
            };
            let aloud = line.register != "muse";
            self.mused.entry(who).or_insert((line.text, aloud));
        }
    }

    /// How many lines the vault holds, and how many arrived this run.
    pub fn vault_standing(&self) -> Option<(usize, usize)> {
        Some((self.vault.as_ref()?.len(), self.kept))
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
        let god = self.god.clone();
        let mut slots: Vec<(&str, &str)> = vec![("god", god.as_str())];
        if let Some(whom) = whom.as_deref() {
            slots.push(("whom", whom));
        }
        self.said(0, &tags, &slots, &[]).map(|said| tidy(&said))
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
        let god = self.god.clone();
        let slots: Vec<(&str, &str)> = std::iter::once(("god", god.as_str()))
            .chain(whom.map(|whom| ("whom", whom)))
            .collect();
        self.said(who.to_bits(), &tags, &slots, &[])
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
        let said = self
            .voice
            .pick(of.who.to_bits(), &tags, &[], &mut self.dice)
            .map(|said| tidy(&said));
        self.settle(of.who.to_bits());
        let Some(said) = said else {
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
        let god = self.god.clone();
        let slots: Vec<(&str, &str)> = std::iter::once(("god", god.as_str()))
            .chain(whom.map(|whom| ("whom", whom)))
            .collect();
        // The register wall: whatever the pool's condition, a prayer never
        // borrows smalltalk. Stale beats absurd.
        self.said(who.to_bits(), &tags, &slots, &["prayer"])
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
        let said = self
            .voice
            .pick(who.to_bits(), &tags, &[], &mut self.dice)
            .map(|said| tidy(&said));
        self.settle(who.to_bits());
        if let Some(said) = said {
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
/// How a soul's spirits stand, in three bands.
///
/// THE PACKET'S BIGGEST HOLE UNTIL NOW. Every field it carried was about who
/// somebody IS - their name, their trade, their village - and none about how
/// they are, so every line came out level in tone whatever had just happened to
/// them. A corpus that reports facts and never feels them is a corpus of
/// captions.
///
/// Three bands rather than a number, because a tag is what makes a fact safe to
/// replay - see [[divus-factus-nobody-makes-things-up]] - and a tag has to be
/// something a moment can be matched on. A line written for a downcast fisher
/// answers only downcast fishers, forever.
///
/// The bands are wide on purpose. Spirits sit at 0.8 by default and a bad day
/// moves them a little; splitting that range finely would make tags that almost
/// never match and lines that can never be reused.
pub(crate) fn mood_tag(spirits: f32) -> &'static str {
    if spirits < 0.45 {
        "downcast"
    } else if spirits < 0.85 {
        "steady"
    } else {
        "heartened"
    }
}

/// The same three bands in the words a person would use, for the packet.
///
/// Plain speech, never the tag itself: the tag is a key the game matches on and
/// the packet is a sentence the model reads. An engine label in a packet comes
/// back out in a line.
fn mood_words(spirits: f32) -> &'static str {
    match mood_tag(spirits) {
        "downcast" => "low, and finding the day heavy",
        "heartened" => "in good spirits",
        _ => "steady enough",
    }
}

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
        Vocation::Scholar => "trade:scholar",
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
            // WHICH REASON, because there are two and they are fixed
            // differently: a missing key is an environment problem and a
            // missing feature is a build problem. Reporting both as "no key"
            // sent Brett looking for a key he had already set.
            if cfg!(feature = "living-voice") {
                info!("no OPENAI_API_KEY in this process: the village speaks from the corpus only");
            } else {
                info!(
                    "built without the `living-voice` feature: ChatGPT cannot be chosen. \
                     Run `cargo run --release --features living-voice` to author dialogue."
                );
            }
        }
        // Where the living voice keeps what it writes. Beside the logs
        // rather than in `assets`, because it is not authored material and
        // must never be mistaken for it.
        let vault = vault::Vault::opened_for_writing(std::path::Path::new("logs/sermo.sqlite"))
            .inspect_err(|error| warn!("no vault: {error}"))
            .ok();
        if let Some(vault) = vault.as_ref() {
            info!("the vault holds {} lines", vault.len());
        }
        app.insert_resource(Voice::default());
        app.insert_resource(Tongue {
            voice,
            dice: crate::rng::Rng::new(0x1e11),
            mused: HashMap::new(),
            replies: HashMap::new(),
            god: "the god".to_string(),
            living,
            voice_from: Voice::default(),
            vault,
            kept: 0,
            dossiers: HashMap::new(),
        });
        app.add_systems(
            Update,
            (
                flush_the_want_list,
                the_village_learns_the_name,
                the_living_voice_answers,
                the_tongue_learns_who_is_who,
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
    // ONE APOSTROPHE, ONE QUOTE. A model writes curly punctuation about half
    // the time, and the vault keys lines by hashing their words - so "{whom}'s
    // child born safely" and "{whom}\u{2019}s child born safely" are two lines
    // to the database and one line to a reader. The corpus was carrying pairs
    // of them. Straight, because that is what the authored corpus is written
    // in and what the font sets identically anyway.
    let straightened: String = line
        .chars()
        .map(|ch| match ch {
            '\u{2018}' | '\u{2019}' | '\u{02bc}' => '\'',
            '\u{201c}' | '\u{201d}' => '"',
            '\u{2013}' | '\u{2014}' => '-',
            other => other,
        })
        .collect();
    let trimmed = straightened.trim().trim_matches('"').trim();
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
    /// A mood is a fact, so it comes with a tag to pin it.
    ///
    /// The rule this serves is the whole truth guarantee: anything the packet
    /// tells the model may end up IN the line, and a line is replayed for years
    /// afterward to whoever the tags match. Telling the model "she is downcast"
    /// without a tag saying so produces a sad line that is later put in a happy
    /// mouth. Bands, and the words the packet uses, are derived from the same
    /// figure so they cannot disagree.
    #[test]
    fn every_mood_the_packet_speaks_has_a_tag_to_pin_it() {
        use super::{mood_tag, mood_words};
        // Every band is reachable, or the tag vocabulary is lying about what
        // the game can say.
        let bands: Vec<&str> = [0.0, 0.3, 0.5, 0.8, 0.9, 1.0]
            .into_iter()
            .map(mood_tag)
            .collect();
        for band in ["downcast", "steady", "heartened"] {
            assert!(bands.contains(&band), "no spirits produce {band}");
        }
        // The words and the tag read the same figure, so a packet can never
        // describe one mood while the tag pins another.
        for spirits in [0.0, 0.44, 0.45, 0.84, 0.85, 1.0] {
            let words = mood_words(spirits);
            let expected = match mood_tag(spirits) {
                "downcast" => "low, and finding the day heavy",
                "heartened" => "in good spirits",
                _ => "steady enough",
            };
            assert_eq!(words, expected, "at {spirits} the words and the tag part");
        }
        // And the default a villager is born with reads as ordinary, not as
        // either extreme - most lines should not be written for a mood.
        assert_eq!(mood_tag(super::super::villager::Morale::default().spirits), "steady");
    }


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
    fn how_you_know_is_read_from_the_memory_being_told() {
        use crate::witness::{Memory, Witnessed};

        let remembering = |firsthand: bool, secondhand: u32| Witnessed {
            total: 3,
            secondhand,
            recent: vec![Memory {
                kind: crate::witness::DivineEventKind::Mauled,
                whom: None,
                divine: false,
                day: 1,
                of: crate::witness::SubjectClass::Person,
                firsthand,
            }],
            ..Default::default()
        };

        // SEEN, because THIS memory was seen - not because the teller has
        // witnessed something at some point in their life. That was the old
        // rule, and under it a villager who once watched a tree come down
        // claimed to have seen the goblins.
        assert_eq!(Retelling::hand_of(&remembering(true, 0)), Hand::Witnessed);

        // The same person, with the same lifetime of witnessing behind them,
        // telling something they were only told.
        assert_eq!(Retelling::hand_of(&remembering(false, 1)), Hand::Heard);

        // And a story that has been round the village enough times.
        assert_eq!(Retelling::hand_of(&remembering(false, 5)), Hand::Distant);

        // Nothing to tell claims nothing.
        assert_eq!(Retelling::hand_of(&Witnessed::default()), Hand::Heard);
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
            voice_from: Voice::Authored,
            vault: None,
            kept: 0,
            dossiers: HashMap::new(),
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
