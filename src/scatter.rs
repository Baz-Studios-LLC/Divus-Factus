//! Trees, rocks and bushes scattered across the terrain.
//!
//! Density follows the same moisture field the terrain colouring uses, so forest
//! grows where the ground is already drawn green and rocks appear where it is
//! already grey — the scatter agrees with the landscape rather than being sprinkled
//! over it.
//!
//! Scenery is split by whether the simulation needs to touch it. Trees and rocks are
//! only ever looked at, so they are **baked into one mesh per chunk**; bushes bear
//! food and can be picked up, so they stay real entities. That distinction is what
//! makes a streamed world affordable: as individual entities the scenery reached
//! 186,000 of them and the per-frame cost of shepherding that many held the frame
//! rate at 30.

use bevy::prelude::*;

use crate::creature::genome::Tone;
use crate::meshbuild::MeshBuilder;
use crate::palette;
use crate::rng::Rng;
use crate::terrain::{Biome, CHUNK_SIZE, Terrain, TerrainAssets, TerrainChunk, WATER_LEVEL};

/// Marks foliage that sways in the wind.
#[derive(Component)]
pub struct Foliage {
    /// Per-instance phase, so a forest does not sway as one object.
    pub phase: f32,
    /// How far this instance leans, in radians.
    pub amplitude: f32,
}

pub struct ScatterPlugin;

impl Plugin for ScatterPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StrippedGround>()
            .init_resource::<DirtyGroves>()
            .add_systems(
                Update,
                (
                    sway_foliage,
                    topple_trees,
                    sink_spent,
                    rebake_groves,
                    collect_groves,
                ),
            )
            // PostUpdate, deliberately: chunks spawned during Update - by
            // streaming or by a building levelling the ground - get their
            // scenery THIS frame, before anything renders. In Update the
            // scenery lagged the chunk by a frame and every construction
            // made the nearby forest blink.
            .add_systems(
                PostUpdate,
                populate_chunks.before(bevy::transform::TransformSystems::Propagate),
            );
    }
}

/// The shapes of tree the generator knows.
///
/// Silhouette is what distinguishes them at a distance, so each differs in outline
/// rather than only in colour: a conifer tapers, a broadleaf is round and heavy, a
/// palm is bare-trunked with a crown, a snag is jagged.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TreeKind {
    /// Stacked slabs tapering to a point.
    Conifer,
    /// A wide, rounded canopy on a short trunk.
    Broadleaf,
    /// Slim pale trunk, sparse high canopy.
    Birch,
    /// Tall bare trunk with a crown of fronds.
    Palm,
    /// Dead: a bare trunk and a few broken branches.
    Snag,
}

impl TreeKind {
    /// Which trees grow in a biome, as weighted choices.
    ///
    /// Returning a slice rather than a single kind keeps forests mixed — a stand of
    /// exactly one species reads as wallpaper.
    pub fn for_biome(biome: Biome) -> &'static [TreeKind] {
        use TreeKind::*;
        match biome {
            Biome::Temperate => &[Broadleaf, Broadleaf, Conifer, Birch],
            Biome::Boreal => &[Conifer, Conifer, Conifer, Snag],
            Biome::Arid => &[Palm, Snag, Snag],
            Biome::Wetland => &[Broadleaf, Broadleaf, Palm, Conifer],
            Biome::Alpine => &[Conifer, Snag],
        }
    }
}

/// Bakes a tree of the given kind into `builder`.
fn bake_tree(builder: &mut MeshBuilder, position: Vec3, kind: TreeKind, rng: &mut Rng) {
    // Half again the old stand: trees used to read stubby beside the houses.
    // One roll drives every dimension, so old worlds keep their exact layout
    // — the same forests, grown up.
    let height = rng.range(3.9, 8.1);
    let yaw = rng.range(0.0, std::f32::consts::TAU);

    let bark = match kind {
        TreeKind::Birch => Tone {
            ramp: palette::RAMP_BONE,
            step: rng.range_i(3, 4) as usize,
        },
        TreeKind::Snag => Tone {
            ramp: palette::RAMP_WOOD,
            step: rng.range_i(0, 1) as usize,
        },
        _ => Tone {
            ramp: palette::RAMP_WOOD,
            step: rng.range_i(1, 3) as usize,
        },
    };
    let leaf = Tone {
        ramp: if rng.chance(0.7) {
            palette::RAMP_FOLIAGE
        } else {
            palette::RAMP_GRASS
        },
        step: rng.range_i(1, 3) as usize,
    };

    let trunk = |builder: &mut MeshBuilder, width: f32, length: f32| {
        builder.push_box(
            Transform::from_translation(position + Vec3::Y * length * 0.5)
                .with_rotation(Quat::from_rotation_y(yaw))
                .with_scale(Vec3::new(width, length, width)),
            palette::color_at(bark.palette_index()),
        );
    };

    match kind {
        TreeKind::Conifer => {
            let canopy_w = height * rng.range(0.40, 0.56);
            trunk(builder, height * rng.range(0.07, 0.10), height * 0.7);

            let layers = rng.range_i(3, 4);
            for layer in 0..layers {
                let t = layer as f32 / layers as f32;
                let w = canopy_w * (1.0 - t * 0.62);
                builder.push_box(
                    Transform::from_translation(position + Vec3::Y * height * (0.46 + t * 0.52))
                        .with_rotation(Quat::from_rotation_y(yaw + rng.range(0.0, 0.8)))
                        .with_scale(Vec3::new(w, height * 0.24, w)),
                    palette::color_at(leaf.shifted(layer).palette_index()),
                );
            }
        }

        TreeKind::Broadleaf => {
            let canopy_w = height * rng.range(0.62, 0.86);
            trunk(builder, height * rng.range(0.09, 0.13), height * 0.6);

            // Two overlapping slabs, the upper one inset, for a rounded crown.
            for (i, (scale, lift)) in [(1.0, 0.66), (0.72, 0.86)].into_iter().enumerate() {
                builder.push_box(
                    Transform::from_translation(position + Vec3::Y * height * lift)
                        .with_rotation(Quat::from_rotation_y(yaw + i as f32 * 0.6))
                        .with_scale(Vec3::new(canopy_w * scale, height * 0.30, canopy_w * scale)),
                    palette::color_at(leaf.shifted(i as i32).palette_index()),
                );
            }
        }

        TreeKind::Birch => {
            let canopy_w = height * rng.range(0.32, 0.44);
            trunk(builder, height * rng.range(0.045, 0.065), height * 0.86);

            for i in 0..2 {
                builder.push_box(
                    Transform::from_translation(
                        position + Vec3::Y * height * (0.78 + i as f32 * 0.16),
                    )
                    .with_rotation(Quat::from_rotation_y(yaw + i as f32 * 0.9))
                    .with_scale(Vec3::new(
                        canopy_w * (1.0 - i as f32 * 0.25),
                        height * 0.2,
                        canopy_w * (1.0 - i as f32 * 0.25),
                    )),
                    palette::color_at(leaf.shifted(i).palette_index()),
                );
            }
        }

        TreeKind::Palm => {
            let trunk_h = height * rng.range(0.9, 1.15);
            trunk(builder, height * rng.range(0.05, 0.075), trunk_h);

            // Fronds radiating from the crown, angled down.
            let fronds = rng.range_i(4, 6);
            for i in 0..fronds {
                let angle = yaw + i as f32 / fronds as f32 * std::f32::consts::TAU;
                let length = height * rng.range(0.34, 0.5);
                builder.push_box(
                    Transform::from_translation(
                        position
                            + Vec3::new(
                                angle.cos() * length * 0.45,
                                trunk_h - height * 0.05,
                                angle.sin() * length * 0.45,
                            ),
                    )
                    .with_rotation(Quat::from_rotation_y(-angle) * Quat::from_rotation_x(0.42))
                    .with_scale(Vec3::new(
                        height * 0.09,
                        height * 0.05,
                        length,
                    )),
                    palette::color_at(leaf.palette_index()),
                );
            }
        }

        TreeKind::Snag => {
            let trunk_h = height * rng.range(0.5, 0.8);
            trunk(builder, height * rng.range(0.07, 0.10), trunk_h);

            for _ in 0..rng.range_i(1, 3) {
                let angle = rng.range(0.0, std::f32::consts::TAU);
                let length = height * rng.range(0.2, 0.36);
                builder.push_box(
                    Transform::from_translation(
                        position
                            + Vec3::new(
                                angle.cos() * length * 0.4,
                                trunk_h * rng.range(0.55, 0.95),
                                angle.sin() * length * 0.4,
                            ),
                    )
                    .with_rotation(
                        Quat::from_rotation_y(-angle) * Quat::from_rotation_x(rng.range(0.5, 1.1)),
                    )
                    .with_scale(Vec3::new(
                        height * 0.05,
                        height * 0.05,
                        length,
                    )),
                    palette::color_at(bark.shifted(1).palette_index()),
                );
            }
        }
    }
}

