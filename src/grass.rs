//! Grass: the ground cover that makes lowland read as *lush* rather than painted.
//!
//! The technique is the standard one behind every game with good grass, sized to
//! this codebase: blades are baked a few thousand at a time into one mesh per
//! chunk (the same trade that fixed the 186k-entity scenery), the wind lives
//! entirely in a vertex shader, and density falls to zero beyond a radius the
//! camera can actually resolve blades at.
//!
//! The material is an *extension* of `StandardMaterial`, overriding only the
//! vertex stage. Lighting, shadows and distance fog stay stock, so grass sits in
//! the world's light instead of floating over it.

use bevy::light::NotShadowCaster;
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

use crate::meshbuild;
use crate::palette;
use crate::rng::{Rng, hash_2d_f32};
use crate::terrain::{Biome, CHUNK_SIZE, Terrain, WATER_LEVEL, rivers};

/// How many chunks out from the camera focus grass exists.
///
/// Well inside the terrain stream: a blade at three hundred units is smaller than
/// a pixel, and generating it would be pure waste.
const GRASS_RADIUS: i32 = 5;

/// Chunks of grass built per frame.
const BUILDS_PER_FRAME: usize = 2;

/// Metres between blade sample points.
const SPACING: f32 = 0.85;

pub struct GrassPlugin;

impl Plugin for GrassPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<GrassMaterial>::default())
            .init_resource::<GrassChunks>()
            .add_systems(Startup, init_grass_material)
            .add_systems(
                Update,
                (drive_wind, stream_grass.after(crate::terrain::TerrainSet)),
            );
    }
}

pub type GrassMaterial = ExtendedMaterial<StandardMaterial, GrassExtension>;

/// The uniform block shared by the main-pass and prepass wind shaders.
#[derive(bevy::render::render_resource::ShaderType, Clone, Debug)]
pub struct GrassParams {
    /// xy: direction. z: speed. w: strength.
    pub wind: Vec4,
    /// x: time. Ticked by [`drive_wind`]; the prepass has no global clock.
    pub clock: Vec4,
}

/// The wind uniform handed to the vertex shaders.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct GrassExtension {
    #[uniform(100)]
    pub params: GrassParams,
}

impl MaterialExtension for GrassExtension {
    fn vertex_shader() -> ShaderRef {
        "shaders/grass.wgsl".into()
    }

    // The prepass must bend blades identically to the main pass, or every pixel
    // where the two disagree becomes a blade-shaped hole showing the sky.
    fn prepass_vertex_shader() -> ShaderRef {
        "shaders/grass_prepass.wgsl".into()
    }
}

/// Advances the wind clock.
/// Advances the wind clock on REAL time.
///
/// It ran on the world's, which multiplies by whatever speed the game is
/// set to - so at four times the grass swayed four times as fast, and at
/// eight it shook. Brett: "sometimes the grass shakes like crazy." It also
/// froze mid-lean whenever the world was paused and snapped when it
/// resumed. Wind is weather a player is watching, not work the village is
/// doing.
fn drive_wind(
    time: Res<Time<Real>>,
    weather: Option<Res<crate::weather::Weather>>,
    mut materials: ResMut<Assets<GrassMaterial>>,
    watch: Res<crate::debug::timings::Timings>,
) {
    let _t = watch.watch("grass: drive_wind");
    let wind = weather.map_or(0.3, |w| w.wind);
    for (_, material) in materials.iter_mut() {
        material.extension.params.clock.x = time.elapsed_secs();
        // z: speed, w: strength - a storm's gusts run fast and bend hard.
        // Strength is the lean's angle scale now (the shader rotates blades
        // about their roots): a calm sways ~15 degrees, a storm lays blades
        // over toward the shader's 1.1-radian clamp at the crests.
        material.extension.params.wind.z = 0.7 + wind * 1.6;
        material.extension.params.wind.w = 0.4 + wind * 0.7;
    }
}

/// Grass chunks currently alive.
#[derive(Resource, Default)]
pub struct GrassChunks {
    entities: HashMap<IVec2, Entity>,
}

impl GrassChunks {
    /// Forget (and despawn) the grass over a circle - called when the ground
    /// under it is worked, so the blades rebuild against the new terrain.
    pub fn invalidate_near(&mut self, commands: &mut Commands, x: f32, z: f32, radius: f32) {
        let min_x = ((x - radius) / crate::terrain::CHUNK_SIZE).floor() as i32;
        let max_x = ((x + radius) / crate::terrain::CHUNK_SIZE).floor() as i32;
        let min_z = ((z - radius) / crate::terrain::CHUNK_SIZE).floor() as i32;
        let max_z = ((z + radius) / crate::terrain::CHUNK_SIZE).floor() as i32;
        self.entities.retain(|coord, entity| {
            let hit = coord.x >= min_x && coord.x <= max_x && coord.y >= min_z && coord.y <= max_z;
            if hit {
                commands.entity(*entity).despawn();
            }
            !hit
        });
    }

