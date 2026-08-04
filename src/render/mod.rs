//! Presentation: anti-aliasing and the post stack.
//!
//! The world renders at full resolution, crisply. An earlier version rendered to a
//! quarter-resolution target and snapped every pixel to the master palette, chasing
//! a pixel-art look — but the geometry here is genuinely 3D, and forcing it through a
//! low-resolution buffer cost sharpness without buying the authority of real pixel
//! art. Sharp low-poly is its own look, and it suits these models better.
//!
//! The palette did not go away. It is still where every colour in the game is
//! authored, which is what keeps procedurally generated art coherent. What went away
//! is the post-process that re-quantised the finished image back onto it.
//!
//! What carries the style now is depth of field. Tilt-shift blur is what makes real
//! scenes read as miniature models on a table — which, for a game about a being
//! looking down at tiny people it invented, is the fantasy rendered literally.

use bevy::anti_alias::fxaa::Fxaa;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{ClearColorConfig, Hdr, RenderTarget};
use bevy::core_pipeline::prepass::DepthPrepass;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::post_process::bloom::Bloom;
use bevy::post_process::dof::{DepthOfField, DepthOfFieldMode};
use bevy::post_process::effect_stack::Vignette;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::{ColorGrading, ColorGradingGlobal};
use bevy::window::PrimaryWindow;

use crate::GameState;
use crate::camera::{CameraRig, CameraStartupSet, GodCamera};
use crate::palette;

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TitleLens>()
            .init_resource::<LookSettings>()
            .add_systems(Startup, setup_pipeline.after(CameraStartupSet))
            .add_systems(
                Update,
                (
                    focus_depth_of_field,
                    paint_the_sky,
                    apply_look_settings.run_if(resource_changed::<LookSettings>),
                    // Last word on the lens: the dream blend overwrites what
                    // the two systems above decided, so it must follow them.
                    dream_lens,
                )
                    .chain(),
            );
    }
}

/// How far the lens is dreaming, 0 to 1.
///
/// At 1 — the splash and title screens — the aperture opens wide and the
/// focal plane pulls well short of the village, so the world behind the
/// lettering is a soft, living blur. The blend eases back to 0 across the
/// opening descent, the world sharpening as the god comes down to it.
#[derive(Resource)]
pub struct TitleLens(f32);

impl Default for TitleLens {
    fn default() -> Self {
        // Unattended captures frame their own shot and skip the front door,
        // so they start sharp rather than easing out of a blur nobody sees.
        // Title portraits (DIVUS_FACTUS_TITLE) keep the dream.
        let capturing = crate::capture_path().is_some() && !crate::title::title_capture();
        TitleLens(if capturing { 0.0 } else { 1.0 })
    }
}

/// Aperture, in f-stops, of the fully dreaming lens.
const DREAM_APERTURE: f32 = 2.6;
/// Where the dreaming lens focuses, as a fraction of the camera distance —
/// deliberately short of the village it is looking at.
const DREAM_FOCUS: f32 = 0.4;

fn dream_lens(
    time: Res<Time<Real>>,
    state: Res<State<GameState>>,
    look: Res<LookSettings>,
    mut lens: ResMut<TitleLens>,
    mut cameras: Query<(&CameraRig, &mut DepthOfField), With<GodCamera>>,
) {
    let dreaming = matches!(state.get(), GameState::Splash | GameState::Title);
    let target = if dreaming { 1.0 } else { 0.0 };
    let step = time.delta_secs() / 2.4;
    lens.0 += (target - lens.0).clamp(-step, step);
    if lens.0 <= 0.001 {
        return;
    }
    for (rig, mut dof) in &mut cameras {
        dof.aperture_f_stops = look.aperture + (DREAM_APERTURE - look.aperture) * lens.0;
        let sharp = rig.distance * look.focus_bias;
        let dreamy = rig.distance * DREAM_FOCUS;
        dof.focal_distance = (sharp + (dreamy - sharp) * lens.0).max(0.1);
    }
}

