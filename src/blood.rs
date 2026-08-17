//! Blood: what a wound leaves in the air and on the ground.
//!
//! Brett: "When a person or a animal/goblin gets hurt they should spill blood
//! on the ground that particle effects off and stays on the ground and fades
//! over time (maybe a day)."
//!
//! Two halves, and they are different things. The SPRAY is a moment — a
//! handful of flecks thrown out and down, gone in under a second, which is
//! what makes a blow read as a blow rather than as a number going up. The
//! STAIN is the memory of it: a dark mark on the ground that outlives the
//! fight and fades over a day, so a player who walks the square the morning
//! after can see where it happened without having watched.
//!
//! The stain is the half worth having. This game is about watching what a
//! village does and reading it afterward; a patch of ground that says
//! somebody died here belongs to that.

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};

/// A fleck of blood in the air.
#[derive(Component)]
pub struct Droplet {
    velocity: Vec3,
    age: f32,
    life: f32,
}

/// A mark on the ground, fading.
#[derive(Component)]
pub struct Stain {
    /// When it was spilled, in world seconds.
    born: f64,
    /// How wide it was when fresh, so the drying is measured from the mark's
    /// own size rather than from whatever it has already shrunk to. Reading
    /// the current scale and multiplying every frame is how the first cut of
    /// this vanished inside a second.
    spread: f32,
}

/// Somebody was wounded, and where.
///
/// Announced rather than recomputed, because the fresh-wound signal already
/// existed and had exactly one consumer: `hurt_flashes` compares `harm`
/// against `last_harm` and then OVERWRITES `last_harm` in the same pass. A
/// second system reading the same two fields would see whatever was left
/// after whichever of them happened to run first — a bug that would show up
/// as blood that mostly works.
#[derive(Message, Debug, Clone, Copy)]
pub struct Wounded {
    /// Who took it. Nothing reads this yet — the blood only needs to know
    /// where — but a wound with no subject is not much of a message, and the
    /// regard and doctrine systems will both want to know who bled. Brett on
    /// exactly this kind of field: "its okay to ad a trait or something that
    /// we know may get used down the road."
    #[allow(dead_code)]
    pub who: Entity,
    pub at: Vec3,
    /// How hard, 0..1 of a whole life. A graze throws less than a killing
    /// blow.
    pub severity: f32,
}

/// How long a stain lasts. Brett asked for "maybe a day", and a day here is
/// [`crate::calendar::DAY_SECONDS`] — so a fight in the morning is still
/// faintly on the ground at dusk and gone by the next.
const A_STAIN_LASTS: f64 = crate::calendar::DAY_SECONDS as f64;

/// Flecks per wound, and how long one hangs in the air.
const FLECKS: usize = 9;
const A_FLECK_LASTS: f32 = 0.75;

/// How wide a stain spreads, before severity scales it.
const A_STAIN_SPREADS: f32 = 0.55;

/// Stains that may lie on the ground at once. Past this the oldest goes,
/// because a village at war for a season should not accumulate ten thousand
/// quads nobody is looking at.
const AT_MOST: usize = 220;

/// The blood color. One handle, made once — every wound in the world shares
/// it, and a material per stain would be a leak with a slow fuse.
#[derive(Resource)]
struct BloodStuff {
    fleck: Handle<StandardMaterial>,
    stain: Handle<StandardMaterial>,
    cube: Handle<Mesh>,
    /// The spatter shapes, dealt out by a wound's own position.
    puddles: Vec<Handle<Mesh>>,
}

pub struct BloodPlugin;

impl Plugin for BloodPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Wounded>()
            .add_systems(Update, (wounds_bleed, drops_fall, stains_fade).chain());
    }
}

