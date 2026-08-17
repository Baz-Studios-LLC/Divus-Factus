//! What a village holds permissible, and where it learned it.
//!
//! Brett, on what the game is actually about: "Part of this game is villages
//! that might succumb to darker things, like cannibalism or human sacrifice.
//! Desperate times call for desperate measures. Does their morality hold or
//! does it break? ... The beauty of the game is watching how these decisions
//! are played out."
//!
//! MORALITY IS NOT A VILLAGE STAT. IT IS THE SHADOW THE GOD CASTS.
//!
//! That is the whole design, and it is the one thing that makes this a god
//! game rather than a dark-events generator. A village does not hold a hidden
//! `morality: f32` that a famine chips away at. It holds a set of tenets, and
//! every one of them is worked out from WHAT THESE PEOPLE HAVE SEEN THEIR GOD
//! DO - which `witness` has been recording, act by act, with a judgement of
//! how divine each one seemed, since long before there was anything to do with
//! it.
//!
//! So three gods get three different famines:
//!
//! - A god who has MENDED and PROVIDED gets a village that holds. Mercy was
//!   demonstrated, so mercy is what the god is under-stood to want, and a
//!   starving man with a corpse in front of him has something to answer to.
//! - A god who has SMITTEN and THROWN gets a village that breaks early, and
//!   without much agony about it. Cruelty was demonstrated. They are not
//!   betraying their god by eating their dead; they are agreeing with it.
//! - A SILENT god gets the worst of the three. Nothing was demonstrated, so
//!   nothing is forbidden, and hunger is the only argument in the room.
//!
//! The player is implicated in the collapse rather than watching it. Which is
//! the entire difference.
//!
//! AND IT IS PRECEDENT, NOT A SWITCH. The first person to do a terrible thing
//! does it alone and against the grain. What decides whether it becomes a thing
//! this village DOES is who saw, and what they made of it - the same
//! `Witnessed` pipeline that carries fear from a scout to a whole settlement.
//! One villager breaks; the story travels; it normalizes or it damns them.
//!
//! WHAT THIS MUST NEVER BECOME is a timer that fires grim events. The test is
//! whether the chronicle can be read backward afterward: the winter, the god's
//! silence, the first body, who ate, who saw, what it became. If a player
//! cannot reconstruct the chain, this was built wrong.

use bevy::prelude::*;

use crate::witness::{DivineEventKind, Witnessed};

/// A thing a village might or might not permit.
///
/// Deliberately few, and each one a genuine hinge rather than a slider. More
/// will come - sacrifice is the next, and it works the opposite way round (see
/// the note on [`Doctrine::permits`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Tenet {
    /// Eating the village's own dead.
    ///
    /// The first one built, because it is the purest case: nobody chooses it,
    /// it is what is left when every other choice is gone.
    EatTheDead,
}

impl Tenet {
    /// How hard this is to come to, before anything about this village is
    /// known. The floor a starving person has to climb over.
    ///
    /// High. It should take a genuine famine and a god who has given them no
    /// reason to hold, and even then it should be one person rather than a
    /// decision.
    pub fn gravity(self) -> f32 {
        match self {
            Tenet::EatTheDead => 0.85,
        }
    }
}

/// What a village holds, and what it has already done.
///
/// One per settlement. Not per person - but a person can still refuse (see
/// [`Doctrine::permits`]), which is where the holdouts and the heretics live.
#[derive(Resource, Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct Doctrine {
    /// How far this village has already gone, per tenet: 0 never, 1 routine.
    ///
    /// PRECEDENT, and it only rises. A village that has eaten its dead once
    /// finds it easier the second winter, and no amount of good weather
    /// un-does it. That is the cost of the first time, and it is what makes
    /// the first time worth watching.
    pub precedent: Vec<(Tenet, f32)>,
}

impl Doctrine {
    pub fn precedent_for(&self, tenet: Tenet) -> f32 {
        self.precedent
            .iter()
            .find(|(t, _)| *t == tenet)
            .map_or(0.0, |(_, weight)| *weight)
    }

    /// Records that it happened, and that it is a little easier now.
    pub fn set_precedent(&mut self, tenet: Tenet, weight: f32) {
        match self.precedent.iter_mut().find(|(t, _)| *t == tenet) {
            Some(entry) => entry.1 = entry.1.max(weight).min(1.0),
            None => self.precedent.push((tenet, weight.min(1.0))),
        }
    }