    /// Forget everything, for a full world reload.
    pub fn invalidate_all(&mut self, commands: &mut Commands) {
        for (_, entity) in self.entities.drain() {
            commands.entity(entity).despawn();
        }
    }
}

#[derive(Resource)]
struct GrassAssets {
    material: Handle<GrassMaterial>,
}

fn init_grass_material(mut commands: Commands, mut materials: ResMut<Assets<GrassMaterial>>) {
    let material = materials.add(GrassMaterial {
        base: StandardMaterial {
            base_color: Color::WHITE,
            // Not for cutout: every fragment passes the mask. Depth-only opaque
            // prepasses get an EMPTY bind-group layout in Bevy, which forbids the
            // wind uniform the prepass vertex needs; alpha-masked materials keep
            // their material bindings in the prepass, so the mask is the price of
            // the wind clock being visible there.
            alpha_mode: AlphaMode::Mask(0.5),
            perceptual_roughness: 1.0,
            reflectance: 0.02,
            // Blades render from both sides, but `double_sided` stays OFF: that
            // flag flips normals on back faces, which pointed every blade's far
            // side downward and lit half the meadow black. With our authored
            // up-normals, both faces should shade identically — like the ground.
            double_sided: false,
            cull_mode: None,
            ..default()
        },
        extension: GrassExtension {
            params: GrassParams {
                wind: Vec4::new(0.83, 0.55, 1.0, 0.28),
                clock: Vec4::ZERO,
            },
        },
    });
    commands.insert_resource(GrassAssets { material });
}

/// How thickly a biome grows grass, and how tall.
fn biome_growth(biome: Biome) -> (f32, f32) {
    match biome {
        Biome::Temperate => (0.8, 1.0),
        Biome::Wetland => (0.95, 1.15),
        Biome::Boreal => (0.4, 0.8),
        Biome::Arid => (0.12, 0.6),
        Biome::Alpine => (0.0, 0.0),
    }
}

/// Bakes one chunk's blades. `None` when nothing grows there.
///
/// Deterministic per chunk and seed, like every other generated thing: a chunk
/// that unloads and reloads grows back identical.
pub fn build_grass_mesh(
    terrain: &Terrain,
    trails: Option<&crate::trails::Trails>,
    seed: u32,
    coord: IVec2,
) -> Option<Mesh> {
    let origin = Vec2::new(coord.x as f32 * CHUNK_SIZE, coord.y as f32 * CHUNK_SIZE);
    // Blades are measured from the chunk's MIDDLE, and the patch is seated
    // there. A blade must stand along its own local up, so the patch is
    // seated rigidly rather than bent per vertex - and a rigid seat bows
    // furthest from its anchor. Anchored at a corner the far corner floats
    // two thirds of a unit, which on knee-high grass is a visible hover;
    // anchored at the middle the worst case is a sixth of a unit, under a
    // blade's own height.
    let centre = origin + Vec2::splat(CHUNK_SIZE * 0.5);
    let mut rng = Rng::new(
        (seed as u64) << 32
            ^ ((coord.x as u32 as u64) << 16)
            ^ (coord.y as u32 as u64)
            ^ 0x6ea5_51de,
    );

    let mut builder = meshbuild::BladeBuilder::default();
    let steps = (CHUNK_SIZE / SPACING) as i32;

    for iz in 0..steps {
        for ix in 0..steps {
            let x = origin.x + ix as f32 * SPACING + rng.range(-0.5, 0.5);
            let z = origin.y + iz as f32 * SPACING + rng.range(-0.5, 0.5);

            let height = terrain.height_at(x, z);
            if height < WATER_LEVEL + 0.4 {
                continue;
            }
            // Tilled ground is bare: no blades through the furrows. Ground
            // merely levelled - a house's terrace - keeps its grass, because a
            // bald ring around every house is worse than a blade under a floor
            // nobody can see. Brett: "every house has a foundation that is
            // taller than the grass", and he draws them that way.
            if terrain.is_bare(x, z) {
                continue;
            }
            if terrain.slope_at(x, z) > 0.4 {
                continue;
            }
            // A path is bare because feet made it bare. The blades thin as
            // the wear deepens rather than vanishing at a threshold, so a
            // road has worn shoulders instead of a mown edge - and the
            // number here is the same one the ground is tinted by, so what
            // is brown and what is bald cannot drift apart. Brett: "where
            // the trails turn brown grass should stop growing."
            if let Some(trails) = trails {
                let bare = trails.bareness(x, z);
                if bare > 0.15 && rng.range(0.0, 1.0) < (bare - 0.15) / 0.55 {
                    continue;
                }
            }
            if terrain
                .river_influence_at(x, z)
                .is_some_and(|(_, d, w)| d < rivers::CHANNEL_HALF_WIDTH * w * 1.35)
            {
                continue;
            }

            let biome = terrain.biome_at(x, z);
            let (density, stature) = biome_growth(biome);
            if density <= 0.0 {
                continue;
            }

            // Patchiness: meadows have thick and thin places, and the same field
            // that mottles the ground colour drives them, so lush grass sits on
            // lush-coloured ground.
            let patch = terrain.ground_patch_at(x, z);
            let keep = density * (0.7 + patch * 0.6);
            if hash_2d_f32(ix + coord.x * 1024, iz + coord.y * 1024, seed ^ 0x9aa5) > keep {
                continue;
            }

            // Blade colour uses the *same* moisture blend as the ground beneath it,
            // one step brighter. Authoring blades on their own scale drifted pale
            // twice in a row — anchoring them to the soil's own formula is what
            // keeps a green field green instead of frosted sage.
            let ramp = biome.ground().0;
            let moisture = terrain.moisture_at(x, z);
            let wet = (moisture * 0.3).clamp(0.0, 1.0);
            let shade = 0.24 + patch * 0.16;
            let root = palette::shade_blend(ramp, &palette::FOLIAGE, wet, shade).to_linear();
            let tip = palette::shade_blend(ramp, &palette::FOLIAGE, wet, (shade + 0.16).min(0.55))
                .to_linear();

            // A tuft, not a lone blade: real grass clumps, and three thin blades
            // sharing a hue read as growth where one wide one reads as a spike.
            for _ in 0..3 {
                builder.push_blade(
                    Vec3::new(
                        x - centre.x + rng.range(-0.16, 0.16),
                        height,
                        z - centre.y + rng.range(-0.16, 0.16),
                    ),
                    // Knee height at the tallest, judged against the people wading it.
                    rng.range(0.18, 0.4) * stature,
                    rng.range(0.055, 0.095),
                    rng.range(0.0, std::f32::consts::TAU),
                    rng.f32(),
                    [root.red, root.green, root.blue, 1.0],
                    [tip.red, tip.green, tip.blue, 1.0],
                );
            }
        }
    }

    builder.build()
}

