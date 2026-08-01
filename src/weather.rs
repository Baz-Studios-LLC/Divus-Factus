//! Weather: fronts that arrive, linger, and pass.
//!
//! One intensity scalar drives everything - cloud cover, the dimming of the
//! sun, the rain, the wind, and whether the storm throws lightning. Fronts
//! shift every few minutes toward a rolled target, so weather has moods
//! rather than switches.
//!
//! The design's quiet centrepiece: a storm's lightning is *real* - it harms
//! what it hits and enters the witness pipeline as the same event a Smite
//! does. The villagers cannot tell heaven's wrath from the sky's, and the
//! legend they build from a bad storm is the player's to inherit. The god
//! the people believe in is the god you get to be.

use bevy::prelude::*;

use crate::creature::anim::CreatureMotion;
use crate::creature::{Corpse, Creature, Vitality};
use crate::rng::Rng;
use crate::terrain::Terrain;
use crate::villager::SettlementSite;
use crate::witness::{DivineEvent, DivineEventKind};

/// What the sky is doing, in words.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WeatherKind {
    Clear,
    Overcast,
    Rain,
    Storm,
}

impl WeatherKind {
    pub fn describe(self) -> &'static str {
        match self {
            WeatherKind::Clear => "clear skies",
            WeatherKind::Overcast => "grey overcast",
            WeatherKind::Rain => "steady rain",
            WeatherKind::Storm => "a storm overhead",
        }
    }
}

/// The sky's present mood, eased between fronts.
#[derive(Resource)]
pub struct Weather {
    /// 0 clear to 1 storm; everything else derives from this.
    pub intensity: f32,
    /// Where the current front is headed.
    pub target: f32,
    /// When the next front rolls in (world-clock seconds).
    pub next_front: f64,
    /// Wind strength, 0..1, following the intensity with its own lag.
    pub wind: f32,
    /// The season's cold, laid over whatever the sky is doing. Set each
    /// frame from the calendar; not saved - it re-derives from the date.
    pub chill: f32,
}

impl Default for Weather {
    fn default() -> Self {
        Weather {
            intensity: 0.15,
            target: 0.15,
            next_front: 0.0,
            wind: 0.2,
            chill: 0.0,
        }
    }
}

impl Weather {
    pub fn kind(&self) -> WeatherKind {
        match self.intensity {
            i if i < 0.3 => WeatherKind::Clear,
            i if i < 0.55 => WeatherKind::Overcast,
            i if i < 0.8 => WeatherKind::Rain,
            _ => WeatherKind::Storm,
        }
    }

    pub fn raining(&self) -> bool {
        self.intensity > 0.55
    }

    /// Air temperature 0 bitter to 1 balmy, from the hour and the sky:
    /// midday clear is warm, a stormy night is cold to the bone.
    pub fn temperature(&self, daylight: f32) -> f32 {
        (0.25 + daylight * 0.6 - self.intensity * 0.3 - self.chill).clamp(0.0, 1.0)
    }

    pub fn temperature_word(&self, daylight: f32) -> &'static str {
        match self.temperature(daylight) {
            t if t < 0.2 => "bitter cold",
            t if t < 0.4 => "cold",
            t if t < 0.65 => "mild",
            t if t < 0.85 => "warm",
            _ => "balmy",
        }
    }

    /// How much the sky slows outdoor hands, 1.0 fair to ~1.6 storm.
    pub fn toil(&self) -> f32 {
        1.0 + self.intensity * 0.6
    }
}

pub struct WeatherPlugin;

impl Plugin for WeatherPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Weather>().add_systems(
            Update,
            (progress_fronts, rain_falls, storm_strikes, fire_spreads).chain(),
        );
    }
}

/// Fronts shift every few minutes; the sky eases toward each new mood.
fn progress_fronts(
    time: Res<Time>,
    clock: Res<crate::calendar::WorldClock>,
    mut rng: Local<Option<Rng>>,
    mut weather: ResMut<Weather>,
) {
    let rng = rng.get_or_insert_with(|| Rng::new(0x5EA50));
    // The calendar's cold rides along even between fronts.
    weather.chill = clock.season().chill();
    if clock.elapsed >= weather.next_front {
        // Clear-ish weather is the ordinary day; storms are events. An
        // override for photographing and testing particular skies.
        weather.target = match std::env::var("DIVUS_FACTUS_WEATHER").as_deref() {
            Ok("clear") => 0.1,
            Ok("rain") => 0.7,
            Ok("storm") => 1.0,
            _ => {
                let roll = rng.f32();
                let base = if roll < 0.45 {
                    rng.range(0.05, 0.3)
                } else if roll < 0.75 {
                    rng.range(0.3, 0.55)
                } else if roll < 0.93 {
                    rng.range(0.55, 0.8)
                } else {
                    rng.range(0.8, 1.0)
                };
                // The season's thumb on the dice: winters run grey and
                // stormy, summers clear.
                (base + clock.season().gloom()).clamp(0.02, 1.0)
            }
        };
        weather.next_front = clock.elapsed + rng.range(120.0, 320.0) as f64;
    }
    let dt = time.delta_secs();
    let target = weather.target;
    weather.intensity += (target - weather.intensity).clamp(-dt / 45.0, dt / 45.0);
    let wind_target = 0.15 + weather.intensity * 0.85;
    weather.wind += (wind_target - weather.wind) * (1.0 - (-dt * 0.5).exp());
}

