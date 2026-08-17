//! Raids: a goblin camp musters a band and walks it onto a village.
//!
//! Everything defensive in this game pointed at a threat that could not
//! arrive. The bell rang about a slow average of what people remembered, the
//! watchtower watched nothing in particular, and the armory armed a village
//! against a rumor. Goblins were pitched three hundred meters out and kept
//! within fifty-five of their fire, so the worst that ever happened was that
//! somebody walked far enough to see them.
//!
//! # WHAT THIS DELIBERATELY DOES NOT ADD
//!
//! A combat model. Guards already close on anything whose species `hunts()`
//! within twenty-six meters and strike it a blow a beat; predators already
//! wear a villager's `Vitality.harm` down at contact. Both were built for
//! wolves and both are species-blind, so a goblin standing in the square is
//! already a fight. What was missing was only the walk.
//!
//! That is on purpose. Gear, guard sub-types and skill all change how a raid
//! GOES, and every one of them is easier to judge once there is a raid to
//! watch go badly.
//!
//! # THE SHAPE OF ONE
//!
//! A camp musters a band and marches on the square. Somebody sees them and
//! runs for the bell — the same errand as any other alarm, so a raid caught
//! late is a raid nobody rang for. The bell turns the village: guards were
//! already converging, and everyone else makes for a roof. The band fights
//! until it is broken or its blood is up and then goes home.
//!
//! A village CAN lose people to this. Brett drew the line himself: "they
//! should be self sufficient, but they shouldnt be protected from getting
//! wiped out from a raid. With that said it is a game and it needs to be
//! balanced." So the thrive law still holds for hunger and shelter, and does
//! not hold here — but a hamlet is not raided in its first spring.

use bevy::prelude::*;

use crate::creature::MoveTarget;
use crate::creature::genome::{CreatureGenome, Species};
use crate::creature::wildlife::Wild;
use crate::creature::{Corpse, Held};
use crate::villager::{MemberOf, Settlement, SettlementGround, Villager};

/// A goblin walking on a village.
#[derive(Component, Debug, Clone, Copy)]
pub struct Raider {
    /// Whether they have reached the fields yet, so the arrival is announced
    /// once rather than every frame — and so a soak can tell a band still
    /// walking from a band that never got there.
    pub arrived: bool,
    /// Whose square they are making for.
    pub town: Entity,
    /// And where that square is, in the flat sim frame — NOT a
    /// `GlobalTransform`, which on this world is a seat on a globe of radius
    /// six thousand and nowhere near anything a walker can path to.
    pub square: Vec3,
    /// When the band set out, so one that cannot get there goes home rather
    /// than walking at a wall until the heat death of the village.
    pub set_out: f64,
}

/// A villager making for a roof because the bell went.
///
/// Its own component rather than a reuse of the weather's `Sheltering`,
/// which is owned by the rain: that system puts everybody back to Idle the
/// moment it is not pouring, and would have marched the village back out of
/// doors mid-raid.
///
/// It is also NOT `Fleeing`, whose whole behavior is to run for the square
/// where the fire and the other people are. During a raid the square is
/// precisely where the goblins are standing.
#[derive(Component, Debug, Clone, Copy)]
pub struct TakingCover {
    pub until: f64,
}

/// Behind a door, and out of a raider's reach.
///
/// Separate from [`TakingCover`], which only means they are TRYING. Somebody
/// halfway across the square with a goblin at their heels is taking cover and
/// is not remotely safe, and conflating the two would have made the bell a
/// magic ward rather than a head start.
#[derive(Component, Debug, Clone, Copy)]
pub struct Indoors;

