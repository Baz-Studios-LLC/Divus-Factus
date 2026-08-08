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

/// A limb the animator drives: two segments, hinged at the knee or elbow.
#[derive(Clone, Copy, Debug)]
pub struct Limb {
    /// The upper joint - shoulder or hip. Rotating it swings the whole limb.
    pub entity: Entity,
    /// The lower joint - elbow or knee - a child of the upper segment.
    /// Rotating it bends the limb, the way the god hand's fingers curl.
    pub lower: Entity,
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

/// How far above the upper segment's foot the hinge sits, as a fraction of
/// limb thickness. The lower segment is slimmer and starts inside the upper
/// box, so the joint stays covered when it bends instead of opening a gap.
const HINGE_TUCK: f32 = 0.3;

/// Spawns a two-segment limb hanging from a joint: an upper box from the
/// shoulder or hip, and a slimmer lower box from a hinge - the elbow or
/// knee - tucked just inside the upper box's foot. Returns both joints.
#[allow(clippy::too_many_arguments)]
fn spawn_limb(
    commands: &mut Commands,
    assets: &CreatureAssets,
    parent: Entity,
    joint: Vec3,
    thickness: f32,
    length: f32,
    split: f32,
    upper_tone: Tone,
    lower_tone: Tone,
    name: &'static str,
    lower_name: &'static str,
) -> (Entity, Entity) {
    let tuck = thickness * HINGE_TUCK;
    let upper_len = length * split;
    let lower_len = length - upper_len + tuck;
    let upper = spawn_part(
        commands,
        assets,
        parent,
        joint,
        Vec3::new(thickness, upper_len, thickness),
        Vec3::NEG_Y,
        upper_tone,
        name,
    );
    // The hinge lives in the upper joint's space, so the upper swing carries
    // the lower segment with it; the lower box is narrower than the upper on
    // purpose - matching widths would put their faces coplanar and z-fight.
    let lower = spawn_part(
        commands,
        assets,
        upper,
        Vec3::new(0.0, -(upper_len - tuck), 0.0),
        Vec3::new(thickness * 0.82, lower_len, thickness * 0.82),
        Vec3::NEG_Y,
        lower_tone,
        lower_name,
    );
    (upper, lower)
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
        Headwear::Hood => {
            // A hood is a hollow, not a helmet: crown, back and cheeks,
            // face open to the weather. The old single block swallowed
            // the whole head - hooded folk went through life as a box.
            spawn_block(
                commands,
                assets,
                head,
                Vec3::new(0.0, head_size * 1.04, head_size * 0.07),
                Vec3::new(head_size * 1.26, head_size * 0.24, head_size * 1.18),
                genome.accent,
                "Hood",
            );
            spawn_block(
                commands,
                assets,
                head,
                Vec3::new(0.0, head_size * 0.54, head_size * 0.58),
                Vec3::new(head_size * 1.26, head_size * 1.06, head_size * 0.22),
                genome.accent,
                "Hood",
            );
            for side in [-1.0f32, 1.0] {
                spawn_block(
                    commands,
                    assets,
                    head,
                    Vec3::new(side * head_size * 0.59, head_size * 0.5, head_size * 0.1),
                    Vec3::new(head_size * 0.2, head_size * 0.98, head_size * 1.08),
                    genome.accent,
                    "Hood",
                );
            }
        }
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
                // Full enough fore and aft to keep the knees' new swing
                // inside the cloth; the animator meets it halfway by giving
                // robe-wearers a shorter, straighter step.
                Vec3::new(torso_w * 1.34, skirt_len, torso_d * 1.5),
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
                // Ends tucked inside the shoulders: any wider and the tilted
                // band reaches into the channel the arms swing through.
                Vec3::new(torso_w * 1.02, torso_len * 0.24, torso_d * 1.16),
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

    // Legs. Half a cycle apart so they alternate; the thigh takes slightly
    // more than half the length, the way legs actually divide.
    let leg_x = torso_w * 0.26;
    let leg_tone = if genome.trousers {
        genome.accent
    } else {
        genome.skin.shifted(-1)
    };
    for (i, side) in [-1.0f32, 1.0].into_iter().enumerate() {
        let (entity, lower) = spawn_limb(
            commands,
            assets,
            body,
            Vec3::new(side * leg_x, leg_len, 0.0),
            th * 0.85,
            leg_len,
            0.52,
            leg_tone,
            leg_tone,
            "Leg",
            "Shin",
        );
        limbs.push(Limb {
            entity,
            lower,
            phase: i as f32 * std::f32::consts::PI,
            is_arm: false,
        });
    }

    // Arms, counter-phased against the leg on the same side.
    let shoulder_y = leg_len + torso_len - th * 0.4;
    for (i, side) in [-1.0f32, 1.0].into_iter().enumerate() {
        let (entity, lower) = spawn_limb(
            commands,
            assets,
            body,
            Vec3::new(side * (torso_w * 0.5 + th * 0.2), shoulder_y, 0.0),
            th * 0.6,
            arm_len,
            0.52,
            genome.skin,
            genome.skin,
            "Arm",
            "Forearm",
        );
        limbs.push(Limb {
            entity,
            lower,
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
        let (entity, lower) = spawn_limb(
            commands,
            assets,
            body,
            Vec3::new(side * leg_x, leg_len, z),
            th * 0.7,
            leg_len,
            0.55,
            genome.skin.shifted(-1),
            genome.skin.shifted(-1),
            "Leg",
            "Shin",
        );
        limbs.push(Limb {
            entity,
            lower,
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

/// The bench's own word for each joint a clip may key.
///
/// The two programs share no code, so the bond between a clip drawn on the bench
/// and a body raised in the world is these ten words and nothing else. They are
/// SIDED - "arm.l" and not "Arm" twice - because the game names both arms the
/// same thing and a clip that raised "an arm" would be raising either.
pub const JOINTS: [&str; 10] = [
    "body",
    "head",
    "leg.l",
    "leg.l.lower",
    "leg.r",
    "leg.r.lower",
    "arm.l",
    "arm.l.lower",
    "arm.r",
    "arm.r.lower",
];

/// Which canonical joint each entity of a built rig answers to.
///
/// Sides are read off the joint's own X rather than the order the limbs were
/// pushed in, because the order is an implementation detail of the builder and
/// the side is a fact about the body.
pub fn joints_of(rig: &CreatureRig, world: &World) -> Vec<(Entity, &'static str)> {
    name_the_joints(rig, |joint| {
        world
            .get::<Transform>(joint)
            .map(|at| at.translation.x)
            .unwrap_or(0.0)
    })
}

/// The same naming, for a caller holding queries rather than a whole world.
pub fn name_the_joints(
    rig: &CreatureRig,
    side_of: impl Fn(Entity) -> f32,
) -> Vec<(Entity, &'static str)> {
    let mut named = vec![(rig.body, "body"), (rig.head, "head")];
    for limb in &rig.limbs {
        let side = side_of(limb.entity);
        let name = match (limb.is_arm, side < 0.0) {
            (false, true) => ("leg.l", "leg.l.lower"),
            (false, false) => ("leg.r", "leg.r.lower"),
            (true, true) => ("arm.l", "arm.l.lower"),
            (true, false) => ("arm.r", "arm.r.lower"),
        };
        named.push((limb.entity, name.0));
        named.push((limb.lower, name.1));
    }
    named
}

/// Writes the bodies the bench poses on, out of the game's own builder.
///
/// The Atelier and the game share no code, so the bench could only ever have a
/// SECOND villager in it - hand-copied, and wrong the first time a proportion
/// moved. It reads these files instead: the real bodies, built by the real
/// builder from real genomes, with the joints named the way a clip names them.
/// Brett asked for "the ability to test on differnt body types and sizes that
/// appear in the game", and this is the only way to be sure that is what they
/// are.
///
/// By hand, when the body changes:
/// `cargo test bake_the_bodies -- --ignored --nocapture`
#[cfg(test)]
mod bake {
    use super::*;
    use crate::creature::genome::{Age, Sex, Species};
    use crate::rng::Rng;

    /// One drawn box, in the frame of the joint it hangs from.
    struct Slab {
        joint: String,
        at: Vec3,
        turn: Quat,
        size: Vec3,
        rgb: [u8; 3],
    }

    /// Walks a built body, gathering every box under the joint it answers to.
    fn gather(
        world: &World,
        entity: Entity,
        joint: &str,
        from_joint: Transform,
        named: &[(Entity, &'static str)],
        cloth: &std::collections::HashMap<AssetId<StandardMaterial>, [u8; 3]>,
        into: &mut Vec<Slab>,
    ) {
        let Some(children) = world.get::<Children>(entity) else {
            return;
        };
        for child in children.iter() {
            let at = world.get::<Transform>(child).copied().unwrap_or_default();
            // A joint of its own starts a new frame; anything else carries the
            // one it is standing in.
            let (joint, from_joint) = match named.iter().find(|(e, _)| *e == child) {
                Some((_, name)) => (*name, Transform::default()),
                None => (joint, from_joint * at),
            };
            if let Some(paint) = world.get::<MeshMaterial3d<StandardMaterial>>(child) {
                into.push(Slab {
                    joint: joint.to_string(),
                    at: from_joint.translation,
                    turn: from_joint.rotation,
                    size: from_joint.scale,
                    rgb: cloth.get(&paint.0.id()).copied().unwrap_or([255, 255, 255]),
                });
            }
            gather(world, child, joint, from_joint, named, cloth, into);
        }
    }

    #[test]
    #[ignore = "a hand-run export, not a check"]
    fn bake_the_bodies() {
        // A span of the village rather than a sample of it: every age and sex
        // the world raises, so a clip can be tried on the smallest body it will
        // ever play on and the largest.
        let mut wanted: Vec<(Age, Sex)> = Vec::new();
        for age in [Age::Child, Age::Adult, Age::Elder] {
            for sex in [Sex::Female, Sex::Male] {
                wanted.push((age, sex));
            }
        }
        let out = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("atelier/data/bodies");
        std::fs::create_dir_all(&out).expect("the bodies folder");

        let mut found = 0;
        for (age, sex) in wanted {
            // Rolled until the dice give the body wanted. The genome decides age
            // and sex itself, and asking it to do so is cheaper than a second
            // constructor that could disagree with the first.
            let Some(seed) = (1u64..4000).find(|seed| {
                let genome = CreatureGenome::random(Species::Human, &mut Rng::new(*seed));
                genome.age == age && genome.sex == sex
            }) else {
                println!("no seed gave a {age:?} {sex:?}");
                continue;
            };
            let genome = CreatureGenome::random(Species::Human, &mut Rng::new(seed));

            let mut app = App::new();
            app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()));
            app.init_asset::<Mesh>().init_asset::<StandardMaterial>();
            app.add_systems(Startup, init_creature_assets);
            app.update();
            let assets = app.world().resource::<CreatureAssets>();
            let assets = CreatureAssets {
                cube: assets.cube.clone(),
                materials: assets.materials.clone(),
            };
            let world = app.world_mut();
            let root = world
                .spawn((Transform::default(), Visibility::default()))
                .id();
            let mut queue = bevy::ecs::world::CommandQueue::default();
            let rig = {
                let mut commands = Commands::new(&mut queue, world);
                build_body(&mut commands, &assets, root, &genome)
            };
            queue.apply(world);

            let named = joints_of(&rig, world);
            // The colours come out first, as plain numbers. The walk wants the
            // world and the materials at once, and a raw pointer to dodge that
            // is a bargain nobody needs to strike for six files.
            let cloth: std::collections::HashMap<AssetId<StandardMaterial>, [u8; 3]> = world
                .resource::<Assets<StandardMaterial>>()
                .iter()
                .map(|(id, stuff)| {
                    let dye = stuff.base_color.to_srgba();
                    (
                        id,
                        [
                            (dye.red * 255.0).round() as u8,
                            (dye.green * 255.0).round() as u8,
                            (dye.blue * 255.0).round() as u8,
                        ],
                    )
                })
                .collect();
            let mut slabs = Vec::new();
            gather(
                world,
                root,
                "body",
                Transform::default(),
                &named,
                &cloth,
                &mut slabs,
            );

            // The joints themselves, each in its parent's frame.
            let parent_of = |name: &str| -> Option<&'static str> {
                match name {
                    "body" => None,
                    "leg.l.lower" => Some("leg.l"),
                    "leg.r.lower" => Some("leg.r"),
                    "arm.l.lower" => Some("arm.l"),
                    "arm.r.lower" => Some("arm.r"),
                    _ => Some("body"),
                }
            };
            let say = |v: Vec3| format!("[{:.4}, {:.4}, {:.4}]", v.x, v.y, v.z);
            let joints: Vec<String> = JOINTS
                .iter()
                .map(|name| {
                    let entity = named
                        .iter()
                        .find(|(_, given)| given == name)
                        .map(|(entity, _)| *entity);
                    let at = entity
                        .and_then(|entity| world.get::<Transform>(entity))
                        .map(|at| at.translation)
                        .unwrap_or(Vec3::ZERO);
                    let parent = match parent_of(name) {
                        Some(parent) => format!("\"{parent}\""),
                        None => "null".to_string(),
                    };
                    format!(
                        "    {{\"name\": \"{name}\", \"parent\": {parent}, \"at\": {}}}",
                        say(at)
                    )
                })
                .collect();
            let boxes: Vec<String> = slabs
                .iter()
                .map(|slab| {
                    format!(
                        "    {{\"joint\": \"{}\", \"at\": {}, \"size\": {}, \
                         \"turn\": [{:.5}, {:.5}, {:.5}, {:.5}], \"rgb\": [{}, {}, {}]}}",
                        slab.joint,
                        say(slab.at),
                        say(slab.size),
                        slab.turn.x,
                        slab.turn.y,
                        slab.turn.z,
                        slab.turn.w,
                        slab.rgb[0],
                        slab.rgb[1],
                        slab.rgb[2],
                    )
                })
                .collect();

            let name = format!(
                "{}-{}",
                match age {
                    Age::Child => "child",
                    Age::Adult => "adult",
                    Age::Elder => "elder",
                },
                match sex {
                    Sex::Female => "woman",
                    Sex::Male => "man",
                }
            );
            let json = format!(
                "{{\n  \"format\": 1,\n  \"kind\": \"body\",\n  \"name\": \"{name}\",\n  \
                 \"high\": {:.4},\n  \"joints\": [\n{}\n  ],\n  \"boxes\": [\n{}\n  ]\n}}\n",
                rig.height,
                joints.join(",\n"),
                boxes.join(",\n"),
            );
            std::fs::write(out.join(format!("{name}.json")), json).expect("write the body");
            println!(
                "baked {name}: {} boxes, {:.2}m tall",
                slabs.len(),
                rig.height
            );
            found += 1;
        }
        assert!(found > 0, "no bodies were baked at all");
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
    fn every_limb_carries_a_hinge() {
        // Knees and elbows: the lower joint must exist, be distinct, and be a
        // descendant of its own upper joint so the swing carries the bend.
        for species in [Species::Human, Species::Wolf] {
            let (world, rig) = build_in_test_world(species, 11);
            for limb in &rig.limbs {
                assert_ne!(limb.entity, limb.lower, "{species:?} hinge is its own hip");
                let parent = world
                    .get::<ChildOf>(limb.lower)
                    .expect("hinge has no parent")
                    .parent();
                assert_eq!(parent, limb.entity, "{species:?} hinge hangs elsewhere");
                let hinge = world
                    .get::<Transform>(limb.lower)
                    .expect("hinge has no transform");
                assert!(
                    hinge.translation.y < 0.0,
                    "{species:?} hinge sits above its own joint"
                );
            }
        }
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
