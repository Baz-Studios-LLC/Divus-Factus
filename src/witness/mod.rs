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
    /// Somebody ate the village's own dead.
    ///
    /// The village's doing, not the god's - like [`Mauled`], and it matters
    /// for the same reason: the people who saw it are changed by it whether or
    /// not anything divine was involved. This is the memory a village's
    /// morality is actually made of.
    ///
    /// [`Mauled`]: DivineEventKind::Mauled
    AteTheDead,
    /// Goblins, seen. Not an attack - a sighting, and that is the whole of
    /// what makes it frightening: something out there keeps a camp, and it is
    /// not one of ours.
    ///
    /// The village does not sort its fears by author (see [`Mauled`]), and it
    /// does not need to be bitten to start wanting a wall. This is the memory
    /// an armory gets built out of.
    ///
    /// [`Mauled`]: DivineEventKind::Mauled
    GoblinsSeen,
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
            // Seen, not heard, and from whatever ground the seer was standing
            // on - which is why it carries further than the scream above. One
            // scout on a ridge brings the whole village a fright.
            DivineEventKind::GoblinsSeen => 70.0,
            // Seen close, and told everywhere. It carries as far as the worst
            // news a village has ever had to pass on.
            DivineEventKind::AteTheDead => 40.0,
            // Rain is only remarked on when the crops answer it.
            DivineEventKind::Rained => 22.0,
            // A pillar of light is visible from every field.
            DivineEventKind::Beckoned => 60.0,
            DivineEventKind::Fell => 55.0,
            // The dread spreads mouth to mouth, which IS the miracle.
            DivineEventKind::DoubtSown => 30.0,
        }
    }

    /// HOW IT LANDS ON THE HEART, -1 to 1: what it does to somebody's spirits
    /// when they learn of it.
    ///
    /// Separate from [`alarm`](Self::alarm), which is unsigned and only asks
    /// how frightening a thing is. A birth is unalarming AND good; a mending
    /// is unalarming and better still. Fright and sorrow are not the same
    /// axis, and a village needs both.
    ///
    /// Brett: "let's say somebody's happy and their neighbor tells them that
    /// someone got struck by lightning and killed - that could adjust their
    /// mood." It could not, before this: the story changed hands and the
    /// listener's spirits did not move, so news travelled through a village
    /// without ever making anyone feel anything.
    pub fn heart(self) -> f32 {
        match self {
            // Somebody is dead. Nothing else in the game is this heavy.
            DivineEventKind::Perished => -0.45,
            DivineEventKind::Fell => -0.40,
            DivineEventKind::Mauled => -0.35,
            // Not grief but horror, and horror at your own neighbours.
            DivineEventKind::AteTheDead => -0.50,
            DivineEventKind::Smote => -0.30,
            DivineEventKind::Quaked => -0.25,
            DivineEventKind::Uprooted => -0.18,
            DivineEventKind::GoblinsSeen => -0.20,
            DivineEventKind::DoubtSown => -0.15,
            DivineEventKind::Thrown => -0.12,
            DivineEventKind::Impact => -0.10,
            // Being picked up by a god is not painful. It is not nothing.
            DivineEventKind::Lifted => -0.04,
            DivineEventKind::SetDown => 0.02,
            // And the good half, which a village needs as much.
            DivineEventKind::Delivered => 0.30,
            DivineEventKind::Mended => 0.28,
            DivineEventKind::Provided => 0.22,
            DivineEventKind::Flourished => 0.18,
            DivineEventKind::Beckoned => 0.08,
            DivineEventKind::Rained => 0.06,
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
            // Under a mauling - nobody is bleeding - but well over anything
            // else, and it is the kind of fright that travels.
            DivineEventKind::GoblinsSeen => 0.62,
            // The most alarming thing in the game, and it is not a monster.
            DivineEventKind::AteTheDead => 0.95,
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
            // A goblin is a goblin. Nobody reads a god into one, and a village
            // that thought its god had sent them would be a different game.
            DivineEventKind::GoblinsSeen => 0.02,
            // Nobody mistakes this for a god's work. It is entirely ours.
            DivineEventKind::AteTheDead => 0.01,
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
            DivineEventKind::Perished | DivineEventKind::Mauled | DivineEventKind::GoblinsSeen => {
                0.03
            }
            // It costs the village its spirits more than anything else can.
            DivineEventKind::AteTheDead => -0.22,
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
            DivineEventKind::GoblinsSeen => "saw goblins out past the fields",
            DivineEventKind::AteTheDead => "saw one of their own eat the dead",
            DivineEventKind::Rained => "stood in rain that came when it was called",
            DivineEventKind::Beckoned => "saw a pillar of light stand on the ground",
            DivineEventKind::Fell => "saw a stone fall out of the empty sky",
            DivineEventKind::DoubtSown => "felt a shadow cross every heart at once",
        }
    }

    /// The same act, retold. A rumor is a witness account with the witness
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
            DivineEventKind::AteTheDead => &[
                "I saw what they did to the body. I wish I had not",
                "we are not what we were before the winter",
                "nobody said anything. That is the part I cannot get past",
                "there was a body and now there is not, and we all know why",
                "I would not have. I tell myself I would not have",
            ],
            DivineEventKind::GoblinsSeen => &[
                "there are goblins out past the fields, and they have a fire",
                "I saw green ones. They have built something out there",
                "they were watching us from a tower they made themselves",
                "goblins. A camp of them, and not far enough away",
                "somebody should tell the mayor what is out in those woods",
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
/// The grain a person is BORN with: five axes, all 0 to 1, all inherited.
///
/// Kept apart from the person they have since become - see [`Temperament`] -
/// because a child inherits its parents' NATURE and not their scars. A man
/// hardened by two famines and a smiting fathers the gentle boy he himself
/// once was, and then has to watch what the world does to him too. That split
/// is the whole reason this type exists.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Nature {
    /// 0 bolts at anything, 1 walks toward it.
    pub boldness: f32,
    /// What they are capable of when it costs somebody else. 0 could never.
    pub darkness: f32,
    /// How well they reason about what they have seen. 0 simple, 1 sharp.
    pub wits: f32,
    /// How heavily another person's suffering sits on them. 0 unmoved.
    pub warmth: f32,
    /// HOW BADLY THEY NEED THE GOD TO NOTICE THEM.
    ///
    /// The axis that belongs to this game and to no other. Every other line in
    /// this struct would be at home in any village simulation; this one only
    /// means anything because there is a god in the world and a player behind
    /// it. Fervor is what makes somebody pray when nothing is wrong, walk
    /// toward a pillar of light, read a sighting as a sign - and, with a low
    /// enough wit or a dark enough grain, propose that somebody be given up
    /// for it.
    pub fervor: f32,
}

