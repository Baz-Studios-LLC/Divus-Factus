//! The world's clock: day and night, and the calendar that will hang off it.
//!
//! One resource ticks forward; everything temporal derives from it. The sun's
//! path, the light's colour, the horizon, the date in the HUD — and eventually
//! work schedules, seasons, festivals and weather — all read the same number,
//! so none of them can drift apart.
//!
//! The visual half lives in [`Sky`]: a small bundle of "what the light is doing
//! right now" recomputed each frame from the clock. Renderer, water and
//! lighting consume `Sky` rather than the clock directly, so when seasons and
//! weather arrive they only need to bend `Sky`, not every consumer.

use bevy::camera::visibility::RenderLayers;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;
use std::f32::consts::PI;

use crate::palette;

/// Real seconds per world day. Ten minutes: long enough that morning and
/// evening are moods rather than a strobe, short enough that a sitting sees
/// several days.
pub const DAY_SECONDS: f32 = 600.0;

/// Fraction of the day the sun is up.
const DAYLIGHT_FRACTION: f32 = 0.72;

/// How far from the eye the sun's cascades reach, and so how far shadows can
/// possibly land. Sized to the world: the default configuration covers a few
/// dozen units, which on a map this size means shadows stop a short way from the
/// camera, which reads as flat.
///
/// The number is public because it decides more than the cascades. Beyond this
/// distance a shadow cannot land at all, which makes it the honest altitude to
/// stop PAYING for them (see [`shadows_can_land`]).
pub const SHADOW_REACH: f32 = 900.0;

/// Whether shadows can still land anywhere the camera can see, given how far
/// back it has pulled.
///
/// Brett asked whether shadow quality could come down, and the measurement said
/// something better: past a certain height they are already gone. The same scene
/// with shadows and without, differing pixels as a share of the frame —
///
///   alt   60: 0.74%      alt  400: 0.46%      alt 1000: 0.01%
///   alt  200: 1.47%      alt  700: 0.10%      alt 1400: 0.00%
///
/// — while costing 3.5ms of a 26ms frame at the top of that range. They fade on
/// their own because the cascades END at [`SHADOW_REACH`]: pull back further than
/// that and the ground the camera is looking at lies beyond the last cascade, so
/// nothing can be cast onto it. Nobody built that fade; it falls out of the
/// reach. All this does is stop paying for what has already faded.
///
/// Which is why the threshold is derived from the reach and not written as an
/// altitude — move one and the other follows. Confirmed at DAWN, where a grazing
/// sun throws the longest shadows in the day, and again at NOON, where they are
/// shortest and hardest: 0.01% either way. The margin above the reach covers
/// ground nearer than the focus, a peak between the eye and the village.
///
/// Hysteresis, because a god hovering exactly on the line would otherwise
/// flicker the whole world's shadows on and off every frame.
pub fn shadows_can_land(distance: f32, currently_cast: bool) -> bool {
    const MARGIN: f32 = 1.05;
    if currently_cast {
        distance < SHADOW_REACH * MARGIN
    } else {
        distance < SHADOW_REACH
    }
}

/// Days in one season, Stardew-fashion: long enough to live a stretch of
/// life in, short enough that the turn is always coming.
pub const DAYS_PER_SEASON: u32 = 28;

/// Seasons in a year.
pub const SEASONS_PER_YEAR: u32 = 4;

/// The quarter of the year, and everything that leans on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Season {
    Spring,
    Summer,
    Autumn,
    Winter,
}

impl Season {
    pub fn name(self) -> &'static str {
        match self {
            Season::Spring => "spring",
            Season::Summer => "summer",
            Season::Autumn => "autumn",
            Season::Winter => "winter",
        }
    }

    /// How readily living things grow: crops, berry bushes, saplings.
    /// Winter all but stops the fields - the larder carries the village
    /// through, or it does not.
    pub fn growth(self) -> f32 {
        match self {
            Season::Spring => 1.2,
            Season::Summer => 1.0,
            Season::Autumn => 0.7,
            Season::Winter => 0.08,
        }
    }

    /// Cold laid over whatever the weather is doing. Winter nights bite.
    pub fn chill(self) -> f32 {
        match self {
            Season::Spring => 0.05,
            Season::Summer => 0.0,
            Season::Autumn => 0.12,
            Season::Winter => 0.35,
        }
    }

    /// The season's thumb on the weather dice: shifted toward grey and
    /// storm in the dark half of the year, toward clear skies in summer.
    pub fn gloom(self) -> f32 {
        match self {
            Season::Spring => 0.05,
            Season::Summer => -0.08,
            Season::Autumn => 0.08,
            Season::Winter => 0.14,
        }
    }

    /// The cast of the light: winter pales and cools, autumn gilds.
    fn light_tint(self) -> Option<(Color, f32)> {
        match self {
            Season::Spring => None,
            Season::Summer => Some((Color::srgb(1.0, 0.96, 0.86), 0.08)),
            Season::Autumn => Some((Color::srgb(1.0, 0.82, 0.55), 0.16)),
            Season::Winter => Some((Color::srgb(0.82, 0.88, 1.0), 0.22)),
        }
    }
}