/// Bakes a rock into `builder`: one to three overlapping boxes at random angles.
fn bake_rock(builder: &mut MeshBuilder, position: Vec3, rng: &mut Rng) {
    let size = rng.range(0.5, 1.9);
    let tone = Tone {
        ramp: palette::RAMP_STONE,
        step: rng.range_i(1, 3) as usize,
    };

    let chunks = rng.range_i(1, 3);
    for i in 0..chunks {
        let s = size * rng.range(0.55, 1.0);
        let offset = Vec3::new(
            rng.range(-0.3, 0.3) * size,
            s * 0.4,
            rng.range(-0.3, 0.3) * size,
        );

        builder.push_box(
            Transform::from_translation(position + offset)
                .with_rotation(Quat::from_euler(
                    EulerRot::YXZ,
                    rng.range(0.0, std::f32::consts::TAU),
                    rng.range(-0.25, 0.25),
                    rng.range(-0.25, 0.25),
                ))
                .with_scale(Vec3::new(
                    s,
                    s * rng.range(0.6, 1.0),
                    s * rng.range(0.7, 1.1),
                )),
            palette::color_at(tone.shifted(i % 2).palette_index()),
        );
    }
}

/// A food-bearing bush. Doubles as the only food source in the slice.
#[derive(Component)]
pub struct FoodSource {
    /// Remaining food, in villager-meals.
    pub amount: f32,
    /// Meals this bush regrows per second.
    pub regrowth: f32,
}

impl FoodSource {
    pub const CAPACITY: f32 = 5.0;
}

/// Spawns a berry bush as a single entity carrying its own baked mesh.
///
/// Bushes stay entities — they bear food, they can be picked up, and villagers query
/// them — but their geometry is baked rather than split across a root, leaf blocks
/// and berry blocks. Seven entities per bush was the largest remaining draw on the
/// entity budget, and that budget is what caps how far the world can stream.
fn spawn_bush(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: Handle<StandardMaterial>,
    position: Vec3,
    rng: &mut Rng,
) -> Entity {
    let size = rng.range(0.65, 1.25);
    let leaf_tone = Tone {
        ramp: palette::RAMP_FOLIAGE,
        step: rng.range_i(2, 4) as usize,
    };

    let mut builder = MeshBuilder::default();

    for i in 0..rng.range_i(2, 3) {
        let s = size * rng.range(0.6, 1.0);
        builder.push_box(
            Transform::from_xyz(
                rng.range(-0.3, 0.3) * size,
                s * 0.35,
                rng.range(-0.3, 0.3) * size,
            )
            .with_rotation(Quat::from_rotation_y(rng.range(0.0, std::f32::consts::TAU)))
            .with_scale(Vec3::new(s, s * 0.7, s)),
            palette::color_at(leaf_tone.shifted(i % 2).palette_index()),
        );
    }

    // Berries: small bright blocks so a bush with food on it is readable from the
    // god's-eye view. These are what the player will actually be looking for.
    let berry_tone = Tone {
        ramp: palette::RAMP_CLOTH_RED,
        step: 4,
    };
    for _ in 0..rng.range_i(3, 5) {
        builder.push_box(
            Transform::from_xyz(
                rng.range(-0.4, 0.4) * size,
                size * rng.range(0.35, 0.75),
                rng.range(-0.4, 0.4) * size,
            )
            .with_scale(Vec3::splat(size * 0.13)),
            palette::color_at(berry_tone.palette_index()),
        );
    }

    commands
        .spawn((
            Name::new("Berry Bush"),
            Mesh3d(meshes.add(builder.build())),
            MeshMaterial3d(material),
            Transform::from_translation(position)
                .with_rotation(Quat::from_rotation_y(rng.range(0.0, std::f32::consts::TAU))),
            Foliage {
                phase: rng.range(0.0, std::f32::consts::TAU),
                amplitude: rng.range(0.02, 0.05),
            },
            FoodSource {
                amount: FoodSource::CAPACITY,
                regrowth: rng.range(0.02, 0.05),
            },
            crate::matter::Matter::bush(),
            crate::hand::PickRadius(size * 0.85),
        ))
        .id()
}