impl Default for Nature {
    fn default() -> Self {
        Nature {
            boldness: 0.5,
            darkness: 0.0,
            wits: sharp_enough(),
            warmth: 0.6,
            fervor: 0.4,
        }
    }
}

impl Nature {
    pub fn random(rng: &mut Rng) -> Self {
        Nature {
            boldness: rng.trait_value(0.15, 0.95),
            // THE LEAST OF THREE, which is what makes the darkest of them rare
            // rather than merely uncommon. A flat roll puts a fifth of any
            // village at the top of the scale; three rolls and keep the lowest
            // puts them in the low single figures.
            darkness: rng
                .trait_value(0.0, 1.0)
                .min(rng.trait_value(0.0, 1.0))
                .min(rng.trait_value(0.0, 1.0)),
            wits: rng.trait_value(0.08, 0.95),
            // Most people are moved by other people, so this leans the
            // opposite way from darkness: the unmoved are the uncommon ones.
            warmth: rng.trait_value(0.0, 1.0).max(rng.trait_value(0.0, 1.0)),
            fervor: rng.trait_value(0.05, 0.95),
        }
    }

    /// A child of these two.
    ///
    /// AVERAGE PLUS DRIFT, not a coin flip per axis. Brett's first instinct
    /// was fifty-fifty from each parent, and the trouble with it is that it
    /// destroys a line in two generations: every axis is a fresh coin, so
    /// nothing accumulates and no family ever visibly resembles itself.
    ///
    /// Sitting the child between its parents and letting it wander gives the
    /// thing worth having - a bloodline you can SEE, bold people begetting
    /// bold people over four generations. And rarely a real jump, because a
    /// dark child in a gentle house is a story no amount of averaging would
    /// ever produce.
    pub fn inherit(mother: &Nature, father: &Nature, rng: &mut Rng) -> Self {
        let mut blend = |a: f32, b: f32| {
            let middle = (a + b) * 0.5;
            // About one axis in seven throws properly rather than drifting.
            let wander = if rng.chance(0.14) {
                rng.range(-0.42, 0.42)
            } else {
                rng.range(-0.11, 0.11)
            };
            (middle + wander).clamp(0.0, 1.0)
        };
        Nature {
            boldness: blend(mother.boldness, father.boldness),
            darkness: blend(mother.darkness, father.darkness),
            wits: blend(mother.wits, father.wits),
            warmth: blend(mother.warmth, father.warmth),
            fervor: blend(mother.fervor, father.fervor),
        }
    }
}

