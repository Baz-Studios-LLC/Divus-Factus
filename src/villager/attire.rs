//! Attire: the trades wear their work.
//!
//! Brett: "I want the villagers to wear outfits depending on their jobs."
//! A calling comes with a livery — cloth, trim, headwear, cut and belt —
//! and taking up the work means changing into it. The genome always
//! carried the clothes; the livery overwrites the rolled wardrobe and the
//! body is rebuilt wearing it. Skin, hair, beard and build stay their own:
//! the work dresses a person, it does not replace them.
//!
//! One glance at a square should read as a roster — the sea-blue fisher,
//! the hooded hunter, the purple priest — the same way the nameplates'
//! faith colours read as a congregation.

use bevy::prelude::*;

use super::work::Vocation;
use crate::creature::Corpse;
use crate::creature::body::{CreatureAssets, CreatureRig, build_body};
use crate::creature::genome::{CreatureGenome, Garment, Headwear, Tone};
use crate::palette;

/// What a calling wears.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Livery {
    pub cloth: Tone,
    pub accent: Tone,
    pub headwear: Headwear,
    pub garment: Garment,
    pub belt: bool,
}

fn tone(ramp: usize, step: usize) -> Tone {
    Tone { ramp, step }
}

/// The wardrobe: one livery per calling, distinct at a glance.
///
/// Cloth carries the trade's colour, headwear its silhouette, the cut its
/// station: tunics for the field trades, wraps for the crafts that work a
/// sash of tools across the chest, robes for the callings of the soul.
pub fn livery(vocation: Vocation) -> Livery {
    use Vocation::*;
    let (cloth, accent, headwear, garment) = match vocation {
        // The field trades, in the colours of what they bring home.
        Gatherer => (
            tone(palette::RAMP_CLOTH_GREEN, 2),
            tone(palette::RAMP_EARTH, 2),
            Headwear::Band,
            Garment::Tunic,
        ),
        Forester => (
            tone(palette::RAMP_FOLIAGE, 1),
            tone(palette::RAMP_WOOD, 2),
            Headwear::Cap,
            Garment::Tunic,
        ),
        Fisher => (
            tone(palette::RAMP_CLOTH_BLUE, 2),
            tone(palette::RAMP_BONE, 3),
            Headwear::Cap,
            Garment::Tunic,
        ),
        Hunter => (
            tone(palette::RAMP_EARTH, 2),
            tone(palette::RAMP_WOOD, 1),
            Headwear::Hood,
            Garment::Tunic,
        ),
        Farmer => (
            tone(palette::RAMP_CLOTH_GOLD, 2),
            tone(palette::RAMP_EARTH, 2),
            Headwear::Cap,
            Garment::Tunic,
        ),
        // The crafts, sashed with the tools of the work.
        Miner => (
            tone(palette::RAMP_STONE, 2),
            tone(palette::RAMP_EARTH, 1),
            Headwear::Cap,
            Garment::Tunic,
        ),
        Builder => (
            tone(palette::RAMP_CLOTH_RUST, 2),
            tone(palette::RAMP_WOOD, 3),
            Headwear::None,
            Garment::Wrap,
        ),
        Cook => (
            tone(palette::RAMP_BONE, 3),
            tone(palette::RAMP_CLOTH_RED, 2),
            Headwear::Band,
            Garment::Wrap,
        ),
        // The callings of the soul and the body, robed.
        Healer => (
            tone(palette::RAMP_CLOTH_TEAL, 2),
            tone(palette::RAMP_BONE, 3),
            Headwear::Band,
            Garment::Robe,
        ),
        Priest => (
            tone(palette::RAMP_CLOTH_PURPLE, 2),
            tone(palette::RAMP_BONE, 3),
            Headwear::Hood,
            Garment::Robe,
        ),
        // The road and the wall.
        Explorer => (
            tone(palette::RAMP_CLOTH_SABLE, 2),
            tone(palette::RAMP_CLOTH_GOLD, 2),
            Headwear::Hood,
            Garment::Tunic,
        ),
        Guard => (
            tone(palette::RAMP_STONE, 1),
            tone(palette::RAMP_CLOTH_RED, 2),
            Headwear::Cap,
            Garment::Wrap,
        ),
    };
    Livery {
        cloth,
        accent,
        headwear,
        garment,
        belt: true,
    }
}