/// Populates chunks with scenery as they stream in.
///
/// Trees and rocks are baked into a single mesh parented to the chunk; bushes are
/// spawned as entities, also parented. Either way, unloading the chunk takes its
/// scatter with it.
///
/// Placement derives from world position and the chunk coordinate, never from spawn
/// order, so a chunk that unloads and reloads comes back identical. Anything else
/// would rearrange the forest every time the player looked away.
fn populate_chunks(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    terrain_assets: Res<TerrainAssets>,
    terrain: Res<Terrain>,
    world_seed: Res<crate::WorldSeed>,
    stripped: Res<StrippedGround>,
    settlement: Option<Res<crate::villager::SettlementSite>>,
    mut library: Local<Option<ScatterMeshes>>,
    chunks: Query<(Entity, &TerrainChunk), Added<TerrainChunk>>,
    watch: Res<crate::debug::timings::Timings>,
) {
    let _t = watch.watch("scatter: populate_chunks");
    if chunks.is_empty() {
        return;
    }
    let library = library.get_or_insert_with(|| ScatterMeshes::build(&mut meshes, world_seed.0));
    // DIVUS_FACTUS_SCARCE starves the land of berry bushes — the famine dial, for
    // exercising the prayer loop without waiting for a bad year.
    let scarcity = if std::env::var("DIVUS_FACTUS_SCARCE").is_ok() {
        0.04
    } else {
        1.0
    };

    for (entity, chunk) in &chunks {
        let mut rng = Rng::new(chunk_scatter_seed(world_seed.0, chunk.coord));

        let origin = Vec2::new(
            chunk.coord.x as f32 * CHUNK_SIZE,
            chunk.coord.y as f32 * CHUNK_SIZE,
        );

        // A stone node, if this chunk holds one: a cluster of real boulders in
        // one place, rather than a stone every few strides everywhere.
        //
        // Brett's idea, and it answers the clutter and the economy at once:
        // "then there could be places where there are certain nodes. they could
        // mine from there with out all the rocvks lol". A miner walks to a
        // OUTCROP and works it; the rest of the world can be as bare as it
        // looks right.
        let node = stone_node(chunk.coord, &terrain);

        let mut builder = MeshBuilder::default();
        // Standing trees gather here and are spawned after the sweep:
        // real entities every one, bucketed into grove visuals so the
        // renderer sees a handful of meshes where the sim sees a forest.
        let mut stands: Vec<(Vec3, TreeKind)> = Vec::new();
        let steps = (CHUNK_SIZE / SCATTER_SPACING) as i32;

        for iz in 0..steps {
            for ix in 0..steps {
                let x = origin.x + ix as f32 * SCATTER_SPACING + rng.range(-1.4, 1.4);
                let z = origin.y + iz as f32 * SCATTER_SPACING + rng.range(-1.4, 1.4);

                let height = terrain.height_at(x, z);
                if height < WATER_LEVEL + 0.3 {
                    continue;
                }

                // Nothing grows in the river bed or on the bare bank strip. Without
                // this, trees stand in the channel and on earth the colour pass
                // just stripped bare.
                if terrain.river_influence_at(x, z).is_some_and(|(_, d, w)| {
                    d < crate::terrain::rivers::CHANNEL_HALF_WIDTH * w * 1.4
                }) {
                    continue;
                }
                // Tilled fields and building pads are tended ground: nothing
                // wild seeds there, however many times the chunk rebuilds.
                if terrain.is_worked(x, z) {
                    continue;
                }

                let slope = terrain.slope_at(x, z);
                let moisture = terrain.moisture_at(x, z);
                let forest = terrain.forest_at(x, z);

                // WORLD positions. These were chunk-local, placed by the
                // chunk's own transform - and when chunks moved their geometry
                // into world space and went to identity, every tree, rock and
                // bush in the world collapsed into a single sixty-four unit
                // cube at the origin. A parent of identity means a child's
                // local IS its world position, and the world bend seats it
                // from there like any other entity.
                let local = Vec3::new(x, height, z);
                // Loose scenery is BAKED into one mesh per chunk, and a mesh
                // cannot be bent per vertex without shearing every rock it
                // holds - so the patch is seated rigidly at the chunk's
                // middle and its pieces are measured from there. Baked in
                // world coordinates under an identity transform, as they
                // briefly were, the whole scenery mesh was seated at the
                // world origin and its rocks stayed FLAT while the ground
                // curved away beneath them: a cloud of boulders hanging in
                // the sky over the treeline.
                let seated = Vec3::new(
                    origin.x + CHUNK_SIZE * 0.5,
                    0.0,
                    origin.y + CHUNK_SIZE * 0.5,
                );

                let biome = terrain.biome_at(x, z);

                // Dry and alpine country carries far fewer trees, which is most of
                // what makes one region look unlike another from the air.
                let tree_chance = match biome {
                    Biome::Arid => 0.10,
                    Biome::Alpine => 0.16,
                    Biome::Boreal => 0.55,
                    Biome::Wetland => 0.62,
                    Biome::Temperate => 0.45,
                };

                if slope > 0.42 {
                    // Steep ground used to be a third rock. Brett, looking at a
                    // hillside of them: "there is so much ground clutter form
                    // rocks... There has to be a way to clean this up lol, it
                    // has to be wasting memory as well." It was: ten thousand
                    // meshes and six and a half thousand of them casting
                    // shadows, most of them a stone the size of a loaf.
                    //
                    // Stone gathers into OUTCROPS now - see `stone_node` - so
                    // the scatter can be as thin as it looks right without the
                    // village going short.
                    // Thinner again. The outcrops carry the stone a village
                    // needs, so what is left on the open ground is there to
                    // look like ground and nothing else - and at six in a
                    // hundred it still read as gravel strewn everywhere.
                    if rng.chance(SLOPE_BOULDER) {
                        let roll = roll_rock(&mut rng);
                        if stripped.is_stripped(x, z) {
                            // Bare ground still burns the rock's dice, or
                            // carrying one rock off would reshuffle every
                            // neighbour on the next chunk rebuild.
                            bake_rock(&mut MeshBuilder::default(), local, &mut rng);
                        } else {
                            // A REAL rock, however far from home. These were
                            // baked scenery beyond the village's reach, which
                            // was fine while rocks were furniture - and wrong
                            // the day the god became a hauler. Brett, denied:
                            // "it is very jarring to not be able to pick up a
                            // resource." Boulders are a rare roll, so the
                            // entity count stays a rounding error; the pebble
                            // litter below is still baked.
                            let rock = spawn_boulder(
                                &mut commands,
                                &mut meshes,
                                terrain_assets.ground_material.clone(),
                                local,
                                &mut rng,
                                roll,
                                Some(library),
                            );
                            commands.entity(rock).insert(ChildOf(entity));
                        }
                    }
                } else if forest > 0.50 && moisture > 0.38 {
                    let density = ((forest - 0.50) / 0.3).clamp(0.0, 1.0);
                    // Trees keep a canopy's berth from worked ground, and
                    // stripped ground stays bare: the axe is permanent. But
                    // EVERY position burns the same dice standing or gone —
                    // suppression discards geometry, never random draws, or
                    // one felled tree would rearrange the whole forest on
                    // the next chunk rebuild.
                    // The square belongs to the village: no tree seeds
                    // inside the banner's circle, whatever the noise says.
                    let square = settlement.as_ref().is_some_and(|site| {
                        Vec2::new(x - site.centre.x, z - site.centre.z).length() < 10.0
                    });
                    let bare =
                        terrain.is_worked_within(x, z, 4.0) || stripped.is_stripped(x, z) || square;
                    if rng.chance(tree_chance * (0.55 + density)) {
                        let kind = *rng.pick(TreeKind::for_biome(biome));
                        if !bare {
                            stands.push((local, kind));
                        }
                    } else if rng.chance(0.16 * scarcity) {
                        let bush = spawn_bush(
                            &mut commands,
                            &mut meshes,
                            terrain_assets.ground_material.clone(),
                            local,
                            &mut rng,
                        );
                        // A worked pad grows nothing - but the dice burned.
                        // Cut-over ground, though, can flower into a heath.
                        if terrain.is_worked_within(x, z, 4.0) {
                            commands.entity(bush).despawn();
                        } else {
                            commands.entity(bush).insert(ChildOf(entity));
                        }
                    }
                } else if rng.chance(LOOSE_BOULDER) {
                    // A stone here and there on open ground, where it used to be
                    // one in eighteen scatter points - which reads as gravel
                    // spilled over the whole world rather than as country.
                    // (The flat-ground rocks were baked scenery at first, and
                    // the whole civic ladder starved for stone on any village
                    // founded on flat land; the QUARRIES carry that now, and
                    // the outcrops before them.)
                    let roll = roll_rock(&mut rng);
                    if stripped.is_stripped(x, z) {
                        bake_rock(&mut MeshBuilder::default(), local, &mut rng);
                    } else {
                        // Live everywhere, same as the slope roll above.
                        let rock = spawn_boulder(
                            &mut commands,
                            &mut meshes,
                            terrain_assets.ground_material.clone(),
                            local,
                            &mut rng,
                            roll,
                            Some(library),
                        );
                        commands.entity(rock).insert(ChildOf(entity));
                    }
                } else if rng.chance(0.05 * scarcity) {
                    let bush = spawn_bush(
                        &mut commands,
                        &mut meshes,
                        terrain_assets.ground_material.clone(),
                        local,
                        &mut rng,
                    );
                    commands.entity(bush).insert(ChildOf(entity));
                } else if moisture > 0.45 && rng.chance(0.012) {
                    let herb = spawn_sacred(
                        &mut commands,
                        &mut meshes,
                        terrain_assets.ground_material.clone(),
                        local,
                        SacredKind::Incense,
                        &mut rng,
                    );
                    commands.entity(herb).insert(ChildOf(entity));
                } else if rng.chance(0.010) {
                    let flowers = spawn_sacred(
                        &mut commands,
                        &mut meshes,
                        terrain_assets.ground_material.clone(),
                        local,
                        SacredKind::Dye,
                        &mut rng,
                    );
                    commands.entity(flowers).insert(ChildOf(entity));
                } else if rng.chance(LOOSE_PEBBLE) {
                    bake_rock(&mut builder, local - seated, &mut rng);
                }
            }
        }

        // The node's own stones: real boulders, gathered in one place, big
        // enough that a miner has a reason to walk there.
        if let Some((at, count)) = node {
            let mut rng = Rng::new(
                (at.x * 1000.0) as i64 as u64 ^ ((at.z * 1000.0) as i64 as u64) << 8 ^ 0x0B0_1DE7,
            );
            for _ in 0..count {
                let about = Vec3::new(
                    at.x + rng.range(-3.5, 3.5),
                    0.0,
                    at.z + rng.range(-3.5, 3.5),
                );
                let stood = Vec3::new(about.x, terrain.height_at(about.x, about.z), about.z);
                if stripped.is_stripped(stood.x, stood.z)
                    || terrain.is_worked_within(stood.x, stood.z, 2.0)
                {
                    continue;
                }
                // Heavier than a loose stone and worth the walk: an outcrop
                // holds what a scatter of gravel used to hold between it.
                let mut roll = roll_rock(&mut rng);
                roll.girth = rng.range(1.4, 2.6);
                roll.mass *= roll.girth;
                roll.radius *= roll.girth;
                let rock = spawn_boulder(
                    &mut commands,
                    &mut meshes,
                    terrain_assets.ground_material.clone(),
                    // Where it actually stands. A chunk's transform is the
                    // identity and its mesh carries world seats, so everything
                    // hanging off one is placed in WORLD coordinates - see
                    // `local` above, which is named for a frame it does not
                    // use. Taking the chunk's middle off first put every node's
                    // boulders within half a chunk of the flat world's origin,
                    // and the flat origin is the top of the globe: from
                    // anywhere else on the planet they hung in the sky, all of
                    // them, in one clump.
                    stood,
                    &mut rng,
                    roll,
                    Some(library),
                );
                commands.entity(rock).insert(ChildOf(entity));
            }
        }

        // The groves: bucket the stands by a coarse grid — pure
        // bookkeeping, invisible on the ground since every tree keeps the
        // exact spot the noise gave it — and raise one merged mesh per
        // bucket with the member trees as meshless entities beside it.
        let mut buckets: Vec<(IVec2, Vec<(Vec3, TreeKind)>)> = Vec::new();
        for (local, kind) in stands {
            let cell = IVec2::new(
                (local.x / GROVE_SPAN).floor() as i32,
                (local.z / GROVE_SPAN).floor() as i32,
            );
            match buckets.iter_mut().find(|(c, _)| *c == cell) {
                Some((_, members)) => members.push((local, kind)),
                None => buckets.push((cell, vec![(local, kind)])),
            }
        }
        for (_, members) in buckets {
            let anchor = members.iter().map(|(at, _)| *at).sum::<Vec3>() / members.len() as f32;
            let mut grove_mesh = MeshBuilder::default();
            let mut bodies: Vec<(Vec3, TreeBody)> = Vec::new();
            for (local, kind) in &members {
                let body = TreeBody::at(*kind, local.x, local.z);
                bake_tree(
                    &mut grove_mesh,
                    *local - anchor,
                    body.kind,
                    &mut Rng::new(body.seed),
                );
                bodies.push((*local, body));
            }
            let grove = commands
                .spawn((
                    Name::new("A grove"),
                    GroveMesh,
                    Mesh3d(meshes.add(grove_mesh.build())),
                    MeshMaterial3d(terrain_assets.ground_material.clone()),
                    Transform::from_translation(anchor),
                    ChildOf(entity),
                ))
                .id();
            for (local, body) in bodies {
                commands.spawn((
                    Name::new("A tree"),
                    FellableTree { maturity: 1.0 },
                    body,
                    InGrove(grove),
                    Transform::from_translation(local),
                    Visibility::default(),
                    crate::hand::PickRadius(1.6),
                    ChildOf(entity),
                ));
            }
        }

        if !builder.is_empty() {
            commands.spawn((
                Name::new("Chunk Scenery"),
                Mesh3d(meshes.add(builder.build())),
                MeshMaterial3d(terrain_assets.ground_material.clone()),
                Transform::from_xyz(
                    origin.x + CHUNK_SIZE * 0.5,
                    0.0,
                    origin.y + CHUNK_SIZE * 0.5,
                ),
                ChildOf(entity),
            ));
        }
    }
}

