//! Builds a creature's body as a hierarchy of boxes.
//!
//! Every part is the same unit cube mesh, scaled and tinted from the shared palette
//! material cache — so a hundred creatures cost one mesh and eighty materials, not
//! one of each per creature.
//!
//! Limb entities sit *at their joint*, with the visible box parented underneath and
//! offset by half its length. Rotating the limb entity therefore swings the box
//! about the shoulder or hip, which is what makes the procedural gait in
//! [`super::anim`] possible without a skinned mesh or an animation asset.

use bevy::prelude::*;

use super::genome::{CreatureGenome, Garment, HairStyle, Headwear, Tone};
use crate::palette;

/// Shared mesh and material handles for every creature in the world.
#[derive(Resource)]
pub struct CreatureAssets {
    pub cube: Handle<Mesh>,
    /// One material per palette entry, indexed by [`palette::palette_index`].
    pub materials: Vec<Handle<StandardMaterial>>,
}

impl CreatureAssets {
    pub fn material(&self, tone: Tone) -> Handle<StandardMaterial> {
        self.materials[tone.palette_index().min(self.materials.len() - 1)].clone()
    }
}

pub fn init_creature_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));

    let materials = (0..palette::PALETTE_LEN)
        .map(|i| {
            materials.add(StandardMaterial {
                base_color: palette::color_at(i),
                perceptual_roughness: 0.92,
                reflectance: 0.03,
                ..default()
            })
        })
        .collect();

    commands.insert_resource(CreatureAssets { cube, materials });
}

/// A limb the animator drives.
#[derive(Clone, Copy, Debug)]
pub struct Limb {
    pub entity: Entity,
    /// Offset into the stride cycle, in radians. Diagonal pairs share a phase.
    pub phase: f32,
    /// Arms swing counter to legs and at reduced amplitude.
    pub is_arm: bool,
}

/// Entity references the animator needs. Built once, at spawn.
#[derive(Component)]
pub struct CreatureRig {
    /// Node that carries whole-body bob, sway and lean.
    pub body: Entity,
    pub head: Entity,
    /// The head's rotation at rest.
    ///
    /// The animator composes its look-around on top of this rather than replacing
    /// the transform outright. Overwriting it wiped the counter-rotation that keeps
    /// a quadruped's head level on its angled neck, and left every animal in the
    /// world staring at the sky.
    pub head_rest: Quat,
    pub limbs: Vec<Limb>,
    pub tail: Option<Entity>,
    /// Standing height, cached so the animator does not re-derive it every frame.
    pub height: f32,
}

/// Spawns a box hanging from a joint.
///
/// `size` is the box's dimensions; the joint is at the entity's own origin and the
/// box extends in `direction` from it.
fn spawn_part(
    commands: &mut Commands,
    assets: &CreatureAssets,
    parent: Entity,
    joint: Vec3,
    size: Vec3,
    direction: Vec3,
    tone: Tone,
    name: &'static str,
) -> Entity {
    let joint_entity = commands
        .spawn((
            Name::new(name),
            Transform::from_translation(joint),
            Visibility::default(),
            ChildOf(parent),
        ))
        .id();

    // The mesh hangs half its extent along `direction` from the joint.
    let offset = direction * size * 0.5;
    commands.spawn((
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(assets.material(tone)),
        Transform::from_translation(offset).with_scale(size),
        ChildOf(joint_entity),
    ));

    joint_entity
}

/// Shoulder width of a biped's torso.
pub fn biped_torso_width(genome: &CreatureGenome) -> f32 {
    genome.thickness() * 2.6
}

/// Edge length of a biped's head.
///
/// Widened when necessary so the head is always clearly broader than the torso.
/// The two boxes intersect, and if their widths coincide their side faces become
/// exactly coplanar and z-fight.
pub fn biped_head_size(genome: &CreatureGenome) -> f32 {
    (genome.proportions.head_size * genome.height()).max(biped_torso_width(genome) * 1.12)
}

/// How much larger than the head each hair slab is, in plan.
pub fn hair_inset(layer: u8) -> f32 {
    1.07 + layer as f32 * 0.06
}

