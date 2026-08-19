//! The visible side of a vocation.
//!
//! Work already has an authoritative source in the simulation: a villager is
//! assigned a calling, walks to a Job, and enters `Activity::Working` when at
//! the worksite. This module only makes that truth readable.

use bevy::prelude::*;

use super::work::Vocation;
use super::{Activity, Villager};
use crate::creature::anim::CreatureMotion;
use crate::creature::body::{CreatureAssets, CreatureRig};
use crate::creature::genome::Tone;
use crate::creature::{Corpse, Held};
use crate::palette;

/// The pivot at a worker's hand. The individual boxes below it form the tool.
#[derive(Component, Debug, Clone, Copy)]
pub struct WorkTool {
    pivot: Entity,
    /// The forearm it hangs from. Kept so the pivot can act as a WRIST — see
    /// [`animate_work_tools`] — rather than being re-found through the rig
    /// every frame.
    forearm: Entity,
}

const WOOD: Tone = Tone {
    ramp: palette::RAMP_WOOD,
    step: 2,
};
const DARK_WOOD: Tone = Tone {
    ramp: palette::RAMP_WOOD,
    step: 0,
};
const BONE: Tone = Tone {
    ramp: palette::RAMP_BONE,
    step: 3,
};
const IRON: Tone = Tone {
    ramp: palette::RAMP_STONE,
    step: 1,
};
const HERB: Tone = Tone {
    ramp: palette::RAMP_CLOTH_TEAL,
    step: 2,
};
const FIRE: Tone = Tone {
    ramp: palette::RAMP_CLOTH_RED,
    step: 3,
};
const FAITH: Tone = Tone {
    ramp: palette::RAMP_CLOTH_PURPLE,
    step: 2,
};

/// A short, player-facing name for the thing a trade brings to work.
pub fn tool_label(vocation: Vocation) -> &'static str {
    match vocation {
        Vocation::Gatherer => "gathering basket",
        Vocation::Fisher => "fishing rod",
        Vocation::Hunter => "hunting bow",
        Vocation::Miner => "mining pick",
        Vocation::Forester => "wood axe",
        Vocation::Builder => "builder's hammer",
        Vocation::Farmer => "field hoe",
        Vocation::Cook => "cooking ladle",
        Vocation::Healer => "healer's satchel",
        Vocation::Priest => "prayer censer",
        Vocation::Scholar => "reading book",
        Vocation::Explorer => "wayfarer's staff",
        Vocation::Guard => "guard spear",
    }
}

fn block(
    commands: &mut Commands,
    assets: &CreatureAssets,
    parent: Entity,
    at: Vec3,
    size: Vec3,
    tone: Tone,
    name: &'static str,
) {
    rotated_block(
        commands,
        assets,
        parent,
        at,
        size,
        tone,
        Quat::IDENTITY,
        name,
    );
}

fn rotated_block(
    commands: &mut Commands,
    assets: &CreatureAssets,
    parent: Entity,
    at: Vec3,
    size: Vec3,
    tone: Tone,
    rotation: Quat,
    name: &'static str,
) {
    commands.spawn((
        Name::new(name),
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(assets.material(tone)),
        Transform::from_translation(at)
            .with_rotation(rotation)
            .with_scale(size),
        ChildOf(parent),
    ));
}