/// World units between scatter sample points.
const SCATTER_SPACING: f32 = 4.5;

/// The chance a scatter point on open ground is a loose pebble, a boulder on
/// the flat, and a boulder on a slope.
///
/// A tenth of what they were, and the third time this has been cut. Brett,
/// looking at a green meadow with a stone every few strides: "There is still
/// entirely too many rocks on the ground... lets go back to removing about 90%
/// of the small rocks and add quarries."
///
/// The pebble is the one that mattered. It is the LAST branch of the scatter
/// chain, so on open meadow — where no tree, bush or herb roll has taken the
/// point first — one point in twenty became a stone, which is about ten a
/// chunk over every chunk in the world. The two boulder rolls were already
/// down at one and two in a hundred and still read as gravel, because a
/// hundred scatter points is a few strides of walking.
///
/// What makes this affordable is that the stone a village needs no longer
/// comes off the ground: it comes out of [`crate::matter::DepositKind::Stone`].
/// The rocks left here are scenery, and are meant to read as a stone in the
/// grass rather than as a supply.
const LOOSE_PEBBLE: f32 = 0.005;
const LOOSE_BOULDER: f32 = 0.0012;
const SLOPE_BOULDER: f32 = 0.002;

/// Side of the grove bucket: trees within one cell share a rendered mesh.
///
/// Fourteen units, and MEASURED rather than inherited. The obvious economy is to
/// merge more per mesh — a chunk holds twenty-one groves at this span, so the
/// world carries some twenty-six thousand grove meshes at the widest zoom, and
/// merging them four to one looks like free money. It is the opposite. Swept at
/// the altitude where frames actually drop, interleaved against thermal drift:
///
///   span 14 -> 30,693 meshes, 27.1ms      span 32 -> 23,620, 31.4ms
///   span 64 -> 21,734 meshes, 31.5ms
///
/// Nine thousand fewer meshes cost four milliseconds. Draw calls are not the
/// bottleneck here; VERTICES are, and a small mesh is culled where a big one is
/// not — a sixty-four unit grove with one corner on screen draws every tree in
/// it. Fine granularity is what keeps the vertex count down.
///
/// Which invites the opposite question, and Brett asked it: if smaller is
/// faster, why merge at all? Swept downward too, and the valley has a floor —
/// spans 4.5, 7 and 10 come out at 27.7, 27.6 and 27.5 against 14's 27.3, so
/// nothing is won below this. The AVERAGE barely moves; what moves is the
/// HITCH. At a span of 4.5 a tree is nearly its own mesh — 51,450 of them —
/// and the worst steady frame goes to 61/88/73ms at play zoom where a span of
/// 14 holds 58/47/38, six comparisons across two altitudes and all six worse.
/// Every mesh is a GPU upload when its chunk streams in, and chunk streaming is
/// where this game's stutter actually lives. So the grove is not a tax on frame
/// rate that we tolerate for tidiness — it is what keeps streaming smooth, and
/// fourteen sits at the bottom of the valley with a cliff on one side and flat
/// ground on the other.
const GROVE_SPAN: f32 = 14.0;

