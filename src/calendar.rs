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

use bevy::prelude::*;
use std::f32::consts::PI;

use crate::palette;

/// Real seconds per world day. Ten minutes: long enough that morning and
/// evening are moods rather than a strobe, short enough that a sitting sees
/// several days.
pub const DAY_SECONDS: f32 = 600.0;

/// Fraction of the day the sun is up.
const DAYLIGHT_FRACTION: f32 = 0.72;

pub struct CalendarPlugin;

impl Plugin for CalendarPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldClock>()
            .init_resource::<Sky>()
            .add_systems(Update, (tick, drive_sky, apply_sky_to_lights).chain());
    }
}

/// World time since founding, in seconds. Everything temporal derives from this.
#[derive(Resource, serde::Serialize, serde::Deserialize)]
pub struct WorldClock {
    pub elapsed: f64,
}

impl Default for WorldClock {
    /// Games open mid-morning: the first thing the player sees should be the
    /// world lit well, not a night they had no say in. `EGREGORE_CLOCK=0.8`
    /// starts elsewhere in the day, for looking at dusk without waiting for it.
    fn default() -> Self {
        let start = std::env::var("EGREGORE_CLOCK")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(0.22);
        WorldClock {
            elapsed: (DAY_SECONDS * start) as f64,
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

    /// Whether decent people are asleep.
    pub fn is_night(&self) -> bool {
        let t = self.time_of_day();
        t >= 0.74 || t < 0.03
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

    /// The date, as the HUD shows it.
    pub fn date_phrase(&self) -> String {
        format!("day {}, {}", self.day(), self.phase_name())
    }
}

fn tick(time: Res<Time>, mut clock: ResMut<WorldClock>) {
    clock.elapsed += time.delta_secs() as f64;
}

/// What the light is doing right now. Derived from the clock every frame;
/// consumed by the lights, the fog, the sky and the water.
#[derive(Resource)]
pub struct Sky {
    /// Unit vector toward the sun (held just above the horizon through the
    /// night so the transform it drives stays well-defined).
    pub sun_direction: Vec3,
    pub sun_color: Color,
    pub sun_illuminance: f32,
    /// The colour of fog, empty sky and the water's reflection.
    pub horizon: Color,
    pub ambient_color: Color,
    pub ambient_brightness: f32,
    pub fill_illuminance: f32,
    /// How much daylight there is, 0 at night to 1 at noon. For anything that
    /// needs to dim with the day — the water's body colour, for one.
    pub daylight: f32,
}

impl Default for Sky {
    fn default() -> Self {
        sky_at(1.0)
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

/// The whole sky, as a function of the sun's elevation.
fn sky_at(elevation: f32) -> Sky {
    // How much daylight there is, easing in shortly after sunrise.
    let lit = smoothstep(-0.04, 0.30, elevation);
    // How close to the horizon the sun is while up — the golden-hour weight.
    let low_sun = (1.0 - smoothstep(0.12, 0.45, elevation)) * smoothstep(-0.10, 0.05, elevation);

    // The sun keeps its azimuth and swings in elevation, clamped just above
    // the horizon so the directional light always has a valid bearing.
    let azimuth = Vec2::new(crate::SUN_DIRECTION.x, crate::SUN_DIRECTION.z).normalize();
    let y = elevation.clamp(0.06, 1.0);
    let flat = (1.0 - y * y).max(0.0).sqrt();
    let sun_direction = Vec3::new(azimuth.x * flat, y, azimuth.y * flat).normalize();

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
        fill_illuminance: 900.0 + 2_500.0 * lit,
        daylight: lit,
    }
}

fn drive_sky(
    clock: Res<WorldClock>,
    weather: Option<Res<crate::weather::Weather>>,
    mut sky: ResMut<Sky>,
) {
    *sky = sky_at(clock.sun_elevation());
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

/// Marks the key light.
#[derive(Component)]
pub struct SunLight;

/// Marks the cool counter-light.
#[derive(Component)]
pub struct FillLight;

fn apply_sky_to_lights(
    sky: Res<Sky>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut suns: Query<(&mut DirectionalLight, &mut Transform), (With<SunLight>, Without<FillLight>)>,
    mut fills: Query<&mut DirectionalLight, (With<FillLight>, Without<SunLight>)>,
) {
    ambient.color = sky.ambient_color;
    ambient.brightness = sky.ambient_brightness;

    for (mut light, mut transform) in &mut suns {
        light.color = sky.sun_color;
        light.illuminance = sky.sun_illuminance.max(1.0);
        *transform =
            Transform::from_translation(sky.sun_direction * 140.0).looking_at(Vec3::ZERO, Vec3::Y);
    }
    for mut light in &mut fills {
        light.illuminance = sky.fill_illuminance;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let midnight = sky_at(-1.0);
        assert!(midnight.ambient_brightness > 10.0, "night went pitch black");
        assert!(
            midnight.sun_illuminance < 100.0,
            "the sun shone at midnight"
        );

        let noon = sky_at(1.0);
        assert!(noon.sun_illuminance > 10_000.0);
        assert!(noon.ambient_brightness > midnight.ambient_brightness);
    }

    #[test]
    fn the_sun_direction_is_always_usable() {
        for elevation in [-1.0, -0.3, 0.0, 0.2, 0.7, 1.0] {
            let sky = sky_at(elevation);
            assert!((sky.sun_direction.length() - 1.0).abs() < 1e-4);
            assert!(sky.sun_direction.y > 0.0, "sun bearing dipped below ground");
        }
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
