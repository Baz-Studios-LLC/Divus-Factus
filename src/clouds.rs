//! The weather deck: one shell of cloud around the whole planet.
//!
//! Brett asked for clouds on the planet when you pull back, and for the same
//! clouds overhead when you are standing in the village — and on a round world
//! those are not two features. A single sphere a few hundred units above sea
//! level is the sky from underneath and the weather from above, so a bank that
//! blew over the village is the bank you find on the ball when you zoom out.
//!
//! Nothing about it is stored or painted: the deck is a field over DIRECTIONS
//! from the planet's centre, evaluated per pixel, drifting because the direction
//! it samples turns slowly about the planet's axis. Same construction as the
//! terrain's own field, and for the same reason — a field on the sphere has no
//! seam to come apart on and no crowding at the poles, which every flat cloud
//! texture wrapped round a ball has both of.

use bevy::camera::visibility::RenderLayers;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::pbr::{Material, MaterialPlugin};
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;

use crate::palette;

/// How high the deck rides above sea level.
///
/// A compromise the round world forces, and worth stating: from the ground the
/// deck wants to be high — well over the treetops, high enough to read as sky.
/// From ORBIT it wants to be low, because a shell standing off a six-thousand
/// unit ball by any real fraction of its radius reads as a halo hanging around
/// the planet rather than as its weather. Four hundred and sixty units cleared
/// every possible peak and looked detached from space, which is exactly what
/// Brett saw. At this height it is a fifty-metre-scale sky from below and two
/// percent of the radius from above, and the tallest few summits in the world
/// stand up into it — which is what mountains do.
///
/// One knob, and the trade-off runs both ways: raise it and the deck is easier
/// to see from the village but stands off the ball from space; lower it and the
/// weather lies on the world but more summits pierce it.
const DECK_HEIGHT: f32 = 150.0;

/// How many noise units the planet's radius is worth. Sets the size of a
/// cloud: at this scale the fine octaves come out a few hundred units across,
/// which is a cloud, and the banks that carry them span a good part of a
/// continent, which is a weather system.
const DECK_SCALE: f32 = 22.0;

/// How fast the deck travels, in radians a second, at a dead calm and in a
/// storm. At this radius the slow end is about four units a second over the
/// ground — a breeze you can watch go past a longhouse — and the fast end is a
/// gale. One circuit of the world takes a few days either way, so the weather
/// on the far side is never quite the weather here.
const CALM_WIND: f32 = 0.00065;
const STORM_WIND: f32 = 0.0031;

/// Whether the god has sent the weather away.
///
/// Its own resource rather than a hidden debug dial, because a god who cannot
/// see the ground is a god who cannot judge it — and because the deck is the one
/// thing in this world that stands between the eye and everything else.
#[derive(Resource)]
pub struct TheSkyIsClear(pub bool);

impl Default for TheSkyIsClear {
    /// `DIVUS_FACTUS_CLOUDS=0` sends the weather away at startup, for
    /// photographing the ground itself. Its own dial because I spent three
    /// captures unable to tell cloud from snow.
    fn default() -> Self {
        TheSkyIsClear(std::env::var("DIVUS_FACTUS_CLOUDS").is_ok_and(|dial| dial == "0"))
    }
}

/// Draws the deck back when the god asks for a clear sky, and returns it when
/// they are done looking.
///
/// Driven by the RESOURCE and not by a key. A view toggle like this does not
/// earn a hotkey — the keyboard is for things a player reaches for mid-thought,
/// and there is a settings page for the rest (see `title::spawn_settings`).
fn part_the_clouds(
    clear: Res<TheSkyIsClear>,
    mut decks: Query<(&mut Visibility, Ref<CloudShell>), With<CloudShell>>,
) {
    // On a change, and also on the frame a deck first appears — otherwise a
    // startup dial would be read before there was anything to hide.
    let fresh = decks.iter().any(|(_, shell)| shell.is_added());
    if !clear.is_changed() && !fresh {
        return;
    }
    for (mut visibility, _) in &mut decks {
        *visibility = if clear.0 {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
}

/// Marks the deck. Excluded from the world bend: it is a sphere already
/// standing in world space around the planet's centre, not a flat thing waiting
/// to be wrapped onto one.
#[derive(Component)]
pub struct CloudShell;

pub struct CloudPlugin;

impl Plugin for CloudPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<CloudMaterial>::default())
            .init_resource::<TheSkyIsClear>()
            .add_systems(Startup, raise_the_deck)
            .add_systems(Update, (part_the_clouds, drive_the_deck).chain());
    }
}

/// The uniform the cloud shader reads.
#[derive(Clone, ShaderType, Debug)]
pub struct CloudParams {
    /// rgb the lit cloud colour, a the deck's greatest opacity.
    pub tint: Vec4,
    /// rgb the colour of cloud the sun has left.
    pub shade: Vec4,
    /// xyz toward the sun, w how much daylight there is.
    pub sun: Vec4,
    /// xyz the planet's axis, w how far the deck has turned on it.
    pub wind: Vec4,
    /// x coverage, y noise scale, z evolution clock, w edge softness.
    pub dials: Vec4,
}