/// Within this range of the settlement, trees and rocks are entities rather
/// than baked scenery — the simulation touches them, so they must be
/// touchable. An axe cannot fell a vertex buffer. Deliberately wider than
/// the villagers' working reach (170), so every rock and tree a worker can
/// walk to is real: nothing in arm's reach is set dressing. (Going fully
/// real everywhere was measured and rejected: 13.6k scenery entities
/// halved the release frame rate, 59fps to 31.)
// Kept for the tests that price the scatter against a working walk; the
// live near-village gate it once served fell when every boulder went live.
#[allow(dead_code)]
pub const TREE_HARVEST_RADIUS: f32 = 190.0;

/// Ground the village has already stripped: felled trees and mined-out
/// boulders, by rounded world position. The scatterer consults it before
/// seeding, so a chunk rebuild never resurrects what hands took away —
/// a woods worked hard stays visibly cut, and a farmed-out country reads
/// as exactly that from the air.
#[derive(Resource, Default)]
pub struct StrippedGround(pub bevy::platform::collections::HashSet<IVec2>);

impl StrippedGround {
    fn key(x: f32, z: f32) -> IVec2 {
        IVec2::new(x.round() as i32, z.round() as i32)
    }

    /// Marks this spot as taken: nothing wild of the felled kind returns.
    pub fn strip(&mut self, x: f32, z: f32) {
        self.0.insert(Self::key(x, z));
    }

    pub fn is_stripped(&self, x: f32, z: f32) -> bool {
        self.0.contains(&Self::key(x, z))
    }
}

/// A tree the axe can reach. Maturity 1 is full-grown; a felled tree is
/// gone for good — the land does not quietly undo the woodcutter.
#[derive(Component)]
pub struct FellableTree {
    pub maturity: f32,
}

/// The rolled identity of one standing tree: everything needed to bake its
/// body on demand — merged into its grove's shared mesh while it stands,
/// or alone the moment it topples, burns, or is carried off in the hand.
#[derive(Component, Clone, Copy)]
pub struct TreeBody {
    pub kind: TreeKind,
    pub seed: u64,
}

impl TreeBody {
    /// Seeded purely by world position, so the same spot always grows the
    /// same tree no matter which rng stream asked.
    pub fn at(kind: TreeKind, x: f32, z: f32) -> TreeBody {
        TreeBody {
            kind,
            seed: ((x.to_bits() as u64) << 32) ^ (z.to_bits() as u64) ^ 0x7233,
        }
    }

    /// Bakes this tree's body, alone, origin at its base.
    pub fn bake(&self, meshes: &mut Assets<Mesh>) -> Handle<Mesh> {
        let mut builder = MeshBuilder::default();
        bake_tree(
            &mut builder,
            Vec3::ZERO,
            self.kind,
            &mut Rng::new(self.seed),
        );
        meshes.add(builder.build())
    }
}

/// Which grove-visual this standing tree is merged into. Groves are pure
/// rendering: a handful of neighbouring trees drawn as one mesh, because
/// a mesh per trunk taxed the frame for every tree standing. The trees
/// themselves are full entities — meshless while merged — and every one
/// of them can be felled, burned, or uprooted individually.
#[derive(Component)]
pub struct InGrove(pub Entity);

/// A grove's merged visual.
#[derive(Component)]
pub struct GroveMesh;

/// Groves whose membership changed this frame and need their mesh rebaked.
/// Every site that detaches a tree (the axe, the hand, the fire, the
/// building clearings) pushes the grove here BEFORE removing the tree.
#[derive(Resource, Default)]
pub struct DirtyGroves(pub Vec<Entity>);

/// How long a CHURNING grove stays quiet before it bakes.
///
/// A burning grove is re-marked with every spread, and each finished bake
/// costs a real GPU upload — rebaking eagerly meant paying that upload over
/// and over while the fire crawled, so a grove under repeated change waits
/// until the change stops.
const GROVE_QUIET: f32 = 2.0;

/// How long a grove marked ONCE waits: not at all.
///
/// The debounce above was charged to every caller, and Brett caught what that
/// costs: fell one tree with the axe and its ghost stands in the merged mesh
/// for two full seconds after it has finished falling. A single fell is not
/// churn — it is one bake, and one bake is affordable at once. So the wait is
/// no longer a constant but a judgement about what marked the grove: a first
/// mark bakes on the next frame, and only a grove marked AGAIN while it is
/// still warm falls back to the long quiet. A fire therefore pays one prompt
/// bake as it catches and then debounces properly for the rest of its spread,
/// and the axe pays nothing.
const GROVE_PROMPT: f32 = 0.0;

/// How long a grove counts as warm. Slightly longer than the quiet itself, so
/// a grove that has only just finished baking still recognises the next mark
/// as churn rather than treating it as a fresh single fell.
const GROVE_WARM: f32 = 2.5;

/// A grove mesh being rebuilt on the compute pool, to be collected when done.
#[derive(Component)]
pub(crate) struct RebakingGrove(bevy::tasks::Task<Mesh>);

/// Rebakes dirty groves from their surviving members, and buries groves
/// with none left.
///
/// The geometry work happens OFF the main thread. A grove is the merged mesh
/// of a whole chunk's stand, and a big one is a quarter of a second of
/// vertex-pushing — a storm that torched one mid-frame used to stall the
/// frame by exactly that much, the scrub's largest single spike. Now this
/// system only gathers the surviving trees' seeds (cheap) and hands the bake
/// to the compute pool; [`collect_groves`] swaps the finished mesh in a frame
/// or two later. The old silhouette stands in the meantime, which nobody has
/// ever seen to complain about.
pub(crate) fn rebake_groves(
    mut commands: Commands,
    time: Res<Time>,
    mut waiting: Local<std::collections::HashMap<Entity, f32>>,
    mut last_marked: Local<std::collections::HashMap<Entity, f32>>,
    mut dirty: ResMut<DirtyGroves>,
    trees: Query<(&Transform, &TreeBody, &InGrove)>,
    groves: Query<&Transform, (With<GroveMesh>, Without<RebakingGrove>)>,
    watch: Res<crate::debug::timings::Timings>,
) {
    let _t = watch.watch("scatter: rebake_groves");
    // Fresh marks (re)start their grove's clock — promptly for a lone fell,
    // at the full quiet for a grove that is still warm from the last one.
    let now = time.elapsed_secs();
    for grove in dirty.0.drain(..) {
        let churning = last_marked
            .get(&grove)
            .is_some_and(|marked| now - marked < GROVE_WARM);
        last_marked.insert(grove, now);
        waiting.insert(grove, if churning { GROVE_QUIET } else { GROVE_PROMPT });
    }
    // Groves nobody has touched in a while are cold: forget them, or the map
    // would remember every grove the world ever felled a tree in.
    last_marked.retain(|_, marked| now - *marked < GROVE_WARM);
    if waiting.is_empty() {
        return;
    }
    let dt = time.delta_secs();
    let ripe: Vec<Entity> = waiting
        .iter_mut()
        .filter_map(|(grove, left)| {
            *left -= dt;
            (*left <= 0.0).then_some(*grove)
        })
        .collect();
    if ripe.is_empty() {
        return;
    }
    let pool = bevy::tasks::AsyncComputeTaskPool::get();
    for grove in ripe {
        waiting.remove(&grove);
        // The grove may have unloaded with its chunk mid-frame, or already
        // be mid-rebake — a re-mark lands again through fire's next spread.
        let Ok(grove_at) = groves.get(grove) else {
            continue;
        };
        // Everything the bake needs, as plain data the task can own.
        let stand: Vec<(Vec3, TreeKind, u64)> = trees
            .iter()
            .filter(|(_, _, home)| home.0 == grove)
            .map(|(at, body, _)| (at.translation - grove_at.translation, body.kind, body.seed))
            .collect();
        if stand.is_empty() {
            commands.entity(grove).despawn();
            continue;
        }
        let task = pool.spawn(async move {
            let mut builder = MeshBuilder::default();
            for (offset, kind, seed) in stand {
                bake_tree(&mut builder, offset, kind, &mut Rng::new(seed));
            }
            builder.build()
        });
        commands.entity(grove).insert(RebakingGrove(task));
    }
}

