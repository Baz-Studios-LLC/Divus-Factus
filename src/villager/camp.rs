//! Camps: a night on the road, and the reason anywhere far is reachable at all.
//!
//! Everything a village does is bounded by the day. `work_hours` opens and
//! closes the muster, `night_routine` walks everyone to a bed, and
//! [`crate::villager::work::WORK_REACH`] is a hundred and seventy because a
//! person has to be home by dark. So the whole map past a half-day's walk was
//! unreachable however good the party — and worse, sleep did not merely
//! interrupt a journey, it CANCELLED one: `explore.rs` removed the `Expedition`
//! the moment its owner stopped Working, with the comment "Sleep broke the
//! journey; they will set out again." They set out again from the square, every
//! morning, forever.
//!
//! Brett: "We should add camps to expeditions for them to recoup at night…
//! They should make camp, set up a camp fire and a few tents. They should go
//! 'in to' the tents and sleep and break camp down in the morning."
//!
//! So a camp is a small ritual with four beats, and the last one matters as
//! much as the first: **it is struck in the morning and the ground is bare
//! again.** Asked whether a camp should persist as a staging post, Brett: "no,
//! it should not persist." Permanent outposts are a real feature with real
//! consequences — how a town's reach grows, where trade runs — and letting a
//! chain of them accrete out of a sleep mechanic would mean never designing
//! roads on purpose.

//! NOT [`crate::camp`], which is the goblin camps — the places goblins keep,
//! built as one batched mesh with a static light and everything deliberately
//! out of plumb. Two different things called a camp, in one codebase, and the
//! only defense is that each says so. This one is villagers, ephemeral, and
//! square: its tents have to be separate entities because each one carries
//! bedrolls and is lifted by the roof cutaway, which a batched mesh cannot be.

use bevy::prelude::*;

use super::home::{Bonfire, Flame, Firelight};
use super::work::buildings::Bed;
use super::{Activity, MoveTarget, Villager};
use crate::creature::Corpse;
use crate::palette as pal;

/// A night's camp: a fire, a few tents, and nothing by morning.
#[derive(Component, Debug)]
pub struct Camp {
    /// The day it went up, so it is struck on the NEXT dawn rather than
    /// flickering out of existence during the one it was pitched in.
    pub pitched: u32,
}

/// One tent. A shelter for the roof-lifting and bed-finding machinery's
/// purposes, and a thing that will not exist tomorrow for every other purpose.
///
/// A ROOT ENTITY that names its camp, rather than a child of it. The sleeping
/// machinery reads a shelter's `Transform` and applies a bed's local offset to
/// it, which is only the world position if the shelter itself stands in world
/// space - every house does, being a root. A tent parented to its fire would
/// have sent its sleepers to walk to a spot measured from the world origin.
#[derive(Component, Debug)]
pub struct Tent {
    pub camp: Entity,
}

/// Bedded down away from home.
///
/// Deliberately NOT a [`super::home::Home`]: a home is what the shelter census
/// counts, what the roofless tally is measured against, and what the planner
/// breaks ground over. A traveler with a tent has somewhere to sleep tonight
/// and no more claim on a roof than they had yesterday.
#[derive(Component, Debug)]
pub struct Camped {
    pub camp: Entity,
    pub tent: Entity,
    /// Which berth in the tent, so two campers do not lie in one bedroll.
    pub berth: u8,
}

/// How many bedrolls a tent holds. Two: a tent is a tent, not a longhouse, and
/// the count is what decides when another one goes up.
pub const TENT_HOLDS: u8 = 2;

/// How far from a fire a traveler will walk to join a camp already pitched
/// rather than start their own. A party arrives strung out along its own line
/// of march, so this has to be generous enough to gather stragglers and tight
/// enough that two genuinely separate parties do not share a hearth.
const CAMP_SHARE: f32 = 26.0;

/// How far from home a traveler has to be before making camp is the sensible
/// thing to do. Inside this they walk the rest of the way and sleep in a bed —
/// nobody pitches a tent in sight of their own roof.
const TOO_CLOSE_TO_CAMP: f32 = 60.0;

/// A tent's footprint and how high its ridge stands.
const TENT_HALF_W: f32 = 0.85;
const TENT_HALF_D: f32 = 1.35;
const TENT_RIDGE: f32 = 1.15;