    /// Whether this person, in this village, will do this thing today.
    ///
    /// Three forces, and they are three different kinds of thing:
    ///
    /// - `desperation` is the push, and it is the only one that is about now.
    /// - [`example`] is the god's, and it is about everything they have seen.
    /// - `precedent` is the village's own, and it never goes back down.
    ///
    /// A PERSON, NOT A VILLAGE. Their own faith is what holds them: someone
    /// who trusts the god has something to answer to and needs to be hungrier
    /// before they stop caring. Which is how the same famine in the same
    /// village produces one man who eats and one who will not - and the second
    /// is the more interesting of the two.
    pub fn permits(&self, tenet: Tenet, desperation: f32, example: f32, trust: f32) -> bool {
        // Faith raises the bar; the god's own example can lower it further
        // than faith raises it, which is the point of the whole design.
        let bar = (tenet.gravity() + trust * 0.35
            - example * 0.55
            - self.precedent_for(tenet) * 0.45)
            // A FLOOR, and it is the difference between this and a mechanic
            // that fires whenever the numbers line up. However cruel the god,
            // however faithless the man, however many winters this village has
            // already done it - somebody has to be genuinely far gone. Without
            // it, a cruel god and an ordinary bad afternoon were enough, which
            // is a village of monsters rather than a village that broke.
            .clamp(0.5, 1.2);
        desperation > bar
    }
}

/// How much this person's god has taught them that cruelty is holy.
///
/// 0 is a god who has only ever been gentle, or who has never been seen at all;
/// 1 is one whose every appearance was a punishment.
///
/// READ OFF WHAT THEY PERSONALLY SAW, not off a global tally, because that is
/// the only version that produces heretics: the woman who watched a man
/// lifted gently and set down is not living in the same theology as the man who
/// watched lightning take his brother, however much they agree about the
/// weather.
pub fn example(held: &Witnessed) -> f32 {
    let mut cruel = 0.0f32;
    let mut kind = 0.0f32;
    for memory in held.recent.iter() {
        match memory.kind {
            // The god was violent, and it was read as the god.
            DivineEventKind::Smote | DivineEventKind::Quaked | DivineEventKind::Fell => {
                cruel += 1.0
            }
            DivineEventKind::Thrown | DivineEventKind::Impact => cruel += 0.6,
            // The god was merciful.
            DivineEventKind::Mended | DivineEventKind::Provided => kind += 1.0,
            DivineEventKind::SetDown | DivineEventKind::Rained => kind += 0.5,
            DivineEventKind::Beckoned | DivineEventKind::Delivered => kind += 0.4,
            _ => {}
        }
    }
    if cruel + kind <= 0.0 {
        // A SILENT GOD IS NOT A KIND ONE.
        //
        // Nothing seen means nothing forbidden, and this returning 0 - "as
        // gentle as possible" - would have made an absent god the safest kind
        // to be, which is exactly backwards for a game about belief. A village
        // that has never seen its god has no reason to hold anything, so
        // silence sits ABOVE mercy and below cruelty.
        return SILENCE;
    }
    (cruel / (cruel + kind)).clamp(0.0, 1.0)
}

/// What an unseen god is worth as an argument against doing something awful.
///
/// Not nothing, and not much. A village with no revelation at all still has
/// its own habits and its own horror; it simply has nobody to answer to.
pub const SILENCE: f32 = 0.34;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::witness::SubjectClass;

    fn who_saw(kinds: &[DivineEventKind]) -> Witnessed {
        let mut held = Witnessed::default();
        for kind in kinds {
            held.record(*kind, None, false, 1, SubjectClass::Person);
        }
        held
    }

    /// THE WHOLE THESIS, as one assertion: the same famine, the same village,
    /// three different gods, three different outcomes.
    #[test]
    fn the_god_decides_what_the_famine_does() {
        let merciful = example(&who_saw(&[
            DivineEventKind::Mended,
            DivineEventKind::Provided,
            DivineEventKind::Mended,
        ]));
        let cruel = example(&who_saw(&[
            DivineEventKind::Smote,
            DivineEventKind::Thrown,
            DivineEventKind::Smote,
        ]));
        let silent = example(&who_saw(&[]));

        assert!(
            cruel > silent && silent > merciful,
            "a cruel god teaches worst, a silent one teaches nothing, and a \
             merciful one is the only argument against: {cruel} / {silent} / {merciful}",
        );

        // Starving, faithful, and in a village that has never done it.
        let village = Doctrine::default();
        let starving = 0.95;
        let trust = 0.35;
        assert!(
            village.permits(Tenet::EatTheDead, starving, cruel, trust),
            "a village whose god has only ever punished them breaks",
        );
        assert!(
            !village.permits(Tenet::EatTheDead, starving, merciful, trust),
            "a village whose god has been merciful holds, even starving",
        );
    }

    /// Faith is what holds a person, and it is held PER PERSON - so the same
    /// famine produces one who eats and one who will not.
    #[test]
    fn the_faithful_hold_out_longer() {
        let village = Doctrine::default();
        let godless = example(&who_saw(&[]));
        let hungry = 0.8;
        let faithless = village.permits(Tenet::EatTheDead, hungry, godless, 0.05);
        let devout = village.permits(Tenet::EatTheDead, hungry, godless, 0.95);
        assert!(
            faithless && !devout,
            "at the same hunger the faithless break first: {faithless} / {devout}",
        );
    }

    /// The first time is the expensive one. After that it is a thing this
    /// village does.
    #[test]
    fn precedent_only_ever_makes_it_easier() {
        let godless = example(&who_saw(&[]));
        let mut village = Doctrine::default();
        let bearable = 0.62;
        assert!(
            !village.permits(Tenet::EatTheDead, bearable, godless, 0.35),
            "the first winter, this hunger is not enough",
        );
        village.set_precedent(Tenet::EatTheDead, 0.7);
        assert!(
            village.permits(Tenet::EatTheDead, bearable, godless, 0.35),
            "the second winter, the same hunger is",
        );

        // And it never washes out.
        village.set_precedent(Tenet::EatTheDead, 0.1);
        assert!(
            village.precedent_for(Tenet::EatTheDead) >= 0.7,
            "a good year does not un-do what the village has already done",
        );
    }

    /// Nobody eats their dead over an ordinary bad afternoon. The floor has to
    /// be high enough that this is a famine and not a mood.
    #[test]
    fn ordinary_hunger_is_not_enough_for_anybody() {
        let village = Doctrine::default();
        let cruel = example(&who_saw(&[DivineEventKind::Smote, DivineEventKind::Smote]));
        for hunger in [0.0, 0.2, 0.4] {
            assert!(
                !village.permits(Tenet::EatTheDead, hunger, cruel, 0.0),
                "hunger of {hunger} must never be enough, under any god",
            );
        }
    }
}