/// Who a person is now, and who they were born as.
///
/// THE ONE PLACE THIS GAME SHOULD NOT FOLLOW DWARF FORTRESS. A dwarf's facets
/// are fixed for life; the wiki is explicit that argument can shift a dwarf's
/// VALUES and never their facets. Which is right for a game with no god in it.
///
/// Here there is a god, and it is the player, so the interesting claim is the
/// opposite one: WHAT YOU DO TO PEOPLE CHANGES THEM. A village that watches
/// its god throw men down hillsides does not merely come to permit dark things
/// - it comes to be full of harder people. And their children begin from where
/// those people STARTED rather than where they ended, which is why the two
/// halves below are kept apart.
#[derive(Component, Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Temperament {
    /// 0 bolts at anything, 1 walks toward it.
    pub boldness: f32,
    /// What this person is capable of when it costs somebody else.
    ///
    /// 0 could never, whatever it took; 1 is capable of nearly anything.
    /// Brett: "some people would never... only the darkest of personalities
    /// would do something like that", and then, plainly: "perceptability to
    /// evil proclivities needs to be tracked doesnt it?" It does, and until
    /// this nothing did.
    pub darkness: f32,
    /// How well this person reasons about what they have seen.
    ///
    /// 0 is simple, 1 is sharp. NOT a competence stat - nothing works faster
    /// for being clever. It is about READING THE WORLD, and it exists for a
    /// mechanism darkness cannot cover. Brett: "sometimes unintelligent may do
    /// dark deeds because they think it is the right thing to do... they may
    /// feel they are being good when they arent."
    #[serde(default = "sharp_enough")]
    pub wits: f32,
    /// How heavily another person's suffering sits on this one.
    ///
    /// Nothing reads it yet, and it is here anyway - Brett: "its okay to ad a
    /// trait or something that we know may get used down the road... Variables
    /// don't really cost anything but they could ad some flavor at a minimum."
    /// It is the axis the regard system will want first: who grieves, who
    /// helps, and who walks past.
    #[serde(default = "half")]
    pub warmth: f32,
    /// How badly this person needs the god to notice them. See [`Nature`].
    #[serde(default = "half")]
    pub fervor: f32,
    /// The grain they were born with. Weathering never touches it, and it is
    /// what their children inherit.
    #[serde(default)]
    pub born: Nature,
}

/// What a person loaded from a save before wits existed is worth.
///
/// Sharp enough to know better, which is the safe way to be wrong about
/// somebody: an old save cannot suddenly fill with people who misread their
/// god.
fn sharp_enough() -> f32 {
    0.7
}

fn half() -> f32 {
    0.5
}