/// A camp: what a band of goblins wants, and what it remembers.
///
/// Brett asked what actually drives a raid, and the honest first answer was
/// nothing — a flat dice roll every ninety seconds, which is weather wearing
/// a motive. Then: "Goblins don't need to be as complex as people, but we
/// should give them needs and wants based off prior actions."
///
/// So: two drivers, not twenty. A NEED and a GRIEVANCE.
///
/// - **Hunger.** They are hunters with a fire, and hunters whose range has
///   been emptied look at the place that keeps its food in a heap. This one
///   the village causes without meaning to, by taking the deer.
/// - **Grudge.** Every goblin this village kills is remembered. Guards cut
///   down anything that hunts within twenty-six meters and hunters take them
///   for the wilds; both were free before this. Now they are a debt.
///
/// The grudge is the half that matters, because it is the half the PLAYER
/// causes. A god who tells the village to leave the green things alone gets
/// a quiet border. A god who has every goblin in reach put down gets what
/// that earns.
#[derive(Component, Debug)]
pub struct Camp {
    /// The fire, which is what makes a camp a camp — its people share it as
    /// their home range and come back to it.
    pub fire: Vec3,
    /// Kin the village has killed, decaying. One killing is a fright; a
    /// season of them is a reason.
    pub grudge: f32,
    /// How hard the village presses on them just by being there. Brett:
    /// "People just being near them could be considered threatening and when
    /// it happens enough they could decide that the people have to go."
    ///
    /// THE DRIVER THAT NEEDS NO VIOLENCE AT ALL. A village that never lifts a
    /// hand to a goblin still sends foresters into their trees, hunters
    /// after their deer and explorers straight through the camp, and from
    /// the fire that is not innocent - it is the same shapes coming closer,
    /// week after week. It also means the god can cause a war by choosing
    /// where the village works.
    pub pressed: f32,
    /// When they last weighed it, and when they last went. A camp that has
    /// just been bloodied does not turn round and march again.
    pub weighed: f64,
    pub last_raid: Option<f64>,
}

/// Which camp a goblin belongs to.
#[derive(Component, Debug, Clone, Copy)]
pub struct OfCamp(pub Entity);

/// Days before the first band will come. A village planted on a Tuesday has
/// ten souls, no walls, no guard and one hall; a raid inside its first season
/// is not a difficulty setting, it is a coin flip on whether the save is
/// worth keeping.
const GRACE_DAYS: u32 = 24;

/// And a floor under the population, because days alone are not maturity — a
/// village that lost half its founders to a bad winter is a hamlet again
/// however old it is.
const GRACE_SOULS: usize = 8;

/// How often a camp weighs it up, in seconds of world time.
const CONSIDERS_EVERY: f64 = 90.0;

/// How long before a camp that has raided will raid again — a fortnight and
/// a half, so a village sees a handful of these in its life and not one a
/// season. A day is [`crate::calendar::DAY_SECONDS`] long, which every
/// constant in this file is measured against and the first draft of them all
/// was not.
const NOT_AGAIN_FOR: f64 = crate::calendar::DAY_SECONDS as f64 * 15.0;

/// What the wanting has to reach before they go.
///
/// Hunger alone will not do it — a camp at its hungriest scores 1.0 and sits
/// just under this, so a village that has never touched them is raided by
/// nobody however lean the winter. It takes hunger AND a grievance, or a
/// grievance large enough on its own.
const ENOUGH_TO_MARCH: f32 = 1.15;

/// What one villager standing in the camp's range is worth per second, and
/// how near counts as standing in it.
///
/// Deliberately slow, and measured in SEASONS. A hunter passing through for
/// a few minutes is nothing at all. A crew of three working the same treeline
/// for a whole season - three people times twenty-eight six-hundred-second
/// days - comes to about one whole reason on its own.
const BEING_UNDERFOOT_COSTS: f32 = 1.0 / (3.0 * 28.0 * crate::calendar::DAY_SECONDS);
const UNDERFOOT_WITHIN: f32 = 60.0;

/// What one killing is worth, and how long it takes to stop mattering.
///
/// Four dead kin is a raid on its own. A single guard defending himself is
/// not — but at this rate one killing is still half-remembered five days
/// later and gone in ten, so a village that keeps doing it accumulates and a
/// village that stopped is forgiven within the season.
const A_KILLING_IS_WORTH: f32 = 0.3;
const GRUDGE_FADES_OVER: f32 = crate::calendar::DAY_SECONDS * 33.0;

/// How near a villager must be to a goblin's death for the camp to hold the
/// village responsible. A goblin killed by a bear in the deep woods is not
/// the village's doing, and a camp that blamed them for it would be a camp
/// raiding over the weather again.
const BLAMED_WITHIN: f32 = 45.0;

