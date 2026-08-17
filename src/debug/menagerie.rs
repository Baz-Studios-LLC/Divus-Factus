//! The menagerie: every creature the world can build, on a turntable.
//!
//! Brett's idea, and it came out of an afternoon of trying to photograph a
//! goblin in the wild: "An in game person and animal view (and goblin viewer)
//! would be pretty cool. On an F key for dev reasons. It could load a random
//! version and have it zoomed in and spinning."
//!
//! WHY IT IS WORTH ITS KEYBINDING. A creature in this game is not a model, it
//! is two dozen numbers, and the only place those numbers are ever seen is
//! standing a hundred meters away at a third of a second's notice. Every
//! judgement about them - is a goblin green enough, is a polar bear big enough,
//! do the ears read - was being made from screenshots of a figure eight pixels
//! tall in country that happened to be lit wrong. This is the bench: one
//! creature, close, turning, against a plain ground, with the next roll of its
//! own genome one key away.
//!
//! It costs almost nothing because the hard part was already built twice. The
//! paperdoll proved a body on a private layer under the world; the portrait
//! studio proved a second stage beside it. This is a third, and it reuses the
//! paperdoll's own lights - directional lights carry across their whole render
//! layer whatever their position, so a stage on `DOLL_LAYER` is lit for free
//! and NO NEW LIGHT is added here. (Adding one is how the whole world once got
//! lit to studio noon; see the note at `DOLL_LAYER`.)

use super::*;
use crate::creature::body::{CreatureAssets, build_body};
use crate::creature::genome::{CreatureGenome, Species};
use crate::rng::Rng;

/// Where the exhibit stands: below the paperdoll and the portrait studio both,
/// on the same private layer, out of every other camera's gaze.
const STAGE: Vec3 = Vec3::new(0.0, -900.0, 0.0);

/// Everything the bench can show, in the order the keys walk through it.
const EXHIBITS: [Species; 9] = [
    Species::Human,
    Species::Goblin,
    Species::Deer,
    Species::Wolf,
    Species::Boar,
    Species::Bear,
    Species::PolarBear,
    Species::Camel,
    Species::Penguin,
];

/// How fast the turntable turns, in radians a second. Slow enough to read a
/// silhouette at every angle, quick enough that waiting for the back is not
/// waiting.
const SPIN: f32 = 0.7;

/// How far back the lens sits, in multiples of the subject's own height.
///
/// A MULTIPLE, not a distance, which is the whole trick of the framing: a
/// penguin is eight tenths of a meter and a polar bear better than two, and a
/// fixed camera would either lose one or crop the other. Every exhibit fills
/// the same share of the frame, which is also what makes the bench honest
/// about SIZE - the only way to see that a polar bear is bigger is the caption,
/// so the caption says so.
const STANDOFF: f32 = 2.6;

/// Whether the bench is open, and what is on it.
#[derive(Resource)]
pub(crate) struct Menagerie {
    pub(crate) open: bool,
    which: usize,
    /// The roll the exhibit was built from. Kept so the caption can name it and
    /// so re-rolling is a visible change rather than a suspicion.
    seed: u64,
    /// Set when the exhibit must be rebuilt - a different species, a new roll,
    /// or the bench opening.
    dirty: bool,
}

impl Default for Menagerie {
    fn default() -> Self {
        // DIVUS_FACTUS_MENAGERIE=goblin opens the bench on a named creature at
        // startup, so an unattended capture can photograph one. The bench is a
        // dev tool and this is how the dev tool gets its own screenshot.
        let asked = std::env::var("DIVUS_FACTUS_MENAGERIE").ok();
        // DIVUS_FACTUS_MENAGERIE_SEED picks which roll, so a capture can walk
        // several of the same creature without anyone touching R.
        let seed = std::env::var("DIVUS_FACTUS_MENAGERIE_SEED")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(1);
        let which = asked
            .as_deref()
            .and_then(|name| {
                EXHIBITS
                    .iter()
                    .position(|s| format!("{s:?}").to_lowercase() == name.to_lowercase())
            })
            .unwrap_or(0);
        Menagerie {
            open: asked.is_some(),
            which,
            seed,
            dirty: asked.is_some(),
        }
    }
}

impl Menagerie {
    fn species(&self) -> Species {
        EXHIBITS[self.which % EXHIBITS.len()]
    }
}

