//! Being seen, and being someone.
//!
//! This is the first piece of the belief system, and deliberately the first: until
//! the world reacts, the Divine Hand is a way to drag props around. The moment a
//! villager turns to look at what you just did, picking someone up stops being a
//! manipulation and becomes an *act* — something that happened, in front of people,
//! who now know it happened.
//!
//! Nothing here decides what anyone believes yet. It answers the question that has to
//! come first: **who saw it, from how far, and what did they make of it?** Doctrine
//! is built on those answers.
//!
//! Reactions are deliberately not uniform. A crowd that flinches in unison reads as
//! one object with a switch on it; the point is that the same act lands differently
//! on different people, which is the whole premise of the game in miniature.

use bevy::prelude::*;

use crate::creature::anim::CreatureMotion;
use crate::creature::{Airborne, Held, MoveTarget, Route};
use crate::rng::Rng;
use crate::villager::{Activity, Villager};

pub struct WitnessPlugin;

impl Plugin for WitnessPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<DivineEvent>().add_systems(
            Update,
            (perceive_events, perceive_deaths, express_reactions)
                .chain()
                .in_set(WitnessSet),
        );
        app.add_systems(Update, mortal_events.before(WitnessSet));
        // Capture tooling: only the player's hand ever sets a subject on an
        // event, so an unattended soak can never exercise the "it happened to
        // Feitreh, your brother" path without this.
        if std::env::var("DIVUS_FACTUS_THROW_TEST").is_ok() {
            app.add_systems(Update, throw_test_harness);
        }
    }
}

/// Hurls one villager before the whole village, periodically, as the hand
/// would. Only registered under DIVUS_FACTUS_THROW_TEST.
///
/// The event is written with a subject but nobody actually moves: what is
/// being tested is the witnessing — the memory with a name and a tie in it,
/// the conversations that retell it, and the truth gate over the words.
fn throw_test_harness(
    time: Res<Time>,
    mut waited: Local<f32>,
    mut events: MessageWriter<DivineEvent>,
    villagers: Query<(Entity, &Transform), With<Villager>>,
) {
    *waited += time.delta_secs();
    if *waited < 20.0 {
        return;
    }
    *waited = 0.0;
    // A different victim each round, so different ties get exercised.
    let count = villagers.iter().count();
    if count == 0 {
        return;
    }
    let pick = (time.elapsed_secs() as usize) % count;
    let Some((victim, at)) = villagers.iter().nth(pick) else {
        return;
    };
    info!("throw test: a villager is hurled before the village");
    events.write(DivineEvent {
        kind: DivineEventKind::Thrown,
        position: at.translation,
        subject: Some(victim),
        intensity: 1.0,
    });
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct WitnessSet;

/// A villager's death is an act of the world, witnessed like any other: the
/// event enters the same attribution mill as lightning, and the few who read
/// the god into it grieve at the god. Wildlife dies unremarked.
fn mortal_events(
    mut deaths: MessageReader<crate::creature::CreatureDied>,
    mut events: MessageWriter<DivineEvent>,
    villagers: Query<(), With<Villager>>,
    watch: Res<crate::debug::timings::Timings>,
) {
    let _t = watch.watch("witness: mortal_events");
    for death in deaths.read() {
        if villagers.get(death.entity).is_err() {
            continue;
        }
        events.write(DivineEvent {
            kind: DivineEventKind::Perished,
            position: death.position,
            subject: Some(death.entity),
            intensity: if death.violent { 0.9 } else { 0.6 },
        });
    }
}

/// Something the god did, that people might have seen.
#[derive(Message, Clone, Copy, Debug)]
pub struct DivineEvent {
    pub kind: DivineEventKind,
    pub position: Vec3,
    /// Who it happened *to*, if anyone.
    pub subject: Option<Entity>,
    /// How forceful, 0 to 1. Scales both how far it carries and how alarming it is.
    pub intensity: f32,
}

/// The kinds of act a villager can witness.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum DivineEventKind {
    /// Someone was lifted off the ground.
    Lifted,
    /// Someone was hurled.
    Thrown,
    /// Someone was set down gently.
    SetDown,
    /// Someone hit the ground hard.
    Impact,
    /// Food set down before the hungry by the hand of god.
    Provided,
    /// Lightning, called down.
    Smote,
    /// A tree, torn living from the earth.
    Uprooted,
    /// The broken, made whole.
    Mended,
    /// The ground itself, thrown like a blanket.
    Quaked,
    /// One of the village died — before witnesses, of hunger or violence.
    Perished,
    /// A child came safe into the world.
    Delivered,
    /// The fields came in heavier than they promised.
    Flourished,
    /// A wolf set upon one of the village. The world's own doing, not the
    /// god's — but the village does not sort its fears by author, and this
    /// is the memory a guard's post is eventually built out of.
    Mauled,
    /// Rain called onto a point: crops drink, fires die. Deniable — rain
    /// is weather to almost everyone, which is the miracle's cover.
    Rained,
    /// A pillar of light stood in the world and people walked to it.
    Beckoned,
    /// A stone fell OUT OF THE SKY. The sky does not do that.
    Fell,
    /// A shadow crossed every heart at once, and nobody can say why.
    DoubtSown,
}

