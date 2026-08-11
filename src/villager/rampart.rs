//! The wall around a town.
//!
//! A rampart is a RING: one circle about the banner, with gaps where its
//! gates stand. The search sees exactly that — see [`crate::navigation::Rampart`]
//! — and the art below draws posts along the same circle, skipping the
//! ground nothing could walk anyway. A lake is part of the defences and
//! costs nothing to be one.
//!
//! Three tiers, and the same ring carries all of them: a timber fence, a
//! stone wall, and one day a castle wall with towers. What changes between
//! them is the price, the look, and what can break through.
//!
//! Brett: "the fence should protect their town from hostile things and
//! make the town feel more like a protected town. Guards could patrol it
//! and stand watch at the gates."

use bevy::prelude::*;

use crate::navigation::{GATE_WIDTH, gate_arc};
use crate::terrain::Terrain;

/// What a town's wall is made of, which is also how good it is.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RampartTier {
    /// Posts and rails. Keeps wolves out, and will not keep them out for
    /// ever once they learn to lean on it.
    Fence,
    /// Coursed stone, chest high and thick.
    Wall,
    /// The full circuit, with towers and a gatehouse.
    Castle,
}

impl RampartTier {
    pub fn name(self) -> &'static str {
        match self {
            RampartTier::Fence => "the fence",
            RampartTier::Wall => "the wall",
            RampartTier::Castle => "the castle wall",
        }
    }

    /// How tall its posts or courses stand.
    fn height(self) -> f32 {
        match self {
            RampartTier::Fence => 1.9,
            RampartTier::Wall => 2.6,
            RampartTier::Castle => 4.2,
        }
    }

    /// How thick, across the line of the wall.
    fn thickness(self) -> f32 {
        match self {
            RampartTier::Fence => 0.22,
            RampartTier::Wall => 0.9,
            RampartTier::Castle => 1.6,
        }
    }

    /// How far apart its uprights stand. A fence is posts with air
    /// between them; a wall is a solid run.
    fn spacing(self) -> f32 {
        match self {
            RampartTier::Fence => 2.2,
            RampartTier::Wall => 1.5,
            RampartTier::Castle => 1.5,
        }
    }
}

/// A town's wall, as the town owns it. Sits on the settlement.
#[derive(Component, Debug, Clone)]
pub struct Rampart {
    pub tier: RampartTier,
    /// Measured from the banner.
    pub radius: f32,
    /// Where the gates stand, in radians about the banner.
    pub gates: Vec<f32>,
    /// Set when the posts have been stood, so they are raised once.
    pub standing: bool,
}

impl Rampart {
    /// How this wall looks to the pathfinder.
    pub fn as_barrier(&self, centre: Vec3) -> crate::navigation::Rampart {
        crate::navigation::Rampart {
            at: Vec2::new(centre.x, centre.z),
            radius: self.radius,
            gates: self.gates.clone(),
            gate_half: gate_arc(self.radius, GATE_WIDTH),
        }
    }
}

/// One upright of a standing wall - a post, a course, a merlon.
#[derive(Component)]
pub struct RampartPart;

/// Where a gate stands. Guards keep this, and the leaves hang here.
#[derive(Component)]
pub struct Gate {
    /// Which way is out, level with the ground. Read by the guard's post
    /// and the leaves, both of which want to know which side is the world
    /// and which side is home.
    #[allow(dead_code)]
    pub out: Vec3,
}

/// Where the wall should stand for a town of this size.
///
/// Outside the outermost building ring with a lane to spare, so the town
/// is walled rather than strangled - and with headroom, because a wall
/// that a season's growth puts buildings outside of is a wall that was
/// built too tight. Each tier is sized for the town it is meant to hold.
pub fn ring_for(tier: RampartTier, population: usize) -> f32 {
    // The building rings reach `14 + n * width`; the planner opens a new
    // ring for every six souls. Room for the town it will be, not the
    // town it is.
    let grown = match tier {
        RampartTier::Fence => population.max(20),
        RampartTier::Wall => population.max(45),
        RampartTier::Castle => population.max(90),
    };
    let rings = 5 + grown / 6;
    (14.0 + rings as f32 * 12.0 + 10.0).max(70.0)
}

