//! The warning bell.
//!
//! A village's fear has driven its decisions for several versions now — the
//! guard rota, the watchtower that jumps the civic queue, the armory — and
//! until this it did all of that silently. Nothing rang, nothing was written
//! down, and no panel said a word. A player watched a town abandon a bakery
//! to raise a tower and had to guess why.
//!
//! Brett: "Maybe a warning bell too. The town hall has a bell in it you could
//! copy."
//!
//! # RINGING IT IS AN ERRAND
//!
//! The first cut of this rang the bell the instant a village's fear crossed a
//! line — no bell in the world, nobody pulling anything, a sound the game
//! made about a number. Brett: "if a goblin comes inside the wall, a person
//! should have to see the goblin and run to the bell to ring it. It shouldnt
//! just auto ring."
//!
//! Which is the same mistake the old peril code made and the same fix. Fear
//! stopped being a god's-eye census of live wolves and became what people
//! carry; the bell now stops being a god's-eye siren and becomes a post in
//! the square that somebody has to reach. That has consequences the automatic
//! version could never have: the runner can be caught on the way, the alarm
//! is late when the village is spread out at work, and a village with nobody
//! left to run does not ring at all.
//!
//! So the crossing does not ring the bell. It sends somebody.

use bevy::prelude::*;

use crate::witness::{Alarm, Peril};

/// The post in the square, with the bell hung in it.
#[derive(Component)]
pub struct BellPost;

/// The bell itself, hung under the headstock — its own entity so it can be
/// swung when it is struck.
#[derive(Component)]
pub struct TheBell;

/// Somebody running for the bell, and what they are running about.
///
/// The whole point of the rewrite: this component IS the alarm, in the gap
/// between seeing the thing and reaching the rope. Everything interesting
/// happens in that gap.
#[derive(Component, Debug, Clone, Copy)]
pub struct RunningForTheBell {
    /// Where the post stands.
    pub post: Vec3,
    /// What they will ring for when they get there. Recorded at the moment
    /// they set off, so a fear that fades while they are still running does
    /// not change what the bell says when it finally goes.
    pub about: Alarm,
    /// When they set off, so an errand that cannot be finished is given up
    /// on rather than held forever.
    pub set_off: f64,
}

/// How close is close enough to reach the rope.
const AT_THE_ROPE: f32 = 1.8;

/// How long a runner is given before the errand is written off.
///
/// A villager can be picked up by the god, dropped in a river, or simply
/// stranded by ground that will not path — and a village whose one runner is
/// stuck in a lake must be able to send somebody else rather than stay silent
/// forever.
const GIVES_UP_AFTER: f64 = 90.0;

/// What this village's bell has already said.
///
/// ON THE SETTLEMENT rather than in a resource, because the second town is
/// simulated exactly as fully as the first and each one is frightened of its
/// own woods. A settlement that has never been afraid has no such component
/// at all, which is also how a village founded in a quiet season avoids
/// ringing its bell on its first morning.
#[derive(Component, Debug, Clone, Copy)]
pub struct Rung {
    /// Where the fear stands now, so the next tick can tell rising from
    /// falling.
    pub at: Alarm,
    /// The worst it has ever rung for. A fright worse than anything this
    /// village has known is news whatever the cooldown says; re-crossing a
    /// line it has already crossed is not.
    pub worst: Alarm,
    /// The day of the last actual peal — `None` until the bell has rung
    /// once. Not the day of the last CHANGE: a village that went uneasy
    /// yesterday has not rung anything, and must not be gagged for it.
    pub last_peal: Option<u32>,
}

/// Days a village must go without a peal before it will ring for the same
/// level twice. Peril is a sum over people and it moves all the time — a
/// memory fades, somebody is born, a survivor dies — so a village sitting on
/// a threshold will cross it back and forth for a week. Without this the bell
/// becomes weather.
const NOT_AGAIN_FOR: u32 = 6;