impl DivineEventKind {
    /// How far away this can be noticed, in world units.
    pub fn carry(self) -> f32 {
        match self {
            DivineEventKind::Lifted => 26.0,
            DivineEventKind::Thrown => 40.0,
            DivineEventKind::SetDown => 16.0,
            DivineEventKind::Impact => 34.0,
            // Word of providence travels: it is the thing people hope to see.
            DivineEventKind::Provided => 38.0,
            // Everyone sees lightning.
            DivineEventKind::Smote => 70.0,
            DivineEventKind::Uprooted => 45.0,
            DivineEventKind::Mended => 36.0,
            DivineEventKind::Quaked => 65.0,
            DivineEventKind::Perished => 30.0,
            DivineEventKind::Delivered => 24.0,
            DivineEventKind::Flourished => 20.0,
            // A scream in the woods. It carries as far as a scream does,
            // which is not far - and that is the point of it: most of
            // these are known only because the one it happened to walked
            // home and said so.
            DivineEventKind::Mauled => 28.0,
            // Rain is only remarked on when the crops answer it.
            DivineEventKind::Rained => 22.0,
            // A pillar of light is visible from every field.
            DivineEventKind::Beckoned => 60.0,
            DivineEventKind::Fell => 55.0,
            // The dread spreads mouth to mouth, which IS the miracle.
            DivineEventKind::DoubtSown => 30.0,
        }
    }

    /// How frightening it is, 0 to 1.
    pub fn alarm(self) -> f32 {
        match self {
            DivineEventKind::Lifted => 0.55,
            DivineEventKind::Thrown => 0.95,
            DivineEventKind::SetDown => 0.12,
            DivineEventKind::Impact => 0.8,
            DivineEventKind::Provided => 0.05,
            DivineEventKind::Smote => 1.0,
            DivineEventKind::Uprooted => 0.6,
            DivineEventKind::Mended => 0.03,
            DivineEventKind::Quaked => 0.9,
            DivineEventKind::Perished => 0.7,
            DivineEventKind::Delivered => 0.04,
            DivineEventKind::Flourished => 0.03,
            DivineEventKind::Mauled => 0.85,
            DivineEventKind::Rained => 0.04,
            DivineEventKind::Beckoned => 0.1,
            DivineEventKind::Fell => 0.92,
            DivineEventKind::DoubtSown => 0.75,
        }
    }

    /// The odds an ordinary witness reads the god into this, before their
    /// grain bends it. Anything nature could have done, nature mostly gets
    /// credit for — lightning is weather to almost everyone who sees it.
    /// The impossible things are impossible to explain away.
    pub fn unmistakably_divine(self) -> f32 {
        match self {
            DivineEventKind::Lifted => 0.9,
            DivineEventKind::Thrown => 0.85,
            DivineEventKind::SetDown => 0.9,
            DivineEventKind::Impact => 0.5,
            DivineEventKind::Provided => 0.6,
            DivineEventKind::Smote => 0.15,
            DivineEventKind::Uprooted => 0.45,
            DivineEventKind::Mended => 0.85,
            DivineEventKind::Quaked => 0.3,
            // The ordinary turns of a life: death, birth, harvest. People
            // die, children come, fields yield — almost nobody needs a god
            // for any of it, and the few who do are the interesting ones.
            DivineEventKind::Perished => 0.22,
            DivineEventKind::Delivered => 0.3,
            DivineEventKind::Flourished => 0.28,
            // A wolf is a wolf. Only the most devout read a hand into it,
            // and they are the ones who will say the woods were owed
            // something.
            DivineEventKind::Mauled => 0.05,
            // Rain is weather; a stone from a clear sky is not.
            DivineEventKind::Rained => 0.18,
            DivineEventKind::Beckoned => 0.88,
            DivineEventKind::Fell => 0.55,
            DivineEventKind::DoubtSown => 0.1,
        }
    }

    /// How hearing this befell somebody moves the listener's heart toward
    /// them. The gossip mill's reach into the regard graph: good fortune
    /// retold makes its subject a little dearer, the god's violence makes
    /// the village step back from its target — the smited must have earned
    /// it, and nobody stands too near the earner — and plain misfortune
    /// draws sympathy. Signed regard per hearing, small on purpose: a
    /// reputation is made of many mouths.
    pub fn warms_toward_subject(self) -> f32 {
        match self {
            DivineEventKind::Provided
            | DivineEventKind::Delivered
            | DivineEventKind::Flourished
            | DivineEventKind::Mended
            | DivineEventKind::Lifted
            | DivineEventKind::SetDown => 0.04,
            DivineEventKind::Smote
            | DivineEventKind::Quaked
            | DivineEventKind::Thrown
            | DivineEventKind::Impact => -0.06,
            DivineEventKind::Perished | DivineEventKind::Mauled => 0.03,
            DivineEventKind::Uprooted
            | DivineEventKind::Rained
            | DivineEventKind::Beckoned
            | DivineEventKind::Fell
            | DivineEventKind::DoubtSown => 0.0,
        }
    }

    /// Whether the one it happened TO carries the memory themselves.
    ///
    /// For the god's own acts they do not: being thrown across a field is
    /// something you are still in the middle of, not something you stand
    /// and watch, and the reaction systems already have hold of you. A
    /// mauling is the exception the whole social machine turns on — the
    /// child who limps home out of the woods is the only witness there
    /// was, and if they do not carry it, nobody in the village ever
    /// learns it happened.
    pub fn befalls_its_subject(self) -> bool {
        matches!(self, DivineEventKind::Mauled)
    }