/// Whether a traveler standing here should pitch for the night or press on to
/// a real bed.
///
/// Pure, because the rule is a judgement and not a mechanism: nobody puts up a
/// tent in sight of their own roof, and a party with no town to its name (a
/// colony band whose citizenship has not changed hands yet) always camps.
pub(crate) fn worth_making_camp(at: Vec3, square: Option<Vec3>) -> bool {
    square.is_none_or(|square| at.distance(square) >= TOO_CLOSE_TO_CAMP)
}

/// Where the `nth` tent of a camp stands, and which way it faces.
///
/// Pure and shared with the tests, because a ring is easy to get wrong in a
/// way that only shows up as two tents inside each other: the angle step has to
/// be incommensurate with a full turn or the fourth tent lands on the first.
pub(crate) fn tent_spot(fire: Vec3, nth: usize) -> (Vec3, f32) {
    let angle = nth as f32 * TENT_STEP + 0.6;
    let (sin, cos) = angle.sin_cos();
    (
        Vec3::new(fire.x + cos * TENT_REACH, fire.y, fire.z + sin * TENT_REACH),
        -angle,
    )
}

/// How far the tents stand off the fire, and how far round the ring each one
/// steps. The step is deliberately not a neat fraction of a turn.
const TENT_REACH: f32 = 3.1;
const TENT_STEP: f32 = 1.9;

/// Anyone whose road is longer than a day: the parties that go out.
///
/// A type alias because it is asked for in three places and the whole point is
/// that all of them mean the same thing by it.
type OnTheRoad = Or<(
    With<super::explore::Expedition>,
    With<super::colony::Colonist>,
    With<super::explore::Escorting>,
)>;

/// Dusk on the road: the party stops, lights a fire, and puts up tents.
///
/// One camp per CLUSTER rather than one per party, which is why there is no
/// party bookkeeping here at all. A colonist band, a surveyor and the guard
/// walking beside them are three different components with three different
/// reasons to be out there; what they have in common is that they are standing
/// near each other in the dark. Whoever gets here first pitches, everybody
/// within `CAMP_SHARE` joins, and tents go up as they are needed.
pub(super) fn pitch_camp(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    site: Option<Res<super::SettlementSite>>,
    grounds: Query<&super::SettlementGround>,
    terrain: Option<Res<crate::terrain::Terrain>>,
    camps: Query<(Entity, &Transform), With<Camp>>,
    tents: Query<(Entity, &Tent)>,
    bedded: Query<&Camped>,
    travelers: Query<
        (Entity, &Transform, Option<&super::MemberOf>),
        (
            With<Villager>,
            OnTheRoad,
            Without<Camped>,
            Without<Corpse>,
            Without<crate::creature::Held>,
            Without<crate::avatar::Ridden>,
        ),
    >,
) {
    if !clock.is_night() {
        return;
    }
    let Some(terrain) = terrain else {
        return;
    };

    // Where each camp stands, and how full each of its tents is. Gathered once:
    // the same answer serves every traveler in the dark, and a party of six all
    // asking at once is exactly the case this runs in.
    let mut pitched: Vec<(Entity, Vec3)> = camps.iter().map(|(c, at)| (c, at.translation)).collect();
    let mut lodging: Vec<(Entity, Entity, u8)> = tents
        .iter()
        .map(|(tent, canvas)| {
            let taken = bedded.iter().filter(|c| c.tent == tent).count() as u8;
            (canvas.camp, tent, taken)
        })
        .collect();

    for (who, at, member) in &travelers {
        // Near enough to sleep at home: walk the last of it. Home is a bed and
        // a roof and a fire somebody else is tending.
        let square = member
            .and_then(|m| grounds.get(m.0).ok())
            .map(|ground| ground.center)
            .or_else(|| site.as_ref().map(|s| s.center));
        if !worth_making_camp(at.translation, square) {
            continue;
        }

        // Somebody's fire already burning within reach: join it.
        let camp = pitched
            .iter()
            .filter(|(_, fire)| fire.distance(at.translation) < CAMP_SHARE)
            .min_by(|a, b| {
                a.1.distance(at.translation)
                    .total_cmp(&b.1.distance(at.translation))
            })
            .map(|(camp, _)| *camp);
        let camp = match camp {
            Some(camp) => camp,
            None => {
                let ground = Vec3::new(
                    at.translation.x,
                    terrain.height_at(at.translation.x, at.translation.z),
                    at.translation.z,
                );
                let camp = raise_the_camp(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    ground,
                    clock.day(),
                );
                pitched.push((camp, ground));
                camp
            }
        };

        // A bedroll in a tent that has one, or a new tent for the two of them.
        let berth = lodging
            .iter_mut()
            .find(|(owner, _, taken)| *owner == camp && *taken < TENT_HOLDS);
        let (tent, berth) = match berth {
            Some((_, tent, taken)) => {
                *taken += 1;
                (*tent, *taken - 1)
            }
            None => {
                // Ringed round the fire, one place further along for each tent
                // the camp already has - so a camp of six reads as a camp and
                // not as three tents in the same spot.
                let standing = lodging.iter().filter(|(owner, ..)| *owner == camp).count();
                let fire = pitched
                    .iter()
                    .find(|(owner, _)| *owner == camp)
                    .map_or(at.translation, |(_, fire)| *fire);
                let tent = raise_a_tent(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    &terrain,
                    camp,
                    fire,
                    standing,
                );
                lodging.push((camp, tent, 1));
                (tent, 0)
            }
        };
        commands.entity(who).insert(Camped { camp, tent, berth });
    }
}

