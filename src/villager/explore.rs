//! The known world, and the people who push its edge.
//!
//! Ordinary villagers live inside a boundary of knowledge: everything the
//! village has stood next to and come home from. The god sees the whole
//! world; the people see this circle — and they mark its edge themselves,
//! with cairns of stacked waystones, so the fog's border is a built thing
//! in the world and not a line on a map.
//!
//! Explorers are the ones who walk past the cairns. An expedition goes out
//! alone (guards come in the next milestone), stands in the unknown long
//! enough to read the land, and comes home with a discovery: a grove, an
//! ore slope, a berry heath, or nothing but wind. What they find becomes a
//! known pocket the working trades can use — and a tale that runs through
//! the gossip mill like any miracle.

use bevy::prelude::*;

use crate::creature::{Airborne, Corpse, Held, MoveTarget};
use crate::scatter::FellableTree;
use crate::terrain::{Terrain, WATER_LEVEL};

use super::{Activity, Chronicle, Person, SettlementSite, Villager, work};

/// How far past the cairns an expedition pushes.
const STRIDE: f32 = 70.0;

/// One place the village knows beyond its home circle.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Pocket {
    pub at: Vec3,
    pub radius: f32,
}

/// Everything the village knows of the world: the home circle, and the
/// pockets its explorers have brought back.
#[derive(Resource, serde::Serialize, serde::Deserialize)]
pub struct KnownWorld {
    pub centre: Vec3,
    pub radius: f32,
    pub pockets: Vec<Pocket>,
}

impl Default for KnownWorld {
    fn default() -> Self {
        KnownWorld {
            centre: Vec3::ZERO,
            radius: 170.0,
            pockets: Vec::new(),
        }
    }
}

impl KnownWorld {
    /// Whether the village knows this ground.
    pub fn knows(&self, at: Vec3) -> bool {
        if at.distance(self.centre) < self.radius {
            return true;
        }
        self.pockets
            .iter()
            .any(|pocket| at.distance(pocket.at) < pocket.radius)
    }
}

/// A waystone stack marking the edge of the known world.
#[derive(Component)]
pub struct Cairn;

/// An expedition in progress.
#[derive(Component)]
pub struct Expedition {
    target: Vec3,
    surveying: f32,
    homeward: bool,
}

/// A guard walking with someone whose road runs past the cairns.
/// Company is armour: the wolves do not test a pair.
#[derive(Component)]
pub struct Escorting {
    pub ward: Entity,
}

