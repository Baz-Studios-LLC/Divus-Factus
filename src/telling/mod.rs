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

use std::collections::HashMap;

use bevy::prelude::*;

pub mod corpus;
#[allow(deprecated)]
use crate::villager::traits::Bearing;
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
    "internet",
    "okay",
    "guys",
    "dude",
];

/// How a teller came by their story.
///

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
// boldness waits for the deep context builder: a timid teller's telling
// should read different, and will, as a tag.
#[allow(dead_code)]
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
// bearing, place, mind and known all await the deep context builder -
// they were prompt fodder, they become tags and slots.
#[allow(dead_code)]
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

#[derive(Resource)]
/// The village's whole voice: the corpus, and the words waiting to be
/// shown. The API is the old teller's exactly - line, muse, take_musing,
/// take_reply, mused_heads - so the nine callers never learned the model
/// beneath them was replaced by a book.
pub struct Tongue {
    voice: corpus::Corpus,
    dice: crate::rng::Rng,
    /// Thoughts picked and waiting to be shown.
    mused: HashMap<Entity, (String, bool, bool)>,
    /// Replies picked and waiting for the conversation's beat.
    replies: HashMap<Entity, String>,
}

impl Tongue {
    /// A line for this telling, if the corpus has one. `None` falls back
    /// to the written phrasing at every caller, same as always.
    pub fn line(&mut self, of: &Retelling) -> Option<String> {
        let kind = format!("event:{:?}", of.key.kind).to_lowercase();
        let hand = match of.key.hand {
            Hand::Witnessed => "saw",
            Hand::Heard => "heard",
            Hand::Distant => "distant",
        };
        let mut tags = vec!["tell", kind.as_str(), hand, faith_tag(of.key.faith)];
        let trade = of.key.voice.map(trade_tag);
        tags.extend(trade);
        if of.told > 2 {
            tags.push("worn");
        }
        let whom = of.key.whom.as_ref().map(|w| w.name.clone());
        let mut slots: Vec<(&str, &str)> = Vec::new();
        if let Some(whom) = whom.as_deref() {
            slots.push(("whom", whom));
        }
        self.voice
            .pick(0, &tags, &slots, &mut self.dice)
            .map(|said| tidy(&said))
    }

    /// Picks someone's thought (or their reply) on the spot. The old
    /// teller composed ahead of the beat; the corpus needs no head start.
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
        if of.prayer {
            tags.push("prayer");
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
            self.mused.insert(of.who, (said, of.aloud, of.prayer));
        }
    }

    /// The thought waiting for this person, if any.
    pub fn take_musing(&mut self, who: Entity) -> Option<(String, bool, bool)> {
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
    #[allow(dead_code)] // wired to a slow timer when the corpus ships
    pub fn note_wanting(&self) {
        self.voice.write_wanting();
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
        Vocation::Carpenter => "trade:carpenter",
        Vocation::Farmer => "trade:farmer",
        Vocation::Mason => "trade:mason",
        Vocation::Cook => "trade:cook",
        Vocation::Healer => "trade:healer",
        Vocation::Priest => "trade:priest",
        Vocation::Explorer => "trade:explorer",
        Vocation::Guard => "trade:guard",
    }
}

/// Which weights the teller actually took up, by name - so the instrument
/// panel can say it out loud. Worth saying: the choice is made from what
/// is on disk, and a village speaking in a smaller voice than you meant it
/// to is otherwise invisible.
#[derive(Resource)]
pub struct SpeakingWith(pub String);

/// Installs the teller. Silent and free when there are no weights to read.
pub struct TellingPlugin;

impl Plugin for TellingPlugin {
    fn build(&self, app: &mut App) {
        // The corpus reads in a blink and answers on the spot: no worker
        // thread, no weights, no model folder. DIVUS_FACTUS_TELLER=0
        // still silences the village for capture runs.
        if std::env::var("DIVUS_FACTUS_TELLER").is_ok_and(|dial| dial == "0") {
            return;
        }
        let voice = corpus::Corpus::load();
        app.insert_resource(SpeakingWith(format!("the corpus - {} lines", voice.len())));
        app.insert_resource(Tongue {
            voice,
            dice: crate::rng::Rng::new(0x1e11),
            mused: HashMap::new(),
            replies: HashMap::new(),
        });
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
}
