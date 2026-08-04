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
) {
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
                    if rng.chance(0.30) {
                        let roll = roll_rock(&mut rng);
                        let near_village = settlement.as_ref().is_some_and(|site| {
                            Vec2::new(x - site.centre.x, z - site.centre.z).length()
                                < TREE_HARVEST_RADIUS
                        });
                        if stripped.is_stripped(x, z) {
                            // Bare ground still burns the rock's dice, or
                            // carrying one rock off would reshuffle every
                            // neighbour on the next chunk rebuild.
                            bake_rock(&mut MeshBuilder::default(), local, &mut rng);
                        } else if near_village {
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
                        } else {
                            bake_rock(&mut builder, local - seated, &mut rng);
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
                } else if rng.chance(0.055) {
                    // Loose stones on open ground: near the settlement they are
                    // real boulders — the miners' bread. (The flat-ground rocks
                    // were baked scenery at first, and the whole civic ladder
                    // starved for stone on any village founded on flat land.)
                    let roll = roll_rock(&mut rng);
                    let near_village = settlement.as_ref().is_some_and(|site| {
                        Vec2::new(x - site.centre.x, z - site.centre.z).length()
                            < TREE_HARVEST_RADIUS
                    });
                    if stripped.is_stripped(x, z) {
                        bake_rock(&mut MeshBuilder::default(), local, &mut rng);
                    } else if near_village {
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
                    } else {
                        bake_rock(&mut builder, local - seated, &mut rng);
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
                } else if rng.chance(0.05) {
                    bake_rock(&mut builder, local - seated, &mut rng);
                }
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
/// it. Fine granularity is what keeps the vertex count down, so the bucket stays
/// small.
const GROVE_SPAN: f32 = 14.0;

/// Within this range of the settlement, trees and rocks are entities rather
/// than baked scenery — the simulation touches them, so they must be
/// touchable. An axe cannot fell a vertex buffer. Deliberately wider than
/// the villagers' working reach (170), so every rock and tree a worker can
/// walk to is real: nothing in arm's reach is set dressing. (Going fully
/// real everywhere was measured and rejected: 13.6k scenery entities
/// halved the release frame rate, 59fps to 31.)
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

/// Groves waiting out their debounce: entity → seconds left until the bake.
///
/// A burning grove is re-marked with every spread, and each finished bake
/// costs a real GPU upload — rebaking eagerly meant paying that upload over
/// and over while the fire crawled. A grove bakes only after it has stayed
/// QUIET this long; the fallen tree's ghost stands in its old merged mesh a
/// beat longer, which is invisible next to a burning stand.
const GROVE_QUIET: f32 = 2.0;

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
    mut dirty: ResMut<DirtyGroves>,
    trees: Query<(&Transform, &TreeBody, &InGrove)>,
    groves: Query<&Transform, (With<GroveMesh>, Without<RebakingGrove>)>,
) {
    // Fresh marks (re)start their grove's quiet clock.
    for grove in dirty.0.drain(..) {
        waiting.insert(grove, GROVE_QUIET);
    }
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
) {
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
fn sway_foliage(time: Res<Time>, mut foliage: Query<(&Foliage, &mut Transform)>) {
    let t = time.elapsed_secs();

    for (foliage, mut transform) in &mut foliage {
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