/// The most a whole life can move any one axis from the grain it started with.
///
/// Bounded on purpose. A person can be hardened, frightened or made devout by
/// what happens to them; they cannot be turned into somebody else. A gentle man
/// ground down by a terrible god ends up hard, never monstrous - the monstrous
/// have to be BORN, which keeps them rare and keeps the roll meaningful.
pub const MOST_A_LIFE_MOVES: f32 = 0.28;

/// Below this nobody is capable of anything the doctrine weighs, ever.
///
/// High, and it must stay high: with the roll above it leaves a handful of
/// souls in a village of thirty who could even be asked.
pub const COULD_NEVER: f32 = 0.55;

/// Below this a person does not reliably work out what a thing means.
///
/// A quarter or so of a village, which is about right: enough that a violent
/// god always has somebody who has taken the wrong lesson, never so many that
/// the village reads as stupid.
pub const SIMPLE: f32 = 0.3;

impl Temperament {
    pub fn random(rng: &mut Rng) -> Self {
        Temperament::of(Nature::random(rng))
    }

    /// Somebody who has lived no life yet: they are exactly their grain.
    pub fn of(born: Nature) -> Self {
        Temperament {
            boldness: born.boldness,
            darkness: born.darkness,
            wits: born.wits,
            warmth: born.warmth,
            fervor: born.fervor,
            born,
        }
    }

    /// A child of these two parents.
    pub fn child(mother: &Temperament, father: &Temperament, rng: &mut Rng) -> Self {
        // FROM THE GRAIN, NOT THE PERSON. What the parents have been through is
        // theirs to carry; what they were born as is the child's to start from.
        Temperament::of(Nature::inherit(&mother.born, &father.born, rng))
    }

    /// Bends one axis by what has happened to them, within what a life can do.
    ///
    /// The bound is always measured from the GRAIN, so a person weathered hard
    /// in one direction cannot then be weathered indefinitely in the other.
    pub fn weather(&mut self, axis: Axis, by: f32) {
        let (now, born) = match axis {
            Axis::Boldness => (&mut self.boldness, self.born.boldness),
            Axis::Darkness => (&mut self.darkness, self.born.darkness),
            Axis::Wits => (&mut self.wits, self.born.wits),
            Axis::Warmth => (&mut self.warmth, self.born.warmth),
            Axis::Fervor => (&mut self.fervor, self.born.fervor),
        };
        *now = (*now + by).clamp(
            (born - MOST_A_LIFE_MOVES).max(0.0),
            (born + MOST_A_LIFE_MOVES).min(1.0),
        );
    }

    /// Whether this person could ever do a thing that costs somebody else,
    /// under any pressure at all.
    ///
    /// The hard gate, and deliberately a gate rather than a slope. Most of a
    /// village answers no here and is never asked a second question.
    pub fn could_ever(&self) -> bool {
        self.darkness > COULD_NEVER
    }

    /// Whether this person could talk themselves into a terrible act by
    /// mistaking it for a good one.
    ///
    /// FAITH RUNS BACKWARDS HERE, which is the whole point of it. For anyone
    /// who can reason, trusting the god is what holds them - they have
    /// something to answer to. For someone who cannot, trust in a cruel god is
    /// the argument FOR, and they are doing their sincere best throughout.
    pub fn misreads_the_god(&self, example: f32, trust: f32) -> bool {
        self.wits < SIMPLE && example > 0.55 && trust > 0.55
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

    /// Kept for the dossier, which will want to say it plainly rather than
    /// only as part of a sentence.
    #[allow(dead_code)]
    pub fn describe_wits(&self) -> &'static str {
        match self.wits {
            w if w < SIMPLE => "simple",
            w if w < 0.6 => "plain",
            w if w < 0.85 => "quick",
            _ => "sharp",
        }
    }