/// Spawns a box as a child of `parent`, centred at `offset`.
///
/// Used for decoration that is not a joint — hair, clothing trim, packs.
fn spawn_block(
    commands: &mut Commands,
    assets: &CreatureAssets,
    parent: Entity,
    offset: Vec3,
    size: Vec3,
    tone: Tone,
    name: &'static str,
) {
    commands.spawn((
        Name::new(name),
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(assets.material(tone)),
        Transform::from_translation(offset).with_scale(size),
        ChildOf(parent),
    ));
}

/// Hair, beard and headwear.
///
/// Everything here intersects the head cube, so every piece is sized to a different
/// cross-section than the head and than each other. Two intersecting boxes that
/// share a face plane z-fight, and on a head that shows up as a flickering patch.
fn spawn_head_features(
    commands: &mut Commands,
    assets: &CreatureAssets,
    genome: &CreatureGenome,
    head: Entity,
    head_size: f32,
) {
    let hair = genome.hair;

    match genome.hair_style {
        HairStyle::Bald => {}

        HairStyle::Cropped => {
            spawn_block(
                commands,
                assets,
                head,
                Vec3::new(0.0, head_size * 0.95, 0.0),
                Vec3::new(head_size * 1.07, head_size * 0.22, head_size * 1.07),
                hair,
                "Hair",
            );
        }

        HairStyle::Topknot => {
            // Stacked slabs, each narrower and higher than the last.
            for layer in 0..genome.hair_volume.max(1) {
                let inset = hair_inset(layer) - layer as f32 * 0.16;
                let y = head_size * (0.92 + layer as f32 * 0.26);
                spawn_block(
                    commands,
                    assets,
                    head,
                    Vec3::new(0.0, y, 0.0),
                    Vec3::new(head_size * inset, head_size * 0.3, head_size * inset),
                    hair,
                    "Hair",
                );
            }
        }

        HairStyle::Long => {
            // Crown, plus a fall down the back of the head and neck.
            spawn_block(
                commands,
                assets,
                head,
                Vec3::new(0.0, head_size * 0.94, 0.0),
                Vec3::new(head_size * 1.08, head_size * 0.26, head_size * 1.08),
                hair,
                "Hair",
            );
            spawn_block(
                commands,
                assets,
                head,
                Vec3::new(0.0, head_size * 0.28, head_size * 0.52),
                Vec3::new(head_size * 0.86, head_size * 1.1, head_size * 0.3),
                hair,
                "Hair",
            );
        }

        HairStyle::Tufts => {
            spawn_block(
                commands,
                assets,
                head,
                Vec3::new(0.0, head_size * 0.93, 0.0),
                Vec3::new(head_size * 1.06, head_size * 0.2, head_size * 1.06),
                hair,
                "Hair",
            );
            for side in [-1.0f32, 1.0] {
                spawn_block(
                    commands,
                    assets,
                    head,
                    Vec3::new(side * head_size * 0.58, head_size * 0.6, 0.0),
                    Vec3::new(head_size * 0.28, head_size * 0.5, head_size * 0.72),
                    hair,
                    "Hair",
                );
            }
        }
    }

    // Beard: protrudes forward past the face so it breaks the head silhouette.
    // Forward is -Z.
    if genome.beard {
        spawn_block(
            commands,
            assets,
            head,
            Vec3::new(0.0, head_size * 0.24, -head_size * 0.46),
            Vec3::new(head_size * 0.54, head_size * 0.44, head_size * 0.34),
            hair,
            "Beard",
        );
    }

    match genome.headwear {
        Headwear::None => {}
        Headwear::Cap => spawn_block(
            commands,
            assets,
            head,
            Vec3::new(0.0, head_size * 1.06, 0.0),
            Vec3::new(head_size * 1.22, head_size * 0.2, head_size * 1.22),
            genome.accent,
            "Cap",
        ),
        Headwear::Hood => spawn_block(
            commands,
            assets,
            head,
            Vec3::new(0.0, head_size * 0.6, head_size * 0.1),
            Vec3::new(head_size * 1.26, head_size * 1.1, head_size * 1.26),
            genome.accent,
            "Hood",
        ),
        Headwear::Band => spawn_block(
            commands,
            assets,
            head,
            Vec3::new(0.0, head_size * 0.74, 0.0),
            Vec3::new(head_size * 1.13, head_size * 0.16, head_size * 1.13),
            genome.accent,
            "Band",
        ),
    }
}