pub struct CalendarPlugin;

impl Plugin for CalendarPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldClock>()
            .init_resource::<Sky>()
            .add_systems(Startup, hang_the_sky)
            .add_systems(
                Update,
                (
                    tick,
                    drive_sky,
                    apply_sky_to_lights,
                    carry_the_bodies,
                    herald_seasons,
                )
                    .chain(),
            );
    }
}

/// World time since founding, in seconds. Everything temporal derives from this.
#[derive(Resource, serde::Serialize, serde::Deserialize)]
pub struct WorldClock {
    pub elapsed: f64,
}

impl Default for WorldClock {
    /// Games open mid-morning: the first thing the player sees should be the
    /// world lit well, not a night they had no say in. `DIVUS_FACTUS_CLOCK=0.8`
    /// starts elsewhere in the day, for looking at dusk without waiting for it.
    fn default() -> Self {
        let start = std::env::var("DIVUS_FACTUS_CLOCK")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(0.22);
        // DIVUS_FACTUS_DAY jumps the calendar for testing a season without
        // living to it: 29 is the first day of summer, 85 of winter.
        let day = std::env::var("DIVUS_FACTUS_DAY")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .map_or(0, |d| d.saturating_sub(1));
        WorldClock {
            elapsed: (DAY_SECONDS * start) as f64 + (DAY_SECONDS as f64) * day as f64,
        }
    }
}

impl WorldClock {
    /// Position within the current day, 0 dawn to 1 the next dawn.
    pub fn time_of_day(&self) -> f32 {
        (self.elapsed / DAY_SECONDS as f64).fract() as f32
    }

    /// The current day, counted from 1.
    pub fn day(&self) -> u32 {
        (self.elapsed / DAY_SECONDS as f64) as u32 + 1
    }

    /// Sine of the sun's elevation: positive by day, negative through the
    /// night, crossing zero at dawn and dusk.
    pub fn sun_elevation(&self) -> f32 {
        let t = self.time_of_day();
        if t < DAYLIGHT_FRACTION {
            (t / DAYLIGHT_FRACTION * PI).sin()
        } else {
            -((t - DAYLIGHT_FRACTION) / (1.0 - DAYLIGHT_FRACTION) * PI).sin()
        }
    }

    /// Where the sun actually IS, as a unit vector from the planet's centre.
    ///
    /// The world is a sphere, so the sun can stop being a lighting trick and
    /// become a body with a position. It travels one circuit of the planet a
    /// day, rising in the east, passing overhead at noon, setting in the west —
    /// and while it does, the far side of the world is genuinely in shadow,
    /// because the light comes from where the sun is rather than from wherever
    /// the ground happens to be standing. Everything the old fake did — a fixed
    /// bearing, an elevation that swung but never set, a clamp holding it above
    /// the horizon so its transform stayed defined — is gone with it.
    ///
    /// The circuit is not uniform in angle, and that is deliberate. Every hour
    /// this game keeps — the work shifts, the midday meal, the evening, sleep —
    /// is tuned against [`DAYLIGHT_FRACTION`], seven tenths of the day in the
    /// light. A sun sweeping at a constant rate would give exactly half, and
    /// villagers would be hoeing in the dark. So the angle is derived FROM the
    /// elevation the clock already dictates: `elevation = cos θ` about the
    /// planet's own axis, which puts the sun overhead at noon, under the world
    /// at midnight, and on the right side of the sky at every hour between.
    pub fn sun_position(&self) -> Vec3 {
        // Home stands at the top of the world (see `globe::planet_stance`),
        // its north pole a quarter turn away, and east the cross of the two.
        let up = Vec3::Y;
        let polar = crate::globe::planet_stance() * Vec3::Y;
        let east = polar.cross(up).normalize_or(Vec3::X);

        let t = self.time_of_day();
        // Noon is the middle of the daylight, midnight the middle of the dark.
        let noon = DAYLIGHT_FRACTION * 0.5;
        let midnight = (DAYLIGHT_FRACTION + 1.0) * 0.5;
        // East of the meridian before noon and after midnight, west between.
        let easterly = t < noon || t >= midnight;
        let angle = self.sun_elevation().clamp(-1.0, 1.0).acos() * if easterly { 1.0 } else { -1.0 };

        (up * angle.cos() + east * angle.sin()).normalize_or(Vec3::Y)
    }