    /// How a villager would put it.
    ///
    /// Memories are phrased from the villager's side, not the system's. They did not
    /// observe a `Thrown` event; something picked them up and threw them.
    pub fn describe(self) -> &'static str {
        match self {
            DivineEventKind::Lifted => "saw someone lifted into the air",
            DivineEventKind::Thrown => "saw someone hurled across the ground",
            DivineEventKind::SetDown => "saw someone set gently down",
            DivineEventKind::Impact => "saw someone strike the earth",
            DivineEventKind::Provided => "saw the god provide for the hungry",
            DivineEventKind::Smote => "saw lightning called down",
            DivineEventKind::Uprooted => "saw a tree torn living from the earth",
            DivineEventKind::Mended => "saw the broken made whole",
            DivineEventKind::Quaked => "felt the earth throw them down",
            DivineEventKind::Perished => "saw one of their own die",
            DivineEventKind::Delivered => "saw a child come safe into the world",
            DivineEventKind::Flourished => "saw the fields come in heavy",
            DivineEventKind::Mauled => "saw a wolf set upon one of their own",
            DivineEventKind::Rained => "stood in rain that came when it was called",
            DivineEventKind::Beckoned => "saw a pillar of light stand on the ground",
            DivineEventKind::Fell => "saw a stone fall out of the empty sky",
            DivineEventKind::DoubtSown => "felt a shadow cross every heart at once",
        }
    }

    /// The same act, retold. A rumour is a witness account with the witness
    /// removed — which is exactly how religions start.
    pub fn rumor(self) -> &'static str {
        self.rumors()[0]
    }

    /// Every phrasing a story wears in the telling. A miracle retold by a
    /// dozen mouths must not sound like one mouth a dozen times.
    pub fn rumors(self) -> &'static [&'static str] {
        match self {
            DivineEventKind::Lifted => &[
                "something lifted a man into the empty air",
                "he just rose, feet kicking at nothing",
                "the air itself took hold of him, I saw it",
                "picked up like a doll, he was",
            ],
            DivineEventKind::Thrown => &[
                "something hurled a man across the land",
                "he flew further than any man should",
                "flung like a stone from a sling, screaming the whole way",
                "one moment standing, the next a speck against the sky",
            ],
            DivineEventKind::SetDown => &[
                "something carried a man and set him down unharmed",
                "carried the whole way and placed like an egg in straw",
                "set down soft as a leaf, not a scratch on him",
            ],
            DivineEventKind::Impact => &[
                "a man was dashed against the earth",
                "the ground met him harder than any fall",
                "thrown down like washing on a stone",
            ],
            DivineEventKind::Provided => &[
                "the god set food before the starving",
                "food from nowhere, laid out like a table",
                "the larder was empty and then it was not",
                "we were hungry and something answered",
            ],
            DivineEventKind::Smote => &[
                "the sky itself struck at the god's word",
                "fire came down and the ground still smokes",
                "one bolt, out of a sky with no storm in it",
                "the flash left its shape behind my eyes",
                "struck - like a hammer through the clouds",
            ],
            DivineEventKind::Uprooted => &[
                "the god pulled a tree from the ground like a weed",
                "a whole oak, roots and all, into the air",
                "the tree screamed at the roots, I heard it",
            ],
            DivineEventKind::Mended => &[
                "the god knit a broken body whole",
                "wounds closed like water smoothing over",
                "he was dying, and then he simply was not",
            ],
            DivineEventKind::Quaked => &[
                "the ground itself buckled at the god's anger",
                "the earth rolled like a shaken rug",
                "cracks opened where the god's finger fell",
            ],
            DivineEventKind::Perished => &[
                "we lost one of our own today",
                "they were alive at dawn and gone by dusk",
                "death walked through the village again",
                "we will bury one of ours tomorrow",
            ],
            DivineEventKind::Delivered => &[
                "a child came safe into the world",
                "there is a new voice in the village",
                "mother and child both came through well",
                "a birth, and an easy one for once",
            ],
            DivineEventKind::Flourished => &[
                "the fields gave more than they promised",
                "the harvest filled every basket we had",
                "the rows came in heavier than we hoped",
                "a good harvest, better than last year",
            ],
            DivineEventKind::Mauled => &[
                "a wolf took one of ours out past the trees",
                "there are wolves in the woods and they are not afraid of us",
                "somebody came home torn open today",
                "it went for the throat and it nearly had it",
            ],
            DivineEventKind::Beckoned => &[
                "there was a light standing on the ground like a tree of it",
                "I walked to the light and I don't remember deciding to",
                "the light stood there until everyone had seen it",
            ],
            DivineEventKind::Fell => &[
                "a stone fell out of a clear sky",
                "the sky threw a rock at us. the sky",
                "there's a boulder where there wasn't one, and no hill it rolled from",
            ],
            DivineEventKind::DoubtSown => &[
                "something walked through us all at once, cold",
                "everyone went quiet at the same breath. everyone",
            ],
            DivineEventKind::Rained => &[
                "the rain came the moment the fields wanted it",
                "it rained on our rows and nowhere else, I'm telling you",
            ],
        }
    }
}

/// How a person tends to respond to the inexplicable.
///
/// Rolled once and fixed for life. Two people seeing the same thing should not do
/// the same thing about it.
#[derive(Component, Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Temperament {
    /// 0 bolts at anything, 1 walks toward it.
    pub boldness: f32,
}

impl Temperament {
    pub fn random(rng: &mut Rng) -> Self {
        Temperament {
            boldness: rng.trait_value(0.15, 0.95),
        }
    }

    /// A word for this temperament, for the inspector.
    pub fn describe(&self) -> &'static str {
        match self.boldness {
            b if b < 0.32 => "timid",
            b if b < 0.5 => "wary",
            b if b < 0.7 => "steady",
            b if b < 0.85 => "bold",
            _ => "fearless",
        }
    }
}

/// What a villager is currently doing about something they saw.
#[derive(Component, Debug)]
pub struct Reaction {
    pub kind: ReactionKind,
    pub focus: Vec3,
    /// Seconds left before they go back to their business.
    pub remaining: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReactionKind {
    /// Stop and stare.
    Watch,
    /// Back away, still watching.
    Recoil,
    /// Go and look.
    Approach,
}

impl ReactionKind {
    pub fn describe(self) -> &'static str {
        match self {
            ReactionKind::Watch => "staring",
            ReactionKind::Recoil => "backing away",
            ReactionKind::Approach => "drawing closer",
        }
    }
}