    pub fn describe_darkness(&self) -> &'static str {
        match self.darkness {
            d if d <= COULD_NEVER => "could never",
            d if d < 0.72 => "hard",
            d if d < 0.88 => "cold",
            _ => "capable of anything",
        }
    }

    /// THE WHOLE PERSON, IN ONE SENTENCE - and this is a rule the axes answer
    /// to rather than a nicety.
    ///
    /// Dwarf Fortress can afford forty-six facets because its entire interface
    /// is a text readout. The test that keeps depth from turning into noise is
    /// whether a number can be SAID about somebody: an axis the inspector
    /// cannot put in this sentence is not earning its place yet, however many
    /// systems read it.
    ///
    /// ONLY THE REMARKABLE ENDS ARE SPOKEN, which is the one thing about DF's
    /// presentation worth taking outright - a dwarf reads as a person because
    /// you are shown the two or three ways they are unusual, never the forty
    /// that sit in the middle of their range.
    pub fn say_the_grain(&self) -> String {
        let mut said: Vec<&str> = vec![self.describe()];
        if self.wits < SIMPLE {
            said.push("simple");
        } else if self.wits > 0.85 {
            said.push("sharp");
        }
        if self.warmth > 0.82 {
            said.push("tender-hearted");
        } else if self.warmth < 0.2 {
            said.push("unmoved by others");
        }
        if self.fervor > 0.85 {
            said.push("hungry for the god's eye");
        } else if self.fervor < 0.12 {
            said.push("indifferent to the god");
        }
        if self.could_ever() {
            said.push(self.describe_darkness());
        }
        said.join(", ")
    }
}