/// Whether a fright is worth somebody's legs, and who has the legs.
///
/// This decides; it does not ring. The decision and the peal are separated by
/// however long it takes one frightened person to cross the fields, which is
/// the point.
#[allow(clippy::type_complexity)]
pub(super) fn somebody_runs_for_the_bell(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    towns: Query<(Entity, &Peril, Option<&Rung>)>,
    // THE FLAT TRANSFORM, not the global one. The sim runs flat and the
    // world is bent onto the sphere for drawing only, so a `GlobalTransform`
    // here is a seat on a globe of radius six thousand and a villager's
    // `Transform` is a point on a plane. Comparing them gave a runner who
    // stood in a field forever, eighty-three meters from a bell that was not
    // where the game had told them it was.
    posts: Query<(&Transform, &crate::villager::MemberOf), With<BellPost>>,
    // Who could go: their own town, what they carry, and where they are.
    // Not the held, the ridden or the dead — a body on a god's palm is not
    // running anywhere.
    folk: Query<
        (
            Entity,
            &Transform,
            &crate::villager::MemberOf,
            &crate::witness::Witnessed,
        ),
        (
            With<crate::villager::Villager>,
            Without<crate::creature::Corpse>,
            Without<crate::creature::Held>,
            Without<crate::avatar::Ridden>,
        ),
    >,
    running: Query<&crate::villager::MemberOf, With<RunningForTheBell>>,
    watch: Res<crate::debug::timings::Timings>,
) {
    let _t = watch.watch("villager: runs for the bell");
    let today = clock.day();

    for (town, peril, rung) in &towns {
        let now = Alarm::of(peril.0);
        let before = rung.map(|r| r.at).unwrap_or_default();
        let worst = rung.map(|r| r.worst).unwrap_or_default();
        let last_peal = rung.and_then(|r| r.last_peal);
        if now == before {
            continue;
        }

        // Falling is silent. A village calming down is a fine thing and it
        // is not an event — it happens by nothing continuing to go wrong.
        // Worth writing down all the same, so the climb back up is seen as a
        // climb.
        if now < before {
            commands.entity(town).insert(Rung {
                at: now,
                worst,
                last_peal,
            });
            continue;
        }

        // TWO WAYS TO EARN A PEAL, and the split is what keeps the bell
        // meaning something. A fright worse than any this village has known
        // always sends somebody — afraid becoming besieged is not a wobble,
        // it is the thing getting worse. Re-crossing a line already crossed
        // has to wait out the cooldown, because peril is a sum over people
        // and a village sitting on a threshold will step over it and back
        // for a week as memories fade.
        let worth_running = now.worth_the_bell()
            && (now > worst
                || last_peal.is_none_or(|day| today.saturating_sub(day) >= NOT_AGAIN_FOR));
        if !worth_running {
            commands.entity(town).insert(Rung {
                at: now,
                worst,
                last_peal,
            });
            continue;
        }

        // One runner per town. Ten people converging on one rope is a comedy,
        // and the second one through the door adds nothing.
        if running.iter().any(|member| member.0 == town) {
            continue;
        }
        let Some(post) = posts
            .iter()
            .find(|(_, member)| member.0 == town)
            .map(|(at, _)| at.translation)
        else {
            continue;
        };

        // WHOEVER SAW IT, nearest the post. Not a guard, not an elder: the
        // person who is carrying the fright and has the shortest way to run.
        // A child who came home torn open raises the alarm herself.
        let Some(runner) = folk
            .iter()
            .filter(|(_, _, member, held)| member.0 == town && held.peril(today) > 0.0)
            .min_by(|(_, a, ..), (_, b, ..)| {
                a.translation
                    .distance_squared(post)
                    .total_cmp(&b.translation.distance_squared(post))
            })
            .map(|(entity, ..)| entity)
        else {
            // Nobody left who saw it and can move. Leave the level unwritten
            // so the village tries again the moment somebody can go.
            continue;
        };

        commands.entity(town).insert(Rung {
            at: now,
            worst,
            last_peal,
        });
        commands.entity(runner).insert(RunningForTheBell {
            post,
            about: now,
            set_off: clock.elapsed,
        });
    }
}