/// How far a band will walk — the whole reach camps are pitched across, so
/// every camp in the world can raid the village it is nearest.
///
/// This was four hundred and sixty, which sounded generous and was not:
/// camps are placed uniformly between three hundred and twenty meters and
/// nine hundred, so roughly three in four fall outside it and all three
/// falling outside was better than even money. The first forced raid mustered
/// nothing at all and looked exactly like a broken system.
const WILL_MARCH: f32 = 900.0;

/// How long a band stays out before it turns for home.
///
/// A SAFETY NET, not a fight timer. A goblin walks about two and a half
/// meters a second, so the far edge of [`WILL_MARCH`] is a six-minute walk on
/// its own; anything shorter than this turns a band around in a field
/// somewhere and the village never learns it was coming.
const BLOOD_IS_UP: f64 = crate::calendar::DAY_SECONDS as f64 * 2.0;

/// How close a raider must be to strike.
const WITHIN_REACH: f32 = 1.8;

/// What a goblin takes out of somebody each second at contact.
///
/// MEASURED AS A DURATION, not guessed. At the wolf's rate a villager dies in
/// under four seconds, and the first live raid killed nine of ten souls in
/// thirty-three - the bell had not finished ringing. That is not the "villages
/// can be wiped" Brett asked for, it is a village that was never in the fight.
///
/// At this rate somebody caught in the open lasts about twelve seconds: long
/// enough to run, long enough for a guard to arrive, long enough for the
/// player to do something about it. Two raiders on one villager still halve
/// that, which is what makes a band frightening rather than a stopwatch.
const A_BLOW_COSTS: f32 = 1.0 / 12.0;

/// How far a raider can be seen inside the fields.
const SEEN_AT: f32 = 34.0;

/// How long the village stays under cover once the bell has gone.
const COWERS_FOR: f64 = 70.0;

/// What a camp wants, 0 upward. Past [`ENOUGH_TO_MARCH`] they go.
///
/// Kept as a bare function so the balance can be read and tested without an
/// app, a world or a goblin.
pub fn want_to_raid(hunger: f32, grudge: f32, pressed: f32) -> f32 {
    hunger + grudge + pressed
}

/// The camps remember what the village did to them, and notice it standing
/// over their fire.
///
/// Reads the deaths the world already announces. A goblin cut down within
/// sight of somebody from the village is a debt; one that fell to a bear in
/// the deep woods is not.
pub(crate) fn camps_remember(
    time: Res<Time>,
    mut deaths: MessageReader<crate::creature::CreatureDied>,
    kinds: Query<&CreatureGenome>,
    folk: Query<&Transform, (With<Villager>, Without<Corpse>)>,
    mut camps: Query<(&mut Camp, Entity)>,
    watch: Res<crate::debug::timings::Timings>,
) {
    let _t = watch.watch("raid: camps remember");
    let dt = time.delta_secs();
    for (mut camp, _) in &mut camps {
        // A grievance is not forever. Left alone, a camp forgets.
        camp.grudge = (camp.grudge - dt / GRUDGE_FADES_OVER).max(0.0);
        camp.pressed = (camp.pressed - dt / GRUDGE_FADES_OVER).max(0.0);
        // AND THE VILLAGE IS COUNTED WHERE IT STANDS. Every soul inside the
        // camp's range, every second, adds a little - so a crew working the
        // same trees all season presses far harder than one explorer who
        // walked past once, without either of them doing a thing wrong.
        let underfoot = folk
            .iter()
            .filter(|at| at.translation.distance(camp.fire) < UNDERFOOT_WITHIN)
            .count();
        camp.pressed += underfoot as f32 * dt * BEING_UNDERFOOT_COSTS;
    }

    for death in deaths.read() {
        if !death.violent {
            continue;
        }
        if kinds
            .get(death.entity)
            .is_ok_and(|genome| genome.species != Species::Goblin)
        {
            continue;
        }
        // Somebody from the village was there. Not proof of who swung, but
        // it is the same standard the game holds people to: what was
        // witnessed, near enough to be seen.
        if !folk
            .iter()
            .any(|at| at.translation.distance(death.position) < BLAMED_WITHIN)
        {
            continue;
        }
        // The camp nearest the killing carries it.
        let Some((mut camp, _)) = camps.iter_mut().min_by(|a, b| {
            a.0.fire
                .distance(death.position)
                .total_cmp(&b.0.fire.distance(death.position))
        }) else {
            continue;
        };
        camp.grudge += A_KILLING_IS_WORTH;
        info!("a camp marks a killing: grudge now {:.2}", camp.grudge);
    }
}