/// Whether this genome is already dressed in the livery.
fn wearing(genome: &CreatureGenome, wear: &Livery) -> bool {
    genome.cloth == wear.cloth
        && genome.accent == wear.accent
        && genome.headwear == wear.headwear
        && genome.garment == wear.garment
        && genome.belt == wear.belt
}

/// Taking up a calling means changing into its clothes.
///
/// The body is REBUILT in the livery — the same machinery that grew it —
/// because parts and their tones are baked at build time. Everything that
/// hangs on the villager's root (shouldered loads, prayer motes, plates)
/// survives; only the rig's own subtree is replaced. Idempotent on
/// purpose: a body already dressed for its work is left alone, so loading
/// a save does not rebuild every adult in the world.
///
/// The swap hides inside the TWIRL: the moment a calling changes, the
/// villager spins on the spot - fast enough to blur who they were - and
/// comes out of the turn already dressed for the new work, with a small
/// breath of the new cloth's essence at the chest. Brett: "make the
/// character spin around fast to change the outfit", and the old full
/// burst was "a little much" - it is a quarter of what it was, timed to
/// the reveal instead of the decision.
///
/// This system only NOTICES; the spin below does the changing. Idempotent
/// on purpose: a body already dressed for its work is left alone, so
/// loading a save does not send every adult in the world pirouetting.
pub(super) fn dress_for_work(
    mut commands: Commands,
    dressed: Query<
        (Entity, &Vocation, &CreatureGenome),
        (
            Or<(Added<Vocation>, Changed<Vocation>)>,
            Without<Corpse>,
            Without<Twirl>,
        ),
    >,
) {
    for (root, vocation, genome) in &dressed {
        if wearing(genome, &livery(*vocation)) {
            continue;
        }
        commands.entity(root).insert(Twirl {
            spun: 0.0,
            redressed: false,
        });
    }
}

/// A body mid-pirouette: how far it has turned, and whether the new
/// clothes are on yet.
#[derive(Component)]
pub(super) struct Twirl {
    spun: f32,
    redressed: bool,
}

/// How fast the change of clothes spins, in radians per second, and how
/// far it goes. Two and a half turns: enough blur to sell the swap, brief
/// enough that a whole square changing trades at the morning muster reads
/// as a flourish and not a ballet.
const TWIRL_PACE: f32 = 34.0;
const TWIRL_TURNS: f32 = 2.5;