/// Morning: the fire is out, the tents come down, and the road goes on.
pub(super) fn strike_camp(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    camps: Query<(Entity, &Camp)>,
    tents: Query<(Entity, &Tent)>,
    campers: Query<(Entity, &Camped)>,
    // Narrowed on purpose. It is only ever asked about entities that hold a
    // `Camped`, but an unfiltered mutable query over every Activity in the world
    // is a loaded gun sitting in a system that means to touch six people.
    mut roused: Query<(&mut Activity, &mut MoveTarget), With<Villager>>,
) {
    if clock.is_night() {
        return;
    }
    let mut struck = 0;
    for (camp, pitched) in &camps {
        // Pitched THIS morning rather than last night: the small hours after
        // midnight are the next day by the calendar, and a camp put up at one
        // in the morning must not be struck at two.
        if clock.day() == pitched.pitched && clock.time_of_day() < 0.03 {
            continue;
        }
        for (who, camped) in &campers {
            if camped.camp != camp {
                continue;
            }
            commands.entity(who).remove::<Camped>();
            // Out of the bedroll and back on their feet. The journey systems
            // take it from here; leaving them Sleeping means a party that
            // never walks again.
            commands
                .entity(who)
                .remove::<super::home::Abed>()
                .remove::<crate::creature::Airborne>();
            if let Ok((mut activity, mut target)) = roused.get_mut(who) {
                if *activity == Activity::Sleeping {
                    *activity = Activity::Idle;
                    target.0 = None;
                }
            }
        }
        // The canvas comes down with the fire. Tents are roots rather than
        // children of the camp - see `Tent` - so nothing takes them with it.
        for (tent, canvas) in &tents {
            if canvas.camp == camp {
                commands.entity(tent).despawn();
            }
        }
        commands.entity(camp).despawn();
        struck += 1;
    }
    if struck > 0 {
        info!("{struck} camp(s) struck at dawn");
    }
}