    /// Whether decent people are asleep.
    pub fn is_night(&self) -> bool {
        let t = self.time_of_day();
        t >= 0.74 || t < 0.03
    }

    /// The bells a day rings for its people: when work may start, when the
    /// square fills for the midday meal, when evening claims everyone home.
    /// One authority, so every system keeps the same hours.
    pub fn work_hours(&self) -> bool {
        let t = self.time_of_day();
        // Morning shift and afternoon shift, split by the midday meal.
        (0.06..0.34).contains(&t) || (0.40..0.62).contains(&t)
    }

    /// The midday meal: tools down, everyone drifts to the square.
    pub fn midday_meal(&self) -> bool {
        let t = self.time_of_day();
        (0.34..0.40).contains(&t)
    }

    /// The evening: supper at the hearth, the tavern, the fire.
    pub fn is_evening(&self) -> bool {
        let t = self.time_of_day();
        (0.62..0.74).contains(&t)
    }

    /// The part of the day, as a villager would name it.
    pub fn phase_name(&self) -> &'static str {
        let t = self.time_of_day();
        match t {
            t if t < 0.08 => "dawn",
            t if t < 0.28 => "morning",
            t if t < 0.44 => "midday",
            t if t < 0.60 => "afternoon",
            t if t < 0.72 => "evening",
            t if t < 0.86 => "night",
            _ => "the small hours",
        }
    }

    /// The season this day falls in.
    pub fn season(&self) -> Season {
        match ((self.day() - 1) / DAYS_PER_SEASON) % SEASONS_PER_YEAR {
            0 => Season::Spring,
            1 => Season::Summer,
            2 => Season::Autumn,
            _ => Season::Winter,
        }
    }

    /// The day within the season, 1 to 28.
    pub fn day_of_season(&self) -> u32 {
        (self.day() - 1) % DAYS_PER_SEASON + 1
    }

    /// The year, counted from 1.
    pub fn year(&self) -> u32 {
        (self.day() - 1) / (DAYS_PER_SEASON * SEASONS_PER_YEAR) + 1
    }

    /// The date, as the HUD shows it: "spring 14, year 1 - morning".
    pub fn date_phrase(&self) -> String {
        format!(
            "{} {}, year {} - {}",
            self.season().name(),
            self.day_of_season(),
            self.year(),
            self.phase_name()
        )
    }
}

/// The calendar date a bare day-number falls on: "spring 14, year 2".
/// For records that stored the day and want to speak like the HUD does.
pub fn date_of_day(day: u32) -> String {
    let season = match ((day - 1) / DAYS_PER_SEASON) % SEASONS_PER_YEAR {
        0 => Season::Spring,
        1 => Season::Summer,
        2 => Season::Autumn,
        _ => Season::Winter,
    };
    format!(
        "{} {}, year {}",
        season.name(),
        (day - 1) % DAYS_PER_SEASON + 1,
        (day - 1) / (DAYS_PER_SEASON * SEASONS_PER_YEAR) + 1
    )
}

fn tick(time: Res<Time>, mut clock: ResMut<WorldClock>) {
    clock.elapsed += time.delta_secs() as f64;
}

/// What the light is doing right now. Derived from the clock every frame;
/// consumed by the lights, the fog, the sky and the water.
#[derive(Resource)]
pub struct Sky {
    /// Unit vector from the planet's centre toward the sun — where the sun
    /// actually is, and below the horizon for half of every day.
    pub sun_direction: Vec3,
    pub sun_color: Color,
    pub sun_illuminance: f32,
    /// The colour of fog, empty sky and the water's reflection.
    pub horizon: Color,
    pub ambient_color: Color,
    pub ambient_brightness: f32,
    pub fill_illuminance: f32,
    /// The moon's strength. It shines at a steady low weight and needs no
    /// ramp: standing opposite the sun, it only ever falls on the half of the
    /// world the sun has left.
    pub moon_illuminance: f32,
    /// Which way the sky's cool fill comes from. Local, not global — see
    /// [`apply_sky_to_lights`].
    pub fill_direction: Vec3,
    /// How much daylight there is, 0 at night to 1 at noon. For anything that
    /// needs to dim with the day — the water's body colour, for one.
    pub daylight: f32,
}