/// Live-tunable knobs for the look.
///
/// Everything here is meant to be dragged around while the game runs. Procedural art
/// is only as good as the ability to steer it, and steering it by editing constants
/// and waiting for a rebuild is not steering.
#[derive(Resource, Debug, Clone)]
pub struct LookSettings {
    /// Bloom intensity.
    pub bloom: f32,
    /// Vignette strength.
    pub vignette: f32,
    /// Depth-of-field aperture in f-stops. Lower is shallower and more miniature.
    /// Zero disables the effect.
    pub aperture: f32,
    /// Scales the distance the lens focuses at, relative to the camera's own focus
    /// point. 1.0 focuses exactly on what the camera is orbiting.
    pub focus_bias: f32,
    /// Overall exposure, in stops.
    pub exposure: f32,
    /// Colour saturation after tonemapping.
    ///
    /// Slightly below 1 by default. The palette's greens are already saturated as
    /// authored, and pushing them further under a warm key light drives the whole
    /// landscape toward lime.
    pub saturation: f32,
}

impl Default for LookSettings {
    fn default() -> Self {
        LookSettings {
            bloom: 0.18,
            vignette: 0.3,
            // Shallow enough to read as a diorama, not so shallow that a villager a
            // few metres off the focus point turns to mush.
            aperture: 12.0,
            focus_bias: 1.0,
            exposure: 0.05,
            saturation: 0.96,
        }
    }
}

impl LookSettings {
    pub fn depth_of_field_enabled(&self) -> bool {
        self.aperture > 0.0
    }
}

/// The colour of the daytime sky.
///
/// It used to be a near-grey, and deliberately: it was the colour the DISTANCE
/// FOG faded the world into, and the two had to match exactly or the boundary
/// between fogged ground and empty sky drew a hard line across the horizon. A
/// blue that strong tinted every mid-distance hill blue rather than hazy, so it
/// was pulled almost half way to bone white — which is why the sky has been
/// grey ever since.
///
/// There is no distance fog any more (a round world hides its own distance over
/// the horizon, which is what fog was faking) and nothing left that has to
/// match. So the sky is allowed to be the sky. A touch of neutral still takes
/// the edge off the palette's flat blue and reads as air rather than paint.
/// The sky is a LIGHT and not a surface, so it is written to the buffer both
/// brighter AND more saturated than the palette step it wants to end up as.
/// The tonemapper pulls bright colours toward white — hand it the palette's own
/// blue and it comes back (100, 123, 151), grey by the time anyone sees it, and
/// simply brightening it makes that worse, not better. Pushed out from its own
/// luminance first, it survives the trip.
pub fn horizon_color() -> Color {
    let sky = palette::shade(&palette::SKY, 0.85).to_linear();
    // Solved rather than guessed, from two measured points through the
    // tonemapper: saturation alone gave a holiday-poster blue (62, 140, 201)
    // and brightness alone gave grey, so these are the pair that land the
    // rendered pixel on an airy daylight sky.
    const SATURATE: f32 = 1.30;
    const GAIN: f32 = 2.44;
    let luminance = 0.2126 * sky.red + 0.7152 * sky.green + 0.0722 * sky.blue;
    let push = |c: f32| ((luminance + (c - luminance) * SATURATE) * GAIN).max(0.0);
    Color::LinearRgba(LinearRgba {
        red: push(sky.red),
        green: push(sky.green),
        blue: push(sky.blue),
        alpha: 1.0,
    })
}

/// Colour grading built from the current look settings.
fn grading(look: &LookSettings) -> ColorGrading {
    ColorGrading {
        global: ColorGradingGlobal {
            exposure: look.exposure,
            post_saturation: look.saturation,
            ..default()
        },
        ..default()
    }
}

/// Full-resolution offscreen target used by capture mode.
#[derive(Resource)]
pub struct CaptureTarget {
    pub image: Handle<Image>,
}

/// The overlay camera that draws the Divine Hand.
///
/// The hand is the cursor, and a cursor must never be occluded — not by a
/// mountain and especially not by the interface. It lives on render layer 1,
/// drawn by this second camera *after* the main pass has composited the world
/// and the UI, so the hand glides over panels the way it glides over terrain.
/// The camera is a child of the god camera with an identity transform, which is
/// what keeps the two views pixel-aligned without a sync system.
#[derive(Component)]
pub struct HandCamera;

/// Render layer the hand occupies. Anything on it draws above world and UI.
pub const HAND_LAYER: usize = 1;