/// The bench's camera, asleep until the bench is open.
#[derive(Component)]
pub(crate) struct MenagerieCamera;

/// Whatever is presently standing on the turntable.
#[derive(Component)]
pub(crate) struct Exhibit;

/// SHIFT+F1 opens it, and the shift is because all twelve function keys were
/// already spoken for - F1 hides the dev panel, and this is that panel's
/// bigger brother.
///
/// While it is open: LEFT and RIGHT walk the exhibits, R rolls a new one of the
/// same kind, and ESC or SHIFT+F1 again shuts it.
pub(crate) fn work_the_menagerie(
    keys: Res<ButtonInput<KeyCode>>,
    mut bench: ResMut<Menagerie>,
) {
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    if shift && keys.just_pressed(KeyCode::F1) {
        bench.open = !bench.open;
        bench.dirty = bench.open;
        return;
    }
    if !bench.open {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        bench.open = false;
        return;
    }
    if keys.just_pressed(KeyCode::ArrowRight) {
        bench.which = (bench.which + 1) % EXHIBITS.len();
        bench.dirty = true;
    }
    if keys.just_pressed(KeyCode::ArrowLeft) {
        bench.which = (bench.which + EXHIBITS.len() - 1) % EXHIBITS.len();
        bench.dirty = true;
    }
    if keys.just_pressed(KeyCode::KeyR) {
        bench.seed = bench.seed.wrapping_add(1);
        bench.dirty = true;
    }
}

/// Rebuilds whatever is on the turntable when the bench is told to.
pub(crate) fn dress_the_exhibit(
    mut commands: Commands,
    mut bench: ResMut<Menagerie>,
    assets: Option<Res<CreatureAssets>>,
    standing: Query<Entity, With<Exhibit>>,
) {
    let Some(assets) = assets else {
        return;
    };
    if !bench.dirty {
        return;
    }
    bench.dirty = false;

    for old in &standing {
        commands.entity(old).despawn();
    }
    if !bench.open {
        return;
    }

    let species = bench.species();
    let genome = CreatureGenome::random(species, &mut Rng::new(bench.seed));
    let root = commands
        .spawn((
            Name::new("Exhibit"),
            Exhibit,
            Transform::from_translation(STAGE),
            Visibility::default(),
            bevy::camera::visibility::RenderLayers::layer(super::people::DOLL_LAYER),
        ))
        .id();
    // THE RIG HAS TO BE PUT ON, and `build_body` does not do it - `spawn_creature`
    // is what normally follows it with an insert, and this bench does not go
    // through `spawn_creature`. Without it the lens has nothing to measure and
    // frames every exhibit as though it were a person.
    let rig = build_body(&mut commands, &assets, root, &genome);
    commands.entity(root).insert(rig);

    // The whole body has to be put on the private layer, and the builder does
    // not do it - every part it spawns lands on the default layer, where the
    // world's own camera would find it standing nine hundred meters underground.
    // The paperdoll has the same rule; this is the same fix.
    commands
        .entity(root)
        .insert(bevy::camera::visibility::RenderLayers::layer(
            super::people::DOLL_LAYER,
        ));
}

/// Puts every part of the exhibit on the bench's layer, and turns it.
///
/// Done as a sweep over descendants rather than at build time because
/// `build_body` spawns dozens of entities across several frames' worth of
/// deferred commands, and there is no seam in it to hang a layer on that does
/// not mean threading one through every limb.
pub(crate) fn turn_the_turntable(
    time: Res<Time<Real>>,
    bench: Res<Menagerie>,
    mut commands: Commands,
    mut exhibits: Query<(Entity, &mut Transform), With<Exhibit>>,
    children: Query<&Children>,
    already: Query<&bevy::camera::visibility::RenderLayers>,
) {
    if !bench.open {
        return;
    }
    let layer = bevy::camera::visibility::RenderLayers::layer(super::people::DOLL_LAYER);
    for (root, mut transform) in &mut exhibits {
        transform.rotation = Quat::from_rotation_y(time.elapsed_secs() * SPIN);
        for part in children.iter_descendants(root) {
            if already.get(part).is_err() {
                commands.entity(part).insert(layer.clone());
            }
        }
    }
}

