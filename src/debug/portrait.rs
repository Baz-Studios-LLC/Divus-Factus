//! The portrait studio: every soul sits for the lens once, and the ledger
//! keeps the face.
//!
//! The paperdoll stage proved the trick — a body rebuilt on a hidden layer,
//! drawn by its own camera to a texture — but it renders one LIVE doll,
//! every frame, for the one open dossier. The studio is the batch version:
//! a second stage far beneath the paperdoll's, one camera that wakes for a
//! few frames per sitter, and one small render target PER SOUL that every
//! card in the game hangs like a painting. The plate's handle never
//! changes, so a re-shoot — a new livery, a child grown — repaints every
//! card that hangs it, everywhere at once, for free.
//!
//! The studio borrows the paperdoll's own floor lights: directional lights
//! shine across their whole render layer regardless of position, so the
//! two stages fifty metres apart are lit identically and NO NEW LIGHT is
//! ever added here. (A light on a shared layer once lit the entire world
//! to studio noon — see the ledger of that in people.rs at DOLL_LAYER.)

use super::*;
use std::collections::{HashMap, HashSet, VecDeque};

use crate::creature::Corpse;
use crate::creature::body::{CreatureAssets, biped_head_size, build_body};
use crate::creature::genome::CreatureGenome;
use crate::villager::Villager;

/// Where the sitter stands: fifty metres below the paperdoll's stage, on
/// the same private layer, out of both cameras' level gazes.
pub(crate) const PORTRAIT_STAGE: Vec3 = Vec3::new(0.0, -650.0, 0.0);

/// The plate a face is painted on. 4:5, the classic portrait ratio — every
/// frame that hangs one keeps this aspect.
const PLATE: (u32, u32) = (256, 320);

/// Width over height of the plate, for the frames that hang it.
pub(crate) const PLATE_ASPECT: f32 = PLATE.0 as f32 / PLATE.1 as f32;

/// How many frames a sitting takes: spawn, stamp, propagate, then the
/// camera wakes at three-and-under and the stage clears at nought.
const SITTING_FRAMES: u8 = 6;

/// The studio's one camera. It sleeps between sittings.
#[derive(Component)]
pub(crate) struct PortraitCamera;

/// The body currently on the studio floor.
#[derive(Component)]
pub(crate) struct PortraitSitter;

/// One face on file: the plate the camera paints, and whether it has been
/// painted at least once. An unpainted plate is transparent black —
/// hanging it would frame nothing.
struct Plate {
    image: Handle<Image>,
    painted: bool,
}

/// A sitting in progress.
struct Sitting {
    person: Entity,
    root: Entity,
    countdown: u8,
}

/// The ledger of faces: who has sat, who is waiting, who is under the
/// lights right now.
#[derive(Resource, Default)]
pub(crate) struct Portraits {
    plates: HashMap<Entity, Plate>,
    queue: VecDeque<Entity>,
    waiting: HashSet<Entity>,
    sitting: Option<Sitting>,
}

impl Portraits {
    /// The person's portrait, if the studio has delivered one.
    pub(crate) fn face_of(&self, who: Entity) -> Option<Handle<Image>> {
        self.plates
            .get(&who)
            .filter(|plate| plate.painted)
            .map(|plate| plate.image.clone())
    }
}

/// A UI frame waiting on its person's face. Its stand-in children are
/// replaced with the portrait the moment the studio delivers.
#[derive(Component)]
pub(crate) struct PortraitSlot(pub Entity);

/// The face is on the frame; the slot needs no more visits.
#[derive(Component)]
pub(crate) struct PortraitHung;