fn spawn_tool(
    commands: &mut Commands,
    assets: &CreatureAssets,
    hand: Entity,
    grip: Vec3,
    vocation: Vocation,
) -> Entity {
    // HUNG OFF THE FOREARM, at the hand, so it goes where the arm goes.
    //
    // This used to hang off the creature's ROOT and be dragged to a fixed
    // point near the hip every frame - which is why nothing was ever held.
    // The arm swung, the walk cycle carried it, the clips moved it, and the
    // axe stayed exactly where it was. Parenting it to the joint makes the
    // grip correct by construction rather than by a number that has to be
    // re-tuned for every animation anybody ever adds.
    let pivot = commands
        .spawn((
            Name::new(tool_label(vocation)),
            Transform::from_translation(grip),
            Visibility::Hidden,
            ChildOf(hand),
        ))
        .id();

    match vocation {
        Vocation::Gatherer => {
            block(
                commands,
                assets,
                pivot,
                Vec3::new(0.0, 0.12, 0.0),
                Vec3::new(0.38, 0.20, 0.30),
                WOOD,
                "Basket",
            );
            block(
                commands,
                assets,
                pivot,
                Vec3::new(0.0, 0.35, 0.0),
                Vec3::new(0.28, 0.07, 0.07),
                BONE,
                "Basket Handle",
            );
        }
        Vocation::Fisher => {
            block(
                commands,
                assets,
                pivot,
                Vec3::new(0.0, 0.72, 0.0),
                Vec3::new(0.055, 1.45, 0.055),
                DARK_WOOD,
                "Rod",
            );
            block(
                commands,
                assets,
                pivot,
                Vec3::new(0.0, 1.45, 0.0),
                Vec3::new(0.018, 0.56, 0.018),
                BONE,
                "Fishing Line",
            );
        }
        Vocation::Hunter => {
            // Three angled limbs make a visible bow curve; the pale central
            // string is deliberately straight so it still reads at distance.
            rotated_block(
                commands,
                assets,
                pivot,
                Vec3::new(0.0, 0.63, 0.0),
                Vec3::new(0.06, 0.68, 0.06),
                DARK_WOOD,
                Quat::from_rotation_z(-0.34),
                "Bow Upper",
            );
            rotated_block(
                commands,
                assets,
                pivot,
                Vec3::new(0.0, -0.02, 0.0),
                Vec3::new(0.06, 0.68, 0.06),
                DARK_WOOD,
                Quat::from_rotation_z(0.34),
                "Bow Lower",
            );
            block(
                commands,
                assets,
                pivot,
                Vec3::new(0.0, 0.31, -0.03),
                Vec3::new(0.018, 1.20, 0.018),
                BONE,
                "Bow String",
            );
        }
        Vocation::Guard => {
            block(
                commands,
                assets,
                pivot,
                Vec3::new(0.0, 0.78, 0.0),
                Vec3::new(0.06, 1.58, 0.06),
                DARK_WOOD,
                "Spear Shaft",
            );
            block(
                commands,
                assets,
                pivot,
                Vec3::new(0.0, 1.62, 0.0),
                Vec3::new(0.16, 0.30, 0.10),
                IRON,
                "Spear Head",
            );
        }
        Vocation::Miner => {
            block(
                commands,
                assets,
                pivot,
                Vec3::new(0.0, 0.56, 0.0),
                Vec3::new(0.07, 1.12, 0.07),
                DARK_WOOD,
                "Pick Handle",
            );
            block(
                commands,
                assets,
                pivot,
                Vec3::new(0.0, 1.10, 0.0),
                Vec3::new(0.70, 0.12, 0.12),
                IRON,
                "Pick Head",
            );
        }
        Vocation::Forester => {
            block(
                commands,
                assets,
                pivot,
                Vec3::new(0.0, 0.42, 0.0),
                Vec3::new(0.07, 0.86, 0.07),
                DARK_WOOD,
                "Axe Handle",
            );
            block(
                commands,
                assets,
                pivot,
                Vec3::new(0.0, 0.88, 0.0),
                Vec3::new(0.38, 0.30, 0.11),
                IRON,
                "Axe Head",
            );
        }
        Vocation::Builder => {
            block(
                commands,
                assets,
                pivot,
                Vec3::new(0.0, 0.25, 0.0),
                Vec3::new(0.07, 0.52, 0.07),
                DARK_WOOD,
                "Hammer Handle",
            );
            block(
                commands,
                assets,
                pivot,
                Vec3::new(0.0, 0.53, 0.0),
                Vec3::new(0.42, 0.15, 0.15),
                IRON,
                "Hammer Head",
            );
        }
        Vocation::Farmer => {
            block(
                commands,
                assets,
                pivot,
                Vec3::new(0.0, 0.64, 0.0),
                Vec3::new(0.06, 1.28, 0.06),
                DARK_WOOD,
                "Hoe Handle",
            );
            block(
                commands,
                assets,
                pivot,
                Vec3::new(0.0, 1.22, 0.0),
                Vec3::new(0.48, 0.10, 0.18),
                IRON,
                "Hoe Blade",
            );
        }
        Vocation::Cook => {
            block(
                commands,
                assets,
                pivot,
                Vec3::new(0.0, 0.35, 0.0),
                Vec3::new(0.06, 0.72, 0.06),
                DARK_WOOD,
                "Ladle Handle",
            );
            block(
                commands,
                assets,
                pivot,
                Vec3::new(0.0, 0.76, 0.0),
                Vec3::new(0.25, 0.16, 0.25),
                FIRE,
                "Ladle Bowl",
            );
        }
        Vocation::Healer => {
            block(
                commands,
                assets,
                pivot,
                Vec3::new(0.0, 0.18, 0.0),
                Vec3::new(0.38, 0.28, 0.23),
                HERB,
                "Healer Satchel",
            );
            block(
                commands,
                assets,
                pivot,
                Vec3::new(0.0, 0.40, 0.0),
                Vec3::new(0.20, 0.18, 0.20),
                BONE,
                "Salve Jar",
            );
        }
        Vocation::Priest => {
            block(
                commands,
                assets,
                pivot,
                Vec3::new(0.0, 0.28, 0.0),
                Vec3::new(0.10, 0.58, 0.10),
                FAITH,
                "Censer Chain",
            );
            block(
                commands,
                assets,
                pivot,
                Vec3::new(0.0, 0.08, 0.0),
                Vec3::new(0.28, 0.20, 0.28),
                BONE,
                "Prayer Censer",
            );
        }
        // A slab held flat: a book, and nothing else a village owns looks like
        // one.
        Vocation::Scholar => {
            block(
                commands,
                assets,
                pivot,
                Vec3::new(0.0, 0.10, 0.0),
                Vec3::new(0.30, 0.08, 0.24),
                BONE,
                "Reading Book",
            );
        }
        Vocation::Explorer => {
            block(
                commands,
                assets,
                pivot,
                Vec3::new(0.0, 0.78, 0.0),
                Vec3::new(0.065, 1.56, 0.065),
                DARK_WOOD,
                "Wayfarer Staff",
            );
            block(
                commands,
                assets,
                pivot,
                Vec3::new(0.0, 1.61, 0.0),
                Vec3::new(0.16, 0.18, 0.16),
                BONE,
                "Staff Cap",
            );
        }
    }
    pivot
}