/// Who a memory is about, as the witness themself would say it.
///
/// A name and a tie rather than an [`Entity`], for two reasons that point the
/// same way. Saves: entities are remapped on load, and a memory that pointed
/// at one would dangle; the words survive as words. And truth: the tie is
/// computed at the moment of seeing, from the witness's own threads — the same
/// act is "your brother struck" to one onlooker and "a neighbour struck" to
/// the one beside them, and that difference belongs IN the memory.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Whom {
    /// Their given name — the part a truth-checked retelling is allowed to say.
    pub name: String,
    /// What they are to the witness: "your brother", "your neighbour".
    pub tie: String,
    /// Who they ARE, for the gossip mill: hearing what befell somebody
    /// moves the listener's regard toward that somebody, and regard is
    /// kept by entity. Never written to disk — a memory that survives a
    /// save speaks only in names and ties, and gossip about the long-gone
    /// simply stops reaching hearts, which is what forgetting is.
    #[serde(skip)]
    pub subject: Option<Entity>,
}

impl Whom {
    /// The phrase a prompt or a chronicle wants: "Feitreh, your brother".
    pub fn phrase(&self) -> String {
        format!("{}, {}", self.name, self.tie)
    }
}

/// What kind of thing an act happened TO. The tellings need this
/// because "I saw somebody lifted clean into the air" is testimony
/// when the subject was a person and a lie when it was a berry bush -
/// the corpus lines wear `of:person`/`of:beast`/`of:thing` tags and
/// this is the fact those tags check. Old saves predate the question;
/// their memories were nearly all about people, and load as such.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SubjectClass {
    #[default]
    Person,
    Beast,
    Thing,
}

impl SubjectClass {
    pub fn tag(self) -> &'static str {
        match self {
            SubjectClass::Person => "of:person",
            SubjectClass::Beast => "of:beast",
            SubjectClass::Thing => "of:thing",
        }
    }
}

/// One thing a villager saw the god do: what it was, and to whom.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(from = "MemoryOnDisk")]
pub struct Memory {
    pub kind: DivineEventKind,
    /// Who it happened to, if it happened to anyone the witness could name.
    #[serde(default)]
    pub whom: Option<Whom>,
    /// Whether THIS witness read the god into it. Most people, most of the
    /// time, do not: lightning is weather, a stumble is a stumble. The devout
    /// see hands everywhere, the skeptics almost nowhere, and what spreads
    /// through gossip is the stance as much as the story.
    #[serde(default = "always")]
    pub divine: bool,
    /// The day it was laid down, so a fear can fade. Old saves predate the
    /// stamp and load as day zero, which reads as long ago — which, for
    /// anything loaded off a disk, it is.
    #[serde(default)]
    pub day: u32,
    /// What the act happened to - a person, a beast, or a mere thing.
    #[serde(default)]
    pub of: SubjectClass,
}

/// Old saves predate doubt: their memories load as believed.
fn always() -> bool {
    true
}

/// A memory as older saves wrote it: the bare kind, no subject. Deserializing
/// through this keeps every pre-subject save loadable — the untagged try-order
/// reads the new object shape first and falls back to the bare string.
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum MemoryOnDisk {
    Whole {
        kind: DivineEventKind,
        #[serde(default)]
        whom: Option<Whom>,
        #[serde(default = "always")]
        divine: bool,
        #[serde(default)]
        day: u32,
        #[serde(default)]
        of: SubjectClass,
    },
    Bare(DivineEventKind),
}

impl From<MemoryOnDisk> for Memory {
    fn from(disk: MemoryOnDisk) -> Memory {
        match disk {
            MemoryOnDisk::Whole {
                kind,
                whom,
                divine,
                day,
                of,
            } => Memory {
                kind,
                whom,
                divine,
                day,
                of,
            },
            MemoryOnDisk::Bare(kind) => Memory {
                kind,
                whom: None,
                divine: true,
                day: 0,
                of: SubjectClass::default(),
            },
        }
    }
}

/// What a villager has seen the god do.
///
/// Capped and counted rather than kept whole. The complete record is what `history`
/// will be for; this is only what the person themself carries.
#[derive(Component, Default, Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct Witnessed {
    /// Most recent first.
    pub recent: Vec<Memory>,
    /// Everything they have ever seen, including what has fallen out of `recent`.
    pub total: u32,
    /// Stories heard from others, never seen with their own eyes.
    pub secondhand: u32,
    /// How many times they have told their story. Enthusiasm for the
    /// retelling wears out; a fresh sight winds it back up.
    #[serde(default)]
    pub told: u32,
}

impl Witnessed {
    /// How many memories a villager keeps.
    pub const CAPACITY: usize = 8;

    fn record(
        &mut self,
        kind: DivineEventKind,
        whom: Option<Whom>,
        divine: bool,
        day: u32,
        of: SubjectClass,
    ) {
        self.recent.insert(
            0,
            Memory {
                kind,
                whom,
                divine,
                day,
                of,
            },
        );
        self.recent.truncate(Self::CAPACITY);
        self.total = self.total.saturating_add(1);
        // A fresh sight rekindles the urge to tell it.
        self.told = 0;
    }

    /// Takes in a story HEARD rather than seen: it joins the memories they
    /// can retell — belief travels through people, and a rumor that could
    /// not be passed on died one hop from its witness — but it counts as
    /// secondhand, never as witness. [`Retelling::hand_of`] reads exactly
    /// that split, so a rumor's reteller says "I was told" without any
    /// further bookkeeping.
    pub fn hear(&mut self, memory: Memory) {
        self.recent.insert(0, memory);
        self.recent.truncate(Self::CAPACITY);
        self.secondhand = self.secondhand.saturating_add(1);
    }

    /// Whether they carry a memory of this kind of act, whoever it befell.
    pub fn remembers(&self, kind: DivineEventKind) -> bool {
        self.recent.iter().any(|memory| memory.kind == kind)
    }

    /// Whether this person has ever seen anything at all.
    pub fn is_innocent(&self) -> bool {
        self.total == 0
    }