/// Clothing beyond the torso block: skirts, sashes, belts and packs.
#[allow(clippy::too_many_arguments)]
fn spawn_garment_features(
    commands: &mut Commands,
    assets: &CreatureAssets,
    genome: &CreatureGenome,
    body: Entity,
    leg_len: f32,
    torso_len: f32,
    torso_w: f32,
    torso_d: f32,
) {
    match genome.garment {
        Garment::Tunic => {}

        Garment::Robe => {
            // Hangs from the waist over the thighs. Wider than the torso so their
            // side faces never coincide, and wide enough to swallow the leg tops.
            let skirt_len = leg_len * 0.55;
            spawn_block(
                commands,
                assets,
                body,
                Vec3::new(0.0, leg_len - skirt_len * 0.5, 0.0),
                Vec3::new(torso_w * 1.26, skirt_len, torso_d * 1.3),
                genome.cloth.shifted(-1),
                "Robe",
            );
        }

        Garment::Wrap => {
            // A sash across the chest, tilted so it does not read as another belt.
            let sash = commands
                .spawn((
                    Name::new("Sash"),
                    Transform::from_xyz(0.0, leg_len + torso_len * 0.55, 0.0)
                        .with_rotation(Quat::from_rotation_z(0.42)),
                    Visibility::default(),
                    ChildOf(body),
                ))
                .id();
            spawn_block(
                commands,
                assets,
                sash,
                Vec3::ZERO,
                Vec3::new(torso_w * 1.5, torso_len * 0.22, torso_d * 1.16),
                genome.accent,
                "Sash",
            );
        }
    }

    if genome.belt {
        spawn_block(
            commands,
            assets,
            body,
            Vec3::new(0.0, leg_len + torso_len * 0.12, 0.0),
            Vec3::new(torso_w * 1.11, torso_len * 0.14, torso_d * 1.11),
            genome.accent.shifted(-1),
            "Belt",
        );
    }

    if genome.satchel {
        // Sits proud of the back, so it shows in silhouette from above.
        spawn_block(
            commands,
            assets,
            body,
            Vec3::new(0.0, leg_len + torso_len * 0.6, torso_d * 0.72),
            Vec3::new(torso_w * 0.66, torso_len * 0.46, torso_d * 0.6),
            genome.accent.shifted(1),
            "Satchel",
        );
    }
}

/// Builds the body for `genome` under `root`, returning the rig.
pub fn build_body(
    commands: &mut Commands,
    assets: &CreatureAssets,
    root: Entity,
    genome: &CreatureGenome,
) -> CreatureRig {
    if genome.species.is_biped() {
        build_biped(commands, assets, root, genome)
    } else {
        build_quadruped(commands, assets, root, genome)
    }
}