/// How near a body somebody has to be to do it, in meters.
///
/// MEASURED, not chosen. At 2.4 the probe sat at `nearest=2.6` indefinitely: a
/// walker who has arrived stops a bit short of the exact spot, and creatures
/// hold each other off besides, so "arrived" is about three meters from a body
/// and never less.
const WITHIN_REACH: f32 = 3.6;

/// How far off somebody will walk to one.
///
/// THEY HAVE TO GO TO IT, which is not a detail. Nothing else in the game
/// brings a living villager to a corpse, so the first version of this waited
/// for a coincidence that never came - a forced famine ran a full minute with
/// every belly at 0.97 and a body on the ground, and nobody was ever within
/// arm's reach of it.
///
/// And the walk is the best part. Watching somebody cross a field toward a body
/// is the scene; a belly that quietly refills from ten meters away is a
/// statistic.
const WILL_WALK_TO: f32 = 45.0;

/// How much of a belly one body fills.
///
/// It is food, and that is the horror of it: the village does not starve that
/// day. The number is deliberately small - a corpse is not a harvest, and a
/// village that solved its famine this way would have solved it, which is not
/// the story being told.
const WHAT_A_BODY_IS_WORTH: f32 = 0.55;

/// How long the village goes before anyone does it again, in world seconds.
///
/// Not a cooldown on the mechanic - a cooldown on the SCENE. Two people at the
/// same body in the same second reads as a swarm rather than as a man alone
/// deciding something.
const NOT_AGAIN_FOR: f64 = 30.0;

/// The first person to break, and everyone who sees them.
///
/// This is the whole design running end to end, and the order of the checks is
/// the argument: hunger first (is anyone actually desperate), then a body
/// (is it even possible), and only then the question of whether this person, in
/// this village, under this god, will do it. Nothing here fires on a timer.
/// DIVUS_FACTUS_FAMINE=1 empties every belly and kills somebody, so the one
/// thing this module exists for can actually be watched.
///
/// A village that thrives is the standing law, which makes a famine the RAREST
/// state in the game and the hardest to sit and wait for - so the transgression
/// below would otherwise be code nobody had ever seen run. It runs once, a
/// little after the world settles.
pub(super) fn force_a_famine(
    clock: Res<crate::calendar::WorldClock>,
    mut done: Local<bool>,
    mut last_death: Local<f64>,
    corpses: Query<Entity, With<crate::creature::Corpse>>,
    mut stores: Query<&mut super::work::Stockpile>,
    mut folk: Query<
        (&mut Needs, &mut crate::creature::Vitality),
        (With<super::Villager>, Without<crate::creature::Corpse>),
    >,
) {
    if !std::env::var("DIVUS_FACTUS_FAMINE").is_ok_and(|v| v == "1") || clock.elapsed < 20.0 {
        return;
    }
    // THE LARDER HAS TO GO, AND KEEP GOING. The first version only set every
    // belly to 0.97 and killed one - and the probe showed hunger back at 0.00
    // within a frame, because the village had six food in the store and simply
    // ate. A famine is not a number on a person, it is an empty larder, and it
    // has to stay empty while the gatherers keep bringing berries in.
    for mut store in &mut stores {
        store.larder = Default::default();
    }
    // PINNED, every frame. Hunger drains toward fed the moment anybody finds a
    // berry, and a famine that has to be re-declared is not a famine.
    // AND A BODY HAS TO BE ON THE GROUND. One death at the start was not
    // enough: the funeral rites carry a corpse off and bury it within a few
    // seconds, so by the time anybody was hungry enough to consider it there
    // was nothing to consider. The harness keeps one available.
    let need_a_body = corpses.is_empty() && clock.elapsed - *last_death > 12.0;
    let mut first = true;
    for (mut needs, mut vitality) in &mut folk {
        needs.hunger = 0.96;
        if first && need_a_body {
            vitality.harm = 1.0;
            first = false;
            *last_death = clock.elapsed;
        }
    }
    if !*done {
        *done = true;
        info!("FAMINE forced: the larder is empty, and one of them is dead");
    }
}