    /// How heavily the teeth sit on this one person today, 0 to 1. A
    /// mauling they carry counts full on the day it happened and nothing
    /// at all once it has faded; a memory heard secondhand is in `recent`
    /// exactly like one seen, which is the point — a story frightens the
    /// people it reaches, not only the person it happened to.
    pub fn peril(&self, today: u32) -> f32 {
        self.recent
            .iter()
            .filter(|memory| memory.kind == DivineEventKind::Mauled)
            .map(|memory| {
                let age = today.saturating_sub(memory.day) as f32;
                (1.0 - age / PERIL_FADES).clamp(0.0, 1.0)
            })
            .fold(0.0f32, f32::max)
    }
}

/// Days for a mauling to stop frightening the one who carries it. Rather
/// longer than a season's work rota, so a bad autumn keeps a guard on the
/// edge of the woods well into the winter, and a quiet year takes the
/// post away again.
pub const PERIL_FADES: f32 = 14.0;

/// How badly a settlement fears the woods, counted in PEOPLE rather than
/// in wolves: the sum of what everyone carries, so a story that reached
/// eight ears weighs eight times what one silent survivor does.
///
/// This is the whole point of the thing. The want for a guard used to be
/// read off a god's-eye census of live wolves within a hundred and thirty
/// metres of the square — so a village feared wolves nobody had ever laid
/// eyes on, and shrugged at a child who came home torn open. Fear belongs
/// to the people who hold it.
pub fn peril_of<'a>(village: impl Iterator<Item = &'a Witnessed>, today: u32) -> f32 {
    village.map(|held| held.peril(today)).sum()
}

/// Chooses how a villager responds to something they just saw.
///
/// Distance weighs as heavily as temperament: the same act is a curiosity from across
/// a field and a threat at arm's length.
fn choose_reaction(kind: DivineEventKind, boldness: f32, closeness: f32) -> ReactionKind {
    let alarm = kind.alarm() * (0.45 + closeness * 0.55);

    if alarm > boldness + 0.25 {
        ReactionKind::Recoil
    } else if alarm < boldness - 0.3 {
        ReactionKind::Approach
    } else {
        ReactionKind::Watch
    }
}

/// Finds who saw what, and starts them reacting.
fn perceive_events(
    mut events: MessageReader<DivineEvent>,
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    mut villagers: Query<
        (
            Entity,
            &Transform,
            &Temperament,
            &mut Witnessed,
            &mut CreatureMotion,
        ),
        (With<Villager>, Without<Held>),
    >,
    // The threads that name a subject: read-only, and none of them appear
    // mutably in the query above, so the two coexist.
    threads: Query<(
        Option<&crate::villager::Person>,
        Option<&crate::villager::Spouse>,
        Option<&crate::villager::Parentage>,
        Option<&crate::creature::genome::CreatureGenome>,
    )>,
    manners: Query<&crate::villager::traits::Traits>,
    mut rng: Option<ResMut<crate::villager::SimRng>>,
    watch: Res<crate::debug::timings::Timings>,
) {
    let _t = watch.watch("witness: perceive_events");
    // Gathers what one entity's threads are, for the kinship question.
    let threads_of = |entity: Entity| -> crate::villager::kin::Threads<'_> {
        let Ok((person, spouse, parents, genome)) = threads.get(entity) else {
            return crate::villager::kin::Threads::default();
        };
        crate::villager::kin::Threads {
            sex: genome.map(|g| g.sex),
            spouse: spouse.map(|s| s.0),
            parents: parents.map(|p| (p.mother, p.father)),
            house: person.map(|p| p.surname.as_str()),
            born_house: person.map(|p| p.born_surname.as_str()),
        }
    };

    let today = clock.day();
    for event in events.read() {
        let carry = event.kind.carry() * (0.6 + event.intensity * 0.6);

        // The subject's name, if they have one — a wolf thrown across a field
        // is remembered, but not by name. Resolved once per event; only the
        // TIE differs per witness.
        let subject_name = event.subject.and_then(|subject| {
            threads
                .get(subject)
                .ok()
                .and_then(|(person, ..)| person.map(|p| p.name.clone()))
        });

        for (entity, transform, temperament, mut witnessed, mut motion) in &mut villagers {
            // The subject of an act is usually not a witness to it: they are
            // the one it happened to, already reacting by being thrown. The
            // exception is the one the whole social machine turns on — see
            // `befalls_its_subject`.
            let befell_them = event.subject == Some(entity);
            if befell_them && !event.kind.befalls_its_subject() {
                continue;
            }

            let distance = transform.translation.distance(event.position);
            if distance > carry {
                continue;
            }

            let closeness = 1.0 - (distance / carry).clamp(0.0, 1.0);
            let kind = choose_reaction(event.kind, temperament.boldness, closeness);

            // Who it happened to, as THIS witness holds it: the same act is
            // "your brother struck" to one onlooker and "a neighbour struck"
            // to the one beside them. Computed here, at the moment of seeing,
            // because this is the last place the subject is an entity — in
            // the memory they are a name and a tie.
            let whom = match (&subject_name, event.subject) {
                // It happened to them. There is no third party to name,
                // and a memory that named them to themselves would come
                // back out of the mouth as somebody else's story.
                _ if befell_them => None,
                (Some(name), Some(subject)) => Some(Whom {
                    name: name.clone(),
                    tie: crate::villager::kin::tie(
                        entity,
                        threads_of(entity),
                        subject,
                        threads_of(subject),
                    )
                    .word()
                    .to_string(),
                    subject: Some(subject),
                }),
                _ => None,
            };
            // The verdict: did the god do this, or did the world? Rolled per
            // witness, bent by their grain — a skeptic shrugs off all but the
            // impossible, the devout see hands in half the weather. The roll
            // is the memory's for life, and it is what the faith systems and
            // the gossip mill read instead of assuming belief.
            let conviction = manners.get(entity).map_or(1.0, |m| m.conviction());
            let divine = rng.as_mut().is_none_or(|rng| {
                rng.0
                    .chance((event.kind.unmistakably_divine() * conviction).min(0.97))
            });
            // What the act happened TO. A subject with a name is a person,
            // one with a body but no name is a beast, and everything else -
            // a sack, a stone, the ground itself - is a thing. Subjectless
            // acts are acts on the world, and the world is a thing too.
            let of = match event.subject.and_then(|subject| threads.get(subject).ok()) {
                Some((Some(_), ..)) => SubjectClass::Person,
                Some((_, _, _, Some(_))) => SubjectClass::Beast,
                _ => SubjectClass::Thing,
            };
            witnessed.record(event.kind, whom, divine, today, of);

            // The one it happened to is already flailing and running from
            // the thing itself. Turning them to WATCH it would stop them
            // where they stand, with the teeth still in them.
            if befell_them {
                continue;
            }

            // A visible start, so it reads as a reaction rather than a decision.
            motion.flail = motion.flail.max(match kind {
                ReactionKind::Recoil => 0.5 * closeness,
                _ => 0.15 * closeness,
            });

            commands.entity(entity).insert(Reaction {
                kind,
                focus: event.position,
                remaining: 2.0 + closeness * 3.0 + event.intensity * 2.0,
            });
        }
    }
}