/// The fire in the middle of a camp: a ring of stones and a small blaze, lit
/// from the off because the party lit it.
///
/// Its own `Bonfire` and its own children, which is why `burn` had to learn to
/// ask a fire what belongs to it instead of asking the world what flames exist.
fn raise_the_camp(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    at: Vec3,
    day: u32,
) -> Entity {
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let stone = materials.add(StandardMaterial {
        base_color: pal::shade(&pal::STONE, 0.45),
        perceptual_roughness: 0.95,
        ..default()
    });
    let log = materials.add(StandardMaterial {
        base_color: pal::shade(&pal::WOOD, 0.3),
        perceptual_roughness: 0.95,
        ..default()
    });
    let flame_deep = materials.add(StandardMaterial {
        base_color: pal::shade(&pal::CLOTH_RED, 0.8),
        emissive: LinearRgba::from(pal::shade(&pal::CLOTH_RED, 0.8)) * 9.0,
        ..default()
    });
    let flame_bright = materials.add(StandardMaterial {
        base_color: pal::shade(&pal::CLOTH_GOLD, 0.95),
        emissive: LinearRgba::from(pal::shade(&pal::CLOTH_GOLD, 0.95)) * 14.0,
        ..default()
    });

    let camp = commands
        .spawn((
            Name::new("A camp on the road"),
            Camp { pitched: day },
            // Fuel enough for the night and no tending: a party feeds its own
            // fire, and there is no woodpile out here to fetch from. It goes
            // out when the camp comes down, which is the same thing.
            Bonfire {
                fuel: 3_000.0,
                tender: None,
            },
            Transform::from_translation(at),
            Visibility::default(),
            crate::hand::PickRadius(2.0),
            crate::hand::Rooted,
            // Seated as one piece, or a camp at a far latitude comes up as
            // cubism. See `globe::RigidlySeated`.
            crate::globe::RigidlySeated,
        ))
        .id();

    // A smaller ring than a village hearth: five stones, close in.
    for step in 0..5 {
        let angle = step as f32 / 5.0 * std::f32::consts::TAU;
        let (sin, cos) = angle.sin_cos();
        commands.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(stone.clone()),
            Transform::from_xyz(cos * 0.7, 0.09, sin * 0.7)
                .with_rotation(Quat::from_rotation_y(-angle))
                .with_scale(Vec3::new(0.34, 0.2, 0.22)),
            ChildOf(camp),
        ));
    }
    // Two sticks leaned together.
    for (yaw, pitch) in [(0.5, 0.5), (2.2, 0.45)] {
        commands.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(log.clone()),
            Transform::from_xyz(0.0, 0.2, 0.0)
                .with_rotation(Quat::from_rotation_y(yaw) * Quat::from_rotation_z(pitch))
                .with_scale(Vec3::new(0.9, 0.13, 0.13)),
            ChildOf(camp),
        ));
    }
    // The flame, and the light it throws. Both children of the camp, both
    // found by `burn` through the camp rather than through the world.
    commands.spawn((
        Flame,
        Mesh3d(cube.clone()),
        MeshMaterial3d(flame_deep),
        Transform::from_xyz(0.0, 0.42, 0.0).with_scale(Vec3::new(0.42, 0.6, 0.42)),
        bevy::light::NotShadowCaster,
        ChildOf(camp),
    ));
    commands.spawn((
        Flame,
        Mesh3d(cube),
        MeshMaterial3d(flame_bright),
        Transform::from_xyz(0.0, 0.52, 0.0).with_scale(Vec3::new(0.24, 0.48, 0.24)),
        bevy::light::NotShadowCaster,
        ChildOf(camp),
    ));
    commands.spawn((
        Firelight,
        PointLight {
            color: pal::shade(&pal::CLOTH_GOLD, 0.9),
            intensity: 0.0,
            range: 24.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(0.0, 1.0, 0.0),
        ChildOf(camp),
    ));
    info!("a party makes camp for the night");
    camp
}