/// The run, and the peal at the end of it.
#[allow(clippy::type_complexity)]
pub(super) fn ring_the_bell(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    mut runners: Query<
        (
            Entity,
            &Transform,
            &RunningForTheBell,
            &crate::villager::MemberOf,
            &mut crate::villager::Activity,
            &mut crate::villager::MoveTarget,
        ),
        (
            Without<crate::creature::Corpse>,
            Without<crate::creature::Held>,
            Without<crate::avatar::Ridden>,
        ),
    >,
    mut towns: Query<(&crate::villager::Settlement, Option<&mut Rung>)>,
    mut notices: MessageWriter<crate::ui::Notice>,
    mut play: MessageWriter<crate::sfx::PlaySfx>,
    watch: Res<crate::debug::timings::Timings>,
) {
    let _t = watch.watch("villager: ring the bell");
    for (runner, at, errand, member, mut activity, mut target) in &mut runners {
        if at.translation.distance(errand.post) > AT_THE_ROPE {
            // Give up on a run that is plainly not happening, so the town can
            // send somebody else rather than wait on a person in a river.
            if clock.elapsed - errand.set_off > GIVES_UP_AFTER {
                commands.entity(runner).remove::<RunningForTheBell>();
                *activity = crate::villager::Activity::Idle;
                target.0 = None;
                continue;
            }
            *activity = crate::villager::Activity::Alarming;
            target.0 = Some(errand.post);
            continue;
        }

        // At the rope.
        commands.entity(runner).remove::<RunningForTheBell>();
        *activity = crate::villager::Activity::Idle;
        target.0 = None;

        let Ok((settlement, rung)) = towns.get_mut(member.0) else {
            continue;
        };
        if let Some(mut rung) = rung {
            rung.worst = rung.worst.max(errand.about);
            rung.last_peal = Some(clock.day());
        }
        // Logged as well as noticed. Deaths say their piece in the log and
        // a peal is the same kind of event - and a soak has no other way to
        // see one, since notices live only in the interface.
        info!(
            "the bell rings in {}: {}",
            settlement.name,
            errand.about.name()
        );
        notices.write(crate::ui::Notice::new(format!(
            "The bell rings in {}. The village is {} - {}.",
            settlement.name,
            errand.about.name(),
            errand.about.tells()
        )));
        // At the post, in the world, so it fades with distance like
        // everything else that actually happens somewhere.
        play.write(crate::sfx::PlaySfx {
            kind: crate::sfx::SfxKind::Alarm,
            at: Some(errand.post),
        });
    }
}

/// The bell swings for a moment after it is struck.
pub(super) fn swing_the_bell(
    time: Res<Time>,
    mut struck: Local<f32>,
    mut peals: MessageReader<crate::sfx::PlaySfx>,
    mut bells: Query<&mut Transform, With<TheBell>>,
) {
    if peals
        .read()
        .any(|peal| peal.kind == crate::sfx::SfxKind::Alarm)
    {
        *struck = 1.0;
    }
    if *struck <= 0.0 {
        return;
    }
    *struck = (*struck - time.delta_secs() * 0.45).max(0.0);
    // Swinging on the beam: a decaying wobble, fast enough to read as a
    // clapper and not as a flag.
    let swing = (*struck * 34.0).sin() * *struck * 0.5;
    for mut bell in &mut bells {
        bell.rotation = Quat::from_rotation_z(swing);
    }
}

/// `DIVUS_FACTUS_FRIGHT=<souls>` — hands that many villagers a fresh memory
/// of the teeth, once, a little after the world settles.
///
/// Fear is the slowest thing in the game to arrange on purpose. A mauling
/// needs a wolf to find somebody and win; a sighting needs a villager to walk
/// the three hundred meters out to a goblin camp. Neither happens inside a
/// soak, so every part of this — the bell, the card, the tower jumping the
/// queue, the muster — was unreachable without sitting and waiting on the
/// weather.
///
/// One soul is `Uneasy`, two is `Afraid`, five is `Besieged`, because a
/// fresh mauling weighs exactly one and `peril_of` sums over people.
pub(super) fn take_fright(
    clock: Res<crate::calendar::WorldClock>,
    mut done: Local<bool>,
    mut folk: Query<&mut crate::witness::Witnessed, With<crate::villager::Villager>>,
) {
    let Ok(souls) = std::env::var("DIVUS_FACTUS_FRIGHT") else {
        return;
    };
    // After the founding, so there are people to frighten.
    if *done || clock.elapsed < 20.0 {
        return;
    }
    *done = true;
    let souls: usize = souls.parse().unwrap_or(1);
    for mut held in folk.iter_mut().take(souls) {
        held.record(
            crate::witness::DivineEventKind::Mauled,
            None,
            false,
            clock.day(),
            crate::witness::SubjectClass::Person,
        );
    }
    info!("FRIGHT: {souls} souls remember the teeth");
}