/// One axis of a temperament, for [`Temperament::weather`].
///
/// Only darkness and warmth are weathered by anything so far. The other three
/// are named because the enum is the vocabulary of the thing and a partial one
/// would have to be widened by whoever first needs boldness to move - which,
/// given that a mauling ought to make somebody warier, will not be long.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Axis {
    Boldness,
    Darkness,
    Wits,
    Warmth,
    Fervor,
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
/// act is "your brother struck" to one onlooker and "a neighbor struck" to
/// the one beside them, and that difference belongs IN the memory.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Whom {
    /// Their given name — the part a truth-checked retelling is allowed to say.
    pub name: String,
    /// What they are to the witness: "your brother", "your neighbor".
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
    /// WHETHER THIS PERSON SAW IT, or was told.
    ///
    /// The distinction was kept only as two counters on the record - `total`
    /// and `secondhand` - and `Retelling::hand_of` read the first of them, so
    /// anybody who had ever witnessed ANYTHING reported "saw" for every story
    /// they told, including ones they had only heard. A villager who once
    /// watched a tree fall would say he saw the goblins.
    ///
    /// Brett, watching one do exactly that: "I am not sure that the person who
    /// said it ever actually saw a goblin." He had not. It belongs on the
    /// memory, because it is a fact about the memory.
    ///
    /// Old saves load as firsthand, which is what they meant when they were
    /// written.
    #[serde(default = "always")]
    pub firsthand: bool,
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
                // A save written before the distinction existed meant
                // firsthand, because that is what everything was then.
                firsthand: true,
            },
            MemoryOnDisk::Bare(kind) => Memory {
                kind,
                whom: None,
                divine: true,
                day: 0,
                of: SubjectClass::default(),
                firsthand: true,
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

    pub(crate) fn record(
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
                // Recorded by the one who was there.
                firsthand: true,
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
        // TOLD, not seen - however the teller came by it. A story loses its
        // firsthand-ness the moment it is passed on, which is the whole
        // difference between "I saw it" and "I heard".
        let memory = Memory {
            firsthand: false,
            ..memory
        };
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
        // TWO FEARS, and the worse of them is what this person carries.
        //
        // A mauling counts full: somebody bled. A sighting counts for rather
        // less on its own - nobody was hurt - but it reaches far more people,
        // and `peril_of` sums over the village, so a camp seen by a dozen
        // souls outweighs one wolf that took one of them. Which is right:
        // a wolf is a bad week and a camp of goblins is a war.
        self.recent
            .iter()
            .filter_map(|memory| {
                let weight = match memory.kind {
                    DivineEventKind::Mauled => 1.0,
                    DivineEventKind::GoblinsSeen => 0.55,
                    _ => return None,
                };
                let age = today.saturating_sub(memory.day) as f32;
                Some((1.0 - age / PERIL_FADES).clamp(0.0, 1.0) * weight)
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
/// meters of the square — so a village feared wolves nobody had ever laid
/// eyes on, and shrugged at a child who came home torn open. Fear belongs
/// to the people who hold it.
pub fn peril_of<'a>(village: impl Iterator<Item = &'a Witnessed>, today: u32) -> f32 {
    village.map(|held| held.peril(today)).sum()
}

/// A settlement's fear, parked on the settlement itself.
///
/// WRITTEN BY THE PLANNER, which already sums it to decide what to build —
/// so this is not a second opinion about how frightened a village is, it is
/// the same number, kept where anything can read it. That matters more than
/// it sounds: fear drove the guard rota, the watchtower and the armory while
/// appearing in no panel, no card and no notice anywhere in the game. A
/// village would stop building a tavern and start building an armory and the
/// player had nothing to read it off.
///
/// Refreshed on the settlement's turn through the planner's round-robin, so
/// it is a few frames stale on a busy map. Nothing here is decided in
/// frames.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Peril(pub f32);

/// A village's fear, in the words a person would use.
///
/// THE THRESHOLDS ARE NOT DECORATIVE. Each one is a place where the village
/// actually starts doing something it was not doing before, so a player who
/// reads the card twice has learned the system:
///
/// - `Uneasy` — one spear gets posted. Any fear at all does this.
/// - `Afraid` — the watchtower jumps the whole civic queue.
/// - `Besieged` — the armory becomes worth wanting and the ceiling comes off
///   the muster.
///
/// Keep these in step with the numbers in [`crate::villager::work`]; the test
/// below holds them to it.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub enum Alarm {
    #[default]
    AtEase,
    Uneasy,
    Afraid,
    Besieged,
}

/// Below this a village is simply calm. Not zero, because a memory fading out
/// over a fortnight leaves a thousandth of itself behind on the last day and
/// a town should not read as uneasy over that.
const AT_EASE: f32 = 0.05;

/// Where the watchtower stops waiting its turn on the civic ladder.
pub const TOWER_JUMPS_THE_QUEUE: f32 = 1.5;

/// Where an armory becomes a thing a village would rather have than a bakery.
pub const ARMORY_IS_WANTED: f32 = 5.0;

impl Alarm {
    pub fn of(peril: f32) -> Self {
        if peril < AT_EASE {
            Alarm::AtEase
        } else if peril < TOWER_JUMPS_THE_QUEUE {
            Alarm::Uneasy
        } else if peril < ARMORY_IS_WANTED {
            Alarm::Afraid
        } else {
            Alarm::Besieged
        }
    }

    /// How the village would describe itself.
    pub fn name(self) -> &'static str {
        match self {
            Alarm::AtEase => "at ease",
            Alarm::Uneasy => "uneasy",
            Alarm::Afraid => "afraid",
            Alarm::Besieged => "expecting an attack",
        }
    }

    /// What the fear is making them do, which is the half a bare word leaves
    /// out. A player who reads "afraid" learns nothing; a player who reads
    /// that a tower is going up ahead of everything else has been told how
    /// the village works.
    pub fn tells(self) -> &'static str {
        match self {
            Alarm::AtEase => "nobody is watching the trees",
            Alarm::Uneasy => "a spear walks the treeline",
            Alarm::Afraid => "a watchtower comes before everything else",
            Alarm::Besieged => "arming comes before comfort",
        }
    }

    /// Whether this is worth waking the village over — the bell is rung on
    /// the way up into these, and `Uneasy` is deliberately not one of them.
    /// One person coming home frightened is a Tuesday. The bell is for when
    /// the village has agreed something is out there.
    pub fn worth_the_bell(self) -> bool {
        matches!(self, Alarm::Afraid | Alarm::Besieged)
    }
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
            &mut Temperament,
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

        for (entity, transform, mut temperament, mut witnessed, mut motion) in &mut villagers {
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
            // "your brother struck" to one onlooker and "a neighbor struck"
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

            // AND IT LEAVES A MARK. This is the claim the whole `Nature` /
            // `Temperament` split exists to make: a god does not merely change
            // what its village permits, it changes the people. Seeing cruelty
            // hardens somebody a little and costs them a little of what they
            // feel for other people; seeing mercy gives some of it back.
            //
            // Tiny per act, bounded by `MOST_A_LIFE_MOVES`, and off the GRAIN
            // they were born with - so a lifetime under a terrible god ends in
            // a hard man and never a monster, and their children still start
            // from where their parents began.
            let (harden, chill) = match event.kind {
                DivineEventKind::Smote | DivineEventKind::Quaked | DivineEventKind::Fell => {
                    (0.010, -0.008)
                }
                DivineEventKind::Thrown | DivineEventKind::Impact => (0.006, -0.005),
                DivineEventKind::Mended | DivineEventKind::Provided => (-0.008, 0.008),
                DivineEventKind::SetDown | DivineEventKind::Rained => (-0.004, 0.004),
                // Watching one of your own eat the dead is the single most
                // hardening thing that can happen in a village, and it is not
                // the god's doing at all.
                DivineEventKind::AteTheDead => (0.030, -0.030),
                _ => (0.0, 0.0),
            };
            if harden != 0.0 {
                temperament.weather(Axis::Darkness, harden);
                temperament.weather(Axis::Warmth, chill);
            }

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

    /// Goblins frighten a village WITHOUT biting anybody, and the fright
    /// spreads further than a wolf's because it is seen rather than heard.
    #[test]
    fn a_sighting_frightens_the_village_too() {
        let mut scout = Witnessed::default();
        scout.record(
            DivineEventKind::GoblinsSeen,
            None,
            false,
            10,
            SubjectClass::Person,
        );
        assert!(
            scout.peril(10) > 0.0,
            "somebody who has seen goblins is frightened, though nobody bled",
        );

        let mut bitten = Witnessed::default();
        bitten.record(
            DivineEventKind::Mauled,
            None,
            false,
            10,
            SubjectClass::Person,
        );
        assert!(
            scout.peril(10) < bitten.peril(10),
            "one sighting must weigh less than one mauling: {} against {}",
            scout.peril(10),
            bitten.peril(10),
        );

        // BUT IT REACHES FURTHER, and that is the whole of why a camp is worse
        // than a wolf. `peril_of` sums over people, so a dozen souls who have
        // seen the camp outweigh the one survivor of a mauling.
        let village: Vec<Witnessed> = (0..12)
            .map(|_| {
                let mut held = Witnessed::default();
                held.record(
                    DivineEventKind::GoblinsSeen,
                    None,
                    false,
                    10,
                    SubjectClass::Person,
                );
                held
            })
            .collect();
        assert!(
            peril_of(village.iter(), 10) > peril_of(std::iter::once(&bitten), 10) * 4.0,
            "a camp the whole village has seen must outweigh one mauling by a lot",
        );
    }

    /// And a sighting fades like every other fright.
    #[test]
    fn a_sighting_fades_too() {
        let mut scout = Witnessed::default();
        scout.record(
            DivineEventKind::GoblinsSeen,
            None,
            false,
            3,
            SubjectClass::Person,
        );
        assert!(scout.peril(3) > scout.peril(9), "it eases with the days");
        assert_eq!(
            scout.peril(3 + PERIL_FADES as u32),
            0.0,
            "and a quiet fortnight takes it away",
        );
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
                // `hear` overwrites this to false; set the honest value in
                // so the fixture reads as what it is - a story being passed.
                firsthand: false,
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
                tie: "your neighbor".into(),
                subject: None,
            }),
            true,
            1,
            SubjectClass::Person,
        );
        assert_eq!(w.recent[0].kind, DivineEventKind::Thrown);
        assert_eq!(
            w.recent[0].whom.as_ref().map(|w| w.phrase()).as_deref(),
            Some("Feitreh, your neighbor"),
            "a memory keeps who it happened to",
        );
    }

    #[test]
    fn lightning_is_weather_to_almost_everyone() {
        // The design's center: the impossible compels, the natural excuses.
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

    /// A BLOODLINE YOU CAN SEE. Average-plus-drift over four generations must
    /// keep a family recognisably itself - which a coin flip per axis cannot,
    /// and which is the entire reason for choosing this over Brett's first
    /// instinct of fifty-fifty.
    #[test]
    fn a_family_resembles_itself_down_the_generations() {
        let mut rng = Rng::new(4);
        let bold = |v: f32| Nature {
            boldness: v,
            ..Nature::default()
        };
        let mut line = (Temperament::of(bold(0.9)), Temperament::of(bold(0.85)));
        for _ in 0..4 {
            let child = Temperament::child(&line.0, &line.1, &mut rng);
            let mate = Temperament::of(bold(rng.trait_value(0.6, 0.95)));
            line = (child, mate);
        }
        assert!(
            line.0.boldness > 0.55,
            "four generations of bold people must still be bold: {}",
            line.0.boldness,
        );

        let mut timid_rng = Rng::new(4);
        let mut timid = (Temperament::of(bold(0.1)), Temperament::of(bold(0.15)));
        for _ in 0..4 {
            let child = Temperament::child(&timid.0, &timid.1, &mut timid_rng);
            let mate = Temperament::of(bold(timid_rng.trait_value(0.05, 0.4)));
            timid = (child, mate);
        }
        assert!(
            timid.0.boldness < line.0.boldness,
            "and a timid line must still be the timid one: {} against {}",
            timid.0.boldness,
            line.0.boldness,
        );
    }

    /// But a house is not a sentence. Sometimes a child is nothing like either
    /// parent, which is the story no amount of averaging would ever tell.
    #[test]
    fn a_dark_child_can_be_born_in_a_gentle_house() {
        let mut rng = Rng::new(31);
        let gentle = Temperament::of(Nature {
            darkness: 0.22,
            ..Nature::default()
        });
        let jumps = (0..400)
            .map(|_| Temperament::child(&gentle, &gentle, &mut rng))
            .filter(|child| child.darkness > gentle.darkness + 0.25)
            .count();
        assert!(
            jumps > 0,
            "in four hundred children of one gentle house, at least one throws",
        );
        assert!(
            jumps < 120,
            "but it must stay a surprise rather than a habit: {jumps} of 400",
        );
    }

    /// A LIFE BENDS A PERSON, AND NEVER REMAKES THEM. The bound is what keeps
    /// the monstrous something you are born as rather than something a bad
    /// enough god can manufacture out of anybody.
    #[test]
    fn weathering_is_bounded_by_the_grain() {
        let mut soul = Temperament::of(Nature {
            darkness: 0.1,
            ..Nature::default()
        });
        for _ in 0..500 {
            soul.weather(Axis::Darkness, 0.2);
        }
        assert!(
            soul.darkness <= 0.1 + MOST_A_LIFE_MOVES + 1e-5,
            "a whole life of horrors moves them this far and no further: {}",
            soul.darkness,
        );
        assert!(
            !soul.could_ever(),
            "and a gentle soul ground down ends up hard, never monstrous",
        );
        assert_eq!(
            soul.born.darkness, 0.1,
            "the grain underneath is untouched, and it is what their children get",
        );
    }

    /// Only the remarkable ends get spoken, which is what keeps a person from
    /// reading as a stat block.
    #[test]
    fn the_unremarkable_are_described_briefly() {
        let ordinary = Temperament::of(Nature::default());
        assert_eq!(
            ordinary.say_the_grain().split(", ").count(),
            1,
            "somebody with nothing unusual about them gets one word: {}",
            ordinary.say_the_grain(),
        );
        let notable = Temperament::of(Nature {
            boldness: 0.95,
            darkness: 0.9,
            wits: 0.1,
            warmth: 0.05,
            fervor: 0.95,
        });
        let said = notable.say_the_grain();
        assert!(
            said.split(", ").count() >= 4,
            "and somebody remarkable gets a real sentence: {said}",
        );
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