/// One tent, pitched at the `nth` place in the ring around the fire.
///
/// A ridge tent: two canvas planes leaned together over two bedrolls, with the
/// bedrolls carrying [`Bed`] so the sleeping machinery needs to learn nothing
/// new. The roof cutaway lifts a `RoofPiece`, and canvas is a roof — so the god
/// can look into a camp the same way it looks into a house.
#[allow(clippy::too_many_arguments)]
fn raise_a_tent(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    terrain: &crate::terrain::Terrain,
    camp: Entity,
    fire: Vec3,
    nth: usize,
) -> Entity {
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let canvas = materials.add(StandardMaterial {
        base_color: pal::shade(&pal::SAND, 0.62),
        perceptual_roughness: 0.98,
        ..default()
    });
    let bedroll = materials.add(StandardMaterial {
        base_color: pal::shade(&pal::CLOTH_RUST, 0.5),
        perceptual_roughness: 0.98,
        ..default()
    });

    // Round the fire, and turned to face it - which is what makes a ring of
    // tents read as a camp rather than as tents that happen to be near a fire.
    let (spot, yaw) = tent_spot(fire, nth);
    // Standing on the ground it is pitched on, not at the fire's height - a
    // camp on a slope reads as tents floating otherwise.
    let at = Vec3::new(spot.x, terrain.height_at(spot.x, spot.z), spot.z);
    let tent = commands
        .spawn((
            Name::new("A tent"),
            Tent { camp },
            // Turned to face the fire, which is what makes a ring of tents read
            // as a camp rather than as tents that happen to be near one.
            Transform::from_translation(at).with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
            crate::hand::PickRadius(TENT_HALF_D),
            crate::hand::Rooted,
            crate::globe::RigidlySeated,
        ))
        .id();

    // The two canvas planes, leaned together into a ridge. Marked as roof, so
    // the god's cutaway takes them off and the sleepers are visible.
    for side in [-1.0_f32, 1.0] {
        commands.spawn((
            super::work::buildings::RoofPart,
            Mesh3d(cube.clone()),
            MeshMaterial3d(canvas.clone()),
            Transform::from_xyz(side * TENT_HALF_W * 0.5, TENT_RIDGE * 0.5, 0.0)
                .with_rotation(Quat::from_rotation_z(side * 0.62))
                .with_scale(Vec3::new(0.07, TENT_RIDGE * 1.5, TENT_HALF_D * 2.0)),
            ChildOf(tent),
        ));
    }
    // And the back wall, so it is a tent and not a lean-to open at both ends.
    commands.spawn((
        super::work::buildings::RoofPart,
        Mesh3d(cube.clone()),
        MeshMaterial3d(canvas),
        Transform::from_xyz(0.0, TENT_RIDGE * 0.4, -TENT_HALF_D)
            .with_scale(Vec3::new(TENT_HALF_W * 2.0, TENT_RIDGE * 0.8, 0.07)),
        ChildOf(tent),
    ));

    // The bedrolls: laid along the tent, heads to the closed end. `lie` is the
    // turn a body takes about the tent's own Y before it is tipped onto its
    // back - see `Bed::lie` for the quarter-turn that reads wrong until you
    // look at a sleeper lying across a mattress.
    for berth in 0..TENT_HOLDS {
        let across = (berth as f32 - (TENT_HOLDS as f32 - 1.0) * 0.5) * 0.62;
        commands.spawn((
            Bed {
                slot: berth,
                lie: std::f32::consts::FRAC_PI_2,
                double: false,
            },
            Mesh3d(cube.clone()),
            MeshMaterial3d(bedroll.clone()),
            Transform::from_xyz(across, 0.11, 0.0).with_scale(Vec3::new(
                0.5,
                0.16,
                TENT_HALF_D * 1.6,
            )),
            ChildOf(tent),
        ));
    }
    tent
}

#[cfg(test)]
mod tests {
    use super::*;

    /// NOBODY PITCHES A TENT IN SIGHT OF THEIR OWN ROOF. A bed, a hearth
    /// somebody else is tending and a larder beat canvas every time.
    #[test]
    fn a_traveler_near_home_walks_the_last_of_it() {
        let square = Vec3::new(100.0, 0.0, 100.0);
        assert!(
            !worth_making_camp(square + Vec3::X * 20.0, Some(square)),
            "twenty strides from the square is a walk, not a night on the road"
        );
        assert!(
            worth_making_camp(square + Vec3::X * 300.0, Some(square)),
            "three hundred strides out is a night on the road"
        );
    }

    /// A PARTY WITH NO TOWN STILL SLEEPS. A colony band's citizenship does not
    /// change hands until the banner goes up, and a walker whose town cannot be
    /// found must not be left standing in the dark on a technicality.
    #[test]
    fn a_party_with_nowhere_to_go_camps() {
        assert!(worth_making_camp(Vec3::new(9.0, 0.0, 9.0), None));
    }

    /// TENTS MUST NOT BE PITCHED INSIDE ONE ANOTHER, which is the one way a
    /// ring can quietly fail: step round by a neat fraction of a turn and the
    /// fourth tent lands on the first. Checked out to a party far larger than
    /// any that will ever walk.
    #[test]
    fn a_ring_of_tents_never_doubles_up() {
        let fire = Vec3::new(-40.0, 12.0, 88.0);
        let spots: Vec<Vec3> = (0..9).map(|nth| tent_spot(fire, nth).0).collect();
        for (i, a) in spots.iter().enumerate() {
            assert!(
                a.distance(fire) > TENT_HALF_D + 1.0,
                "tent {i} is pitched in the fire"
            );
            for (j, b) in spots.iter().enumerate().skip(i + 1) {
                let apart = a.distance(*b);
                assert!(
                    apart > TENT_HALF_W * 2.0,
                    "tents {i} and {j} are {apart:.2} apart, which is inside each other"
                );
            }
        }
    }

