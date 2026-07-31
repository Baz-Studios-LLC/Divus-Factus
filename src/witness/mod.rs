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
    }
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct WitnessSet;

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
        }
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

/// What a villager has seen the god do.
///
/// Capped and counted rather than kept whole. The complete record is what `history`
/// will be for; this is only what the person themself carries.
#[derive(Component, Default, Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct Witnessed {
    /// Most recent first.
    pub recent: Vec<DivineEventKind>,
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

    fn record(&mut self, kind: DivineEventKind) {
        self.recent.insert(0, kind);
        self.recent.truncate(Self::CAPACITY);
        self.total = self.total.saturating_add(1);
        // A fresh sight rekindles the urge to tell it.
        self.told = 0;
    }

    /// Whether this person has ever seen anything at all.
    pub fn is_innocent(&self) -> bool {
        self.total == 0
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
) {
    for event in events.read() {
        let carry = event.kind.carry() * (0.6 + event.intensity * 0.6);

        for (entity, transform, temperament, mut witnessed, mut motion) in &mut villagers {
            // The subject of an act is not a witness to it. They are the one it
            // happened to, and they are already reacting by being thrown.
            if event.subject == Some(entity) {
                continue;
            }

            let distance = transform.translation.distance(event.position);
            if distance > carry {
                continue;
            }

            let closeness = 1.0 - (distance / carry).clamp(0.0, 1.0);
            let kind = choose_reaction(event.kind, temperament.boldness, closeness);

            witnessed.record(event.kind);

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
) {
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
) {
    let dt = time.delta_secs();

    for (entity, mut reaction, transform, mut target, mut route, mut motion, mut activity) in
        &mut reacting
    {
        reaction.remaining -= dt;
        if reaction.remaining <= 0.0 {
            commands.entity(entity).remove::<Reaction>();
            motion.look_at = None;
            continue;
        }

        // Turn to face it. This is the part that actually reads on screen — a head
        // tracking you across a field is worth more than any number.
        motion.look_at = Some(reaction.focus);

        let away = transform.translation - reaction.focus;
        let distance = away.length();

        match reaction.kind {
            ReactionKind::Watch => {
                target.0 = None;
                route.waypoints.clear();
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

        *activity = Activity::Idle;
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
    fn memory_is_capped_but_the_count_is_not() {
        let mut w = Witnessed::default();
        assert!(w.is_innocent());
        for _ in 0..50 {
            w.record(DivineEventKind::Lifted);
        }
        assert_eq!(w.recent.len(), Witnessed::CAPACITY);
        assert_eq!(w.total, 50);
        assert!(!w.is_innocent());
    }

    #[test]
    fn the_newest_memory_comes_first() {
        let mut w = Witnessed::default();
        w.record(DivineEventKind::Lifted);
        w.record(DivineEventKind::Thrown);
        assert_eq!(w.recent[0], DivineEventKind::Thrown);
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
}