/// Streams grass in a disc around the camera focus, nearest first.
fn stream_grass(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    terrain: Option<Res<Terrain>>,
    assets: Option<Res<GrassAssets>>,
    trails: Option<Res<crate::trails::Trails>>,
    world_seed: Res<crate::WorldSeed>,
    mut chunks: ResMut<GrassChunks>,
    cameras: Query<&crate::camera::CameraRig>,
    watch: Res<crate::debug::timings::Timings>,
) {
    let _t = watch.watch("grass: stream_grass");
    let (Some(terrain), Some(assets), Ok(rig)) = (terrain, assets, cameras.single()) else {
        return;
    };
    let centre = terrain.chunk_of(rig.focus.x, rig.focus.z);

    // Hysteresis on unload so the boundary does not thrash as the camera drifts.
    let drop = (GRASS_RADIUS + 1) * (GRASS_RADIUS + 1);
    chunks.entities.retain(|coord, entity| {
        let d = *coord - centre;
        if d.x * d.x + d.y * d.y > drop {
            commands.entity(*entity).despawn();
            false
        } else {
            true
        }
    });

    let mut wanted = Vec::new();
    for dz in -GRASS_RADIUS..=GRASS_RADIUS {
        for dx in -GRASS_RADIUS..=GRASS_RADIUS {
            if dx * dx + dz * dz <= GRASS_RADIUS * GRASS_RADIUS {
                wanted.push(centre + IVec2::new(dx, dz));
            }
        }
    }
    wanted.sort_by_key(|c| {
        let d = *c - centre;
        d.x * d.x + d.y * d.y
    });

    let mut built = 0;
    for coord in wanted {
        if built >= BUILDS_PER_FRAME {
            break;
        }
        if chunks.entities.contains_key(&coord) {
            continue;
        }

        let entity = match build_grass_mesh(&terrain, trails.as_deref(), world_seed.0, coord) {
            Some(mesh) => commands
                .spawn((
                    Name::new("Grass"),
                    Mesh3d(meshes.add(mesh)),
                    MeshMaterial3d(assets.material.clone()),
                    Transform::from_xyz(
                        (coord.x as f32 + 0.5) * CHUNK_SIZE,
                        0.0,
                        (coord.y as f32 + 0.5) * CHUNK_SIZE,
                    ),
                    NotShadowCaster,
                ))
                .id(),
            // Barren chunks still get a marker entity so they are not re-examined
            // every frame.
            None => commands
                .spawn((Name::new("Grass (barren)"), Transform::default()))
                .id(),
        };

        chunks.entities.insert(coord, entity);
        built += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grassy_chunk(terrain: &Terrain) -> Option<IVec2> {
        for iz in -30..30 {
            for ix in -30..30 {
                let x = ix as f32 * CHUNK_SIZE + 32.0;
                let z = iz as f32 * CHUNK_SIZE + 32.0;
                let biome = terrain.biome_at(x, z);
                if matches!(biome, Biome::Temperate | Biome::Wetland)
                    && terrain.height_at(x, z) > WATER_LEVEL + 2.0
                    && terrain.slope_at(x, z) < 0.2
                {
                    return Some(terrain.chunk_of(x, z));
                }
            }
        }
        None
    }

    #[test]
    fn grassland_grows_grass_and_is_deterministic() {
        let terrain = Terrain::new(77);
        let coord = grassy_chunk(&terrain).expect("no grassland anywhere near spawn");

        let a = build_grass_mesh(&terrain, None, 77, coord).expect("grassland grew nothing");
        let b = build_grass_mesh(&terrain, None, 77, coord).expect("second build failed");
        assert_eq!(a.count_vertices(), b.count_vertices());
        assert!(a.count_vertices() >= 300, "suspiciously sparse meadow");
    }

    #[test]
    fn blades_pin_their_roots_and_free_their_tips() {
        // The shader bends by uv.x. Roots must carry 0 — a swaying root slides the
        // whole field across the ground.
        let terrain = Terrain::new(77);
        let coord = grassy_chunk(&terrain).expect("no grassland");
        let mesh = build_grass_mesh(&terrain, None, 77, coord).unwrap();

        let Some(bevy::mesh::VertexAttributeValues::Float32x2(uvs)) =
            mesh.attribute(Mesh::ATTRIBUTE_UV_0)
        else {
            panic!("no uvs");
        };

        // Blades are triangles: two roots then a tip.
        assert_eq!(uvs.len() % 3, 0);
        for blade in uvs.chunks(3) {
            assert_eq!(blade[0][0], 0.0, "root sways");
            assert_eq!(blade[1][0], 0.0, "root sways");
            assert!(blade[2][0] > 0.9, "tip is pinned");
        }
    }

    /// Grass thins where feet have worn the ground, and is gone where they
    /// have worn it bare.
    ///
    /// Brett: "where the trails turn brown grass should stop growing." The
    /// thinning is gradual on purpose - a road with a mown edge looks more
    /// artificial than one with no edge at all - and it reads the same
    /// number the ground is tinted by, so what is brown and what is bald
    /// cannot drift apart.
    #[test]
    fn a_worn_path_grows_no_grass() {
        let terrain = Terrain::new(77);
        let coord = grassy_chunk(&terrain).expect("no grassland anywhere near spawn");
        let before = build_grass_mesh(&terrain, None, 77, coord)
            .expect("grassland grew nothing")
            .count_vertices();
        assert!(before > 0, "no grassland found to wear down");

        // Wear the whole chunk to bare earth.
        let mut trails = crate::trails::Trails::default();
        let origin = Vec2::new(coord.x as f32 * CHUNK_SIZE, coord.y as f32 * CHUNK_SIZE);
        let mut walked = Vec::new();
        let step = 1.0;
        let mut z = -step;
        while z < CHUNK_SIZE + step {
            let mut x = -step;
            while x < CHUNK_SIZE + step {
                walked.push((
                    ((origin.x + x) / crate::trails::CELL).floor() as i32,
                    ((origin.y + z) / crate::trails::CELL).floor() as i32,
                    50.0,
                ));
                x += step;
            }
            z += step;
        }
        trails.restore(walked.into_iter());

        let after = build_grass_mesh(&terrain, Some(&trails), 77, coord)
            .map_or(0, |mesh| mesh.count_vertices());
        assert!(
            after * 4 < before,
            "ground worn to bare earth still grows {after} of its {before} blades",
        );
    }

    /// And untouched ground is untouched: an empty trail map takes nothing.
    #[test]
    fn wild_ground_keeps_every_blade() {
        let terrain = Terrain::new(77);
        let at = grassy_chunk(&terrain).expect("no grassland anywhere near spawn");
        let wild = build_grass_mesh(&terrain, None, 77, at).expect("grassland grew nothing");
        let empty = crate::trails::Trails::default();
        let same = build_grass_mesh(&terrain, Some(&empty), 77, at)
            .expect("the same ground grew nothing the second time");
        assert_eq!(
            wild.count_vertices(),
            same.count_vertices(),
            "an empty trail map thinned wild grass",
        );
    }
}