impl Default for CloudParams {
    fn default() -> Self {
        CloudParams {
            tint: Vec4::new(1.0, 1.0, 1.0, 0.78),
            shade: Vec4::new(0.16, 0.2, 0.3, 1.0),
            sun: Vec3::Y.extend(1.0),
            wind: Vec3::Y.extend(0.0),
            // The last dial is how wide the cloud's EDGE is, in field units. It
            // was 0.16, which is most of the field's whole spread — so every
            // patch of sky sat somewhere on the ramp and the planet wore a thin
            // haze from pole to pole. Tight enough now that a cloud has an edge
            // and the sky between clouds is sky.
            dials: Vec4::new(0.35, DECK_SCALE, 0.0, 0.07),
        }
    }
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Default)]
pub struct CloudMaterial {
    #[uniform(0)]
    pub params: CloudParams,
}

impl Material for CloudMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/clouds.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // Both faces, and this is the whole point of the deck: from the village
        // the god is INSIDE the shell looking up at its underside, and from
        // orbit outside it looking down at its top. Cull either and the clouds
        // exist in only one of the two views — which, with the default back-face
        // culling, is exactly what happened: weather on the ball and an empty
        // sky over the village.
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

/// Hangs the deck, once.
fn raise_the_deck(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<CloudMaterial>>,
) {
    let radius = crate::terrain::PLANET_RADIUS + DECK_HEIGHT;
    commands.spawn((
        Name::new("The Weather"),
        CloudShell,
        // Enough facets that the shell's silhouette is a circle from orbit and
        // its underside is smooth overhead. The cost is one sphere.
        Mesh3d(meshes.add(Sphere::new(radius).mesh().ico(6).unwrap())),
        MeshMaterial3d(materials.add(CloudMaterial::default())),
        // Seen from INSIDE at the village and from outside at altitude, so
        // neither face may be thrown away.
        NotShadowCaster,
        NotShadowReceiver,
        Transform::from_translation(crate::globe::planet_centre()),
        RenderLayers::from_layers(&[0, crate::globe::GLOBE_LAYER]),
    ));
}

/// Turns the deck with the wind, and tells it where the sun is.
fn drive_the_deck(
    time: Res<Time>,
    sky: Res<crate::calendar::Sky>,
    weather: Option<Res<crate::weather::Weather>>,
    mut materials: ResMut<Assets<CloudMaterial>>,
    mut turned: Local<f32>,
) {
    // A clear day still has weather in it. Handing the weather's own intensity
    // straight through meant a fair sky - which is most skies, and every first
    // day - came out with no cloud in it at all, so the deck only existed in
    // rain. Mapped onto the upper two thirds instead: fair is scattered cloud,
    // rain is a covering, a storm closes over.
    let (coverage, wind) = weather.map_or((0.19, 0.25), |w| (0.10 + w.intensity * 0.75, w.wind));
    let rate = CALM_WIND + (STORM_WIND - CALM_WIND) * wind;
    *turned += rate * time.delta_secs();

    for (_, material) in materials.iter_mut() {
        let params = &mut material.params;
        params.sun = sky.sun_direction.extend(sky.daylight);
        params.wind = (crate::globe::planet_stance() * Vec3::Y).extend(*turned);
        params.dials.x = coverage;
        params.dials.z = time.elapsed_secs();
        // Cloud takes the light the ground takes: white at noon, gilded at
        // dusk, and the sunless side goes the colour of the night sky rather
        // than to black, so an overcast night is still a sky and not a lid.
        // Overdriven past white, for the same reason the sky is (see
        // `render::horizon_color`): the tonemapper darkens what it is handed,
        // and cloud handed its own colour comes back grey. Cloud in sunlight is
        // the brightest thing in a landscape and has to be written like it.
        //
        // And mostly WHITE rather than the sun's own colour. Taking the sun's
        // light straight made the deck cream at noon, because the key light is a
        // warm bone-white; a third of it is enough that dawn and dusk still gild
        // the tops without the middle of the day looking like old paper.
        const SUNLIT: f32 = 2.3;
        let white = Vec3::ONE;
        let sunlight = LinearRgba::from(sky.sun_color).to_vec3();
        params.tint = ((white + (sunlight - white) * 0.35) * SUNLIT).extend(
            // Thicker weather is a heavier deck, and a storm closes over.
            // Nearly opaque at the core: from orbit a thin deck reads as a
            // smear of haze rather than as cloud lying on the world.
            0.80 + coverage * 0.18,
        );
        // Cloud the sun has left. Not black and not slate: an overcast at night
        // still catches the moon and the sky, so it stays a pale cold grey —
        // dark enough to be night, light enough to still read as cloud.
        params.shade = LinearRgba::from(crate::calendar::mix_colors(
            palette::shade(&palette::SKY, 0.5),
            palette::shade(&palette::STONE, 0.75),
            0.45,
        ))
        .to_vec3()
        .extend(1.0);
    }
}