/// Whether a camp wants it badly enough, and how big a band it sends.
#[allow(clippy::type_complexity)]
pub(crate) fn muster_a_warband(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    towns: Query<(Entity, &SettlementGround), With<Settlement>>,
    souls: Query<&MemberOf, (With<Villager>, Without<Corpse>)>,
    mut camps: Query<(Entity, &mut Camp)>,
    band: Query<(Entity, &Wild, &OfCamp), (Without<Corpse>, Without<Held>, Without<Raider>)>,
    mut forced_done: Local<bool>,
    watch: Res<crate::debug::timings::Timings>,
) {
    let _t = watch.watch("raid: muster");
    // ONE band, not every camp in the world. The harness sent all three at
    // once and nine goblins met a village of ten - which tested nothing
    // except what a rout looks like.
    let forced = std::env::var("DIVUS_FACTUS_RAID").is_ok_and(|v| v == "1");
    // ACROSS FRAMES, not within one. A flag local to the loop stopped the
    // second camp going on the same tick and let it go on the next, so the
    // harness still sent all three - nine goblins against ten souls - and
    // still tested nothing but a rout.
    if forced && *forced_done {
        return;
    }
    let mut sent_one = false;

    for (camp_entity, mut camp) in &mut camps {
        if forced && sent_one {
            break;
        }
        if clock.elapsed - camp.weighed < CONSIDERS_EVERY && !forced {
            continue;
        }
        camp.weighed = clock.elapsed;
        if camp
            .last_raid
            .is_some_and(|then| clock.elapsed - then < NOT_AGAIN_FOR)
        {
            continue;
        }

        // Who is at home and could go.
        let here: Vec<(Entity, f32)> = band
            .iter()
            .filter(|(_, _, of)| of.0 == camp_entity)
            .map(|(goblin, wild, _)| (goblin, wild.hunger))
            .collect();
        if here.is_empty() {
            continue;
        }
        // THE NEED: how hungry the camp is, averaged. One lean goblin among
        // six is not a reason to go to war.
        let hunger = here.iter().map(|(_, hunger)| *hunger).sum::<f32>() / here.len() as f32;
        let wanting = want_to_raid(hunger, camp.grudge, camp.pressed);
        if wanting < ENOUGH_TO_MARCH && !forced {
            continue;
        }

        // The nearest village worth walking to.
        let Some((town, ground, souls_there)) = towns
            .iter()
            .map(|(town, ground)| {
                let count = souls.iter().filter(|member| member.0 == town).count();
                (town, ground, count)
            })
            .filter(|(_, ground, count)| {
                ground.center.distance(camp.fire) < WILL_MARCH && *count >= GRACE_SOULS
            })
            .min_by(|a, b| {
                a.1.center
                    .distance(camp.fire)
                    .total_cmp(&b.1.center.distance(camp.fire))
            })
        else {
            continue;
        };
        if clock.day() < GRACE_DAYS && !forced {
            continue;
        }

        // SCALED TO THE VILLAGE, and capped by what the camp actually has.
        // A band of four against a town of twenty is a nuisance; the same
        // four against a hamlet of eight is the end of it. Roughly one raider
        // per three souls, which a village with guards and a tower turns back
        // and a village that ignored its own fear does not.
        let want = (souls_there / 3).max(2);
        let sent = want.min(here.len());
        for (goblin, _) in here.iter().take(sent) {
            commands.entity(*goblin).insert(Raider {
                arrived: false,
                town,
                square: ground.center,
                set_out: clock.elapsed,
            });
        }
        camp.last_raid = Some(clock.elapsed);
        sent_one = true;
        *forced_done = true;
        info!(
            "a warband of {sent} sets out - hunger {:.2}, grudge {:.2}, pressed {:.2} - {:.0} strides off",
            hunger,
            camp.grudge,
            camp.pressed,
            ground.center.distance(camp.fire)
        );
        // The going is the answer to both grievances. A camp that marches and
        // is beaten back does not hold the same debt the next morning -
        // otherwise one bad season would put a village under permanent siege.
        camp.grudge *= 0.35;
        camp.pressed *= 0.35;
    }
}