impl Default for Sky {
    fn default() -> Self {
        sky_toward(Vec3::Y, 1.0)
    }
}

/// Smooth 0-to-1 ramp between two edges.
fn smoothstep(lo: f32, hi: f32, x: f32) -> f32 {
    let t = ((x - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Linear blend between two colours, used by everything that follows the sun.
pub fn mix_colors(a: Color, b: Color, t: f32) -> Color {
    let (a, b) = (a.to_linear(), b.to_linear());
    Color::LinearRgba(LinearRgba {
        red: a.red + (b.red - a.red) * t,
        green: a.green + (b.green - a.green) * t,
        blue: a.blue + (b.blue - a.blue) * t,
        alpha: 1.0,
    })
}

/// The whole sky, as a function of the sun's elevation ABOVE THE GROUND BEING
/// LOOKED AT, and of where the sun stands.
///
/// Two arguments now, because on a round world those are two different things:
/// the sun has one position, and every place on the planet sees it at its own
/// height above its own horizon. The elevation shapes the light's colour and
/// weight — dawn, noon, dusk — while the position aims it, and the far side of
/// the world is dark for the honest reason that the sun is on the other side of
/// it.
fn sky_toward(sun_direction: Vec3, elevation: f32) -> Sky {
    // How much daylight there is, easing in shortly after sunrise.
    let lit = smoothstep(-0.04, 0.30, elevation);
    // How close to the horizon the sun is while up — the golden-hour weight.
    let low_sun = (1.0 - smoothstep(0.12, 0.45, elevation)) * smoothstep(-0.10, 0.05, elevation);

    // Noon-white bending toward gold as the sun drops.
    let sun_color = mix_colors(
        palette::shade(&palette::BONE, 1.0),
        palette::shade(&palette::CLOTH_GOLD, 0.85),
        low_sun * 0.65,
    );

    // The horizon walks day → dusk gold → deep night blue.
    let day_horizon = crate::render::horizon_color();
    let dusk = mix_colors(
        day_horizon,
        palette::shade(&palette::CLOTH_GOLD, 0.62),
        0.55,
    );
    let night = mix_colors(
        palette::shade(&palette::SKY, 0.10),
        Color::srgb(0.010, 0.014, 0.030),
        0.55,
    );
    let horizon = mix_colors(
        mix_colors(night, dusk, smoothstep(-0.22, 0.02, elevation)),
        day_horizon,
        lit,
    );

    Sky {
        sun_direction,
        sun_color,
        sun_illuminance: 17_000.0 * lit,
        horizon,
        ambient_color: mix_colors(
            palette::shade(&palette::SKY, 0.25),
            palette::shade(&palette::SKY, 0.7),
            lit,
        ),
        // Never fully dark: the player must always be able to read the world
        // they are god of. Night is a mood, not a lost turn.
        ambient_brightness: 36.0 + 94.0 * lit,
        // The fill's night floor came down when the moon arrived. It used to
        // carry the whole night by itself from a fixed bearing, which is why
        // nothing had a direction after dusk; now the moon does that, and the
        // fill is back to being what it is for — keeping shadowed faces blue
        // rather than black.
        fill_illuminance: 480.0 + 2_500.0 * lit,
        // Flat, because the geometry does the work. See the field.
        moon_illuminance: 430.0,
        // Overwritten with the local bearing by `drive_sky`; this is the
        // straight-up default for the one frame before it runs.
        fill_direction: Vec3::Y,
        daylight: lit,
    }
}

fn drive_sky(
    clock: Res<WorldClock>,
    weather: Option<Res<crate::weather::Weather>>,
    cameras: Query<&crate::camera::CameraRig>,
    mut sky: ResMut<Sky>,
) {
    let sun = clock.sun_position();
    // How high the sun stands over the ground the god is LOOKING AT, which on
    // a sphere is not the same question as what time it is. The clock is the
    // village's hour and the simulation's — its people sleep and work by it —
    // but the sky belongs to a place. Carry the view to the far side of the
    // planet and it is night there, in the middle of the village's afternoon,
    // because the sun is over the horizon they share.
    let looking_at = cameras
        .iter()
        .next()
        .map_or(Vec3::ZERO, |rig| rig.focus)
        .with_y(0.0);
    let (_, turn) = crate::globe::bend_frame(looking_at);
    let up = turn * Vec3::Y;
    *sky = sky_toward(sun, sun.dot(up));

    // The cool fill is a SKY light, and a sky is local. Left at a fixed world
    // bearing it lit the whole planet from one side at three thousand lux with
    // no falloff, which is what flattened the terminator to nothing: the night
    // half of the world came out very nearly as bright as the day half, and a
    // sun that orbits a planet with no day and night line on it is a sun that
    // may as well not. Turned into the frame of the ground being looked at, it
    // is exactly the shaping light it always was where the player is, and it
    // falls away to nothing over the horizon where a sky should.
    sky.fill_direction = turn * FILL_BEARING;
    // The season casts the light before the weather does: winter pale and
    // cool, autumn gilded, summer faintly honeyed.
    if let Some((tint, strength)) = clock.season().light_tint() {
        sky.sun_color = mix_colors(sky.sun_color, tint, strength * sky.daylight);
        sky.horizon = mix_colors(sky.horizon, tint, strength * 0.5 * sky.daylight);
        if clock.season() == Season::Winter {
            sky.sun_illuminance *= 0.88;
        }
    }
    // Weather sits on top of the hour: the sun dims behind the deck and the
    // horizon greys - and because fog reads the horizon, rain greys the
    // whole distance with it.
    if let Some(weather) = weather {
        let i = weather.intensity;
        sky.sun_illuminance *= 1.0 - i * 0.55;
        sky.fill_illuminance *= 1.0 - i * 0.3;
        sky.ambient_brightness *= 1.0 - i * 0.25;
        let grey = Color::srgb(0.52, 0.55, 0.58);
        sky.horizon = mix_colors(sky.horizon, grey, i * 0.45 * sky.daylight);
        sky.sun_color = mix_colors(sky.sun_color, grey, i * 0.4);
    }
}

/// Announces the turn of each season - the calendar's one fanfare.
fn herald_seasons(
    clock: Res<WorldClock>,
    mut last: Local<Option<Season>>,
    mut notices: MessageWriter<crate::ui::Notice>,
) {
    let season = clock.season();
    match *last {
        None => *last = Some(season),
        Some(previous) if previous != season => {
            *last = Some(season);
            info!("the season turns: {}", season.name());
            notices.write(crate::ui::Notice::fanfare(format!(
                "{} settles over the land",
                match season {
                    Season::Spring => "Spring",
                    Season::Summer => "Summer",
                    Season::Autumn => "Autumn",
                    Season::Winter => "Winter",
                }
            )));
        }
        _ => {}
    }
}

/// Marks the key light.
#[derive(Component)]
pub struct SunLight;

/// Marks the cool counter-light.
#[derive(Component)]
pub struct FillLight;

/// Where the sky's cool fill comes from, in the frame of the ground below it:
/// up and over the left shoulder. The bearing the world has always been lit
/// from — it is only its frame that changed.
const FILL_BEARING: Vec3 = Vec3::new(-0.50, 0.55, -0.67);

/// Marks the moon: a dim cool light standing always opposite the sun.
///
/// It needs no schedule and gets none. Antipodal to the sun, it falls on
/// exactly the half of the planet the sun has left, so the geometry alone puts
/// moonlight on the night side and nothing on the day side — and a moon
/// opposite its sun is a moon always full, which is the one phase this needs
/// to be.
#[derive(Component)]
pub struct MoonLight;

/// The sun and the moon THEMSELVES — the bodies, not their light.
///
/// Excluded from the world bend: their places are already world positions out
/// in space, not flat ground waiting to be wrapped onto a sphere.
#[derive(Component, PartialEq, Eq, Clone, Copy, Debug)]
pub enum Celestial {
    Sun,
    Moon,
}

/// How far out the two bodies ride, and how big they are there.
///
/// Both well inside the camera's seventy-thousand-unit far plane, and far
/// enough out that they read as sky rather than scenery: from the ground the
/// sun is a disc a couple of degrees across, and from orbit you can watch it
/// come round the planet.
const SUN_ORBIT: f32 = 42_000.0;
const SUN_SIZE: f32 = 760.0;
const MOON_ORBIT: f32 = 26_000.0;
const MOON_SIZE: f32 = 520.0;

fn apply_sky_to_lights(
    sky: Res<Sky>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut suns: Query<(&mut DirectionalLight, &mut Transform), (With<SunLight>, Without<FillLight>)>,
    mut fills: Query<(&mut DirectionalLight, &mut Transform), (With<FillLight>, Without<SunLight>)>,
    mut moons: Query<
        (&mut DirectionalLight, &mut Transform),
        (With<MoonLight>, Without<SunLight>, Without<FillLight>),
    >,
    eyes: Query<&crate::camera::CameraRig, With<crate::camera::GodCamera>>,
) {
    ambient.color = sky.ambient_color;
    ambient.brightness = sky.ambient_brightness;

    let centre = crate::globe::planet_centre();
    for (mut light, mut transform) in &mut suns {
        light.color = sky.sun_color;
        light.illuminance = sky.sun_illuminance.max(1.0);
        // No shadows in the dark: with the sun's light at nothing, any
        // shadow it casts is pure artifact - and the artifacts move.
        //
        // `DIVUS_FACTUS_SHADOWS=0` lifts them for measurement, beside the fog
        // and cloud dials. The shadow pass is worth 2.8ms of a 27ms frame at the
        // altitude where frames drop, so it is a number worth being able to take.
        // And no shadows from too far back, where the cascades cannot reach
        // the ground anyway: 3.5ms for a frame that measures identical.
        let pulled_back = eyes.iter().next().map(|rig| rig.distance);
        light.shadow_maps_enabled = sky.daylight > 0.02
            && !std::env::var("DIVUS_FACTUS_SHADOWS").is_ok_and(|dial| dial == "0")
            && pulled_back
                .is_none_or(|distance| shadows_can_land(distance, light.shadow_maps_enabled));
        // A grazing sun slides every shadow off its caster - the depth bias
        // that is right for a light overhead is nowhere near enough for one
        // coming in almost flat, and trees end up casting shadows that detach
        // and sweep the land. The old sun never set, so it never met the
        // problem; this one does, twice a day. The bias opens up as it drops.
        let grazing = 1.0 - smoothstep(0.05, 0.5, sky.sun_direction.dot(Vec3::Y).abs());
        light.shadow_depth_bias = 0.02 + grazing * 0.35;
        light.shadow_normal_bias = 1.8 + grazing * 5.0;
        // Aimed from where the sun IS. A directional light only cares about
        // the direction, but the distance keeps the transform readable in the
        // inspector and matches the body's own place.
        *transform = Transform::from_translation(centre + sky.sun_direction * SUN_ORBIT)
            .looking_at(centre, Vec3::Y);
    }
    for (mut light, mut transform) in &mut fills {
        light.illuminance = sky.fill_illuminance;
        *transform = Transform::from_translation(centre + sky.fill_direction * 9_000.0)
            .looking_at(centre, Vec3::Y);
    }

    for (mut light, mut transform) in &mut moons {
        light.illuminance = sky.moon_illuminance;
        *transform = Transform::from_translation(centre - sky.sun_direction * MOON_ORBIT)
            .looking_at(centre, Vec3::Y);
    }
}

/// Carries the two bodies round with their light.
fn carry_the_bodies(sky: Res<Sky>, mut bodies: Query<(&mut Transform, &Celestial)>) {
    let centre = crate::globe::planet_centre();
    for (mut transform, body) in &mut bodies {
        transform.translation = match body {
            Celestial::Sun => centre + sky.sun_direction * SUN_ORBIT,
            Celestial::Moon => centre - sky.sun_direction * MOON_ORBIT,
        };
    }
}

/// Hangs the sun and the moon in the sky, once.
///
/// Unlit, both of them: a sun that took lighting would be lit by itself, and
/// the moon has to glow at night, when by construction nothing else does. They
/// cast and receive no shadows for the same reason.
fn hang_the_sky(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let bodies = [
        (
            Celestial::Sun,
            "The Sun",
            SUN_SIZE,
            palette::shade(&palette::BONE, 1.0),
            palette::shade(&palette::CLOTH_GOLD, 1.0),
            9.0,
        ),
        (
            Celestial::Moon,
            "The Moon",
            MOON_SIZE,
            palette::shade(&palette::BONE, 0.62),
            palette::shade(&palette::SKY, 0.9),
            1.4,
        ),
    ];
    for (which, name, size, body, glow, strength) in bodies {
        commands.spawn((
            Name::new(name),
            which,
            Mesh3d(meshes.add(Sphere::new(size).mesh().ico(4).unwrap())),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: body,
                emissive: LinearRgba::from(glow) * strength,
                unlit: true,
                ..default()
            })),
            Transform::from_translation(crate::globe::planet_centre() + Vec3::Y * SUN_ORBIT),
            // Seen from the ground, from the air, and from orbit: the world's
            // layer and the planet's both.
            RenderLayers::from_layers(&[0, crate::globe::GLOBE_LAYER]),
            NotShadowCaster,
            NotShadowReceiver,
        ));
    }
}

