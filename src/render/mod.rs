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
use bevy::pbr::{DistanceFog, FogFalloff};
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
use crate::terrain::LoadedChunks;

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
                    track_fog_to_zoom,
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
    /// How far the player can see, as a fraction of the streamed view distance.
    ///
    /// Below 1 so fog closes in before the last loaded chunk does. The alternative is
    /// what it replaced: the landscape stopping dead at a jagged edge with sky behind
    /// it, which advertises the streaming instead of hiding it.
    pub visibility: f32,
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
            visibility: 0.88,
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

/// The colour the world fades into, used for both the fog and the empty sky.
///
/// These two *must* be the same. Geometry past the end of the fog is drawn in the
/// fog colour; if the sky behind it is a different colour, the boundary between them
/// draws a hard line — which is what made the square edge of the sea show up as a
/// grey band across the horizon. Matching them means fully-fogged ground is
/// indistinguishable from no ground at all.
///
/// Sky blue on its own tints everything it touches, so mid-distance ground reads as
/// blue rather than hazy. Pulling it toward neutral keeps it reading as air.
pub fn horizon_color() -> Color {
    let sky = palette::shade(&palette::SKY, 0.62).to_linear();
    let neutral = palette::shade(&palette::BONE, 0.95).to_linear();
    let t = 0.42;
    Color::LinearRgba(LinearRgba {
        red: sky.red + (neutral.red - sky.red) * t,
        green: sky.green + (neutral.green - sky.green) * t,
        blue: sky.blue + (neutral.blue - sky.blue) * t,
        alpha: 1.0,
    })
}

/// Distance fog matched to the streamed view and the current zoom.
///
/// Two things this has to get right, both learned the hard way:
///
/// Fog is measured from the *camera*, but chunks load around the *focus*. Zoomed out,
/// the camera is already hundreds of units back, so fog sized from the view radius
/// alone swallows the entire scene. Offsetting by the orbit distance keeps whatever
/// the player is actually looking at clear at any zoom.
///
/// And it has to start late. Beginning a third of the way out means most of what is
/// on screen is behind haze; the job is to hide the edge of the world, not to veil
/// the world.
fn distance_fog(look: &LookSettings, camera_distance: f32, radius: f32) -> DistanceFog {
    let reach = radius * look.visibility.clamp(0.1, 1.0);

    // Loaded ground extends `radius` from the focus, so from the camera it reaches
    // roughly `camera_distance + radius`. Fog must land just inside that.
    let end = camera_distance + reach;
    let start = camera_distance + reach * 0.62;

    DistanceFog {
        color: horizon_color(),
        directional_light_color: palette::shade(&palette::BONE, 1.0),
        // Light bleeding through the haze toward the sun, which keeps the far
        // distance from going flat.
        directional_light_exponent: 24.0,
        falloff: FogFalloff::Linear { start, end },
    }
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
        distance_fog(&look, 60.0, 6.0 * 64.0),
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
                far: 3000.0,
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
        if rig.distance < crate::camera::FIRST_PERSON {
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

/// Keeps the fog band ahead of the camera as it zooms, and both fog and empty
/// sky tracking the time of day. The two must stay identical — see
/// [`horizon_color`] — so they are written together, from the same source.
fn track_fog_to_zoom(
    look: Res<LookSettings>,
    chunks: Option<Res<LoadedChunks>>,
    sky: Option<Res<crate::calendar::Sky>>,
    mut clear: ResMut<ClearColor>,
    mut cameras: Query<(&CameraRig, &mut DistanceFog), With<GodCamera>>,
) {
    let radius = chunks.map_or(384.0, |c| c.radius_units());
    for (rig, mut fog) in &mut cameras {
        *fog = distance_fog(&look, rig.distance, radius);
        if let Some(sky) = &sky {
            fog.color = sky.horizon;
            fog.directional_light_color = sky.sun_color;
            clear.0 = sky.horizon;
        }
    }
}

fn apply_look_settings(
    mut commands: Commands,
    look: Res<LookSettings>,
    mut effects: Query<(Entity, &mut Bloom, &mut Vignette, &mut ColorGrading), With<GodCamera>>,
    mut existing: Query<&mut DepthOfField, With<GodCamera>>,
) {
    for (entity, mut bloom, mut vignette, mut color_grading) in &mut effects {
        bloom.intensity = look.bloom;
        vignette.intensity = look.vignette;
        *color_grading = grading(&look);

        // Depth of field is added and removed as a component rather than left in
        // place at zero aperture, so switching it off costs nothing per frame.
        if !look.depth_of_field_enabled() {
            commands.entity(entity).remove::<DepthOfField>();
        } else if let Ok(mut dof) = existing.single_mut() {
            dof.aperture_f_stops = look.aperture;
        } else {
            commands.entity(entity).insert(DepthOfField {
                mode: DepthOfFieldMode::Bokeh,
                aperture_f_stops: look.aperture,
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
        assert!(look.visibility > 0.0 && look.visibility <= 1.0);
    }

    /// Widest radius the world ever streams to, matching `terrain::VIEW_CHUNKS`.
    const TEST_RADIUS: f32 = 20.0 * 64.0;

    fn falloff_of(look: &LookSettings, camera_distance: f32) -> (f32, f32) {
        let FogFalloff::Linear { start, end } =
            distance_fog(look, camera_distance, TEST_RADIUS).falloff
        else {
            panic!("expected linear falloff");
        };
        (start, end)
    }

    #[test]
    fn fog_closes_in_before_the_last_chunk() {
        // The whole point: the player must never see the edge of the streamed world.
        // Ground reaches `camera_distance + radius`; fog has to land inside that.
        let radius = TEST_RADIUS;
        for visibility in [0.1, 0.5, 0.88, 1.0] {
            for camera_distance in [12.0, 60.0, 200.0, 900.0] {
                let look = LookSettings {
                    visibility,
                    ..default()
                };
                let (start, end) = falloff_of(&look, camera_distance);
                assert!(
                    end <= camera_distance + radius,
                    "fog reaches {end}, past the loaded ground",
                );
                assert!(start < end, "fog starts after it ends");
                assert!(start > 0.0);
            }
        }
    }

    #[test]
    fn the_subject_stays_clear_at_every_zoom() {
        // Regression: fog was sized from the view radius alone, ignoring that it is
        // measured from the camera. Zoomed out, the camera sat far enough back that
        // the whole scene fell inside the fog band and the world went blue.
        let look = LookSettings::default();
        for camera_distance in [12.0, 60.0, 200.0, 600.0, 900.0] {
            let (start, _) = falloff_of(&look, camera_distance);
            assert!(
                start > camera_distance,
                "at zoom {camera_distance} fog starts at {start}, in front of the subject",
            );
        }
    }

    #[test]
    fn fog_starts_late_enough_to_leave_the_view_readable() {
        // Most of what is on screen should be clear. Fog hides the edge of the world,
        // it does not veil the world.
        let look = LookSettings::default();
        let (start, end) = falloff_of(&look, 60.0);
        let clear_span = start - 60.0;
        let fogged_span = end - start;
        assert!(
            clear_span > fogged_span,
            "clear {clear_span} vs fogged {fogged_span}",
        );
    }

    #[test]
    fn an_absurd_visibility_still_produces_valid_fog() {
        let look = LookSettings {
            visibility: 25.0,
            ..default()
        };
        let (start, end) = falloff_of(&look, 60.0);
        assert!(end <= 60.0 + TEST_RADIUS);
        assert!(start < end);
    }
}