fn setup_pipeline(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<Entity, With<GodCamera>>,
    look: Res<LookSettings>,
) {
    let Ok(camera) = cameras.single() else {
        return;
    };

    // The sky must match the fog exactly; see `horizon_color`.
    commands.insert_resource(ClearColor(horizon_color()));

    commands.entity(camera).insert((
        Hdr,
        // With the hand overlay camera in play there are two cameras; the
        // interface belongs to this one, underneath the hand.
        bevy::ui::IsDefaultUiCamera,
        Tonemapping::TonyMcMapface,
        // The water shader reads scene depth to work out how deep it is. Without
        // this it has nothing to sample and renders as an opaque sheet.
        DepthPrepass,
        grading(&look),
        // MSAA would be sharper, but it does not compose with the depth-of-field
        // pass. FXAA is the option that survives the post stack.
        Fxaa::default(),
        Bloom {
            intensity: look.bloom,
            ..Bloom::NATURAL
        },
        Vignette {
            intensity: look.vignette,
            ..default()
        },
        DepthOfField {
            mode: DepthOfFieldMode::Bokeh,
            aperture_f_stops: look.aperture,
            ..default()
        },
    ));

    // The hand's own camera; see `HandCamera`. It inherits the god camera's
    // transform by being its child and shares its projection, so the two views
    // are pixel-aligned. It runs *no* tonemapping: the image it composites onto
    // has already been through the main camera's TonyMcMapface pass, and running
    // the curve a second time greys the whole world out. The hand is drawn raw
    // on top, and skips the atmosphere — no fog, no depth of field. A cursor is
    // not part of the scenery.
    let overlay = commands
        .spawn((
            Name::new("Hand Camera"),
            HandCamera,
            Camera3d::default(),
            Camera {
                order: 1,
                clear_color: ClearColorConfig::None,
                ..default()
            },
            // Stacked cameras must agree on HDR or the writeback that carries the
            // first camera's image into the second pass breaks entirely.
            Hdr,
            Tonemapping::None,
            Projection::Perspective(PerspectiveProjection {
                fov: 0.62,
                near: 0.5,
                // As far as the world camera sees. The hand hovers over the
                // ground it is pointing at, and once the wheel could climb to
                // orbit that ground went past three thousand units - so the
                // cursor was clipped clean out of the frame by its own
                // camera's far plane, which reads exactly like a cursor that
                // has stopped working.
                far: 70_000.0,
                ..default()
            }),
            RenderLayers::layer(HAND_LAYER),
            Transform::IDENTITY,
            ChildOf(camera),
        ))
        .id();

    // Capture mode renders into an offscreen texture instead of the window. Reading
    // back a window's swapchain depends on the compositor actually drawing it, which
    // macOS will not do while the window is unfocused — an unattended run otherwise
    // comes back solid black.
    if crate::capture_path().is_some() {
        let size = windows
            .single()
            .map(|w| UVec2::new(w.physical_width(), w.physical_height()))
            .unwrap_or(UVec2::new(1600, 900));

        let capture = images.add(Image::new_target_texture(
            size.x.max(1),
            size.y.max(1),
            TextureFormat::Rgba8UnormSrgb,
            None,
        ));

        // Both cameras must aim at the same target, or the capture shows a world
        // with no hand in it.
        commands
            .entity(camera)
            .insert(RenderTarget::Image(capture.clone().into()));
        commands
            .entity(overlay)
            .insert(RenderTarget::Image(capture.clone().into()));
        commands.insert_resource(CaptureTarget { image: capture });
    }
}

/// Keeps the lens focused on whatever the camera is orbiting.
///
/// Without this the focal plane sits at a fixed distance and the subject drifts out
/// of focus as soon as the player zooms, which reads as a broken camera rather than
/// as a stylistic choice.
/// Whether the diorama lens belongs on the camera at all, in one place.
///
/// Two systems manage the lens — the per-frame aim below, and the look-
/// settings apply — and when each held its own copy of this rule they
/// disagreed at the edges: a look change could re-insert the component with
/// Bevy's DEFAULT focal distance, a few units, which blurs the entire world
/// into a grey mush until the other system notices. Every grey in that
/// family answers to this one function now.
pub(crate) fn lens_belongs(rig: &CameraRig) -> bool {
    !(rig.in_a_body
        || rig.distance < crate::camera::FIRST_PERSON
        || rig.distance > crate::globe::ASCENT)
}