/// A death stops the village.
///
/// Everyone near enough turns toward it; the timid back away from it. Deaths are
/// not divine events — they go unrecorded in `Witnessed` — but they are the events
/// the divine ones will be judged against: a god who was *here* when someone
/// starved is a fact the doctrine system will have opinions about.
fn perceive_deaths(
    mut deaths: MessageReader<crate::creature::CreatureDied>,
    mut commands: Commands,
    mut villagers: Query<
        (Entity, &Transform, &Temperament, &mut CreatureMotion),
        (
            With<Villager>,
            Without<Held>,
            Without<crate::creature::Corpse>,
        ),
    >,
    watch: Res<crate::debug::timings::Timings>,
) {
    let _t = watch.watch("witness: perceive_deaths");
    for death in deaths.read() {
        // Violent deaths carry further, the way a scream carries further than a sigh.
        let carry = if death.violent { 44.0 } else { 26.0 };

        for (entity, transform, temperament, mut motion) in &mut villagers {
            let distance = transform.translation.distance(death.position);
            if distance > carry {
                continue;
            }
            let closeness = 1.0 - (distance / carry).clamp(0.0, 1.0);

            let kind = if death.violent && temperament.boldness < 0.45 {
                ReactionKind::Recoil
            } else {
                ReactionKind::Watch
            };

            motion.flail = motion.flail.max(0.35 * closeness);
            commands.entity(entity).insert(Reaction {
                kind,
                focus: death.position,
                remaining: 4.0 + closeness * 4.0,
            });
        }
    }
}