#[cfg(test)]
mod tests {
    /// The measured visibility ladder: camera distance against how much of the
    /// frame changed when shadows were taken away, at dawn and at noon both.
    ///
    /// This is the evidence the cutoff rests on, kept as data so the code cannot
    /// drift from it. Lower [`SHADOW_REACH`] without re-measuring and this fails,
    /// because shadows would stop being drawn at a height where the pixels say
    /// they are still plainly there.
    pub(super) const MEASURED: [(f32, f32); 6] = [
        (60.0, 0.74),
        (200.0, 1.47),
        (400.0, 0.46),
        (700.0, 0.10),
        (1000.0, 0.01),
        (1400.0, 0.00),
    ];

    /// Below what share of the frame a difference is indistinguishable from
    /// run-to-run noise. Measured, not chosen: two runs of the same scene whose
    /// only difference was shadows at an altitude where shadows cannot land came
    /// out 0.03%-0.06% apart, villagers having walked in between.
    pub(super) const NOISE: f32 = 0.06;

    use super::*;

    #[test]
    fn the_calendar_keeps_stardew_time() {
        let at_day = |day: u32| WorldClock {
            elapsed: (day - 1) as f64 * DAY_SECONDS as f64 + 1.0,
        };
        assert_eq!(at_day(1).season(), Season::Spring);
        assert_eq!(at_day(1).day_of_season(), 1);
        assert_eq!(at_day(28).season(), Season::Spring);
        assert_eq!(at_day(29).season(), Season::Summer);
        assert_eq!(at_day(29).day_of_season(), 1);
        assert_eq!(at_day(57).season(), Season::Autumn);
        assert_eq!(at_day(85).season(), Season::Winter);
        assert_eq!(at_day(112).year(), 1);
        assert_eq!(at_day(113).season(), Season::Spring);
        assert_eq!(at_day(113).year(), 2);
        assert!(at_day(90).date_phrase().starts_with("winter 6, year 1"));
    }