/// Gives every calling one durable, low-poly work prop. The visual is rooted on
/// the villager instead of the rebuildable body rig, so changing clothes cannot
/// lose a tool halfway through a shift.
pub fn equip_work_tools(
    mut commands: Commands,
    assets: Option<Res<CreatureAssets>>,
    // THE RIG IS REQUIRED NOW, because the tool hangs off a joint in it. A
    // villager whose body has not been built yet simply waits a frame, which
    // is what already happens to everything else that reads the rig.
    workers: Query<
        (Entity, &Vocation, &CreatureRig),
        (With<Villager>, Without<WorkTool>, Without<Corpse>),
    >,
) {
    let Some(assets) = assets else {
        return;
    };
    for (entity, vocation, rig) in &workers {
        // The right arm, by the order the body builds them. A person who
        // somehow has no arms carries nothing, rather than carrying it at
        // their feet.
        let Some(arm) = rig.limbs.iter().filter(|limb| limb.is_arm).next_back() else {
            continue;
        };
        let pivot = spawn_tool(&mut commands, &assets, arm.lower, arm.grip, *vocation);
        commands.entity(entity).insert(WorkTool {
            pivot,
            forearm: arm.lower,
        });
    }
}

/// Whether this trade's implement is CARRIED rather than SWUNG.
///
/// A carried tool is held steady against whatever the arm is doing; a swung
/// one rides the arm, because the arm's own motion is the work. Getting this
/// backwards is the difference between a guard shouldering a spear and a
/// guard plowing the ground with one.
fn steadied(vocation: Vocation) -> bool {
    matches!(
        vocation,
        Vocation::Guard | Vocation::Explorer | Vocation::Hunter | Vocation::Priest
    )
}