/// Drives villagers who are mid-reaction, and ends reactions that have run out.
///
/// Overrides ordinary business while it lasts: someone staring at a miracle is not
/// thinking about lunch.
fn express_reactions(
    mut commands: Commands,
    time: Res<Time>,
    mut reacting: Query<
        (
            Entity,
            &mut Reaction,
            &Transform,
            &mut MoveTarget,
            &mut Route,
            &mut CreatureMotion,
            &mut Activity,
        ),
        Without<Airborne>,
    >,
    watch: Res<crate::debug::timings::Timings>,
) {
    let _t = watch.watch("witness: express_reactions");
    let dt = time.delta_secs();

    for (entity, mut reaction, transform, mut target, mut route, mut motion, mut activity) in
        &mut reacting
    {
        reaction.remaining -= dt;
        if reaction.remaining <= 0.0 {
            commands.entity(entity).remove::<Reaction>();
            motion.look_at = None;
            // Back to their own day - unless the sight put them on their
            // knees to ask for something, which is the prayer system's
            // business and not this one's. The pose follows the activity
            // on its own, one frame later.
            if matches!(*activity, Activity::Marvelling) {
                *activity = Activity::Idle;
            }
            continue;
        }

        // Turn to face it. This is the part that actually reads on screen — a head
        // tracking you across a field is worth more than any number.
        motion.look_at = Some(reaction.focus);

        let away = transform.translation - reaction.focus;
        let distance = away.length();

        match reaction.kind {
            // Not "stop and stare" any more: down on their knees, facing
            // it. Worship, not a request - nothing is being asked for and
            // no prayer opens. Brett: "in truth they should fall to their
            // knees in worship. It doesnt mean they pray, they just do
            // the animation."
            ReactionKind::Watch => {
                target.0 = None;
                route.waypoints.clear();
                // Still, so the walk cycle does not play under a kneel.
                // The kneel itself belongs to `belief::take_a_knee`,
                // which owns that pose for the whole game and reads it off
                // `Activity::Marvelling` below.
                motion.speed = 0.0;
            }
            ReactionKind::Recoil => {
                if distance < 14.0 && distance > 0.01 {
                    target.0 = Some(reaction.focus + away.normalize() * 18.0);
                }
            }
            ReactionKind::Approach => {
                if distance > 4.0 {
                    target.0 = Some(reaction.focus + away.normalize() * 3.0);
                }
            }
        }

        // The witness OWNS the body while the reaction lasts.
        //
        // This wrote `Idle` every frame instead, which handed the villager
        // straight back to the activity chooser - which gave them
        // somewhere to walk, which this overwrote, at frame rate. Brett
        // watched a crowd witness a miracle and shake to pieces: "they
        // start shaking like crazy. it looks like the door bug." It was
        // the door bug, in a different room: two writers, no owner.
        //
        // A prayer already underway is the one thing not interrupted - a
        // kneeler who sees the god lifts their eyes and keeps kneeling.
        if !matches!(*activity, Activity::Praying) {
            *activity = Activity::Marvelling;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_timid_recoil_and_the_bold_come_closer() {
        // The same act must not produce the same response in everyone, or a crowd
        // reads as one object with a switch on it.
        assert_eq!(
            choose_reaction(DivineEventKind::Thrown, 0.1, 0.9),
            ReactionKind::Recoil,
        );
        assert_eq!(
            choose_reaction(DivineEventKind::SetDown, 0.9, 0.9),
            ReactionKind::Approach,
        );
    }

    #[test]
    fn distance_takes_the_edge_off() {
        // A hurling at arm's length is a threat; the same act across a field is a
        // curiosity.
        let timid = 0.35;
        assert_eq!(
            choose_reaction(DivineEventKind::Thrown, timid, 1.0),
            ReactionKind::Recoil,
        );
        assert_ne!(
            choose_reaction(DivineEventKind::Thrown, timid, 0.0),
            ReactionKind::Recoil,
        );
    }

    #[test]
    fn violence_carries_further_than_gentleness() {
        assert!(DivineEventKind::Thrown.carry() > DivineEventKind::SetDown.carry());
        assert!(DivineEventKind::Thrown.alarm() > DivineEventKind::SetDown.alarm());
    }

    #[test]
    fn a_gentle_act_never_frightens_the_bold() {
        for boldness in [0.6, 0.75, 0.9, 1.0] {
            for closeness in [0.0, 0.5, 1.0] {
                assert_ne!(
                    choose_reaction(DivineEventKind::SetDown, boldness, closeness),
                    ReactionKind::Recoil,
                    "boldness {boldness} recoiled from being set down",
                );
            }
        }
    }

    #[test]
    fn only_a_mauling_is_carried_by_the_one_it_happened_to() {
        // Everything the god does, the subject is in the middle of and
        // the reaction systems already have hold of. The teeth are the
        // exception, and if that ever stops being true the child who
        // limps home out of the woods stops being able to tell anyone.
        assert!(DivineEventKind::Mauled.befalls_its_subject());
        for kind in [
            DivineEventKind::Lifted,
            DivineEventKind::Thrown,
            DivineEventKind::SetDown,
            DivineEventKind::Impact,
            DivineEventKind::Provided,
            DivineEventKind::Smote,
            DivineEventKind::Uprooted,
            DivineEventKind::Mended,
            DivineEventKind::Quaked,
            DivineEventKind::Perished,
            DivineEventKind::Delivered,
            DivineEventKind::Flourished,
        ] {
            assert!(!kind.befalls_its_subject(), "{kind:?}");
        }
    }

    #[test]
    fn a_fear_fades() {
        let mut bitten = Witnessed::default();
        bitten.record(
            DivineEventKind::Mauled,
            None,
            false,
            10,
            SubjectClass::Person,
        );
        assert_eq!(bitten.peril(10), 1.0, "the day it happened");
        assert!(bitten.peril(17) > 0.0, "a week on, still frightened");
        assert!(bitten.peril(17) < 1.0, "but less so");
        assert_eq!(bitten.peril(10 + PERIL_FADES as u32), 0.0, "and then gone");
        assert_eq!(bitten.peril(400), 0.0, "and it stays gone");
    }

    #[test]
    fn nothing_but_the_teeth_frightens_the_village_this_way() {
        let mut seen = Witnessed::default();
        for kind in [
            DivineEventKind::Smote,
            DivineEventKind::Quaked,
            DivineEventKind::Perished,
        ] {
            seen.record(kind, None, true, 5, SubjectClass::Person);
        }
        assert_eq!(
            seen.peril(5),
            0.0,
            "lightning is the god's business, not the guard's"
        );
    }

    #[test]
    fn the_story_frightens_more_people_than_the_wolf_bit() {
        // The whole point. One survivor is one village's worth of unease;
        // the same account in eight heads is eight, and THAT is what puts
        // a spear on the treeline.
        let bitten = {
            let mut held = Witnessed::default();
            held.record(
                DivineEventKind::Mauled,
                None,
                false,
                3,
                SubjectClass::Person,
            );
            held
        };
        let told = {
            let mut held = Witnessed::default();
            held.hear(Memory {
                kind: DivineEventKind::Mauled,
                whom: None,
                divine: false,
                day: 3,
                of: SubjectClass::default(),
            });
            held
        };
        let quiet = Witnessed::default();

        let alone = [&bitten, &quiet, &quiet];
        let spread = [&bitten, &told, &told];
        assert_eq!(peril_of(alone.into_iter(), 3), 1.0);
        assert_eq!(peril_of(spread.into_iter(), 3), 3.0);
        assert_eq!(
            peril_of(spread.into_iter(), 3 + PERIL_FADES as u32),
            0.0,
            "and a quiet season empties the post again"
        );
    }

    #[test]
    fn memory_is_capped_but_the_count_is_not() {
        let mut w = Witnessed::default();
        assert!(w.is_innocent());
        for _ in 0..50 {
            w.record(DivineEventKind::Lifted, None, true, 1, SubjectClass::Person);
        }
        assert_eq!(w.recent.len(), Witnessed::CAPACITY);
        assert_eq!(w.total, 50);
        assert!(!w.is_innocent());
    }

    #[test]
    fn the_newest_memory_comes_first() {
        let mut w = Witnessed::default();
        w.record(DivineEventKind::Lifted, None, true, 1, SubjectClass::Person);
        w.record(
            DivineEventKind::Thrown,
            Some(Whom {
                name: "Feitreh".into(),
                tie: "your neighbour".into(),
                subject: None,
            }),
            true,
            1,
            SubjectClass::Person,
        );
        assert_eq!(w.recent[0].kind, DivineEventKind::Thrown);
        assert_eq!(
            w.recent[0].whom.as_ref().map(|w| w.phrase()).as_deref(),
            Some("Feitreh, your neighbour"),
            "a memory keeps who it happened to",
        );
    }

    #[test]
    fn lightning_is_weather_to_almost_everyone() {
        // The design's centre: the impossible compels, the natural excuses.
        // A skeptic (conviction 0.5) attributes a bolt ~7% of the time; even
        // the devout (1.5) stay under a quarter. Nobody explains away a man
        // hanging in the empty air.
        assert!(DivineEventKind::Smote.unmistakably_divine() <= 0.2);
        assert!(DivineEventKind::Quaked.unmistakably_divine() <= 0.35);
        assert!(DivineEventKind::Lifted.unmistakably_divine() >= 0.85);
        assert!(DivineEventKind::Mended.unmistakably_divine() >= 0.8);
        // The ordinary turns of a life stay ordinary to most: death, birth
        // and harvest all read as the world's doing three times in four.
        assert!(DivineEventKind::Perished.unmistakably_divine() <= 0.25);
        assert!(DivineEventKind::Delivered.unmistakably_divine() <= 0.35);
        assert!(DivineEventKind::Flourished.unmistakably_divine() <= 0.35);
    }

    #[test]
    fn memories_from_before_subjects_still_load() {
        // A save written when `recent` was a bare list of kinds must come
        // back as memories with no subject — the rename price was paid once,
        // and save compatibility is not broken twice for one feature.
        let old = r#"{"recent":["Smote","Lifted"],"total":2,"secondhand":1}"#;
        let loaded: Witnessed = serde_json::from_str(old).expect("an old memory must load");
        assert_eq!(loaded.recent.len(), 2);
        assert_eq!(loaded.recent[0].kind, DivineEventKind::Smote);
        assert_eq!(loaded.recent[0].whom, None);

        // And what is written now reads back whole.
        let mut fresh = Witnessed::default();
        fresh.record(
            DivineEventKind::Thrown,
            Some(Whom {
                name: "Feitreh".into(),
                tie: "your brother".into(),
                subject: None,
            }),
            false,
            3,
            SubjectClass::Person,
        );
        let round: Witnessed =
            serde_json::from_str(&serde_json::to_string(&fresh).unwrap()).unwrap();
        assert_eq!(round.recent, fresh.recent);
    }

    #[test]
    fn a_settlement_holds_a_range_of_temperaments() {
        let mut rng = Rng::new(9);
        let mut timid = 0;
        let mut bold = 0;
        for _ in 0..300 {
            let t = Temperament::random(&mut rng);
            if t.boldness < 0.4 {
                timid += 1;
            }
            if t.boldness > 0.7 {
                bold += 1;
            }
        }
        assert!(timid > 15, "only {timid} timid villagers in 300");
        assert!(bold > 15, "only {bold} bold villagers in 300");
    }

    #[test]
    fn every_temperament_has_a_word_for_it() {
        let mut rng = Rng::new(3);
        for _ in 0..500 {
            let t = Temperament::random(&mut rng);
            assert!(!t.describe().is_empty());
        }
    }

    /// A witness of the god's own hand goes to their knees, and the
    /// witness system OWNS them while it lasts.
    ///
    /// It used to write `Idle` every frame, which handed the villager
    /// straight back to the activity chooser; that gave them somewhere to
    /// walk, this cleared it, and the two fought at frame rate. Brett:
    /// "they start shaking like crazy. it looks like the door bug." The
    /// activity is the ownership, so this is the thing to pin.
    #[test]
    fn a_witness_kneels_and_nothing_else_owns_them() {
        use bevy::ecs::system::RunSystemOnce;

        let mut app = App::new();
        app.init_resource::<Time>();
        app.init_resource::<crate::debug::timings::Timings>();
        let seen = Vec3::new(4.0, 0.0, 0.0);
        let soul = app
            .world_mut()
            .spawn((
                crate::villager::Villager,
                Transform::default(),
                Reaction {
                    kind: ReactionKind::Watch,
                    focus: seen,
                    remaining: 5.0,
                },
                // Mid-errand, with somewhere to be: the walk must stop.
                MoveTarget(Some(Vec3::new(30.0, 0.0, 30.0))),
                Route::default(),
                CreatureMotion::new(0.0),
                Activity::Wandering,
            ))
            .id();

        app.world_mut().run_system_once(express_reactions).unwrap();
        assert_eq!(
            *app.world().get::<Activity>(soul).unwrap(),
            Activity::Marvelling,
            "the witness owns the body while the sight lasts",
        );
        assert_eq!(
            app.world().get::<MoveTarget>(soul).unwrap().0,
            None,
            "whatever they were walking to, they are not walking to it now",
        );
        assert_eq!(
            app.world().get::<CreatureMotion>(soul).unwrap().look_at,
            Some(seen),
            "they are looking at what they saw",
        );

        // And the pose follows the activity, from its one owner.
        app.world_mut()
            .run_system_once(crate::villager::belief::take_a_knee)
            .unwrap();
        assert!(
            app.world().get::<CreatureMotion>(soul).unwrap().kneeling,
            "worship is a kneel, not a stare",
        );

        // When it passes they get up and go back to their own day.
        app.world_mut().get_mut::<Reaction>(soul).unwrap().remaining = 0.0;
        app.world_mut().run_system_once(express_reactions).unwrap();
        assert_eq!(
            *app.world().get::<Activity>(soul).unwrap(),
            Activity::Idle,
            "the sight lets go of them",
        );
        app.world_mut()
            .run_system_once(crate::villager::belief::take_a_knee)
            .unwrap();
        assert!(
            !app.world().get::<CreatureMotion>(soul).unwrap().kneeling,
            "and they stand back up",
        );
    }
}