/// The march, the fighting at the end of it, and the walk home.
#[allow(clippy::type_complexity)]
pub(crate) fn the_warband_marches(
    time: Res<Time>,
    clock: Res<crate::calendar::WorldClock>,
    mut commands: Commands,
    mut raiders: Query<
        (Entity, &Transform, &mut Raider, &mut MoveTarget, &mut Wild),
        (Without<Corpse>, Without<Held>, Without<Villager>),
    >,
    mut folk: Query<
        (
            Entity,
            &Transform,
            &MemberOf,
            &mut crate::creature::Vitality,
            &mut crate::creature::anim::CreatureMotion,
            Has<Indoors>,
        ),
        (With<Villager>, Without<Corpse>, Without<Held>),
    >,
    watch: Res<crate::debug::timings::Timings>,
) {
    let _t = watch.watch("raid: the march");
    let dt = time.delta_secs();

    for (raider, at, mut raid, mut target, mut wild) in &mut raiders {
        // Home again. The band is a band for as long as its blood is up;
        // after that they are goblins in a field with a long walk ahead.
        if clock.elapsed - raid.set_out > BLOOD_IS_UP {
            commands.entity(raider).remove::<Raider>();
            target.0 = Some(wild.home);
            continue;
        }
        // A raider is not out here for a meal, and a hungry one that wandered
        // off to graze mid-raid would break the band up. Kept fed for the
        // duration so the wilderness's own appetites leave them alone.
        wild.hunger = wild.hunger.min(0.3);

        let here = at.translation;
        if !raid.arrived && here.distance(raid.square) < SEEN_AT {
            raid.arrived = true;
            info!("a warband reaches the fields of the village");
        }
        // The nearest of this town's people who is OUT IN THE OPEN. Not the
        // lone-and-far-from-home test the wolves use — that rule is the whole
        // difference between stalking and raiding, and a band that would not
        // touch anybody near the square is not a raid at all.
        //
        // BUT COVER HAS TO WORK, or the bell is decoration. Somebody who got
        // the warning and made it to a roof is behind a door; a raider walks
        // past them and looks for whoever is still standing in the square.
        // That is the entire point of building the bell as an errand: ring it
        // in time and the village lives, ring it late and the ones caught
        // outside do not.
        let mark = folk
            .iter()
            .filter(|(_, _, member, ..)| member.0 == raid.town)
            .filter(|(_, _, _, _, _, under_roof)| !*under_roof)
            .map(|(who, spot, ..)| (who, spot.translation))
            .min_by(|a, b| a.1.distance(here).total_cmp(&b.1.distance(here)));

        match mark {
            Some((quarry, spot)) if spot.distance(here) < SEEN_AT => {
                if spot.distance(here) > WITHIN_REACH {
                    target.0 = Some(spot);
                    continue;
                }
                target.0 = None;
                if let Ok((_, _, _, mut vitality, mut motion, _)) = folk.get_mut(quarry) {
                    vitality.harm += dt * A_BLOW_COSTS;
                    vitality.violent = true;
                    vitality.undoing = crate::creature::Undoing::Blow;
                    motion.flail = 1.0;
                }
            }
            // Nobody in sight yet: keep walking at the square.
            _ => {
                if here.distance(raid.square) > 3.0 {
                    target.0 = Some(raid.square);
                } else {
                    target.0 = None;
                }
            }
        }
    }
}