/// Collects finished grove bakes and swaps the new mesh in.
pub(crate) fn collect_groves(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut baking: Query<(Entity, &mut RebakingGrove)>,
    watch: Res<crate::debug::timings::Timings>,
) {
    let _t = watch.watch("scatter: collect_groves");
    // AT MOST ONE swap per frame: the bake is free by now, but handing the
    // renderer a fresh multi-megabyte mesh is a real upload, and two groves
    // finishing together used to pay both in the same frame.
    for (grove, mut rebake) in &mut baking {
        let Some(mesh) =
            bevy::tasks::block_on(bevy::tasks::futures_lite::future::poll_once(&mut rebake.0))
        else {
            continue;
        };
        commands
            .entity(grove)
            .remove::<RebakingGrove>()
            .insert(Mesh3d(meshes.add(mesh)));
        break;
    }
}

/// Pulls a standing tree out of its grove to be seen alone: it gets its
/// own baked body, and the grove rebakes without it this same frame. The
/// caller decides what happens next — fire, the hand; the axe has its
/// own path (a one-off topple actor).
pub fn stand_alone(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: Handle<StandardMaterial>,
    tree: Entity,
    body: &TreeBody,
    home: &InGrove,
    dirty: &mut DirtyGroves,
) {
    dirty.0.push(home.0);
    let mesh = body.bake(meshes);
    commands
        .entity(tree)
        .remove::<InGrove>()
        .insert((Mesh3d(mesh), MeshMaterial3d(material)));
}

/// A felled tree mid-fall: it leans, crashes, lies a beat, and sinks
/// away. Pure theatre — the timber went home on the forester's shoulder —
/// but a tree that blinks out of the world breaks the fiction the whole
/// game is built on.
#[derive(Component)]
pub struct Toppling {
    /// Horizontal axis to rotate about; the crown falls away from the axe.
    pub axis: Vec3,
    pub base_rot: Quat,
    pub base_y: f32,
    pub elapsed: f32,
}

/// Anything worked down to nothing sinks out of the world instead of
/// blinking: the last load leaves the pile, and the ground takes back
/// what is left.
#[derive(Component, Default)]
pub struct Sinking {
    pub elapsed: f32,
}

pub(crate) fn sink_spent(
    time: Res<Time>,
    mut commands: Commands,
    mut spent: Query<(Entity, &mut Sinking, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (thing, mut sinking, mut transform) in &mut spent {
        sinking.elapsed += dt;
        transform.translation.y -= dt * 2.6;
        if sinking.elapsed > 1.1 {
            commands.entity(thing).despawn();
        }
    }
}

pub(crate) fn topple_trees(
    time: Res<Time>,
    mut commands: Commands,
    mut falling: Query<(Entity, &mut Toppling, &mut Transform)>,
) {
    /// Seconds from the last axe blow to the crash.
    const FALL: f32 = 1.15;
    /// Seconds the felled trunk lies where it landed.
    const REST: f32 = 1.6;
    /// Seconds to sink out of the world.
    const SINK: f32 = 1.0;
    for (tree, mut topple, mut transform) in &mut falling {
        topple.elapsed += time.delta_secs();
        let t = topple.elapsed;
        if t < FALL {
            // Gravity's ease: a slow lean that becomes a crash.
            let lean = (t / FALL) * (t / FALL);
            let angle = std::f32::consts::FRAC_PI_2 * 0.97 * lean;
            transform.rotation = Quat::from_axis_angle(topple.axis, angle) * topple.base_rot;
        } else if t < FALL + REST + SINK {
            let sunk = ((t - FALL - REST) / SINK).clamp(0.0, 1.0);
            transform.translation.y = topple.base_y - sunk * 2.4;
        } else {
            commands.entity(tree).despawn();
        }
    }
}

impl FellableTree {
    /// Grown enough to be worth an axe.
    pub fn harvestable(&self) -> bool {
        self.maturity > 0.85
    }
}

/// The shared wardrobe of scenery meshes: a dozen rolled variants per tree
/// kind and a rack of rocks, built once. Entity trees and rocks near the
/// village wear these shared handles so they batch on the GPU — giving
/// every entity its own mesh measurably taxed the frame for each one
/// standing. Which variant a given spot wears comes from a position hash,
/// never from the chunk's dice, so the scatter stream stays untouched.
pub struct ScatterMeshes {
    rocks: Vec<Handle<Mesh>>,
}

impl ScatterMeshes {
    const VARIANTS: i32 = 12;

    pub fn build(meshes: &mut Assets<Mesh>, seed: u32) -> Self {
        let mut rng = Rng::new((seed as u64) ^ 0x7ee5_11b);
        let rocks = (0..Self::VARIANTS)
            .map(|_| {
                let mut builder = MeshBuilder::default();
                bake_rock(&mut builder, Vec3::ZERO, &mut rng);
                meshes.add(builder.build())
            })
            .collect();
        ScatterMeshes { rocks }
    }

    /// A stable per-position pick, independent of any rng stream.
    fn spot(local: Vec3) -> Rng {
        Rng::new(((local.x.to_bits() as u64) << 32) ^ (local.z.to_bits() as u64) ^ 0x5107)
    }

    fn rock(&self, local: Vec3) -> (Handle<Mesh>, f32) {
        let mut spot = Self::spot(local);
        let handle = self.rocks[spot.range_i(0, Self::VARIANTS - 1) as usize].clone();
        (handle, spot.range(0.0, std::f32::consts::TAU))
    }
}

/// Spawns one loose boulder entity, near the settlement where the simulation
/// (and the hand, and the miners) can touch it.
/// The rarer gifts of the land: not food, but what faith and finery are
/// made from. Incense herbs for the shrine's coals, dyeflowers for the
/// weaver's vats — scarce enough that finding a stand of either is news.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SacredKind {
    Incense,
    Dye,
}

#[derive(Component)]
pub struct SacredFlora {
    pub kind: SacredKind,
    pub amount: f32,
}