    #[test]
    fn the_sun_rises_sets_and_returns() {
        let at = |t: f32| WorldClock {
            elapsed: (DAY_SECONDS * t) as f64,
        };
        assert!(at(0.3).sun_elevation() > 0.5, "midday should be bright");
        assert!(at(0.85).sun_elevation() < -0.3, "night should be dark");
        // Dawn of day two looks like dawn of day one.
        let e1 = at(0.1).sun_elevation();
        let e2 = at(1.1).sun_elevation();
        assert!((e1 - e2).abs() < 1e-4);
    }

    #[test]
    fn days_are_counted_from_one_and_advance() {
        let mut clock = WorldClock { elapsed: 0.0 };
        assert_eq!(clock.day(), 1);
        clock.elapsed = (DAY_SECONDS * 2.5) as f64;
        assert_eq!(clock.day(), 3);
        assert!(!clock.date_phrase().is_empty());
    }

    #[test]
    fn night_still_leaves_the_world_readable() {
        let midnight = sky_toward(Vec3::NEG_Y, -1.0);
        assert!(midnight.ambient_brightness > 10.0, "night went pitch black");
        assert!(
            midnight.sun_illuminance < 100.0,
            "the sun shone at midnight"
        );
        // And the moon is what carries it: a light with a direction, so a hill
        // still has a lit side after dusk.
        assert!(midnight.moon_illuminance > 100.0, "no moon at midnight");

        let noon = sky_toward(Vec3::Y, 1.0);
        assert!(noon.sun_illuminance > 10_000.0);
        assert!(noon.ambient_brightness > midnight.ambient_brightness);
    }