/// Dresses a frame with a person's face: the true portrait if one is on
/// file, else the little engraved bust as a stand-in — and a slot marker
/// either way, so the frame is rehung the moment a face arrives.
pub(crate) fn set_the_face(
    commands: &mut Commands,
    frame: Entity,
    portraits: &Portraits,
    who: Entity,
    tint: Color,
) {
    commands.entity(frame).insert(PortraitSlot(who));
    if let Some(image) = portraits.face_of(who) {
        hang(commands, frame, image);
        return;
    }
    // The stand-in bust, centred whatever the frame's size.
    let seat = commands
        .spawn((
            Node {
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            ChildOf(frame),
        ))
        .id();
    super::village::person_glyph(commands, seat, tint);
}

/// Hangs the painting itself.
fn hang(commands: &mut Commands, frame: Entity, image: Handle<Image>) {
    commands.entity(frame).insert(PortraitHung);
    commands.spawn((
        bevy::ui::widget::ImageNode::new(image),
        Node {
            width: percent(100),
            height: percent(100),
            ..default()
        },
        ChildOf(frame),
    ));
}

/// Raises the studio: one sleeping camera on the paperdoll's layer, aimed
/// at its own empty floor until someone is seated.
pub(crate) fn spawn_portrait_studio(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    // A camera must own SOME target from birth, so it gets a pinhole of a
    // plate nobody will ever hang.
    let idle = images.add(bevy::image::Image::new_target_texture(
        4,
        4,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    commands.spawn((
        Name::new("Portrait Camera"),
        PortraitCamera,
        Camera3d::default(),
        Camera {
            order: -21,
            is_active: false,
            clear_color: bevy::camera::ClearColorConfig::Custom(Color::NONE),
            ..default()
        },
        bevy::camera::RenderTarget::Image(idle.into()),
        Transform::from_translation(PORTRAIT_STAGE + Vec3::new(0.0, 1.4, 1.5))
            .looking_at(PORTRAIT_STAGE + Vec3::Y * 1.4, Vec3::Y),
        bevy::camera::visibility::RenderLayers::layer(super::people::DOLL_LAYER),
    ));
}

/// Books a sitting for every soul whose look has changed: a body newly
/// grown, a livery newly donned, a child newly tall. `Changed` covers
/// `Added`, so a fresh village — or a loaded save — queues everyone once.
pub(crate) fn want_portraits(
    mut portraits: ResMut<Portraits>,
    changed: Query<Entity, (With<Villager>, Changed<CreatureGenome>, Without<Corpse>)>,
) {
    let studio = &mut *portraits;
    for who in &changed {
        if studio.waiting.insert(who) {
            studio.queue.push_back(who);
        }
    }
}

/// The studio itself: seats the next soul, wakes the camera mid-sitting,
/// clears the stage and marks the plate painted at the end. One sitter at
/// a time, a handful of frames each — a village of fifty is on the wall
/// within seconds, and nobody's frame rate hears about it.
pub(crate) fn run_the_studio(
    mut commands: Commands,
    mut portraits: ResMut<Portraits>,
    mut images: ResMut<Assets<Image>>,
    assets: Option<Res<CreatureAssets>>,
    genomes: Query<&CreatureGenome>,
    mut lens: Query<
        (&mut Camera, &mut Transform, &mut bevy::camera::RenderTarget),
        With<PortraitCamera>,
    >,
) {
    let Some(assets) = assets else {
        return;
    };
    let Ok((mut camera, mut stance, mut plate_in_lens)) = lens.single_mut() else {
        return;
    };
    let studio = &mut *portraits;

    if let Some(sitting) = &mut studio.sitting {
        sitting.countdown -= 1;
        match sitting.countdown {
            // The body has spawned, its parts are stamped, its transforms
            // have propagated: open the shutter.
            3 => camera.is_active = true,
            0 => {
                camera.is_active = false;
                commands.entity(sitting.root).despawn();
                if let Some(plate) = studio.plates.get_mut(&sitting.person) {
                    plate.painted = true;
                }
                studio.sitting = None;
            }
            _ => {}
        }
        if studio.sitting.is_some() {
            return;
        }
    }

    // Seat the next soul that still has a body to photograph.
    while let Some(person) = studio.queue.pop_front() {
        studio.waiting.remove(&person);
        let Ok(genome) = genomes.get(person) else {
            // Gone before their sitting came up.
            continue;
        };

        let plate = studio.plates.entry(person).or_insert_with(|| Plate {
            image: images.add(bevy::image::Image::new_target_texture(
                PLATE.0,
                PLATE.1,
                bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
                None,
            )),
            painted: false,
        });
        *plate_in_lens = bevy::camera::RenderTarget::Image(plate.image.clone().into());

        // The sitter: the person's real body, rebuilt on the studio floor
        // at a gentle three-quarter turn. A body's face points -Z and the
        // camera stands on +Z, so the half-turn faces them; the extra
        // shave of angle is what makes it a portrait and not a passport.
        let root = commands
            .spawn((
                Name::new("Portrait Sitter"),
                PortraitSitter,
                Transform::from_translation(PORTRAIT_STAGE)
                    .with_rotation(Quat::from_rotation_y(std::f32::consts::PI - 0.35)),
                Visibility::default(),
                bevy::camera::visibility::RenderLayers::layer(super::people::DOLL_LAYER),
            ))
            .id();
        let rig = build_body(&mut commands, &assets, root, genome);
        commands.entity(root).insert(rig);

        // Frame the head from the genome's own arithmetic, so a child's
        // portrait is as full as an elder's. The head joint sits at the
        // top of the neck less an eighth of a head (the builder sinks it
        // into the torso), and the box grows a full head upward from it.
        let p = &genome.proportions;
        let high = genome.height();
        let head = biped_head_size(genome);
        let head_centre =
            (p.leg_length + p.torso_length + p.neck_length) * high + head * (0.5 - 0.12);
        let eye = PORTRAIT_STAGE + Vec3::new(0.0, head_centre + head * 0.12, head * 3.4);
        let aim = PORTRAIT_STAGE + Vec3::new(0.0, head_centre - head * 0.08, 0.0);
        *stance = Transform::from_translation(eye).looking_at(aim, Vec3::Y);

        studio.sitting = Some(Sitting {
            person,
            root,
            countdown: SITTING_FRAMES,
        });
        return;
    }
}

/// `RenderLayers` does not inherit: every part the body builder spawned
/// has to be stamped onto the studio's private layer, or the sitter poses
/// in the middle of the world.
pub(crate) fn stamp_sitter_layers(
    mut commands: Commands,
    sitters: Query<Entity, With<PortraitSitter>>,
    children: Query<&Children>,
    unstamped: Query<(), Without<bevy::camera::visibility::RenderLayers>>,
) {
    for sitter in &sitters {
        for part in children.iter_descendants(sitter) {
            if unstamped.get(part).is_ok() {
                commands
                    .entity(part)
                    .insert(bevy::camera::visibility::RenderLayers::layer(
                        super::people::DOLL_LAYER,
                    ));
            }
        }
    }
}

/// Walks the gallery: any frame still wearing its stand-in gets the true
/// portrait the moment the studio delivers it.
pub(crate) fn hang_the_portraits(
    mut commands: Commands,
    portraits: Res<Portraits>,
    slots: Query<(Entity, &PortraitSlot), Without<PortraitHung>>,
) {
    for (frame, slot) in &slots {
        let Some(image) = portraits.face_of(slot.0) else {
            continue;
        };
        commands.entity(frame).despawn_related::<Children>();
        hang(&mut commands, frame, image);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::creature::genome::{Sex, Species};
    use bevy::ecs::system::RunSystemOnce;

    /// A world with the studio raised and the body bench stocked.
    fn studio_world() -> App {
        let mut app = App::new();
        app.init_resource::<Assets<Image>>();
        app.init_resource::<Assets<Mesh>>();
        app.init_resource::<Assets<StandardMaterial>>();
        app.init_resource::<Portraits>();
        app.insert_resource(CreatureAssets {
            cube: Handle::default(),
            materials: vec![Handle::default(); crate::palette::PALETTE_LEN],
        });
        app.world_mut()
            .run_system_once(spawn_portrait_studio)
            .unwrap();
        app
    }

    fn a_soul(app: &mut App) -> Entity {
        let mut rng = crate::rng::Rng::new(11);
        let genome = CreatureGenome::adult(Species::Human, Sex::Female, &mut rng);
        app.world_mut()
            .spawn((
                Villager,
                genome,
                Transform::default(),
                Visibility::default(),
            ))
            .id()
    }

    /// Runs one studio frame. Booking is called once per test by hand:
    /// `run_system_once` builds a fresh system each call, so a booking
    /// system run every tick would see every genome as freshly changed
    /// and re-book the sitter forever.
    fn tick(app: &mut App) {
        app.world_mut().run_system_once(run_the_studio).unwrap();
    }

    /// A soul queues once however many times their change is noticed —
    /// `run_system_once` builds a fresh system each call, so every call
    /// sees the genome as changed, which is exactly the double-booking
    /// the waiting set exists to refuse.
    #[test]
    fn a_soul_is_booked_once_not_per_glance() {
        let mut app = studio_world();
        a_soul(&mut app);
        app.world_mut().run_system_once(want_portraits).unwrap();
        app.world_mut().run_system_once(want_portraits).unwrap();
        let portraits = app.world().resource::<Portraits>();
        assert_eq!(portraits.queue.len(), 1, "booked twice for one look");
    }

    /// The whole sitting: seated, painted, stage cleared — and the face
    /// on file at the end.
    #[test]
    fn a_sitting_paints_the_plate_and_clears_the_stage() {
        let mut app = studio_world();
        let who = a_soul(&mut app);
        app.world_mut().run_system_once(want_portraits).unwrap();

        tick(&mut app);
        {
            let portraits = app.world().resource::<Portraits>();
            assert!(portraits.sitting.is_some(), "nobody was seated");
            assert!(portraits.face_of(who).is_none(), "painted before sat");
        }
        let mut sitters = app.world_mut().query_filtered::<(), With<PortraitSitter>>();
        assert_eq!(sitters.iter(app.world()).count(), 1, "no body on stage");

        for _ in 0..SITTING_FRAMES {
            tick(&mut app);
        }
        let portraits = app.world().resource::<Portraits>();
        assert!(portraits.sitting.is_none(), "the stage never cleared");
        assert!(portraits.face_of(who).is_some(), "no face on file");
        let mut sitters = app.world_mut().query_filtered::<(), With<PortraitSitter>>();
        assert_eq!(sitters.iter(app.world()).count(), 0, "a sitter was left");
    }

    /// A soul who dies in the waiting room is skipped without a panic,
    /// and the next in line is seated instead.
    #[test]
    fn the_departed_are_skipped_for_the_living() {
        let mut app = studio_world();
        let gone = a_soul(&mut app);
        let living = a_soul(&mut app);
        app.world_mut().run_system_once(want_portraits).unwrap();
        app.world_mut().entity_mut(gone).despawn();

        app.world_mut().run_system_once(run_the_studio).unwrap();
        let portraits = app.world().resource::<Portraits>();
        let seated = portraits.sitting.as_ref().map(|s| s.person);
        assert_eq!(seated, Some(living), "the living soul was not seated");
    }

    /// A re-shoot paints the SAME plate: every card hanging the handle
    /// updates for free, which is the whole economy of the studio.
    #[test]
    fn a_second_sitting_keeps_the_same_plate() {
        let mut app = studio_world();
        let who = a_soul(&mut app);
        app.world_mut().run_system_once(want_portraits).unwrap();
        for _ in 0..=SITTING_FRAMES {
            tick(&mut app);
        }
        let first = app
            .world()
            .resource::<Portraits>()
            .face_of(who)
            .expect("no first face");

        // A change of clothes books a second sitting.
        app.world_mut()
            .entity_mut(who)
            .get_mut::<CreatureGenome>()
            .unwrap()
            .set_changed();
        app.world_mut().run_system_once(want_portraits).unwrap();
        for _ in 0..=SITTING_FRAMES {
            tick(&mut app);
        }
        let second = app
            .world()
            .resource::<Portraits>()
            .face_of(who)
            .expect("no second face");
        assert_eq!(first, second, "the plate moved and every card went stale");
    }
}