pub(super) fn the_starving_and_the_dead(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    mut last_time: Local<f64>,
    mut doctrine: ResMut<Doctrine>,
    mut alarms: MessageWriter<crate::witness::DivineEvent>,
    // A DEAD VILLAGER IS NOT A VILLAGER. `succumb` strips `Villager`, `Needs`
    // and `Activity` off a body on the way to making it a corpse - so a query
    // for "a corpse that is one of ours" matched nothing, ever, and the whole
    // module sat silent through a forced famine with a body on the ground.
    // What marks one of the village's own dead is that they still have a name.
    dead: Query<(Entity, &Transform), (With<crate::creature::Corpse>, With<super::Person>)>,
    mut starving: Query<
        (
            &Transform,
            &mut Needs,
            &super::Person,
            &Witnessed,
            Option<&crate::villager::belief::Faith>,
            Option<&mut super::Chronicle>,
            &mut crate::creature::MoveTarget,
        ),
        (With<super::Villager>, Without<crate::creature::Corpse>),
    >,
) {
    if clock.elapsed - *last_time < NOT_AGAIN_FOR {
        return;
    }
    let bodies: Vec<(Entity, Vec3)> = dead
        .iter()
        .map(|(entity, at)| (entity, at.translation))
        .collect();
    if bodies.is_empty() {
        return;
    }

    for (at, mut needs, person, held, faith, mut chronicle, mut target) in &mut starving {
        // Desperation is hunger and nothing else for now. When thirst and cold
        // exist they belong in here too, and the shape does not change.
        let desperation = needs.hunger;
        if desperation < 0.5 {
            continue;
        }
        let Some((body, body_at)) = bodies
            .iter()
            .copied()
            .filter(|(_, spot)| spot.distance(at.translation) < WILL_WALK_TO)
            .min_by(|a, b| {
                a.1.distance(at.translation)
                    .total_cmp(&b.1.distance(at.translation))
            })
        else {
            continue;
        };
        let trust = faith.map_or(0.35, |faith| faith.trust);
        if !doctrine.permits(Tenet::EatTheDead, desperation, example(held), trust) {
            // THE ONES WHO REFUSE ARE THE POINT. Somebody stood over a body
            // hungry enough to think about it and did not - and the only
            // difference between them and the one who did is what they have
            // seen and what they believe.
            continue;
        }

        // Decided, but not there yet: walk. The decision is made once, at the
        // far end of the field, and then they carry it across.
        if body_at.distance(at.translation) > WITHIN_REACH {
            target.0 = Some(body_at);
            continue;
        }

        *last_time = clock.elapsed;
        needs.hunger = (needs.hunger - WHAT_A_BODY_IS_WORTH).max(0.0);
        commands.entity(body).despawn();

        // IT BECOMES A THING THIS VILLAGE HAS DONE. Only a little - one man in
        // a bad winter is not a custom - but it never goes back down, and the
        // second winter starts from here.
        let already = doctrine.precedent_for(Tenet::EatTheDead);
        doctrine.set_precedent(Tenet::EatTheDead, already.max(0.35) + 0.12);

        // And everyone near enough sees it. The same pipe that carries a
        // scout's fright to the whole village carries this - which is what
        // decides whether it becomes normal or whether this person is never
        // spoken to again.
        alarms.write(crate::witness::DivineEvent {
            kind: crate::witness::DivineEventKind::AteTheDead,
            position: body_at,
            subject: None,
            intensity: 1.0,
        });

        info!("{} ate the dead", person.name);
        // ON THEIR OWN LIFE, because that is where it belongs. A person's
        // chronicle is the raw material their theology is spun from - and this
        // is the single loudest line any of them will ever carry.
        if let Some(chronicle) = chronicle.as_mut() {
            chronicle.record(clock.day(), "ate the dead, and the village saw it");
        }
        // One person, one scene. The rest of the hungry get their own moment
        // or their own refusal, later.
        return;
    }
}

use super::Needs;