/// A stand of sacred flora, baked like a bush: smoke-grey herb stalks
/// with pale tips, or a low green clump crowned in vivid blossom.
fn spawn_sacred(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: Handle<StandardMaterial>,
    position: Vec3,
    kind: SacredKind,
    rng: &mut Rng,
) -> Entity {
    let mut builder = MeshBuilder::default();
    match kind {
        SacredKind::Incense => {
            for _ in 0..rng.range_i(4, 6) {
                let h = rng.range(0.5, 0.9);
                builder.push_box(
                    Transform::from_xyz(rng.range(-0.35, 0.35), h * 0.5, rng.range(-0.35, 0.35))
                        .with_scale(Vec3::new(0.08, h, 0.08)),
                    palette::color_at(
                        Tone {
                            ramp: palette::RAMP_FOLIAGE,
                            step: 1,
                        }
                        .palette_index(),
                    ),
                );
                builder.push_box(
                    Transform::from_xyz(rng.range(-0.35, 0.35), h + 0.05, rng.range(-0.35, 0.35))
                        .with_scale(Vec3::splat(0.13)),
                    palette::color_at(
                        Tone {
                            ramp: palette::RAMP_BONE,
                            step: 3,
                        }
                        .palette_index(),
                    ),
                );
            }
        }
        SacredKind::Dye => {
            builder.push_box(
                Transform::from_xyz(0.0, 0.2, 0.0).with_scale(Vec3::new(0.8, 0.4, 0.8)),
                palette::color_at(
                    Tone {
                        ramp: palette::RAMP_FOLIAGE,
                        step: 2,
                    }
                    .palette_index(),
                ),
            );
            for _ in 0..rng.range_i(4, 7) {
                builder.push_box(
                    Transform::from_xyz(
                        rng.range(-0.4, 0.4),
                        rng.range(0.4, 0.65),
                        rng.range(-0.4, 0.4),
                    )
                    .with_scale(Vec3::splat(0.15)),
                    palette::color_at(
                        Tone {
                            ramp: if rng.chance(0.5) {
                                palette::RAMP_CLOTH_BLUE
                            } else {
                                palette::RAMP_CLOTH_RED
                            },
                            step: 4,
                        }
                        .palette_index(),
                    ),
                );
            }
        }
    }
    commands
        .spawn((
            Name::new(match kind {
                SacredKind::Incense => "A stand of incense herb",
                SacredKind::Dye => "A clump of dyeflowers",
            }),
            Mesh3d(meshes.add(builder.build())),
            MeshMaterial3d(material),
            Transform::from_translation(position)
                .with_rotation(Quat::from_rotation_y(rng.range(0.0, std::f32::consts::TAU))),
            SacredFlora {
                kind,
                amount: rng.range(2.0, 4.0),
            },
            crate::hand::PickRadius(1.0),
        ))
        .id()
}

/// A rock's rolled identity, drawn SEPARATELY from the geometry dice so
/// every scatter path — live boulder, baked scenery, or suppressed bare
/// ground — consumes exactly the same random draws. Determinism is what
/// keeps a rebuilt chunk from rearranging the world.
pub(crate) struct RockRoll {
    pub mass: f32,
    pub radius: f32,
    pub girth: f32,
}

/// One rock in six is a real outcrop: shoulder-high, many loads of stone,
/// mined for days before it is gone.
/// Where this chunk's stone gathers, if it gathers here at all.
///
/// One chunk in nine or so carries an outcrop, seated on the stoniest ground it
/// can find within itself, and every stone in it is a real boulder a miner can
/// work. Deterministic from the chunk's own coordinate, so a node is in the same
/// place every time the chunk is drawn and does not wander when a maker walks
/// away and comes back.
fn stone_node(coord: IVec2, terrain: &crate::terrain::Terrain) -> Option<(Vec3, usize)> {
    let mut rng = Rng::new(
        (coord.x as i64 * 73_856_093 ^ coord.y as i64 * 19_349_663).unsigned_abs() ^ 0x5EED_57_0E,
    );
    if !rng.chance(0.11) {
        return None;
    }
    // The stoniest spot in the chunk, out of a handful of tries: a node on a
    // slope reads as an outcrop, and one in a meadow reads as litter.
    let origin = Vec2::new(coord.x as f32 * CHUNK_SIZE, coord.y as f32 * CHUNK_SIZE);
    let mut best: Option<(f32, Vec3)> = None;
    for _ in 0..12 {
        let at = origin + Vec2::new(rng.range(6.0, 58.0), rng.range(6.0, 58.0));
        let high = terrain.height_at(at.x, at.y);
        if high < crate::terrain::WATER_LEVEL + 1.0 {
            continue;
        }
        let stoniness = terrain.slope_at(at.x, at.y);
        if best.as_ref().is_none_or(|(had, _)| stoniness > *had) {
            best = Some((stoniness, Vec3::new(at.x, high, at.y)));
        }
    }
    best.map(|(_, at)| (at, rng.range_i(4, 9) as usize))
}

pub(crate) fn roll_rock(rng: &mut Rng) -> RockRoll {
    let mass = rng.range(90.0, 170.0);
    let radius = rng.range(0.7, 1.05);
    let outcrop = rng.chance(0.18);
    let girth = if outcrop { rng.range(2.2, 3.0) } else { 1.0 };
    RockRoll {
        mass: mass * girth,
        radius: radius * girth,
        girth,
    }
}

pub(crate) fn spawn_boulder(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: Handle<StandardMaterial>,
    local: Vec3,
    rng: &mut Rng,
    roll: RockRoll,
    library: Option<&ScatterMeshes>,
) -> Entity {
    // The dice burn identically either way; the library, when offered,
    // just decides which shared body the rock wears.
    let mut builder = MeshBuilder::default();
    bake_rock(&mut builder, Vec3::ZERO, rng);
    let (mesh, yaw) = match library {
        Some(library) => library.rock(local),
        None => (meshes.add(builder.build()), 0.0),
    };
    let outcrop = roll.girth > 1.0;
    commands
        .spawn((
            Name::new(if outcrop { "An outcrop" } else { "A boulder" }),
            crate::matter::Boulder,
            crate::matter::Matter::boulder(roll.mass, roll.radius),
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_translation(local)
                .with_rotation(Quat::from_rotation_y(yaw))
                .with_scale(Vec3::splat(roll.girth)),
            crate::hand::PickRadius(1.3 * roll.girth),
        ))
        .id()
}

/// Seed for a chunk's scatter.
///
/// Derived from the chunk coordinate so the same chunk always generates the same
/// trees, however many times it is loaded.
pub fn chunk_scatter_seed(world_seed: u32, coord: IVec2) -> u64 {
    (world_seed as u64) << 32
        ^ ((coord.x as u32 as u64) << 16)
        ^ (coord.y as u32 as u64)
        ^ 0x5ca7_7e12
}

/// Leans foliage back and forth. Two sine waves at different rates so the motion
/// does not read as a loop.
/// Ground further than this from the eye holds its lean. Nobody can tell
/// a frozen bush from a swaying one at three hundred paces - but the
/// transform tree can: every bush written per frame is a dirty transform
/// the propagation and the renderer pay for again, and the far ones were
/// most of the whole world's per-frame dirt.
const SWAY_REACH: f32 = 300.0;

fn sway_foliage(
    time: Res<Time>,
    cameras: Query<&GlobalTransform, With<crate::camera::GodCamera>>,
    mut foliage: Query<(&Foliage, &mut Transform, &GlobalTransform)>,
    watch: Res<crate::debug::timings::Timings>,
) {
    let _t = watch.watch("scatter: sway_foliage");
    let t = time.elapsed_secs();
    let Ok(eye) = cameras.single() else {
        return;
    };
    let eye = eye.translation();

    for (foliage, mut transform, stood) in &mut foliage {
        // The read comes before the write on purpose: skipping the write
        // is the whole saving. Last frame's global position is plenty for
        // a reach this coarse.
        if stood.translation().distance_squared(eye) > SWAY_REACH * SWAY_REACH {
            continue;
        }
        let a = (t * 0.9 + foliage.phase).sin();
        let b = (t * 1.7 + foliage.phase * 1.3).sin();
        let lean = (a * 0.7 + b * 0.3) * foliage.amplitude;

        let (yaw, _, _) = transform.rotation.to_euler(EulerRot::YXZ);
        transform.rotation = Quat::from_rotation_y(yaw)
            * Quat::from_rotation_x(lean)
            * Quat::from_rotation_z(lean * 0.6);
    }
}