/// Says where the alarm has got to, every few seconds, while
/// `DIVUS_FACTUS_FRIGHT` is set.
///
/// The chain has four links — the planner writing the fear down, a post in
/// the square, somebody eligible to run, and the run itself — and a soak that
/// simply never rings tells you nothing about which one is missing.
pub(super) fn probe_the_alarm(
    time: Res<Time>,
    mut since: Local<f32>,
    towns: Query<(
        Entity,
        &crate::villager::Settlement,
        Option<&Peril>,
        Option<&Rung>,
    )>,
    posts: Query<&crate::villager::MemberOf, With<BellPost>>,
    runners: Query<(&Transform, &RunningForTheBell)>,
    folk: Query<
        (&crate::villager::MemberOf, &crate::witness::Witnessed),
        With<crate::villager::Villager>,
    >,
    clock: Res<crate::calendar::WorldClock>,
) {
    if std::env::var("DIVUS_FACTUS_FRIGHT").is_err() {
        return;
    }
    *since += time.delta_secs();
    if *since < 10.0 {
        return;
    }
    *since = 0.0;
    let today = clock.day();
    for (town, settlement, peril, rung) in &towns {
        let carrying = folk
            .iter()
            .filter(|(member, held)| member.0 == town && held.peril(today) > 0.0)
            .count();
        let running = runners
            .iter()
            .map(|(at, errand)| at.translation.distance(errand.post))
            .fold(f32::INFINITY, f32::min);
        info!(
            "ALARM {}: peril={:?} alarm={:?} rung={:?} posts={} carrying={} nearest_runner={:.1}",
            settlement.name,
            peril.map(|p| p.0),
            peril.map(|p| Alarm::of(p.0)),
            rung.map(|r| (r.at, r.worst, r.last_peal)),
            posts.iter().filter(|member| member.0 == town).count(),
            carrying,
            running
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    /// The words on the card have to name the places the village actually
    /// changes behavior, or they are decoration. These are the same numbers
    /// the civic ladder and the muster read.
    #[test]
    fn the_words_name_the_thresholds() {
        assert_eq!(Alarm::of(0.0), Alarm::AtEase);
        // A fortnight-old memory on its last day is not a fright.
        assert_eq!(Alarm::of(0.01), Alarm::AtEase);
        // One survivor: somebody walks the treeline now.
        assert_eq!(Alarm::of(0.6), Alarm::Uneasy);
        // The tower stops waiting its turn.
        assert_eq!(
            Alarm::of(crate::witness::TOWER_JUMPS_THE_QUEUE),
            Alarm::Afraid
        );
        // The armory becomes worth wanting.
        assert_eq!(Alarm::of(crate::witness::ARMORY_IS_WANTED), Alarm::Besieged);
    }

    /// The bell is for the village agreeing something is out there, not for
    /// one person coming home frightened.
    #[test]
    fn one_frightened_soul_does_not_ring_the_bell() {
        assert!(!Alarm::Uneasy.worth_the_bell());
        assert!(!Alarm::AtEase.worth_the_bell());
        assert!(Alarm::Afraid.worth_the_bell());
        assert!(Alarm::Besieged.worth_the_bell());
    }

    /// Ordering is what makes "more frightened than it was" mean anything,
    /// and it is the whole guard against the bell tolling as fear fades.
    #[test]
    fn fear_is_ordered_so_falling_is_silent() {
        assert!(Alarm::AtEase < Alarm::Uneasy);
        assert!(Alarm::Uneasy < Alarm::Afraid);
        assert!(Alarm::Afraid < Alarm::Besieged);
    }

    /// The whole errand, end to end: a fright, somebody who saw it setting
    /// off, and no peal at all until they reach the post. This is the part
    /// Brett asked for by name - "It shouldnt just auto ring" - so the test
    /// asserts the SILENCE while the runner is still crossing the field as
    /// hard as it asserts the peal at the end.
    #[test]
    fn somebody_has_to_reach_the_rope() {
        let mut app = App::new();
        app.init_resource::<crate::debug::timings::Timings>();
        app.init_resource::<crate::calendar::WorldClock>();
        app.add_message::<crate::ui::Notice>();
        app.add_message::<crate::sfx::PlaySfx>();

        let town = app
            .world_mut()
            .spawn((
                crate::villager::Settlement {
                    name: "Ashfen".into(),
                    founded: 1,
                    banner_ramp: 0,
                    sigil: 0,
                },
                Peril(0.0),
            ))
            .id();
        let post = Vec3::new(5.0, 0.0, 0.0);
        app.world_mut().spawn((
            BellPost,
            crate::villager::MemberOf(town),
            Transform::from_translation(post),
        ));

        // One villager, out in the fields, who has seen the teeth. Far
        // enough from the post that the run is a real distance.
        let mut held = crate::witness::Witnessed::default();
        held.record(
            crate::witness::DivineEventKind::Mauled,
            None,
            false,
            0,
            crate::witness::SubjectClass::Person,
        );
        let soul = app
            .world_mut()
            .spawn((
                crate::villager::Villager,
                crate::villager::MemberOf(town),
                Transform::from_translation(Vec3::new(-30.0, 0.0, 0.0)),
                held,
                crate::villager::Activity::Working,
                crate::villager::MoveTarget(None),
            ))
            .id();

        let tick = |app: &mut App| {
            let _ = app.world_mut().run_system_once(somebody_runs_for_the_bell);
            let _ = app.world_mut().run_system_once(ring_the_bell);
            let mut peals = 0;
            let mut cursor = app
                .world_mut()
                .resource_mut::<Messages<crate::sfx::PlaySfx>>();
            for message in cursor.drain() {
                if message.kind == crate::sfx::SfxKind::Alarm {
                    peals += 1;
                }
            }
            peals
        };

        // Calm: nobody is going anywhere.
        assert_eq!(tick(&mut app), 0);
        assert!(app.world().get::<RunningForTheBell>(soul).is_none());

        // The village becomes afraid. Somebody sets off - and the bell does
        // NOT ring, because they are thirty meters out in a field.
        *app.world_mut().get_mut::<Peril>(town).unwrap() = Peril(2.0);
        assert_eq!(tick(&mut app), 0, "the fright alone must not ring the bell");
        let errand = app
            .world()
            .get::<RunningForTheBell>(soul)
            .expect("the one who saw it should have set off");
        assert_eq!(errand.about, Alarm::Afraid);
        assert_eq!(
            *app.world().get::<crate::villager::Activity>(soul).unwrap(),
            crate::villager::Activity::Alarming,
            "and they should be running, not working"
        );
        assert_eq!(
            app.world()
                .get::<crate::villager::MoveTarget>(soul)
                .unwrap()
                .0,
            Some(post),
            "with the post as their destination"
        );

        // Still crossing the field. Still silent, however many frames pass.
        assert_eq!(tick(&mut app), 0);
        assert_eq!(tick(&mut app), 0);

        // They arrive.
        app.world_mut()
            .get_mut::<Transform>(soul)
            .unwrap()
            .translation = post;
        assert_eq!(tick(&mut app), 1, "reaching the rope rings the bell");
        assert!(
            app.world().get::<RunningForTheBell>(soul).is_none(),
            "and the errand is done with"
        );

        // And it is the peal, not the fear, that is remembered - so the same
        // fright does not send a second runner.
        let rung = app.world().get::<Rung>(town).unwrap();
        assert_eq!(rung.worst, Alarm::Afraid);
        // Day one: the world's first day is 1, not 0.
        assert_eq!(rung.last_peal, Some(1));
        assert_eq!(tick(&mut app), 0);
    }

    /// A village nobody can run in. The bell is a place, so a town whose
    /// only witness is dead, held in the god's hand or simply absent stays
    /// silent - and must try again rather than write the fright off.
    #[test]
    fn a_town_with_nobody_to_send_stays_silent() {
        let mut app = App::new();
        app.init_resource::<crate::debug::timings::Timings>();
        app.init_resource::<crate::calendar::WorldClock>();
        app.add_message::<crate::ui::Notice>();
        app.add_message::<crate::sfx::PlaySfx>();

        let town = app
            .world_mut()
            .spawn((
                crate::villager::Settlement {
                    name: "Ashfen".into(),
                    founded: 1,
                    banner_ramp: 0,
                    sigil: 0,
                },
                Peril(6.0),
            ))
            .id();
        app.world_mut().spawn((
            BellPost,
            crate::villager::MemberOf(town),
            Transform::default(),
        ));

        let _ = app.world_mut().run_system_once(somebody_runs_for_the_bell);
        assert!(
            app.world().get::<Rung>(town).is_none(),
            "the fright is not written off just because nobody could go"
        );
    }
}