fn build_biped(
    commands: &mut Commands,
    assets: &CreatureAssets,
    root: Entity,
    genome: &CreatureGenome,
) -> CreatureRig {
    let h = genome.height();
    let p = &genome.proportions;
    let th = genome.thickness();

    let leg_len = p.leg_length * h;
    let torso_len = p.torso_length * h;
    let arm_len = p.arm_length * h;
    let neck_len = p.neck_length * h;

    // Body node: everything below it inherits bob, sway and lean.
    let body = commands
        .spawn((
            Name::new("Body"),
            Transform::default(),
            Visibility::default(),
            ChildOf(root),
        ))
        .id();

    let torso_w = biped_torso_width(genome);
    let torso_d = th * 1.5;

    // The head is sunk into the torso, so the two boxes intersect. See
    // `biped_head_size` for why it is forced to be the wider of the two.
    let head_size = biped_head_size(genome);

    // Torso rises from the hips.
    // The torso is not animated independently; the body node carries its motion.
    spawn_part(
        commands,
        assets,
        body,
        Vec3::new(0.0, leg_len, 0.0),
        Vec3::new(torso_w, torso_len, torso_d),
        Vec3::Y,
        genome.cloth,
        "Torso",
    );

    // Head sits above the torso, sunk slightly into it. A visible neck gap at this
    // scale reads as a floating head rather than as anatomy.
    let head = spawn_part(
        commands,
        assets,
        body,
        Vec3::new(0.0, leg_len + torso_len + neck_len - head_size * 0.12, 0.0),
        Vec3::splat(head_size),
        Vec3::Y,
        genome.skin,
        "Head",
    );

    spawn_head_features(commands, assets, genome, head, head_size);
    spawn_garment_features(
        commands, assets, genome, body, leg_len, torso_len, torso_w, torso_d,
    );

    let mut limbs = Vec::with_capacity(4);

    // Legs. Half a cycle apart so they alternate.
    let leg_x = torso_w * 0.26;
    for (i, side) in [-1.0f32, 1.0].into_iter().enumerate() {
        let entity = spawn_part(
            commands,
            assets,
            body,
            Vec3::new(side * leg_x, leg_len, 0.0),
            Vec3::new(th * 0.85, leg_len, th * 0.85),
            Vec3::NEG_Y,
            if genome.trousers {
                genome.accent
            } else {
                genome.skin.shifted(-1)
            },
            "Leg",
        );
        limbs.push(Limb {
            entity,
            phase: i as f32 * std::f32::consts::PI,
            is_arm: false,
        });
    }

    // Arms, counter-phased against the leg on the same side.
    let shoulder_y = leg_len + torso_len - th * 0.4;
    for (i, side) in [-1.0f32, 1.0].into_iter().enumerate() {
        let entity = spawn_part(
            commands,
            assets,
            body,
            Vec3::new(side * (torso_w * 0.5 + th * 0.2), shoulder_y, 0.0),
            Vec3::new(th * 0.6, arm_len, th * 0.6),
            Vec3::NEG_Y,
            genome.skin,
            "Arm",
        );
        limbs.push(Limb {
            entity,
            phase: (i as f32 + 1.0) * std::f32::consts::PI,
            is_arm: true,
        });
    }

    CreatureRig {
        body,
        head,
        head_rest: Quat::IDENTITY,
        limbs,
        tail: None,
        height: h,
    }
}