fn focus_depth_of_field(
    mut commands: Commands,
    look: Res<LookSettings>,
    mut cameras: Query<(Entity, &CameraRig, Option<&mut DepthOfField>), With<GodCamera>>,
) {
    for (entity, rig, dof) in &mut cameras {
        // Behind a mortal's eyes there is no diorama to photograph, and the
        // sum below is actively harmful there: the focal plane sits at the
        // camera's own orbit distance, and that distance is NOUGHT when the
        // god is wearing a body. The plane fell to the ten-centimetre floor
        // and the whole world, the ground included, went to mush.
        //
        // The lens is taken off entirely, by REMOVING the component — not by
        // setting the aperture to zero, which is what this tried first and
        // which is worse than doing nothing. Bevy builds its blur from
        // `focal_length² / (sensor_height * aperture_f_stops)` and guards
        // that division nowhere, so a zero aperture is a divide by zero, an
        // infinite circle of confusion, and a screen of pure smear. The
        // comment in `apply_look_settings` about adding and removing the
        // component rather than leaving it at zero aperture was right, and
        // this is the second reason for it.
        // Nor from anywhere past the play zoom: the diorama lens focused at
        // three thousand units smeared the whole climb - the band between
        // the play ceiling and orbit was a blur that read as a broken
        // renderer rather than a style.
        if !lens_belongs(rig) {
            if dof.is_some() {
                commands.entity(entity).remove::<DepthOfField>();
            }
            continue;
        }
        let focus = (rig.distance * look.focus_bias).max(0.1);
        match dof {
            // The aperture is left to `apply_look_settings` and `dream_lens`,
            // which own it; this system only ever aims the plane.
            Some(mut dof) => dof.focal_distance = focus,
            // Put back on the way out of a body.
            None if look.depth_of_field_enabled() => {
                commands.entity(entity).insert(DepthOfField {
                    mode: DepthOfFieldMode::Bokeh,
                    aperture_f_stops: look.aperture,
                    focal_distance: focus,
                    ..default()
                });
            }
            None => {}
        }
    }
}

/// Keeps the empty sky tracking the time of day.
///
/// This used to keep a distance-fog band ahead of the camera as well — the
/// fog that hid the streamed world's edge from the day the world first
/// streamed. The planet retired it: real terrain now stands behind the
/// loaded ground at every height, and the honest fix for "the world ends in
/// mist" turned out to be for the world not to end.
pub(crate) fn paint_the_sky(sky: Option<Res<crate::calendar::Sky>>, mut clear: ResMut<ClearColor>) {
    if let Some(sky) = &sky {
        clear.0 = sky.horizon;
    }
}

fn apply_look_settings(
    mut commands: Commands,
    look: Res<LookSettings>,
    mut effects: Query<
        (
            Entity,
            &CameraRig,
            &mut Bloom,
            &mut Vignette,
            &mut ColorGrading,
        ),
        With<GodCamera>,
    >,
    mut existing: Query<&mut DepthOfField, With<GodCamera>>,
) {
    for (entity, rig, mut bloom, mut vignette, mut color_grading) in &mut effects {
        bloom.intensity = look.bloom;
        vignette.intensity = look.vignette;
        *color_grading = grading(&look);

        // Depth of field is added and removed as a component rather than left in
        // place at zero aperture, so switching it off costs nothing per frame.
        // And never inserted where the lens does not belong: this used to
        // re-add it on any look change regardless of height, with the
        // DEFAULT focal distance - a few units - and the whole world past
        // arm's reach dissolved into featureless grey until it was hunted
        // down by altitude, twice.
        if !look.depth_of_field_enabled() || !lens_belongs(rig) {
            commands.entity(entity).remove::<DepthOfField>();
        } else if let Ok(mut dof) = existing.single_mut() {
            dof.aperture_f_stops = look.aperture;
        } else {
            commands.entity(entity).insert(DepthOfField {
                mode: DepthOfFieldMode::Bokeh,
                aperture_f_stops: look.aperture,
                // Aimed on arrival, not defaulted: the default focal
                // distance is a few units, and one frame of it is a grey
                // flash across the whole world.
                focal_distance: (rig.distance * look.focus_bias).max(0.1),
                ..default()
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_zero_aperture_disables_depth_of_field() {
        let off = LookSettings {
            aperture: 0.0,
            ..default()
        };
        assert!(!off.depth_of_field_enabled());
        assert!(LookSettings::default().depth_of_field_enabled());
    }

    #[test]
    fn default_look_is_within_sane_bounds() {
        let look = LookSettings::default();
        assert!(look.bloom >= 0.0 && look.bloom <= 1.0);
        assert!(look.vignette >= 0.0 && look.vignette <= 1.0);
        assert!(look.aperture > 0.0);
        assert!(look.focus_bias > 0.0);
    }
}