/// One falling streak of the rain pool.
#[derive(Component)]
struct RainDrop {
    seed: f32,
}

const RAIN_POOL: usize = 260;

/// Rain as a pool of falling streaks in a volume that follows the camera.
#[allow(clippy::type_complexity)]
fn rain_falls(
    mut commands: Commands,
    time: Res<Time>,
    weather: Res<Weather>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    cameras: Query<&crate::camera::CameraRig>,
    mut drops: Query<(&RainDrop, &mut Transform, &mut Visibility)>,
) {
    let Ok(rig) = cameras.single() else {
        return;
    };
    if drops.is_empty() {
        // Build the pool once, on first demand.
        if !weather.raining() {
            return;
        }
        let streak = meshes.add(Cuboid::new(0.045, 1.6, 0.045));
        let water = materials.add(StandardMaterial {
            base_color: Color::srgba(0.75, 0.82, 0.95, 0.32),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        });
        for i in 0..RAIN_POOL {
            commands.spawn((
                RainDrop {
                    seed: i as f32 * 0.61803,
                },
                Mesh3d(streak.clone()),
                MeshMaterial3d(water.clone()),
                Transform::from_xyz(0.0, -100.0, 0.0),
                Visibility::Hidden,
                bevy::light::NotShadowCaster,
            ));
        }
        return;
    }

    let centre = rig.focus;
    let t = time.elapsed_secs();
    let shown = (weather.intensity.max(0.0) - 0.5).max(0.0) * 2.0;
    let slant = weather.wind * 8.0;
    for (index, (drop, mut transform, mut visibility)) in drops.iter_mut().enumerate() {
        let live = (index as f32) < shown * RAIN_POOL as f32;
        let wanted = if live {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
        if !live {
            continue;
        }
        // Each streak cycles down its own column on its own phase; the
        // column follows the camera focus, so rain is wherever you look.
        let phase = (t * 0.65 + drop.seed).fract();
        let column_x = ((drop.seed * 137.7).fract() - 0.5) * 110.0;
        let column_z = ((drop.seed * 291.3).fract() - 0.5) * 110.0;
        let top = centre.y + 34.0;
        let fall = phase * 46.0;
        transform.translation = Vec3::new(
            centre.x + column_x + slant * phase,
            top - fall,
            centre.z + column_z,
        );
        transform.rotation = Quat::from_rotation_z(weather.wind * 0.17);
    }
}

/// The storm throws real lightning. Real harm, and the same witnessed event
/// a Smite emits - misattribution is the point.
#[allow(clippy::type_complexity)]
fn storm_strikes(
    mut commands: Commands,
    time: Res<Time>,
    weather: Res<Weather>,
    terrain: Option<Res<Terrain>>,
    site: Option<Res<SettlementSite>>,
    mut rng: Local<Option<Rng>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut notices: MessageWriter<crate::ui::Notice>,
    mut witnessed: MessageWriter<DivineEvent>,
    mut victims: Query<
        (Entity, &Transform, &mut Vitality, &mut CreatureMotion),
        (With<Creature>, Without<Corpse>),
    >,
    trees: Query<
        (
            Entity,
            &GlobalTransform,
            &crate::scatter::TreeBody,
            &crate::scatter::InGrove,
        ),
        With<crate::scatter::FellableTree>,
    >,
    terrain_assets: Res<crate::terrain::TerrainAssets>,
    mut dirty_groves: ResMut<crate::scatter::DirtyGroves>,
) {
    if weather.kind() != WeatherKind::Storm {
        return;
    }
    let (Some(terrain), Some(site)) = (terrain, site) else {
        return;
    };
    let rng = rng.get_or_insert_with(|| Rng::new(0xB017));
    // Roughly one strike per twenty storm-seconds, somewhere near town.
    if !rng.chance((time.delta_secs() * 0.05).min(1.0)) {
        return;
    }
    let angle = rng.range(0.0, std::f32::consts::TAU);
    let reach = rng.range(8.0, 130.0);
    let (sin, cos) = angle.sin_cos();
    let x = site.centre.x + cos * reach;
    let z = site.centre.z + sin * reach;
    let at = Vec3::new(x, terrain.height_at(x, z), z);

    crate::miracles::lightning_bolt(&mut commands, &mut meshes, &mut materials, at);

    // The first soul the bolt catches is the event's subject, exactly as a
    // Smite's would be: witnesses remember WHO the sky took, by name and by
    // kinship, and the storm becomes doctrine fodder like any miracle.
    let mut struck: Option<Entity> = None;
    for (entity, transform, mut vitality, mut motion) in &mut victims {
        if transform.translation.distance(at) > 4.5 {
            continue;
        }
        vitality.harm += 1.2;
        vitality.violent = true;
        motion.flail = 1.0;
        struck.get_or_insert(entity);
    }

    // Anything wooden at the strike point catches — and steps out of its
    // grove to burn where everyone can see it.
    for (tree, tree_at, body, home) in &trees {
        if tree_at.translation().distance(at) < 5.0 {
            crate::scatter::stand_alone(
                &mut commands,
                &mut meshes,
                terrain_assets.ground_material.clone(),
                tree,
                body,
                home,
                &mut dirty_groves,
            );
            commands.entity(tree).insert(Burning {
                remaining: rng.range(14.0, 22.0),
            });
        }
    }

    info!("lightning strikes from the storm");
    notices.write(crate::ui::Notice::new("Lightning strikes from the storm"));
    // The same event a Smite writes: witnesses cannot tell the difference,
    // and the faith they gain or lose lands on the god all the same.
    witnessed.write(DivineEvent {
        kind: DivineEventKind::Smote,
        position: at,
        subject: struck,
        intensity: 1.0,
    });
}

/// A tree on fire. It burns down to nothing, and while it burns it can pass
/// the flame to its neighbours - unless the rain gets there first.
#[derive(Component)]
pub struct Burning {
    pub remaining: f32,
}

/// The visible flame on a burning thing.
#[derive(Component)]
struct TreeFlame;

#[allow(clippy::type_complexity)]
fn fire_spreads(
    mut commands: Commands,
    time: Res<Time>,
    weather: Res<Weather>,
    mut rng: Local<Option<Rng>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut witnessed: MessageWriter<DivineEvent>,
    mut burning: Query<
        (Entity, &GlobalTransform, &mut Burning, Option<&Children>),
        With<crate::scatter::FellableTree>,
    >,
    flames: Query<(), With<TreeFlame>>,
    unburnt: Query<
        (
            Entity,
            &GlobalTransform,
            &crate::scatter::TreeBody,
            &crate::scatter::InGrove,
        ),
        (With<crate::scatter::FellableTree>, Without<Burning>),
    >,
    grove_kit: (
        Res<crate::terrain::TerrainAssets>,
        ResMut<crate::scatter::DirtyGroves>,
        ResMut<crate::scatter::StrippedGround>,
    ),
) {
    let (terrain_assets, mut dirty_groves, mut stripped) = grove_kit;
    let rng = rng.get_or_insert_with(|| Rng::new(0xF12E));
    let dt = time.delta_secs();
    // Rain fights the fire; a downpour wins quickly.
    let quench = if weather.raining() {
        1.0 + weather.intensity * 3.0
    } else {
        1.0
    };
    for (tree, at, mut fire, children) in &mut burning {
        // The flame shows itself once.
        let lit = children
            .map(|c| c.iter().any(|child| flames.get(child).is_ok()))
            .unwrap_or(false);
        if !lit {
            commands.spawn((
                TreeFlame,
                Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: crate::palette::shade(&crate::palette::CLOTH_GOLD, 0.95),
                    emissive: LinearRgba::from(crate::palette::shade(
                        &crate::palette::CLOTH_RED,
                        0.8,
                    )) * 10.0,
                    ..default()
                })),
                Transform::from_xyz(0.0, 2.2, 0.0).with_scale(Vec3::new(1.3, 2.6, 1.3)),
                bevy::light::NotShadowCaster,
                ChildOf(tree),
            ));
            witnessed.write(DivineEvent {
                kind: DivineEventKind::Smote,
                position: at.translation(),
                subject: None,
                intensity: 0.6,
            });
        }
        fire.remaining -= dt * quench;
        if fire.remaining <= 0.0 {
            // Burned through: the tree is gone for good, and the ground
            // remembers - a burn scar does not regrow on a chunk rebuild.
            stripped.strip(at.translation().x, at.translation().z);
            commands.entity(tree).despawn();
            continue;
        }
        // Sparks leap while it rages - farther and more often in wind.
        if rng.chance((dt * (0.06 + weather.wind * 0.1)).min(1.0)) {
            let reach = 4.0 + weather.wind * 5.0;
            if let Some((next, _, body, home)) = unburnt
                .iter()
                .map(|(e, t, b, h)| (e, t.translation().distance(at.translation()), b, h))
                .filter(|(_, d, ..)| *d < reach)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(e, d, b, h)| (e, d, b, h))
            {
                crate::scatter::stand_alone(
                    &mut commands,
                    &mut meshes,
                    terrain_assets.ground_material.clone(),
                    next,
                    body,
                    home,
                    &mut dirty_groves,
                );
                commands.entity(next).insert(Burning {
                    remaining: rng.range(14.0, 22.0),
                });
            }
        }
    }
}
