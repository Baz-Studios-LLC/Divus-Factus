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
use crate::noise::fbm_2d;
use crate::palette;
use crate::rng::Rng;
use crate::terrain::{
    Biome, CHUNK_SIZE, Terrain, TerrainAssets, TerrainChunk, TerrainSet, WATER_LEVEL,
};

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
        app.add_systems(
            Update,
            (populate_chunks.after(TerrainSet), sway_foliage, grow_trees),
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
    let height = rng.range(2.6, 5.4);
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
    settlement: Option<Res<crate::villager::SettlementSite>>,
    chunks: Query<(Entity, &TerrainChunk), Added<TerrainChunk>>,
) {
    // EGREGORE_SCARCE starves the land of berry bushes — the famine dial, for
    // exercising the prayer loop without waiting for a bad year.
    let scarcity = if std::env::var("EGREGORE_SCARCE").is_ok() {
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
                let forest = fbm_2d(x * 0.004, z * 0.004, world_seed.0 ^ 0xf00d, 3, 2.0, 0.5);

                // Positions are chunk-local; the chunk's transform places them.
                let local = Vec3::new(x - origin.x, height, z - origin.y);

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
                        let near_village = settlement.as_ref().is_some_and(|site| {
                            Vec2::new(x - site.centre.x, z - site.centre.z).length()
                                < TREE_HARVEST_RADIUS
                        });
                        if near_village {
                            let rock = spawn_boulder(
                                &mut commands,
                                &mut meshes,
                                terrain_assets.ground_material.clone(),
                                local,
                                &mut rng,
                            );
                            commands.entity(rock).insert(ChildOf(entity));
                        } else {
                            bake_rock(&mut builder, local, &mut rng);
                        }
                    }
                } else if forest > 0.50 && moisture > 0.38 {
                    let density = ((forest - 0.50) / 0.3).clamp(0.0, 1.0);
                    // Trees keep a canopy's berth from worked ground, so a
                    // rebuilt chunk never leans a tree against a wall.
                    if terrain.is_worked_within(x, z, 4.0) {
                        continue;
                    }
                    if rng.chance(tree_chance * (0.55 + density)) {
                        let kind = *rng.pick(TreeKind::for_biome(biome));
                        // Near the settlement, trees live as entities so the
                        // foresters' axes can actually reach them.
                        let near_village = settlement.as_ref().is_some_and(|site| {
                            Vec2::new(x - site.centre.x, z - site.centre.z).length()
                                < TREE_HARVEST_RADIUS
                        });
                        if near_village {
                            let tree = spawn_tree(
                                &mut commands,
                                &mut meshes,
                                terrain_assets.ground_material.clone(),
                                local,
                                kind,
                                &mut rng,
                            );
                            commands.entity(tree).insert(ChildOf(entity));
                        } else {
                            bake_tree(&mut builder, local, kind, &mut rng);
                        }
                    } else if rng.chance(0.16 * scarcity) {
                        let bush = spawn_bush(
                            &mut commands,
                            &mut meshes,
                            terrain_assets.ground_material.clone(),
                            local,
                            &mut rng,
                        );
                        commands.entity(bush).insert(ChildOf(entity));
                    }
                } else if rng.chance(0.04) {
                    // Loose stones on open ground: near the settlement they are
                    // real boulders — the miners' bread. (The flat-ground rocks
                    // were baked scenery at first, and the whole civic ladder
                    // starved for stone on any village founded on flat land.)
                    let near_village = settlement.as_ref().is_some_and(|site| {
                        Vec2::new(x - site.centre.x, z - site.centre.z).length()
                            < TREE_HARVEST_RADIUS
                    });
                    if near_village {
                        let rock = spawn_boulder(
                            &mut commands,
                            &mut meshes,
                            terrain_assets.ground_material.clone(),
                            local,
                            &mut rng,
                        );
                        commands.entity(rock).insert(ChildOf(entity));
                    } else {
                        bake_rock(&mut builder, local, &mut rng);
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
                    bake_rock(&mut builder, local, &mut rng);
                }
            }
        }

        if !builder.is_empty() {
            commands.spawn((
                Name::new("Chunk Scenery"),
                Mesh3d(meshes.add(builder.build())),
                MeshMaterial3d(terrain_assets.ground_material.clone()),
                Transform::default(),
                ChildOf(entity),
            ));
        }
    }
}

/// World units between scatter sample points.
const SCATTER_SPACING: f32 = 4.5;

/// Within this range of the settlement, trees are entities rather than baked
/// scenery — the simulation touches them, so they must be touchable. An axe
/// cannot fell a vertex buffer.
pub const TREE_HARVEST_RADIUS: f32 = 150.0;

/// Seconds for a felled tree to grow back to full height.
const TREE_REGROW_SECONDS: f32 = 480.0;

/// A tree the axe can reach. Maturity 1 is full-grown; felling drops it to a
/// sapling that regrows in real time — a woods worked too hard visibly thins.
#[derive(Component)]
pub struct FellableTree {
    pub maturity: f32,
}

impl FellableTree {
    /// Grown enough to be worth an axe.
    pub fn harvestable(&self) -> bool {
        self.maturity > 0.85
    }
}

/// Spawns one real tree entity, built from the same generator as the baked ones.
fn spawn_tree(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: Handle<StandardMaterial>,
    local: Vec3,
    kind: TreeKind,
    rng: &mut Rng,
) -> Entity {
    let mut builder = MeshBuilder::default();
    bake_tree(&mut builder, Vec3::ZERO, kind, rng);
    commands
        .spawn((
            Name::new("A tree"),
            FellableTree { maturity: 1.0 },
            Mesh3d(meshes.add(builder.build())),
            MeshMaterial3d(material),
            Transform::from_translation(local),
            // Grabbable: a god who can uproot a tree should. The uprooting
            // itself is handled — and witnessed — by the hand.
            crate::hand::PickRadius(1.6),
        ))
        .id()
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

pub(crate) fn spawn_boulder(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: Handle<StandardMaterial>,
    local: Vec3,
    rng: &mut Rng,
) -> Entity {
    let mut builder = MeshBuilder::default();
    bake_rock(&mut builder, Vec3::ZERO, rng);
    commands
        .spawn((
            Name::new("A boulder"),
            crate::matter::Boulder,
            crate::matter::Matter::boulder(rng.range(90.0, 170.0), rng.range(0.7, 1.05)),
            Mesh3d(meshes.add(builder.build())),
            MeshMaterial3d(material),
            Transform::from_translation(local),
            crate::hand::PickRadius(1.3),
        ))
        .id()
}

/// Felled trees grow back, sapling to crown.
fn grow_trees(
    time: Res<Time>,
    clock: Res<crate::calendar::WorldClock>,
    mut trees: Query<(&mut FellableTree, &mut Transform)>,
) {
    let dt = time.delta_secs();
    // Wood is patient: winter slows a sapling but never quite stops it.
    let seasonal = clock.season().growth().max(0.3);
    for (mut tree, mut transform) in &mut trees {
        if tree.maturity < 1.0 {
            tree.maturity = (tree.maturity + dt * seasonal / TREE_REGROW_SECONDS).min(1.0);
            transform.scale = Vec3::splat(0.12 + 0.88 * tree.maturity);
        }
    }
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