/// Idle explorers walk out past the cairns, read the land, and come home
/// with what they found.
#[allow(clippy::type_complexity)]
pub(super) fn expeditions(
    mut commands: Commands,
    time: Res<Time>,
    clock: Res<crate::calendar::WorldClock>,
    terrain: Option<Res<Terrain>>,
    site: Option<Res<SettlementSite>>,
    homes: (Query<&super::MemberOf>, Query<&super::SettlementGround>),
    // Bundled with a spare slot's worth of company: this system sits at
    // Bevy's parameter ceiling.
    mut known: ResMut<KnownWorld>,
    weather: Option<Res<crate::weather::Weather>>,
    mut rng: ResMut<super::SimRng>,
    mut notices: MessageWriter<crate::ui::Notice>,
    mut say: (
        Option<ResMut<crate::telling::Tongue>>,
        Option<Res<crate::attention::Attention>>,
    ),
    stores: Query<&crate::villager::work::Stockpile>,
    trees: Query<(&GlobalTransform, &FellableTree)>,
    bushes: Query<(&GlobalTransform, &crate::scatter::FoodSource)>,
    deposits: Query<(&GlobalTransform, &crate::matter::Deposit)>,
    mut explorers: Query<
        (
            Entity,
            &Transform,
            &work::Vocation,
            &Person,
            &mut Activity,
            &mut MoveTarget,
            Option<&mut Expedition>,
            Option<&mut Chronicle>,
        ),
        (
            With<Villager>,
            Without<Corpse>,
            Without<Held>,
            Without<Airborne>,
        ),
    >,
) {
    let (Some(terrain), Some(site)) = (terrain, site) else {
        return;
    };
    let (members, grounds) = homes;
    let dt = time.delta_secs();

    // What the village wants and cannot reach is what sends people out.
    // Scarcity is the engine of the map: a full woodpile keeps everyone
    // home, an empty one with no known tree left to fell puts someone on
    // the road. The two wants that kill are timber and food.
    let timber_short = stores.iter().all(|s| s.timber < 8.0) || stores.iter().next().is_none();
    let wood_known = trees
        .iter()
        .any(|(at, tree)| tree.harvestable() && known.knows(at.translation()));
    let wood_want = timber_short && !wood_known;
    let food_short = stores.iter().all(|s| s.food() < 10.0) || stores.iter().next().is_none();
    let berries_known = bushes
        .iter()
        .any(|(at, bush)| bush.amount > 0.5 && known.knows(at.translation()));
    let food_want = food_short && !berries_known;
    // Hungry villages muster expeditions in earnest; content ones only
    // when wanderlust strikes.
    let urgency = if wood_want || food_want { 0.02 } else { 0.002 };

    // Idle guards, ready to fall in beside whoever sets out.
    let mut guard_pool: Vec<(Entity, Vec3)> = explorers
        .iter()
        .filter(|(_, _, vocation, _, activity, ..)| {
            **vocation == work::Vocation::Guard
                && matches!(**activity, Activity::Idle | Activity::Wandering)
        })
        .map(|(guard, at, ..)| (guard, at.translation))
        .collect();

    for (entity, at, vocation, person, mut activity, mut target, expedition, chronicle) in
        &mut explorers
    {
        if *vocation != work::Vocation::Explorer {
            continue;
        }

        // An expedition underway runs to its end, whatever the hour.
        if let Some(mut expedition) = expedition {
            if *activity != Activity::Working {
                // Hunger or sleep broke the journey; they will set out again.
                commands.entity(entity).remove::<Expedition>();
                continue;
            }
            if expedition.homeward {
                let square = members
                    .get(entity)
                    .ok()
                    .and_then(|member| grounds.get(member.0).ok())
                    .map_or(site.centre, |ground| ground.centre);
                if at.translation.distance(square) > 6.0 {
                    target.0 = Some(square);
                } else {
                    commands.entity(entity).remove::<Expedition>();
                    *activity = Activity::Idle;
                    target.0 = None;
                }
                continue;
            }
            if at.translation.distance(expedition.target) > 3.0 {
                target.0 = Some(expedition.target);
                continue;
            }
            // Standing in the unknown, reading the land.
            target.0 = None;
            expedition.surveying += dt;
            if expedition.surveying < 6.0 {
                continue;
            }

            // What is actually here decides what is found.
            let spot = expedition.target;
            let near_trees = trees
                .iter()
                .filter(|(t, _)| t.translation().distance(spot) < 45.0)
                .count();
            let near_bushes = bushes
                .iter()
                .filter(|(b, _)| b.translation().distance(spot) < 45.0)
                .count();
            let high_ground = terrain.height_at(spot.x, spot.z) > WATER_LEVEL + 12.0;
            // A deposit within sight of the survey is the find of a
            // lifetime: rarer than any wood, and named accordingly.
            let near_deposit = deposits
                .iter()
                .filter(|(at, deposit)| {
                    deposit.amount > 0.5 && at.translation().distance(spot) < 45.0
                })
                .map(|(_, deposit)| deposit.kind)
                .next();
            let (what, radius) = if let Some(kind) = near_deposit {
                match kind {
                    crate::matter::DepositKind::Iron => ("a hillside veined with iron", 45.0),
                    crate::matter::DepositKind::Clay => ("a bank of good red clay", 40.0),
                }
            } else if near_trees >= 6 {
                ("a green wood past the cairns", 55.0)
            } else if near_bushes >= 3 {
                ("a heath heavy with berries", 45.0)
            } else if high_ground {
                ("a slope of good bare stone", 45.0)
            } else {
                ("nothing but wind out there", 0.0)
            };

            if radius > 0.0 {
                known.pockets.push(Pocket { at: spot, radius });
                notices.write(crate::ui::Notice::fanfare(format!(
                    "{} found {}",
                    person.name, what
                )));
            }
            // Every return stretches the cairn ring a little: even a walk
            // that found nothing proves the ground between.
            known.radius += 9.0;
            // A watched homecoming is told in the explorer's own words.
            let composed = say
                .0
                .as_mut()
                .filter(|_| {
                    crate::attention::regard(say.1.as_deref(), at.translation).worth_composing()
                })
                .map(|tongue| {
                    tongue.muse(crate::telling::Musing {
                        who: entity,
                        voice: Some(crate::villager::work::Vocation::Explorer),
                        bearing: crate::villager::traits::Bearing::Plain,
                        faith: crate::telling::FaithBand::Wavering,
                        body: Vec::new(),
                        place: Vec::new(),
                        mind: format!("you walked far past the cairns and found {what}"),
                        heard: None,
                        aloud: true,
                        known: Vec::new(),
                    })
                })
                .is_some();
            // Unwatched or unanswered: the moment passes quietly. Nothing
            // written plays anywhere any more.
            let _ = composed;
            if let Some(mut chronicle) = chronicle {
                chronicle.record(
                    clock.day(),
                    format!("walked past the cairns and found {what}"),
                );
            }
            info!("{} explored and found {}", person.name, what);
            expedition.homeward = true;
            continue;
        }

        // A fresh expedition musters only in working hours, from the idle.
        if !matches!(*activity, Activity::Idle | Activity::Wandering) {
            continue;
        }
        if !work::is_work_hour(clock.time_of_day()) || !rng.0.chance(urgency) {
            continue;
        }
        // Nobody walks past the cairns into a storm.
        if weather
            .as_ref()
            .is_some_and(|w| w.kind() == crate::weather::WeatherKind::Storm)
        {
            continue;
        }
        // A want the village cannot meet aims the walk: a forest shows on
        // the horizon long before anyone has stood under it, so a village
        // short of wood heads for the nearest green it can see and does
        // not yet know — likewise berries. Only the contented wander at
        // random.
        let mut found = None;
        if wood_want {
            found = trees
                .iter()
                .filter(|(at, tree)| tree.harvestable() && !known.knows(at.translation()))
                .map(|(at, _)| at.translation())
                .min_by(|a, b| {
                    a.distance(known.centre)
                        .total_cmp(&b.distance(known.centre))
                })
                .map(|at| Vec3::new(at.x, terrain.height_at(at.x, at.z), at.z));
        }
        if found.is_none() && food_want {
            found = bushes
                .iter()
                .filter(|(at, bush)| bush.amount > 0.5 && !known.knows(at.translation()))
                .map(|(at, _)| at.translation())
                .min_by(|a, b| {
                    a.distance(known.centre)
                        .total_cmp(&b.distance(known.centre))
                })
                .map(|at| Vec3::new(at.x, terrain.height_at(at.x, at.z), at.z));
        }
        // Otherwise, a frontier point: out past the edge, on walkable ground.
        for _ in 0..24 {
            if found.is_some() {
                break;
            }
            let angle = rng.0.range(0.0, std::f32::consts::TAU);
            let reach = known.radius + rng.0.range(STRIDE * 0.4, STRIDE);
            let (sin, cos) = angle.sin_cos();
            let x = known.centre.x + cos * reach;
            let z = known.centre.z + sin * reach;
            if terrain.is_walkable(x, z) && terrain.height_at(x, z) > WATER_LEVEL + 1.5 {
                found = Some(Vec3::new(x, terrain.height_at(x, z), z));
                break;
            }
        }
        let Some(frontier) = found else {
            continue;
        };
        *activity = Activity::Working;
        commands.entity(entity).insert(Expedition {
            target: frontier,
            surveying: 0.0,
            homeward: false,
        });
        target.0 = Some(frontier);
        // A guard falls in if one is free: nobody should walk past the
        // cairns alone while the village can spare a spear.
        if let Some(index) = guard_pool
            .iter()
            .enumerate()
            .filter(|(_, (_, spot))| spot.distance(at.translation) < 60.0)
            .min_by(|a, b| {
                a.1.1
                    .distance(at.translation)
                    .total_cmp(&b.1.1.distance(at.translation))
            })
            .map(|(index, _)| index)
        {
            let (guard, _) = guard_pool.swap_remove(index);
            commands
                .entity(guard)
                .insert((Escorting { ward: entity }, Activity::Working));
            info!("{} walks out with an escort", person.name);
        }
    }
}