    /// The sun is a body with a place now, and its place is only ever a unit
    /// vector from the planet's centre — including through the night, when it
    /// is under the world and the direction has to stay usable.
    #[test]
    fn the_sun_direction_is_always_usable() {
        for i in 0..240 {
            let clock = WorldClock {
                elapsed: (DAY_SECONDS * i as f32 / 120.0) as f64,
            };
            let sun = clock.sun_position();
            assert!(
                (sun.length() - 1.0).abs() < 1e-4,
                "sun bearing is not a direction at {i}: {sun}"
            );
        }
    }

    /// It rises in the east, stands overhead at noon, sets in the west, and
    /// spends the night under the world. None of which the old one did: it
    /// held one bearing all day and was clamped above the horizon so it could
    /// never set at all.
    #[test]
    fn the_sun_crosses_the_sky_and_goes_under_the_world() {
        let at = |t: f32| {
            WorldClock {
                elapsed: (DAY_SECONDS * t) as f64,
            }
            .sun_position()
        };
        // East at home is +x; see `globe::bend_frame`.
        let morning = at(0.12);
        let evening = at(0.60);
        assert!(morning.x > 0.2, "the morning sun was not in the east");
        assert!(evening.x < -0.2, "the evening sun was not in the west");
        assert!(morning.y > 0.0 && evening.y > 0.0, "day is above ground");

        let noon = at(DAYLIGHT_FRACTION * 0.5);
        assert!(noon.dot(Vec3::Y) > 0.999, "noon is not overhead");

        let midnight = at((DAYLIGHT_FRACTION + 1.0) * 0.5);
        assert!(
            midnight.dot(Vec3::Y) < -0.999,
            "midnight is not under the world"
        );
    }