/// Where the gates go: on the ways people actually walk.
///
/// Four for a fence, at the compass points, turned so no gate opens onto
/// the banner's own back. Roads are not modelled yet; when paths wear
/// into the ground the gates should be sited on the deepest of them,
/// which is the whole reason paths and walls belong in the same season of
/// work.
fn gates_for(tier: RampartTier) -> Vec<f32> {
    let count = match tier {
        RampartTier::Fence => 3,
        RampartTier::Wall => 4,
        RampartTier::Castle => 4,
    };
    let turn = std::f32::consts::FRAC_PI_4;
    (0..count)
        .map(|i| turn + i as f32 / count as f32 * std::f32::consts::TAU)
        .collect()
}

/// Raises the first fence once a town is big enough to want one.
///
/// A stand-in for the civic ladder while the tiers are being built: the
/// want, the timber price and the builders' hands come next. For now a
/// town of enough souls with timber to spare fences itself, so the ring
/// can be walked, watched and soaked. `DIVUS_FACTUS_FENCE=1` raises it on
/// the first morning instead, for the harness.
pub(crate) fn raise_the_fence(
    mut commands: Commands,
    mut towns: Query<(Entity, &super::Settlement, &super::work::Stockpile), Without<Rampart>>,
    folk: Query<&super::MemberOf, (With<super::Villager>, Without<crate::creature::Corpse>)>,
    mut notices: MessageWriter<crate::ui::Notice>,
) {
    let eager = std::env::var("DIVUS_FACTUS_FENCE").is_ok();
    for (town, settlement, store) in &mut towns {
        let souls = folk.iter().filter(|member| member.0 == town).count();
        if !eager && (souls < 14 || store.timber < 120.0) {
            continue;
        }
        let tier = RampartTier::Fence;
        commands.entity(town).insert(Rampart {
            tier,
            radius: ring_for(tier, souls),
            gates: gates_for(tier),
            standing: false,
        });
        notices.write(crate::ui::Notice::fanfare(format!(
            "{} has fenced itself in",
            settlement.name
        )));
    }
}

