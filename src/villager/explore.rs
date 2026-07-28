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

/// Idle explorers walk out past the cairns, read the land, and come home
/// with what they found.
#[allow(clippy::type_complexity)]
pub(super) fn expeditions(
    mut commands: Commands,
    time: Res<Time>,
    clock: Res<crate::calendar::WorldClock>,
    terrain: Option<Res<Terrain>>,
    site: Option<Res<SettlementSite>>,
    mut known: ResMut<KnownWorld>,
    weather: Option<Res<crate::weather::Weather>>,
    mut rng: ResMut<super::SimRng>,
    mut notices: MessageWriter<crate::ui::Notice>,
    mut say: MessageWriter<crate::ui::Say>,
    trees: Query<&GlobalTransform, With<FellableTree>>,
    bushes: Query<&GlobalTransform, With<crate::scatter::FoodSource>>,
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
    let dt = time.delta_secs();

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
                if at.translation.distance(site.centre) > 6.0 {
                    target.0 = Some(site.centre);
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
                .filter(|t| t.translation().distance(spot) < 45.0)
                .count();
            let near_bushes = bushes
                .iter()
                .filter(|b| b.translation().distance(spot) < 45.0)
                .count();
            let high_ground = terrain.height_at(spot.x, spot.z) > WATER_LEVEL + 12.0;
            let (what, radius) = if near_trees >= 6 {
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
            say.write(crate::ui::Say {
                speaker: entity,
                text: what.to_string(),
                thought: false,
            });
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
        if !work::is_work_hour(clock.time_of_day()) || !rng.0.chance(0.002) {
            continue;
        }
        // Nobody walks past the cairns into a storm.
        if weather
            .as_ref()
            .is_some_and(|w| w.kind() == crate::weather::WeatherKind::Storm)
        {
            continue;
        }
        // Pick a frontier point: out past the edge, on walkable ground.
        let mut found = None;
        for _ in 0..24 {
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