fn stuff(
    commands: &mut Commands,
    existing: Option<&BloodStuff>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> Option<BloodStuff> {
    if existing.is_some() {
        return None;
    }
    // Dark and matte. Bright red reads as paint; blood on grass is nearly
    // brown and only looks red where it is thick.
    let deep = Color::srgb(0.34, 0.05, 0.06);
    let made = BloodStuff {
        fleck: materials.add(StandardMaterial {
            base_color: deep,
            perceptual_roughness: 1.0,
            ..default()
        }),
        stain: materials.add(StandardMaterial {
            base_color: deep.with_alpha(0.85),
            perceptual_roughness: 1.0,
            alpha_mode: AlphaMode::Blend,
            // It lies ON the ground, so it must not fight the ground for the
            // same depth. Without this a stain flickers in and out as the
            // camera moves, which is the classic look of two coplanar
            // surfaces arguing.
            depth_bias: 8.0,
            ..default()
        }),
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        puddles: (0..SPATTER_SHAPES)
            .map(|i| meshes.add(spatter(i as u64)))
            .collect(),
    };
    commands.insert_resource(BloodStuff {
        fleck: made.fleck.clone(),
        stain: made.stain.clone(),
        cube: made.cube.clone(),
        puddles: made.puddles.clone(),
    });
    Some(made)
}

/// How many spatter shapes are cut, once, and dealt out to every wound
/// afterward.
///
/// Six is plenty. A stain is also randomly turned and scaled where it is
/// spawned, so six shapes give far more than six distinct marks - and it
/// costs six meshes for the life of the program rather than one per death.
const SPATTER_SHAPES: usize = 6;

/// One spatter: a blot where the blood pooled, and the specks it threw.
///
/// THE FIRST CUT OF THIS WAS A SQUARE. One flat unit cube, scaled - which on
/// the ground reads as exactly what it is. Brett: "the blood doesn't look
/// like blood splatter, it looks like a red square, lol."
///
/// This world is made of boxes and the blood should be too, so the answer is
/// not a decal texture - it is MORE boxes, of uneven size and angle: a few
/// broad quads overlapping off-center for the body of it, then a scatter of
/// small ones thrown clear. All of it in one mesh, so a spatter still costs
/// a single draw.
///
/// The quads are stacked in tiny y increments because they are otherwise
/// coplanar and would argue over depth. Blending them over one another is
/// deliberate: where they overlap the mark is darker, which is what a thick
/// part of a puddle looks like.
fn spatter(seed: u64) -> Mesh {
    let mut rng = crate::rng::Rng::stream(seed, "spatter");
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    let mut quad = |center: Vec2, half: Vec2, yaw: f32, lift: f32| {
        let (sin, cos) = yaw.sin_cos();
        let turn = |x: f32, z: f32| Vec2::new(x * cos - z * sin, x * sin + z * cos);
        let base = positions.len() as u32;
        for (dx, dz) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            let at = center + turn(dx * half.x, dz * half.y);
            positions.push([at.x, lift, at.y]);
            normals.push([0.0, 1.0, 0.0]);
            uvs.push([(dx + 1.0) * 0.5, (dz + 1.0) * 0.5]);
        }
        indices.extend([base, base + 2, base + 1, base, base + 3, base + 2]);
    };

    // THE BODY. Overlapping and off-center, so no edge of it is straight and
    // the whole thing has no middle to speak of.
    let mut lift = 0.0;
    for _ in 0..4 {
        let off = Vec2::new(rng.range(-0.22, 0.22), rng.range(-0.22, 0.22));
        let half = Vec2::new(rng.range(0.26, 0.46), rng.range(0.24, 0.42));
        quad(off, half, rng.range(0.0, std::f32::consts::TAU), lift);
        lift += 0.004;
    }

    // WHAT IT THREW. Smaller the further out it went, and thrown to one side
    // rather than in a ring - blood comes off a body in a direction.
    let heading = rng.range(0.0, std::f32::consts::TAU);
    for _ in 0..rng.range_i(10, 17) {
        let out = rng.range(0.55, 2.1);
        let angle = heading + rng.gaussian() * 0.7;
        let (sin, cos) = angle.sin_cos();
        let at = Vec2::new(cos * out, sin * out);
        // Far specks are small, and none of them is square.
        let scale = (1.5 - out * 0.55).max(0.14);
        let half = Vec2::new(rng.range(0.05, 0.13) * scale, rng.range(0.04, 0.10) * scale);
        quad(at, half, rng.range(0.0, std::f32::consts::TAU), lift);
        lift += 0.004;
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::MAIN_WORLD | bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// A wound throws blood, and leaves a mark where it was taken.
fn wounds_bleed(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    terrain: Option<Res<crate::terrain::Terrain>>,
    mut wounds: MessageReader<Wounded>,
    kit: Option<Res<BloodStuff>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    standing: Query<&Stain>,
    watch: Res<crate::debug::timings::Timings>,
) {
    let _t = watch.watch("blood: wounds bleed");
    let made = stuff(&mut commands, kit.as_deref(), &mut meshes, &mut materials);
    let kit = match (kit.as_deref(), made.as_ref()) {
        (Some(kit), _) => kit,
        (None, Some(made)) => made,
        (None, None) => return,
    };
    let Some(terrain) = terrain else {
        // No ground to stain yet. Read the messages anyway so they do not
        // queue up and all land at once the moment a world exists.
        wounds.clear();
        return;
    };

    let mut room = AT_MOST.saturating_sub(standing.iter().count());
    for wound in wounds.read() {
        let at = wound.at;
        // THE SPRAY: out and down, in a fan. Cheap cubes, gone inside a
        // second - the point is the beat, not the fluid.
        let flecks = (FLECKS as f32 * (0.4 + wound.severity * 1.6)).round() as usize;
        for i in 0..flecks.min(FLECKS * 2) {
            // A fan rather than a sphere: golden-angle around, biased upward
            // and outward, so it reads as thrown from a body and not as an
            // explosion.
            let angle = i as f32 * 2.399_963;
            let (sin, cos) = angle.sin_cos();
            let out = 0.8 + (i % 3) as f32 * 0.6;
            let up = 1.4 + (i % 4) as f32 * 0.5;
            let size = 0.045 + (i % 3) as f32 * 0.018;
            commands.spawn((
                Droplet {
                    velocity: Vec3::new(cos * out, up, sin * out),
                    age: 0.0,
                    life: A_FLECK_LASTS * (0.7 + (i % 5) as f32 * 0.12),
                },
                Mesh3d(kit.cube.clone()),
                MeshMaterial3d(kit.fleck.clone()),
                // Thrown from the middle of a body, not from its feet.
                Transform::from_translation(at + Vec3::Y * 0.9).with_scale(Vec3::splat(size)),
                bevy::light::NotShadowCaster,
            ));
        }

        // THE MARK, at their feet on the actual ground. A slab rather than a
        // decal: this world is cubes, and a cube pressed flat is what
        // everything else here is made of.
        if room == 0 {
            continue;
        }
        room -= 1;
        let ground = terrain.height_at(at.x, at.z);
        let spread = A_STAIN_SPREADS * (0.55 + wound.severity * 1.4);
        commands.spawn((
            Stain {
                born: clock.elapsed,
                spread,
            },
            // WHICH shape, chosen off the ground it was spilled on, so the
            // same spot always bleeds the same way and a reloaded save does
            // not rearrange its dead.
            Mesh3d(
                kit.puddles[(crate::rng::hash_2d((at.x * 8.0) as i32, (at.z * 8.0) as i32, 0x1005)
                    as usize)
                    % kit.puddles.len()]
                .clone(),
            ),
            MeshMaterial3d(kit.stain.clone()),
            // FLAT, and seated by the globe. A stain spawned at a
            // `GlobalTransform` would sit on a sphere of radius six thousand
            // and nowhere near the body that bled.
            Transform::from_translation(Vec3::new(at.x, ground + 0.03, at.z))
                .with_rotation(Quat::from_rotation_y(at.x + at.z))
                // Evenly, now. The shape is already lopsided; squashing it on
                // one axis as well only made the square look like a rectangle.
                .with_scale(Vec3::splat(spread)),
            crate::globe::RigidlySeated,
            bevy::light::NotShadowCaster,
        ));
    }
}

/// How wide a stain is at `age`, 0 fresh to 1 gone.
///
/// It never dries to nothing: a mark that shrank to a point would blink out,
/// and the last thing a fading stain should do is call attention to itself.
/// It goes at 30% and is despawned by the clock, not by the curve.
fn dried(age: f32, spread: f32) -> f32 {
    spread * (0.3 + (1.0 - age.clamp(0.0, 1.0)) * 0.7)
}

/// The flecks fly, fall and are gone.
fn drops_fall(
    mut commands: Commands,
    time: Res<Time>,
    mut drops: Query<(Entity, &mut Droplet, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (fleck, mut drop, mut at) in &mut drops {
        drop.age += dt;
        if drop.age >= drop.life {
            commands.entity(fleck).despawn();
            continue;
        }
        drop.velocity.y -= 14.0 * dt;
        let velocity = drop.velocity;
        at.translation += velocity * dt;
    }
}

/// And the mark fades over a day.
fn stains_fade(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    mut stains: Query<(Entity, &Stain, &mut Transform)>,
) {
    for (mark, stain, mut at) in &mut stains {
        let age = (clock.elapsed - stain.born) / A_STAIN_LASTS;
        if age >= 1.0 {
            commands.entity(mark).despawn();
            continue;
        }
        // FADED BY SHRINKING, not by alpha. Every stain in the world shares
        // one material - that is what keeps a season of fighting from
        // leaking a thousand handles - so fading one by its color would fade
        // all of them at once. Drying inward reads as drying anyway.
        //
        // Measured from the ORIGINAL spread every frame, never from the
        // current scale: a multiply-by-a-fraction each frame is a shrink of
        // sixty percent per second, and the mark is a speck before anybody
        // looks at it.
        let shrink = dried(age as f32, stain.spread);
        at.scale = Vec3::splat(shrink);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stain outlives the fight and is gone the next day, which is what
    /// makes it worth having: the morning after tells you what happened.
    #[test]
    /// The mark is a SPATTER and not a square. This is pinned because the
    /// first cut of it really was one flat unit cube, and on the ground that
    /// reads as exactly what it is - Brett: "it looks like a red square, lol."
    #[test]
    fn a_stain_is_not_a_square() {
        for seed in 0..SPATTER_SHAPES as u64 {
            let mesh = spatter(seed);
            let Some(bevy::render::mesh::VertexAttributeValues::Float32x3(points)) =
                mesh.attribute(Mesh::ATTRIBUTE_POSITION)
            else {
                panic!("a spatter has no points");
            };

            // Many pieces, not one.
            assert!(
                points.len() >= 40,
                "spatter {seed} is only {} corners - too few to be anything but a slab",
                points.len()
            );

            // And the pieces are of different sizes. Four quads of one size
            // in a row would be a tiled square, which is a square.
            let mut areas: Vec<f32> = Vec::new();
            for quad in points.chunks(4) {
                let wide = (quad[1][0] - quad[0][0]).hypot(quad[1][2] - quad[0][2]);
                let deep = (quad[3][0] - quad[0][0]).hypot(quad[3][2] - quad[0][2]);
                areas.push(wide * deep);
            }
            let biggest = areas.iter().copied().fold(0.0f32, f32::max);
            let smallest = areas.iter().copied().fold(f32::MAX, f32::min);
            assert!(
                biggest > smallest * 8.0,
                "spatter {seed} is all one size ({smallest:.4} to {biggest:.4}) - \
                 a pool and the specks it threw should not measure the same"
            );

            // It reaches further than its own body: something was thrown.
            let furthest = points
                .iter()
                .map(|p| p[0].hypot(p[2]))
                .fold(0.0f32, f32::max);
            assert!(
                furthest > 0.8,
                "spatter {seed} never got past its own puddle ({furthest:.2})"
            );
        }
    }

    #[test]
    fn a_stain_dries_without_blinking_out() {
        let fresh = dried(0.0, 1.0);
        let old = dried(0.99, 1.0);
        assert_eq!(fresh, 1.0, "fresh blood is the size it was spilled at");
        assert!(old < fresh, "and it draws in as it dries");
        assert!(
            old > 0.25,
            "but never to a point - a mark that shrinks to nothing blinks, \
             and the last thing a fading stain should do is catch the eye"
        );
        // Past its day it is clamped rather than inverted, because the clock
        // and not the curve is what despawns it.
        assert_eq!(dried(4.0, 1.0), dried(1.0, 1.0));
    }
}

#[cfg(test)]
mod look {
    use super::*;

    /// Prints the shapes as a picture, for a human to judge. Ignored: this is
    /// a look at the thing, not a check on it.
    #[test]
    #[ignore]
    fn draw_the_spatter() {
        for seed in 0..2u64 {
            let mesh = spatter(seed);
            let Some(bevy::render::mesh::VertexAttributeValues::Float32x3(points)) =
                mesh.attribute(Mesh::ATTRIBUTE_POSITION)
            else {
                continue;
            };
            let mut grid = [[b' '; 73]; 37];
            // Walk each quad in its OWN space and transform out, so a
            // rotated quad draws rotated. Rasterizing its bounding box
            // instead drew every one of them axis-aligned, which made the
            // whole spatter look like the square it is not.
            for quad in points.chunks(4) {
                let o = Vec2::new(quad[0][0], quad[0][2]);
                let u = Vec2::new(quad[1][0], quad[1][2]) - o;
                let v = Vec2::new(quad[3][0], quad[3][2]) - o;
                let steps = ((u.length() + v.length()) * 90.0) as i32;
                for i in 0..=steps {
                    for j in 0..=steps {
                        let at = o + u * (i as f32 / steps as f32) + v * (j as f32 / steps as f32);
                        let col = ((at.x + 1.8) / 3.6 * 72.0).round() as i32;
                        let row = ((at.y + 1.8) / 3.6 * 36.0).round() as i32;
                        if (0..73).contains(&col) && (0..37).contains(&row) {
                            grid[row as usize][col as usize] = b'#';
                        }
                    }
                }
            }
            println!("--- spatter {seed} ---");
            for row in grid {
                println!("{}", String::from_utf8_lossy(&row).trim_end());
            }
        }
    }
}