/// Stands the posts of a wall that has just been raised.
///
/// Skips the ground nothing can walk: where the ring crosses water or a
/// cliff there is nothing to defend, and a gap there is not a gap anyone
/// can use. Skips the gates as well, obviously - that is what a gate is.
#[allow(clippy::too_many_arguments)]
pub(crate) fn stand_the_posts(
    mut commands: Commands,
    terrain: Option<Res<Terrain>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    // What already stands on the line. A wall is raised around a town
    // that is already built, and a post driven through somebody's wall
    // is worse than a gap.
    built: Query<(&Transform, &super::work::Shell)>,
    mut towns: Query<(Entity, &super::SettlementGround, &mut Rampart)>,
) {
    let Some(terrain) = terrain else {
        return;
    };
    for (town, ground, mut wall) in &mut towns {
        if wall.standing {
            continue;
        }
        wall.standing = true;
        let tier = wall.tier;
        let gate_half = gate_arc(wall.radius, GATE_WIDTH);
        let post = meshes.add(Cuboid::new(
            tier.thickness(),
            tier.height(),
            tier.spacing() * 0.92,
        ));
        let timber = materials.add(StandardMaterial {
            base_color: match tier {
                RampartTier::Fence => crate::palette::shade(&crate::palette::WOOD, 0.34),
                _ => crate::palette::shade(&crate::palette::STONE, 0.5),
            },
            perceptual_roughness: 1.0,
            ..default()
        });

        let steps = ((std::f32::consts::TAU * wall.radius) / tier.spacing()).ceil() as usize;
        let mut stood = 0;
        for step in 0..steps {
            let angle = step as f32 / steps as f32 * std::f32::consts::TAU;
            // A gate is a gap, and so is the ground either side of one, so
            // the posts do not crowd the opening.
            if wall.gates.iter().any(|gate| {
                let mut apart = (angle - gate).abs() % std::f32::consts::TAU;
                if apart > std::f32::consts::PI {
                    apart = std::f32::consts::TAU - apart;
                }
                apart <= gate_half
            }) {
                continue;
            }
            let (sin, cos) = angle.sin_cos();
            let (x, z) = (
                ground.centre.x + cos * wall.radius,
                ground.centre.z + sin * wall.radius,
            );
            if !terrain.is_walkable(x, z) {
                continue;
            }
            // Nor through a roof somebody is living under. The ring is
            // sized to clear the town, but a town is not a perfect
            // circle and a house on the outermost lane can stand on the
            // line.
            let here = Vec3::new(x, 0.0, z);
            if built.iter().any(|(site, shell)| {
                let reach = shell.half_w.max(shell.half_d) + 1.2;
                site.translation.with_y(0.0).distance(here) < reach
            }) {
                continue;
            }
            let at = Vec3::new(x, terrain.height_at(x, z), z);
            commands.spawn((
                RampartPart,
                Name::new("A length of the wall"),
                Mesh3d(post.clone()),
                MeshMaterial3d(timber.clone()),
                // Sunk a little, so a post on a slope has no daylight
                // under it, and turned to face out of the ring.
                Transform::from_translation(at + Vec3::Y * (tier.height() * 0.5 - 0.25))
                    .with_rotation(Quat::from_rotation_y(-angle)),
                crate::globe::RigidlySeated,
                crate::villager::MemberOf(town),
            ));
            stood += 1;
        }

        for angle in wall.gates.clone() {
            let (sin, cos) = angle.sin_cos();
            let (x, z) = (
                ground.centre.x + cos * wall.radius,
                ground.centre.z + sin * wall.radius,
            );
            if !terrain.is_walkable(x, z) {
                continue;
            }
            let at = Vec3::new(x, terrain.height_at(x, z), z);
            let gate = commands
                .spawn((
                    Gate {
                        out: Vec3::new(cos, 0.0, sin),
                    },
                    Name::new("The gate"),
                    Transform::from_translation(at).with_rotation(Quat::from_rotation_y(-angle)),
                    Visibility::default(),
                    crate::globe::RigidlySeated,
                    crate::villager::MemberOf(town),
                ))
                .id();
            // Two leaves, hung on the gate's own posts and swinging apart
            // - the same machinery as a cottage door, at a cart's width.
            // The gate stands turned so its own +X faces out of the ring,
            // which is exactly what a doorway on the +X wall means, so
            // the door hanger needs no new arithmetic to understand it.
            let half = GATE_WIDTH * 0.5;
            for side in [-1.0_f32, 1.0] {
                super::work::buildings::hang_the_door(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    gate,
                    &super::work::buildings::Doorway {
                        at: Vec2::new(0.0, side * half * 0.5),
                        out: Vec2::X,
                    },
                    side,
                    half,
                    // The middle of the opening, so both leaves read the
                    // same traffic and swing as a pair.
                    Vec2::ZERO,
                    // Nothing drawn to adopt: a gate is ours to build.
                    &[],
                );
            }
            // And a post at each jamb, so the opening reads as a gate
            // rather than a hole where the fence forgot itself.
            for side in [-1.0_f32, 1.0] {
                commands.spawn((
                    RampartPart,
                    Name::new("A gatepost"),
                    Mesh3d(meshes.add(Cuboid::new(0.4, tier.height() + 0.9, 0.4))),
                    MeshMaterial3d(timber.clone()),
                    Transform::from_translation(
                        at + Vec3::new(-sin, 0.0, cos) * side * half
                            + Vec3::Y * ((tier.height() + 0.9) * 0.5 - 0.25),
                    ),
                    crate::globe::RigidlySeated,
                    crate::villager::MemberOf(town),
                ));
            }
        }
        info!(
            "{} went up around the town: {stood} lengths on a ring {:.0} strides out, {} gates",
            tier.name(),
            wall.radius,
            wall.gates.len(),
        );
    }
}