    /// THE WHOLE RITUAL, END TO END: a party far from home in the dark gets a
    /// fire, a tent and a bedroll, and by morning the ground is bare again.
    ///
    /// Worth an App test rather than unit checks on the pieces, because every
    /// bug this could have is a bug BETWEEN the pieces - a tent nobody sleeps
    /// in, a camp nobody strikes, canvas left standing after the fire is gone.
    /// Two soaks failed to produce a single camp naturally (no expedition ever
    /// fired), which is exactly why this cannot be left to observation.
    #[test]
    fn a_party_pitches_at_dusk_and_strikes_at_dawn() {
        use bevy::ecs::system::RunSystemOnce;

        let mut app = App::new();
        app.insert_resource(Time::<()>::default());
        app.insert_resource(crate::terrain::Terrain::new(77));
        app.init_resource::<Assets<Mesh>>();
        app.init_resource::<Assets<StandardMaterial>>();
        // Full dark. `is_night` is `>= 0.74`, and the day it is pitched has to
        // be the day `strike_camp` refuses to strike it on.
        app.insert_resource(crate::calendar::WorldClock {
            elapsed: (crate::calendar::DAY_SECONDS as f64) * 3.8,
        });

        // Three travelers standing together, a long way from anywhere. Given
        // `Escorting` because it is the one road-component with public fields;
        // `OnTheRoad` treats all three the same.
        let ward = app.world_mut().spawn_empty().id();
        let far = Vec3::new(900.0, 0.0, 900.0);
        let party: Vec<Entity> = (0..3)
            .map(|n| {
                app.world_mut()
                    .spawn((
                        Villager,
                        super::super::explore::Escorting { ward },
                        Transform::from_translation(far + Vec3::X * (n as f32 * 2.0)),
                        Activity::Idle,
                        MoveTarget(None),
                    ))
                    .id()
            })
            .collect();

        app.world_mut().run_system_once(pitch_camp).unwrap();

        // One fire between them, and two tents for three sleepers.
        let camps = app.world_mut().query::<&Camp>().iter(app.world()).count();
        assert_eq!(camps, 1, "three travelers standing together share one fire");
        let tents = app.world_mut().query::<&Tent>().iter(app.world()).count();
        assert_eq!(
            tents, 2,
            "three sleepers at {TENT_HOLDS} to a tent want two tents"
        );
        // Everybody has a bedroll, and no two share one.
        let mut berths: Vec<(Entity, u8)> = Vec::new();
        for who in &party {
            let camped = app
                .world()
                .get::<Camped>(*who)
                .expect("a traveler in the dark was left without a tent");
            assert!(
                !berths.contains(&(camped.tent, camped.berth)),
                "two campers were put in one bedroll"
            );
            berths.push((camped.tent, camped.berth));
        }
        // And the bedrolls are real furniture, so the sleeping machinery can
        // find them: a tent that promises two and holds none is a tent whose
        // campers stand outside it all night.
        let bedrolls = app
            .world_mut()
            .query::<&crate::villager::work::buildings::Bed>()
            .iter(app.world())
            .count();
        assert_eq!(bedrolls, tents * TENT_HOLDS as usize);

        // Morning.
        app.world_mut()
            .resource_mut::<crate::calendar::WorldClock>()
            .elapsed = (crate::calendar::DAY_SECONDS as f64) * 4.3;
        app.world_mut().run_system_once(strike_camp).unwrap();

        assert_eq!(
            app.world_mut().query::<&Camp>().iter(app.world()).count(),
            0,
            "the fire is out"
        );
        assert_eq!(
            app.world_mut().query::<&Tent>().iter(app.world()).count(),
            0,
            "and the canvas came down with it - tents are roots, so nothing \
             takes them along"
        );
        for who in &party {
            assert!(
                app.world().get::<Camped>(*who).is_none(),
                "somebody is still asleep in a tent that no longer exists"
            );
        }
    }

    /// A CAMP GATHERS A PARTY RATHER THAN A CROWD.
    ///
    /// `CAMP_SHARE` has to be wide enough to collect a band strung out along
    /// its own line of march and tight enough that it is not simply "everyone
    /// outdoors". The ring of tents must fit comfortably inside it, or a
    /// traveler standing beside a tent would start a second camp.
    #[test]
    fn a_camp_reaches_further_than_its_own_tents() {
        let fire = Vec3::ZERO;
        let furthest = (0..6)
            .map(|nth| tent_spot(fire, nth).0.distance(fire))
            .fold(0.0, f32::max);
        assert!(
            CAMP_SHARE > furthest * 2.0,
            "a camp reaches {CAMP_SHARE} and its own tents stand {furthest:.1} out"
        );
        assert!(
            CAMP_SHARE < TOO_CLOSE_TO_CAMP,
            "a camp that gathers from further than the walk home is not a camp"
        );
    }
}