/// Somebody sees the band and runs for the bell.
///
/// Brett: "if a goblin comes inside the wall, a person should have to see the
/// goblin and run to the bell to ring it." This is that case, and it is why
/// the bell was built as an errand: a raid is rung for late, or not at all,
/// depending on who happened to be looking the right way.
///
/// It jumps the ordinary route — a fright crossing a threshold in the slow
/// average of what the village remembers — because a raider inside the fields
/// is not a rumor anybody needs to weigh.
#[allow(clippy::type_complexity)]
pub(crate) fn the_alarm_is_raised(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    raiders: Query<(&Transform, &Raider), (Without<Corpse>, Without<Held>)>,
    posts: Query<(&Transform, &MemberOf), With<crate::villager::alarm::BellPost>>,
    folk: Query<
        (Entity, &Transform, &MemberOf),
        (
            With<Villager>,
            Without<Corpse>,
            Without<Held>,
            Without<crate::avatar::Ridden>,
        ),
    >,
    running: Query<&MemberOf, With<crate::villager::alarm::RunningForTheBell>>,
    rung: Query<&crate::villager::alarm::Rung>,
    mut notices: MessageWriter<crate::ui::Notice>,
    watch: Res<crate::debug::timings::Timings>,
) {
    let _t = watch.watch("raid: the alarm");
    for (raider_at, raid) in &raiders {
        // Already rung for this one, or somebody is already on their way.
        if running.iter().any(|member| member.0 == raid.town) {
            continue;
        }
        if rung
            .get(raid.town)
            .is_ok_and(|rung| rung.last_peal.is_some_and(|day| day == clock.day()))
        {
            continue;
        }
        let here = raider_at.translation;
        // WHO SAW IT: one of this town's own, close enough to make out a
        // goblin, and the nearest such person to the raider — the one whose
        // eyes it walked into.
        let Some(witness) = folk
            .iter()
            .filter(|(_, at, member)| {
                member.0 == raid.town && at.translation.distance(here) < SEEN_AT
            })
            .map(|(who, at, _)| (who, at.translation))
            .min_by(|a, b| a.1.distance(here).total_cmp(&b.1.distance(here)))
            .map(|(who, _)| who)
        else {
            continue;
        };
        let Some(post) = posts
            .iter()
            .find(|(_, member)| member.0 == raid.town)
            .map(|(at, _)| at.translation)
        else {
            continue;
        };
        commands
            .entity(witness)
            .insert(crate::villager::alarm::RunningForTheBell {
                post,
                about: crate::witness::Alarm::Besieged,
                set_off: clock.elapsed,
            });
        notices.write(crate::ui::Notice::new(
            "Goblins in the fields — somebody is running for the bell",
        ));
        info!("a raider is seen inside the fields; somebody runs for the bell");
    }
}

/// The bell has gone and there are goblins about: everyone who is not a guard
/// makes for a roof.
#[allow(clippy::type_complexity)]
pub(crate) fn take_cover(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    raiders: Query<&Raider, (Without<Corpse>, Without<Held>)>,
    rung: Query<(Entity, &crate::villager::alarm::Rung)>,
    homes: Query<
        &Transform,
        Or<(
            With<crate::villager::work::Hut>,
            With<crate::villager::work::Longhouse>,
        )>,
    >,
    mut folk: Query<
        (
            Entity,
            &Transform,
            &MemberOf,
            Option<&crate::villager::home::Home>,
            Option<&crate::villager::work::Vocation>,
            Option<&TakingCover>,
            &mut crate::villager::Activity,
            &mut MoveTarget,
        ),
        (With<Villager>, Without<Corpse>, Without<Held>),
    >,
    watch: Res<crate::debug::timings::Timings>,
) {
    let _t = watch.watch("raid: take cover");
    // Towns with a band on them AND a bell rung today. Both, because a
    // village that has not been warned has not been warned — the point of an
    // errand is that it can fail.
    let warned: Vec<Entity> = rung
        .iter()
        .filter(|(town, rung)| {
            rung.last_peal.is_some_and(|day| day == clock.day())
                && raiders.iter().any(|raid| raid.town == *town)
        })
        .map(|(town, _)| town)
        .collect();

    for (who, at, member, home, vocation, covering, mut activity, mut target) in &mut folk {
        // Guards do not hide. They were already converging on anything that
        // hunts within twenty-six meters before this file existed.
        if matches!(vocation, Some(crate::villager::work::Vocation::Guard)) {
            continue;
        }
        if warned.contains(&member.0) {
            if covering.is_none() {
                commands.entity(who).insert(TakingCover {
                    until: clock.elapsed + COWERS_FOR,
                });
            }
            *activity = crate::villager::Activity::Sheltering;
            // Their own roof if they have one. Anyone roofless simply is not
            // safe, which is its own argument for building the houses.
            match home.and_then(|home| homes.get(home.0).ok()) {
                Some(roof) if at.translation.distance(roof.translation) > 1.4 => {
                    target.0 = Some(roof.translation);
                    commands.entity(who).remove::<Indoors>();
                }
                // Under their own roof: a raider walks past them now.
                Some(_) => {
                    target.0 = None;
                    commands.entity(who).insert(Indoors);
                }
                // Roofless, and so not safe anywhere. Which is its own
                // argument for building the houses.
                None => target.0 = None,
            }
            continue;
        }
        // The raid is over, or was never theirs.
        if let Some(covering) = covering
            && clock.elapsed > covering.until
        {
            commands.entity(who).remove::<TakingCover>();
            commands.entity(who).remove::<Indoors>();
            if *activity == crate::villager::Activity::Sheltering {
                *activity = crate::villager::Activity::Idle;
                target.0 = None;
            }
        }
    }
}