    /// The whole point of the exercise: night is a PLACE. The same instant is
    /// day at home and dark on the far side of the planet, because the sun is
    /// somewhere rather than everywhere.
    #[test]
    fn the_far_side_is_dark_at_home_noon() {
        let clock = WorldClock {
            elapsed: (DAY_SECONDS * DAYLIGHT_FRACTION * 0.5) as f64,
        };
        let sun = clock.sun_position();

        let home = sky_toward(sun, sun.dot(Vec3::Y));
        let antipode = sky_toward(sun, sun.dot(Vec3::NEG_Y));
        assert!(home.daylight > 0.95, "noon at home was not bright");
        assert!(
            antipode.daylight < 0.05,
            "the far side of the world shared home's noon"
        );
        // And the moon shines on with no regard for either: opposite the sun,
        // it lands where the sun does not, and the geometry sorts it out.
        assert_eq!(home.moon_illuminance, antipode.moon_illuminance);
    }

    #[test]
    fn every_hour_has_a_name() {
        for i in 0..40 {
            let clock = WorldClock {
                elapsed: (DAY_SECONDS * i as f32 / 40.0) as f64,
            };
            assert!(!clock.phase_name().is_empty());
        }
    }
}
#[cfg(test)]
mod shadow_cutoff_tests {
    use super::tests::{MEASURED, NOISE};
    use super::*;

    #[test]
    fn shadows_are_kept_wherever_they_can_be_seen() {
        for (distance, visible) in MEASURED {
            if visible <= NOISE {
                continue;
            }
            assert!(
                shadows_can_land(distance, true),
                "shadows change {visible}% of the frame at distance {distance} -                  far above the {NOISE}% noise floor - so they must still be cast                  there. SHADOW_REACH is {SHADOW_REACH}, which is too short for                  what was measured."
            );
        }
    }

    #[test]
    fn shadows_are_dropped_where_they_cannot_land() {
        for (distance, visible) in MEASURED {
            if visible > NOISE {
                continue;
            }
            assert!(
                !shadows_can_land(distance, true),
                "shadows change only {visible}% of the frame at distance                  {distance} - noise - yet are still being paid for, at 3.5ms of                  a 26ms frame."
            );
        }
    }

    #[test]
    fn the_threshold_does_not_flicker_when_hovering_on_it() {
        // Just inside the margin: a god drifting across the line holds whatever
        // state they arrived in rather than switching every frame.
        let on_the_line = SHADOW_REACH * 1.02;
        assert!(
            shadows_can_land(on_the_line, true),
            "shadows already cast must survive a small drift outward"
        );
        assert!(
            !shadows_can_land(on_the_line, false),
            "shadows already dropped must not come back on the same drift, or              the two states chase each other every frame"
        );
    }
}