/// Spins the twirling, dresses them at the blurriest beat, and lets the
/// turn run down. The redress happens mid-spin - the body is rebuilt in
/// the new livery between one frame's blur and the next, which is the
/// whole trick: nobody ever sees the mesh change, they see a spin end
/// with different clothes.
pub(super) fn twirl_to_redress(
    mut commands: Commands,
    time: Res<Time>,
    assets: Option<Res<CreatureAssets>>,
    mut visuals: (ResMut<Assets<Mesh>>, ResMut<Assets<StandardMaterial>>),
    mut spinning: Query<
        (
            Entity,
            &mut Transform,
            &mut Twirl,
            &Vocation,
            &mut CreatureGenome,
            &mut CreatureRig,
        ),
        (Without<Corpse>, Without<crate::creature::Held>),
    >,
) {
    let Some(assets) = assets else {
        return;
    };
    let full = TWIRL_TURNS * std::f32::consts::TAU;
    for (root, mut at, mut twirl, vocation, mut genome, mut rig) in &mut spinning {
        let step = TWIRL_PACE * time.delta_secs();
        twirl.spun += step;
        at.rotate_y(step);

        if !twirl.redressed && twirl.spun >= full * 0.5 {
            twirl.redressed = true;
            let wear = livery(*vocation);
            genome.cloth = wear.cloth;
            genome.accent = wear.accent;
            genome.headwear = wear.headwear;
            genome.garment = wear.garment;
            genome.belt = wear.belt;

            let chest = at.translation + Vec3::Y * 1.1;
            crate::matter::puff_of(
                &mut commands,
                &mut visuals.0,
                &mut visuals.1,
                chest,
                &[
                    palette::color_at(wear.cloth.palette_index()),
                    palette::color_at(wear.accent.palette_index()),
                ],
            );

            commands.entity(rig.body).despawn();
            *rig = build_body(&mut commands, &assets, root, &genome);
        }

        if twirl.spun >= full {
            commands.entity(root).remove::<Twirl>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every calling is telling you what it is from across the square.
    #[test]
    fn every_calling_wears_its_own_livery() {
        use Vocation::*;
        let callings = [
            Gatherer, Fisher, Hunter, Miner, Forester, Builder, Farmer, Cook, Healer, Priest,
            Explorer, Guard,
        ];
        for (i, a) in callings.iter().enumerate() {
            for b in callings.iter().skip(i + 1) {
                assert_ne!(
                    livery(*a),
                    livery(*b),
                    "{a:?} and {b:?} would be indistinguishable at a glance"
                );
            }
        }
    }

    /// The dresser changes the clothes and rebuilds the body once —
    /// and leaves an already-dressed body alone, or every save load
    /// would rebuild every adult in the world.
    #[test]
    fn taking_a_calling_changes_the_clothes_once() {
        use crate::creature::genome::{Sex, Species};
        use bevy::ecs::system::RunSystemOnce;

        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>();
        app.init_resource::<Assets<StandardMaterial>>();
        let cube = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(Cuboid::new(1.0, 1.0, 1.0));
        let materials: Vec<_> = {
            let mut assets = app.world_mut().resource_mut::<Assets<StandardMaterial>>();
            (0..palette::PALETTE_LEN)
                .map(|_| assets.add(StandardMaterial::default()))
                .collect()
        };
        let assets = CreatureAssets {
            cube: cube.clone(),
            materials: materials.clone(),
        };
        app.insert_resource(CreatureAssets { cube, materials });

        let mut rng = crate::rng::Rng::new(7);
        let genome = CreatureGenome::adult(Species::Human, Sex::Female, &mut rng);
        let root = app
            .world_mut()
            .spawn((Transform::default(), Visibility::default(), genome.clone()))
            .id();
        // Grown first, dressed after — the order every villager lives.
        let rig = {
            let world = app.world_mut();
            let mut queue = bevy::ecs::world::CommandQueue::default();
            let rig = {
                let mut commands = Commands::new(&mut queue, world);
                build_body(&mut commands, &assets, root, &genome)
            };
            queue.apply(world);
            rig
        };
        let bare_body = rig.body;
        app.world_mut().entity_mut(root).insert(rig);
        app.world_mut().entity_mut(root).insert(Vocation::Priest);

        // The change is a choreography now: noticing starts the twirl,
        // and the clothes go on at the blurriest beat of the spin.
        app.init_resource::<Time>();
        app.world_mut().run_system_once(dress_for_work).unwrap();
        assert!(
            app.world().get::<Twirl>(root).is_some(),
            "a new calling starts the twirl"
        );
        assert_eq!(
            app.world().get::<CreatureRig>(root).unwrap().body,
            bare_body,
            "nothing changes until the spin blurs"
        );
        // Spin past the midpoint: the redress lands mid-blur.
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(300));
        app.world_mut().run_system_once(twirl_to_redress).unwrap();
        let genome = app.world().get::<CreatureGenome>(root).unwrap();
        assert_eq!(genome.headwear, Headwear::Hood, "a priest is hooded");
        assert_eq!(
            genome.cloth.ramp,
            palette::RAMP_CLOTH_PURPLE,
            "a priest wears the god's purple"
        );
        let dressed_body = app.world().get::<CreatureRig>(root).unwrap().body;
        assert_ne!(bare_body, dressed_body, "the body is rebuilt in the livery");

        // The turn runs down and the twirl leaves.
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(300));
        app.world_mut().run_system_once(twirl_to_redress).unwrap();
        assert!(
            app.world().get::<Twirl>(root).is_none(),
            "the twirl ends when the turns are spent"
        );

        // Again: nothing to do, no new twirl, nothing rebuilt.
        app.world_mut().entity_mut(root).insert(Vocation::Priest);
        app.world_mut().run_system_once(dress_for_work).unwrap();
        assert!(
            app.world().get::<Twirl>(root).is_none(),
            "an already-dressed body is left alone"
        );
        assert_eq!(
            app.world().get::<CreatureRig>(root).unwrap().body,
            dressed_body,
            "an already-dressed body is not rebuilt"
        );
    }
}