/// Wakes the bench's camera and frames whatever is standing on it.
pub(crate) fn mind_the_lens(
    mut commands: Commands,
    bench: Res<Menagerie>,
    capture: Option<Res<crate::render::CaptureTarget>>,
    exhibits: Query<&crate::creature::body::CreatureRig, With<Exhibit>>,
    mut lens: Query<(Entity, &mut Camera, &mut Transform), With<MenagerieCamera>>,
    mut aimed: Local<bool>,
) {
    let Ok((lens_entity, mut camera, mut transform)) = lens.single_mut() else {
        return;
    };
    camera.is_active = bench.open;
    // IN CAPTURE MODE THE WORLD IS DRAWN OFFSCREEN, and the screenshot is taken
    // from that image rather than from the window - so a bench camera left
    // pointing at the window renders perfectly and appears in no screenshot at
    // all. It took one confusing capture of an ordinary-looking world to find
    // that. Pointed at the same plate as everything else, the bench photographs.
    // OVERWRITTEN ONCE, not filled in. A camera is born already pointing at the
    // primary window - `RenderTarget` is a required component with a default -
    // so a guard that only acted when the target was MISSING never acted at
    // all, and the bench rendered beautifully into a window nobody was
    // photographing. `aimed` remembers whether this has been done.
    if let Some(capture) = capture.as_deref()
        && !*aimed
    {
        *aimed = true;
        commands
            .entity(lens_entity)
            .insert(bevy::camera::RenderTarget::Image(
                capture.image.clone().into(),
            ));
    }

    if !bench.open {
        return;
    }
    // Framed off the subject's OWN height, so every exhibit fills the same
    // share of the glass whatever it is.
    let height = exhibits.single().map(|rig| rig.height).unwrap_or(1.7);
    let eye = STAGE + Vec3::new(0.0, height * 0.62, height * STANDOFF);
    *transform = Transform::from_translation(eye).looking_at(STAGE + Vec3::Y * height * 0.5, Vec3::Y);
}

/// Raises the bench: one sleeping camera, and a plain floor to stand on.
pub(crate) fn build_the_menagerie(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let layer = bevy::camera::visibility::RenderLayers::layer(super::people::DOLL_LAYER);

    commands.spawn((
        Name::new("Menagerie Camera"),
        Camera3d::default(),
        Camera {
            // ABOVE the world's own cameras, so the bench covers the game
            // rather than appearing behind it, and opaque so there is no
            // question what is being looked at.
            order: 40,
            is_active: false,
            clear_color: bevy::camera::ClearColorConfig::Custom(Color::srgb(0.07, 0.075, 0.09)),
            ..default()
        },
        Transform::from_translation(STAGE + Vec3::new(0.0, 1.1, 4.4))
            .looking_at(STAGE + Vec3::Y * 0.9, Vec3::Y),
        MenagerieCamera,
        layer.clone(),
    ));

    // A disc to stand on, so the eye has a ground plane and the feet are not
    // floating in a void.
    commands.spawn((
        Name::new("Menagerie Floor"),
        Mesh3d(meshes.add(Cylinder::new(2.4, 0.08))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.13, 0.14, 0.17),
            perceptual_roughness: 1.0,
            ..default()
        })),
        Transform::from_translation(STAGE - Vec3::Y * 0.04),
        layer,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every creature the world can build is on the bench.
    ///
    /// The whole point of it is to be able to LOOK at a new species, so a
    /// species that can be spawned and cannot be looked at is the one failure
    /// this must not have - and the way it would happen is somebody adding a
    /// variant and not this line.
    #[test]
    fn the_bench_shows_every_creature_in_the_world() {
        for species in [
            Species::Human,
            Species::Deer,
            Species::Wolf,
            Species::Boar,
            Species::Camel,
            Species::Bear,
            Species::PolarBear,
            Species::Penguin,
            Species::Goblin,
        ] {
            assert!(
                EXHIBITS.contains(&species),
                "{species:?} can be born into the world but not shown on the bench",
            );
        }
    }

    /// Walking the exhibits wraps rather than running off the end.
    #[test]
    fn the_bench_walks_round_in_a_circle() {
        let mut bench = Menagerie::default();
        bench.which = EXHIBITS.len() - 1;
        bench.which = (bench.which + 1) % EXHIBITS.len();
        assert_eq!(bench.which, 0, "walking past the last exhibit returns to the first");
    }
}