/// Regrows food over time, so a settlement that strips the map can recover.
pub fn regrow_food(
    time: Res<Time>,
    clock: Res<crate::calendar::WorldClock>,
    mut sources: Query<&mut FoodSource>,
) {
    let dt = time.delta_secs();
    for mut source in &mut sources {
        // Bushes follow the season; winter berries are a memory.
        source.amount = (source.amount + source.regrowth * dt * clock.season().growth())
            .min(FoodSource::CAPACITY);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Scatter points in one chunk — the dice this world rolls per chunk.
    fn points_per_chunk() -> f32 {
        let steps = (CHUNK_SIZE / SCATTER_SPACING) as i32;
        (steps * steps) as f32
    }

    /// A chunk of open meadow must not read as gravel.
    ///
    /// Brett has now asked for this three times, which is twice more than any
    /// number in a game should need. So the claim goes in the suite rather
    /// than in a comment: a sixty-four unit square of empty ground is allowed
    /// a couple of stones in it, not a dozen.
    ///
    /// The pebble is the roll that matters, because it is the LAST branch of
    /// the scatter chain — on open ground, where no tree, bush or herb has
    /// taken the point first, it is the only thing left to be. Its old one in
    /// twenty put about ten stones in every chunk in the world.
    #[test]
    fn open_ground_is_ground_and_not_gravel() {
        // The worst case is the honest one: a meadow, where every point falls
        // through to the last branch.
        let stones = points_per_chunk() * (LOOSE_PEBBLE + LOOSE_BOULDER);
        assert!(
            stones <= 2.0,
            "a chunk of open ground carries about {stones:.1} stones - Brett has \
             called that gravel three times now"
        );
        // And not so bare that stone stops existing as a texture at all: the
        // point was to thin it, not to sweep the world.
        assert!(
            stones > 0.5,
            "only {stones:.2} stones a chunk - the ground has been swept clean, \
             which is a different kind of wrong"
        );
    }

    /// The village's stone does not come off the ground any more.
    ///
    /// A guard on the reasoning rather than on the look: thinning the loose
    /// rock is only affordable BECAUSE the quarries carry the economy. If the
    /// scatter ever goes back to being a supply, one of these two numbers has
    /// drifted and the other should have moved with it.
    #[test]
    fn loose_stone_is_scenery_and_not_a_supply() {
        // Within a miner's reach of the square, at the old rate, there were
        // hundreds of harvestable stones - more than the whole civic ladder
        // ever wanted, which is why nothing needed a quarry.
        let chunks_in_reach = std::f32::consts::PI * (TREE_HARVEST_RADIUS / CHUNK_SIZE).powi(2);
        let harvestable = chunks_in_reach * points_per_chunk() * LOOSE_BOULDER;
        assert!(
            harvestable < 30.0,
            "{harvestable:.0} loose boulders stand within a miner's reach - at \
             a couple of stone each that is the whole civic ladder off the \
             ground, and the quarries are decoration"
        );
    }

    /// Drives [`rebake_groves`] with a grove holding one tree, and reports
    /// whether the bake was started this frame.
    ///
    /// Real app, real system, real `Local` state carried across frames — the
    /// wait is decided by state that only exists BETWEEN runs, so a unit test
    /// on a pure function could not have caught the bug this guards.
    fn grove_bench() -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<DirtyGroves>()
            // The bake carries a stopwatch now, and a system asking for a
            // resource that is not there is an ERROR rather than a no-op.
            .init_resource::<crate::debug::timings::Timings>()
            .add_systems(Update, rebake_groves);
        let grove = app
            .world_mut()
            .spawn((GroveMesh, Transform::default()))
            .id();
        // Two trees, so a fell leaves a survivor and the grove is rebaked
        // rather than buried.
        for offset in [0.0, 3.0] {
            app.world_mut().spawn((
                Transform::from_xyz(offset, 0.0, 0.0),
                TreeBody::at(TreeKind::Broadleaf, offset, 0.0),
                InGrove(grove),
            ));
        }
        (app, grove)
    }

    fn mark_and_run(app: &mut App, grove: Entity) -> bool {
        app.world_mut().resource_mut::<DirtyGroves>().0.push(grove);
        app.update();
        app.world().get::<RebakingGrove>(grove).is_some()
    }

    #[test]
    fn one_felled_tree_rebakes_its_grove_at_once() {
        let (mut app, grove) = grove_bench();
        // A lone mark: the axe. The ghost may not stand for two seconds.
        assert!(
            mark_and_run(&mut app, grove),
            "a grove marked once must start its bake on the same frame, or a \
             felled tree's ghost stands in the merged mesh until the debounce \
             expires - which is the bug this test exists for"
        );
    }

    #[test]
    fn a_churning_grove_still_waits_out_its_quiet() {
        let (mut app, grove) = grove_bench();
        assert!(mark_and_run(&mut app, grove), "first mark bakes promptly");
        // Take the bake away as `collect_groves` would, then mark again while
        // the grove is still warm: this is fire, and it must debounce or a
        // spreading burn pays a GPU upload per tree.
        app.world_mut().entity_mut(grove).remove::<RebakingGrove>();
        assert!(
            !mark_and_run(&mut app, grove),
            "a grove re-marked while warm must wait, not bake again at once"
        );
    }

    #[test]
    fn food_regrowth_is_capped() {
        let mut source = FoodSource {
            amount: 0.0,
            regrowth: 1.0,
        };
        for _ in 0..10_000 {
            source.amount = (source.amount + 1.0 * 0.1).min(FoodSource::CAPACITY);
        }
        assert_eq!(source.amount, FoodSource::CAPACITY);
    }

    #[test]
    fn scatter_placement_avoids_water() {
        let terrain = Terrain::new(2024);
        let mut rng = Rng::stream(1, "scatter-test");
        let mut checked = 0;

        for _ in 0..5_000 {
            let x = rng.range(-2_000.0, 2_000.0);
            let z = rng.range(-2_000.0, 2_000.0);
            let height = terrain.height_at(x, z);
            if height < WATER_LEVEL + 0.3 {
                continue;
            }
            assert!(height >= WATER_LEVEL);
            checked += 1;
        }
        assert!(checked > 100, "test found too few dry samples");
    }

    #[test]
    fn chunk_scatter_seeds_are_stable_and_distinct() {
        // A chunk must regenerate identically after unloading, and neighbours must
        // not share a layout. Both fall out of how the per-chunk seed is derived.
        let a = chunk_scatter_seed(2024, IVec2::new(3, -7));
        assert_eq!(a, chunk_scatter_seed(2024, IVec2::new(3, -7)), "not stable");

        let mut seen = std::collections::HashSet::new();
        for z in -16..16 {
            for x in -16..16 {
                assert!(
                    seen.insert(chunk_scatter_seed(2024, IVec2::new(x, z))),
                    "seed collision at ({x}, {z})",
                );
            }
        }
    }

    #[test]
    fn every_tree_kind_bakes_to_geometry() {
        let mut rng = Rng::new(4);
        for kind in [
            TreeKind::Conifer,
            TreeKind::Broadleaf,
            TreeKind::Birch,
            TreeKind::Palm,
            TreeKind::Snag,
        ] {
            let mut builder = MeshBuilder::default();
            bake_tree(&mut builder, Vec3::ZERO, kind, &mut rng);
            assert!(!builder.is_empty(), "{kind:?} baked to nothing");
        }

        let mut builder = MeshBuilder::default();
        bake_rock(&mut builder, Vec3::ZERO, &mut rng);
        assert!(!builder.is_empty(), "a rock baked to nothing");
    }

    #[test]
    fn every_biome_grows_a_mix_of_trees() {
        // A stand of exactly one species reads as wallpaper.
        for biome in [
            Biome::Temperate,
            Biome::Boreal,
            Biome::Arid,
            Biome::Wetland,
            Biome::Alpine,
        ] {
            let kinds = TreeKind::for_biome(biome);
            assert!(!kinds.is_empty(), "{biome:?} has no trees");

            let distinct: std::collections::HashSet<_> = kinds.iter().collect();
            assert!(distinct.len() >= 2, "{biome:?} grows only one kind of tree");
        }
    }
}