fn build_quadruped(
    commands: &mut Commands,
    assets: &CreatureAssets,
    root: Entity,
    genome: &CreatureGenome,
) -> CreatureRig {
    let h = genome.height();
    let p = &genome.proportions;
    let th = genome.thickness();

    let leg_len = p.leg_length * h;
    let body_len = p.torso_length * h * 1.7;
    let head_size = p.head_size * h;
    let neck_len = p.neck_length * h;

    let body = commands
        .spawn((
            Name::new("Body"),
            Transform::default(),
            Visibility::default(),
            ChildOf(root),
        ))
        .id();

    let barrel_w = th * 2.0;
    let barrel_h = th * 2.1;

    // Barrel runs along Z, with -Z as forward.
    commands.spawn((
        Name::new("Barrel"),
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(assets.material(genome.skin)),
        Transform::from_xyz(0.0, leg_len + barrel_h * 0.5, 0.0)
            .with_scale(Vec3::new(barrel_w, barrel_h, body_len)),
        ChildOf(body),
    ));

    // Neck angles up and *forward* from the front of the barrel. Forward is -Z, and
    // a positive rotation about X tips the neck's local +Y toward +Z — backwards,
    // over the animal's own back. The sign has to be negative.
    let neck_pitch = -0.55;
    let neck_base = Vec3::new(0.0, leg_len + barrel_h * 0.8, -body_len * 0.45);
    let neck = commands
        .spawn((
            Name::new("Neck"),
            Transform::from_translation(neck_base).with_rotation(Quat::from_rotation_x(neck_pitch)),
            Visibility::default(),
            ChildOf(body),
        ))
        .id();
    commands.spawn((
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(assets.material(genome.skin)),
        Transform::from_xyz(0.0, neck_len * 0.5, 0.0).with_scale(Vec3::new(
            th * 1.1,
            neck_len.max(th),
            th * 1.1,
        )),
        ChildOf(neck),
    ));

    // Head at the end of the neck, counter-rotated back to level. This rest pose is
    // recorded on the rig so the animator can compose its look-around on top of it
    // instead of overwriting it.
    let head_rest = Quat::from_rotation_x(-neck_pitch);
    let head = commands
        .spawn((
            Name::new("Head"),
            Transform::from_xyz(0.0, neck_len.max(th), 0.0).with_rotation(head_rest),
            Visibility::default(),
            ChildOf(neck),
        ))
        .id();

    // The muzzle runs forward from the head joint, so everything attached to the
    // head is placed relative to the joint rather than to the mesh.
    commands.spawn((
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(assets.material(genome.skin)),
        Transform::from_xyz(0.0, head_size * 0.1, -head_size * 0.45).with_scale(Vec3::new(
            head_size * 0.85,
            head_size * 0.8,
            head_size * 1.25,
        )),
        ChildOf(head),
    ));

    if genome.horns {
        let horn_tone = Tone {
            ramp: palette::RAMP_BONE,
            step: 3,
        };
        for side in [-1.0f32, 1.0] {
            commands.spawn((
                Name::new("Horn"),
                Mesh3d(assets.cube.clone()),
                MeshMaterial3d(assets.material(horn_tone)),
                // Anchored to the crown of the head mesh, not floating above the
                // joint: horns that miss the head read as debris in mid-air.
                Transform::from_xyz(side * head_size * 0.26, head_size * 0.42, -head_size * 0.3)
                    .with_rotation(Quat::from_rotation_z(side * 0.4))
                    .with_scale(Vec3::new(
                        head_size * 0.16,
                        head_size * 0.62,
                        head_size * 0.16,
                    )),
                ChildOf(head),
            ));
        }
    }

    // Four legs. Diagonal pairs share a phase, which is what makes a quadruped
    // read as trotting rather than hopping.
    let mut limbs = Vec::with_capacity(4);
    let leg_x = barrel_w * 0.34;
    let leg_z = body_len * 0.36;
    let pi = std::f32::consts::PI;

    for (side, z, phase) in [
        (-1.0f32, -leg_z, 0.0),
        (1.0, -leg_z, pi),
        (-1.0, leg_z, pi),
        (1.0, leg_z, 0.0),
    ] {
        let entity = spawn_part(
            commands,
            assets,
            body,
            Vec3::new(side * leg_x, leg_len, z),
            Vec3::new(th * 0.7, leg_len, th * 0.7),
            Vec3::NEG_Y,
            genome.skin.shifted(-1),
            "Leg",
        );
        limbs.push(Limb {
            entity,
            phase,
            is_arm: false,
        });
    }

    let tail = genome.tail.then(|| {
        spawn_part(
            commands,
            assets,
            body,
            Vec3::new(0.0, leg_len + barrel_h * 0.7, body_len * 0.5),
            Vec3::new(th * 0.5, th * 2.2, th * 0.5),
            Vec3::NEG_Y,
            genome.skin.shifted(1),
            "Tail",
        )
    });

    CreatureRig {
        body,
        head,
        head_rest,
        limbs,
        tail,
        height: h,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::creature::genome::Species;
    use crate::rng::Rng;

    /// Spawns a creature into a bare world and returns its rig.
    fn build_in_test_world(species: Species, seed: u64) -> (World, CreatureRig) {
        let mut world = World::new();

        // Minimal stand-in assets: the builder only needs handles, not real assets.
        let assets = CreatureAssets {
            cube: Handle::default(),
            materials: vec![Handle::default(); palette::PALETTE_LEN],
        };

        let genome = CreatureGenome::random(species, &mut Rng::new(seed));
        let root = world
            .spawn((Transform::default(), Visibility::default()))
            .id();

        let mut queue = bevy::ecs::world::CommandQueue::default();
        let rig = {
            let mut commands = Commands::new(&mut queue, &world);
            build_body(&mut commands, &assets, root, &genome)
        };
        queue.apply(&mut world);

        (world, rig)
    }

    #[test]
    fn heads_are_never_flush_with_torsos() {
        // Regression: head and torso widths were rolled independently and could land
        // on the same value. The boxes intersect, so identical widths put their side
        // faces exactly coplanar and they z-fought — visible as flickering patches on
        // villagers' heads.
        for seed in 0..2_000 {
            let genome = CreatureGenome::random(Species::Human, &mut Rng::new(seed));
            let head = biped_head_size(&genome);
            let torso = biped_torso_width(&genome);
            assert!(
                head > torso * 1.1,
                "seed {seed}: head {head} vs torso {torso}",
            );
        }
    }

    #[test]
    fn hair_encloses_the_head_without_matching_it() {
        // Each slab has to be strictly wider than the head in plan, and clear its
        // crown, or its faces sit flush against the head's and z-fight.
        for layer in 0..4u8 {
            let inset = hair_inset(layer);
            assert!(inset > 1.0, "layer {layer} does not clear the head sides");
        }

        // Vertically: the lowest slab must overlap the head (no floating gap) while
        // its top clears the crown.
        let head_size = 1.0;
        for layer in 0..4u8 {
            let centre = head_size * (0.92 + layer as f32 * 0.2);
            let half = head_size * 0.15;
            assert!(
                centre + half > head_size,
                "layer {layer} top does not clear"
            );
            if layer == 0 {
                assert!(centre - half < head_size, "layer 0 floats above the head");
            }
        }
    }

    #[test]
    fn bipeds_get_two_legs_and_two_arms() {
        let (_world, rig) = build_in_test_world(Species::Human, 1);
        assert_eq!(rig.limbs.len(), 4);
        assert_eq!(rig.limbs.iter().filter(|l| l.is_arm).count(), 2);
        assert_eq!(rig.limbs.iter().filter(|l| !l.is_arm).count(), 2);
        assert!(rig.tail.is_none());
    }

    #[test]
    fn quadrupeds_get_four_legs_and_a_tail() {
        for species in [Species::Deer, Species::Wolf, Species::Boar] {
            let (_world, rig) = build_in_test_world(species, 2);
            assert_eq!(rig.limbs.len(), 4, "{species:?}");
            assert!(rig.limbs.iter().all(|l| !l.is_arm));
            assert!(rig.tail.is_some(), "{species:?} has no tail");
        }
    }

    #[test]
    fn legs_alternate_in_phase() {
        // Both legs in phase would make a creature hop rather than walk.
        let (_world, rig) = build_in_test_world(Species::Human, 3);
        let legs: Vec<f32> = rig
            .limbs
            .iter()
            .filter(|l| !l.is_arm)
            .map(|l| l.phase)
            .collect();
        assert!((legs[0] - legs[1]).abs() > 1.0);
    }

    #[test]
    fn quadruped_legs_form_diagonal_pairs() {
        let (_world, rig) = build_in_test_world(Species::Wolf, 4);
        let phases: Vec<f32> = rig.limbs.iter().map(|l| l.phase).collect();
        // Front-left matches back-right; front-right matches back-left.
        assert!((phases[0] - phases[3]).abs() < 1e-5);
        assert!((phases[1] - phases[2]).abs() < 1e-5);
        assert!((phases[0] - phases[1]).abs() > 1.0);
    }

    #[test]
    fn quadrupeds_record_a_levelling_rest_pose() {
        // Regression: the animator used to overwrite the head's transform outright,
        // discarding the counter-rotation that keeps a quadruped's head level on its
        // angled neck. Every animal in the world ended up staring at the sky.
        for species in [Species::Deer, Species::Wolf, Species::Boar] {
            let (_world, rig) = build_in_test_world(species, 5);
            assert!(
                rig.head_rest.angle_between(Quat::IDENTITY) > 0.1,
                "{species:?} has no levelling rest pose",
            );
        }

        // Bipeds hold their heads level already, so theirs is identity.
        let (_world, rig) = build_in_test_world(Species::Human, 5);
        assert!(rig.head_rest.angle_between(Quat::IDENTITY) < 1e-5);
    }

    #[test]
    fn rig_height_matches_the_genome() {
        let genome = CreatureGenome::random(Species::Human, &mut Rng::new(7));
        let (_world, rig) = build_in_test_world(Species::Human, 7);
        assert!((rig.height - genome.height()).abs() < 1e-5);
    }

    #[test]
    fn every_part_is_reachable_from_the_root() {
        // A part spawned without a parent would float at the world origin.
        let (world, rig) = build_in_test_world(Species::Human, 8);
        for limb in &rig.limbs {
            assert!(world.get::<ChildOf>(limb.entity).is_some());
        }
        assert!(world.get::<ChildOf>(rig.head).is_some());
        assert!(world.get::<ChildOf>(rig.body).is_some());
    }
}