/// Places an implement at the hand only while its owner is visibly working.
/// Guards and explorers keep their polearm or staff while on their rounds.
pub fn animate_work_tools(
    time: Res<Time>,
    workers: Query<
        (
            &Vocation,
            &Activity,
            &CreatureMotion,
            Option<&Held>,
            &WorkTool,
        ),
        (With<Villager>, Without<Corpse>),
    >,
    // ONE PARAM SET, because both halves want `Transform` and Bevy will not
    // hand out a read and a write over the same component in one system
    // however disjoint the entities happen to be (B0001). So: read every
    // forearm first, then write every pivot.
    mut transforms: ParamSet<(Query<(&mut Transform, &mut Visibility)>, Query<&Transform>)>,
) {
    let time = time.elapsed_secs();
    let mut settled: Vec<(Entity, Option<Quat>, Visibility)> = Vec::new();
    for (vocation, activity, motion, held, tool) in &workers {
        let active = *activity == Activity::Working && motion.speed < 0.25 && held.is_none();
        let carried = matches!(vocation, Vocation::Guard | Vocation::Explorer)
            && *activity == Activity::Working
            && held.is_none();
        if !(active || carried) {
            settled.push((tool.pivot, None, Visibility::Hidden));
            continue;
        }

        let beat =
            time * match vocation {
                Vocation::Forester | Vocation::Miner => 4.7,
                Vocation::Builder => 7.5,
                Vocation::Hunter => 2.1,
                Vocation::Guard => 4.2,
                Vocation::Fisher => 2.4,
                Vocation::Gatherer | Vocation::Farmer => 3.5,
                Vocation::Cook | Vocation::Healer | Vocation::Priest | Vocation::Scholar => 2.8,
                Vocation::Explorer => 1.6,
            } + motion.idle_offset;
        // ONLY THE ANGLE NOW. Where the tool is, is settled by the joint it
        // hangs from; what is left here is how it is HELD.
        let (lean, roll) = match vocation {
            Vocation::Forester | Vocation::Miner => (-0.70 + beat.sin() * 0.95, 0.18),
            Vocation::Builder => (-0.32 + beat.sin().max(0.0) * 0.72, 0.08),
            Vocation::Farmer => (-0.56 + beat.sin() * 0.42, 0.22),
            Vocation::Fisher => (-0.40 + beat.sin() * 0.16, -0.45),
            Vocation::Hunter => (-0.32 + beat.sin().max(0.0) * 0.22, -0.48),
            Vocation::Guard | Vocation::Explorer => (-0.10 + beat.sin() * 0.06, 0.06),
            Vocation::Gatherer => (-0.18 + beat.sin() * 0.24, -0.20),
            Vocation::Cook => (-0.22, beat.sin() * 0.38),
            Vocation::Healer => (-0.15 + beat.sin() * 0.14, 0.12),
            Vocation::Priest => (0.08, beat.sin() * 0.22),
            // Barely moving: a page turns, and that is the whole gesture.
            Vocation::Scholar => (-0.24 + beat.sin() * 0.05, 0.0),
        };
        // THE PIVOT IS A WRIST, and that is the whole trick.
        //
        // A tool parented to the forearm inherits everything the forearm
        // does, and a working elbow bends the better part of sixty degrees.
        // For a SWUNG tool that is exactly right - the axe follows the arm,
        // and the arm's own strike is the chop. For a CARRIED one it is not:
        // a guard walking his rounds does not let a spear rotate to wherever
        // his forearm happens to be pointing, he keeps it up, because that is
        // what a wrist is for. The first cut of this had them dragging their
        // spears through the grass ahead of their own feet.
        //
        // So a carried tool cancels the forearm's bend before applying its
        // own angle; a swung one simply rides along.
        let wrist = if steadied(*vocation) {
            transforms
                .p1()
                .get(tool.forearm)
                .map(|arm| arm.rotation.inverse())
                .unwrap_or(Quat::IDENTITY)
        } else {
            Quat::IDENTITY
        };
        let held_at = wrist * Quat::from_rotation_z(roll) * Quat::from_rotation_x(lean);
        settled.push((tool.pivot, Some(held_at), Visibility::Inherited));
    }

    let mut pivots = transforms.p0();
    for (pivot, held_at, visible) in settled {
        let Ok((mut at, mut showing)) = pivots.get_mut(pivot) else {
            continue;
        };
        *showing = visible;
        if let Some(held_at) = held_at {
            at.rotation = held_at;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_calling_has_a_named_implement() {
        use Vocation::*;
        let callings = [
            Gatherer, Fisher, Hunter, Miner, Forester, Builder, Farmer, Cook, Healer, Priest,
            Explorer, Guard,
        ];
        for calling in callings {
            assert!(!tool_label(calling).is_empty());
        }
        assert_ne!(tool_label(Miner), tool_label(Forester));
        assert_ne!(tool_label(Guard), tool_label(Hunter));
    }
}