/// Says where the raid has got to, while `DIVUS_FACTUS_RAID` is set.
///
/// A band that never arrives and a band that was never sent look identical in
/// a log, and the march is the one part of this with no test that can reach
/// it - it runs on the locomotion, the terrain and half the wilderness.
pub(crate) fn probe_the_raid(
    time: Res<Time>,
    mut since: Local<f32>,
    raiders: Query<(&Transform, &Raider)>,
    camps: Query<&Camp>,
) {
    if std::env::var("DIVUS_FACTUS_RAID").is_err() {
        return;
    }
    *since += time.delta_secs();
    if *since < 15.0 {
        return;
    }
    *since = 0.0;
    let out: Vec<f32> = raiders
        .iter()
        .map(|(at, raid)| at.translation.distance(raid.square))
        .collect();
    if out.is_empty() {
        return;
    }
    let nearest = out.iter().copied().fold(f32::INFINITY, f32::min);
    let furthest = out.iter().copied().fold(0.0f32, f32::max);
    info!(
        "RAID: {} on the march, nearest {:.0} from the square, furthest {:.0}; camps {}",
        out.len(),
        nearest,
        furthest,
        camps.iter().count()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The band is sized off the village, and a camp cannot send more than it
    /// has. Four goblins against twenty souls is a nuisance; four against
    /// eight is the end of the village.
    #[test]
    fn a_band_is_sized_off_the_village() {
        let sent = |souls: usize, camp: usize| (souls / 3).max(2).min(camp);
        assert_eq!(sent(8, 6), 2, "a hamlet draws a small band");
        assert_eq!(sent(21, 6), 6, "a town draws everybody the camp has");
        assert_eq!(sent(30, 4), 4, "and never more than the camp has");
        assert_eq!(sent(3, 6), 2, "never fewer than a pair");
    }

    /// THE BALANCE THAT DECIDES WHETHER THE PLAYER CAUSED THIS.
    ///
    /// Hunger alone must never be enough. If it were, raids would be weather
    /// - they would arrive on a schedule set by how fast goblins get hungry,
    /// and nothing the god or the village did would matter. A camp has to be
    /// hungry AND wronged, or wronged badly enough on its own.
    #[test]
    fn hunger_alone_never_marches() {
        // Starving, and never touched by anybody.
        assert!(
            want_to_raid(1.0, 0.0, 0.0) < ENOUGH_TO_MARCH,
            "a camp nobody has bothered does not raid however lean the winter"
        );
        // Starving, and the village has killed one of them.
        assert!(
            want_to_raid(1.0, A_KILLING_IS_WORTH, 0.0) >= ENOUGH_TO_MARCH,
            "hungry and bloodied is enough"
        );
        // Fed, but the village has been at them for a season.
        assert!(
            want_to_raid(0.3, A_KILLING_IS_WORTH * 3.0, 0.0) >= ENOUGH_TO_MARCH,
            "a debt large enough stands on its own"
        );
    }

    /// Encroachment is slow on purpose: a hunter passing through is nothing,
    /// a crew working the same treeline all season is the village moving in.
    /// Brett: "People just being near them could be considered threatening
    /// and when it happens enough they could decide that the people have to
    /// go."
    #[test]
    fn being_underfoot_takes_a_season_to_matter() {
        let day = crate::calendar::DAY_SECONDS;
        let pressing = |souls: f32, days: f32| souls * days * day * BEING_UNDERFOOT_COSTS;

        // One hunter, half a day in their range.
        assert!(
            pressing(1.0, 0.5) < A_KILLING_IS_WORTH,
            "a hunter passing through is not worth a killing"
        );
        // A forestry crew of three, all season.
        assert!(
            want_to_raid(0.4, 0.0, pressing(3.0, 28.0)) >= ENOUGH_TO_MARCH,
            "but the village moving in is its own reason, with no blood spilled"
        );
        // And a village that keeps its distance is never a reason at all -
        // one soul wandering past for a day a week, over a season.
        assert!(
            want_to_raid(1.0, 0.0, pressing(1.0, 4.0)) < ENOUGH_TO_MARCH,
            "keeping out of their range keeps the peace"
        );
    }

    /// Left alone, a camp forgets. Otherwise the first guard who ever
    /// defended himself puts the village under permanent siege.
    ///
    /// Measured the way the system does it - a decay per second, run for a
    /// stretch of days - rather than asserted against the constant, which is
    /// how the first draft of this test passed while the numbers were an
    /// order of magnitude out.
    #[test]
    fn a_grievance_fades() {
        let day = crate::calendar::DAY_SECONDS;
        let after = |days: f32, from: f32| (from - days * day / GRUDGE_FADES_OVER).max(0.0);

        let one_killing = A_KILLING_IS_WORTH;
        assert!(
            after(5.0, one_killing) > 0.0,
            "five days on, a killing is still remembered"
        );
        assert_eq!(
            after(20.0, one_killing),
            0.0,
            "and inside a season it is forgotten"
        );
        // Four killings, though, outlive the season - and four killings is a
        // raid on its own the moment it happens.
        assert!(
            after(20.0, one_killing * 4.0) > 0.0,
            "a village that made a habit of it is not forgiven so quickly"
        );
    }

    /// HOW LONG SOMEBODY CAUGHT IN THE OPEN LASTS, which is the number that
    /// decides whether a raid is a fight or a stopwatch.
    ///
    /// The first live raid ran at the wolf's rate and killed nine of ten
    /// souls in thirty-three seconds - the bell had not finished ringing.
    /// Brett's line is that a village CAN be wiped: "they shouldnt be
    /// protected from getting wiped out from a raid. With that said it is a
    /// game and it needs to be balanced." A village that was never in the
    /// fight is not the first thing, it is the absence of the second.
    #[test]
    fn a_villager_caught_outside_has_time_to_run() {
        let seconds_to_kill = |raiders: f32| 1.0 / (A_BLOW_COSTS * raiders);

        assert!(
            (10.0..20.0).contains(&seconds_to_kill(1.0)),
            "one goblin takes about a dozen seconds - time to run, time for a \
             guard to come, time for the god to intervene"
        );
        assert!(
            seconds_to_kill(3.0) < 5.0,
            "but a band on one person is still quick, or numbers would mean nothing"
        );
        // A guard strikes 0.7 of a life every 1.1 seconds, so this is the
        // trade the whole defense rests on: one guard beats one raider
        // comfortably and loses to three.
        let guard_kills_in = 1.1 / 0.7;
        assert!(
            guard_kills_in < seconds_to_kill(1.0),
            "a guard wins one on one"
        );
        assert!(
            guard_kills_in * 3.0 > seconds_to_kill(3.0),
            "and is overwhelmed by a band"
        );
    }
}