/// The escort's whole job: stay at the ward's shoulder until the road
/// brings them both home, then stand down.
#[allow(clippy::type_complexity)]
pub(super) fn escort_duty(
    mut commands: Commands,
    wards: Query<(&Transform, Has<Expedition>), With<Villager>>,
    mut escorts: Query<
        (Entity, &Escorting, &mut MoveTarget, &mut Activity),
        (With<Villager>, Without<Corpse>, Without<Held>),
    >,
) {
    for (guard, escorting, mut target, mut activity) in &mut escorts {
        let stand_down = match wards.get(escorting.ward) {
            Ok((_, on_expedition)) => !on_expedition,
            Err(_) => true,
        };
        if stand_down {
            commands.entity(guard).remove::<Escorting>();
            *activity = Activity::Idle;
            target.0 = None;
            continue;
        }
        if let Ok((ward_at, _)) = wards.get(escorting.ward) {
            target.0 = Some(ward_at.translation - Vec3::new(1.4, 0.0, 1.1));
        }
    }
}

/// Raises the waystone cairns along the edge of the known world, and again
/// each time the edge moves.
pub(super) fn raise_cairns(
    mut commands: Commands,
    known: Res<KnownWorld>,
    terrain: Option<Res<Terrain>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    old: Query<Entity, With<Cairn>>,
) {
    if !known.is_changed() {
        return;
    }
    let Some(terrain) = terrain else {
        return;
    };
    for cairn in &old {
        commands.entity(cairn).despawn();
    }

    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let stone = materials.add(StandardMaterial {
        base_color: crate::palette::shade(&crate::palette::STONE, 0.6),
        perceptual_roughness: 1.0,
        ..default()
    });

    let count = ((std::f32::consts::TAU * known.radius) / 36.0)
        .floor()
        .max(8.0) as u32;
    for i in 0..count {
        let angle = i as f32 / count as f32 * std::f32::consts::TAU;
        let (sin, cos) = angle.sin_cos();
        let x = known.centre.x + cos * known.radius;
        let z = known.centre.z + sin * known.radius;
        if !terrain.is_walkable(x, z) || terrain.height_at(x, z) < WATER_LEVEL + 1.0 {
            continue;
        }
        let at = Vec3::new(x, terrain.height_at(x, z), z);
        let cairn = commands
            .spawn((
                Cairn,
                Name::new("A waystone cairn"),
                Transform::from_translation(at).with_rotation(Quat::from_rotation_y(angle)),
                Visibility::default(),
                crate::hand::Rooted,
            ))
            .id();
        // Three stones, biggest at the bottom, each turned a little.
        for (level, (size, yaw)) in [(0.5_f32, 0.0_f32), (0.36, 0.5), (0.24, 1.1)]
            .into_iter()
            .enumerate()
        {
            commands.spawn((
                Mesh3d(cube.clone()),
                MeshMaterial3d(stone.clone()),
                Transform::from_xyz(0.0, 0.18 + level as f32 * 0.3, 0.0)
                    .with_rotation(Quat::from_rotation_y(yaw))
                    .with_scale(Vec3::new(size, 0.3, size * 0.9)),
                ChildOf(cairn),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_world_is_known_in_circles() {
        let mut known = KnownWorld {
            centre: Vec3::ZERO,
            radius: 100.0,
            pockets: Vec::new(),
        };
        assert!(known.knows(Vec3::new(50.0, 0.0, 0.0)));
        assert!(!known.knows(Vec3::new(300.0, 0.0, 0.0)));
        known.pockets.push(Pocket {
            at: Vec3::new(300.0, 0.0, 0.0),
            radius: 40.0,
        });
        assert!(known.knows(Vec3::new(310.0, 0.0, 20.0)));
        assert!(
            !known.knows(Vec3::new(200.0, 0.0, 0.0)),
            "the ground between home and a pocket stays unknown",
        );
    }
}
