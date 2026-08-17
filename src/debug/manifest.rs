//! The manifestation: what stands on the dais in the god's room.
//!
//! The god has no form in the eyes of mortals, so the room does not hold a
//! figure. It holds what belief looks like from the inside: gems of
//! translucent light, rings of bars tumbling around them, shards on wild
//! orbits, and smoke climbing from the dais and swirling about the heart -
//! all built from the same cuboids as the world, because the god is made
//! of the same stuff as the things that believe in it.
//!
//! No two worlds raise the same god. At every founding the manifestation
//! rolls its aspect from the world's seed - its own pair of vivid tints to
//! shimmer between, its own count of hearts, its own tally and cant and
//! precession of rings, its own flock of shards - the way a village rolls
//! its sigil. And nothing here is tame: the whole apparition surges, whole
//! seconds of sudden speed and trembling that gather and pass like weather,
//! because worship is not a steady thing. It leans with the legend - a god
//! of gifts burns toward white; a god of storms cools to ember, quickens,
//! and its smoke coils low instead of rising - and it swells or thins with
//! presence, because a god scarcely believed in is scarcely there. The
//! leanings are eased, never snapped, so the room changes the way a
//! reputation does: too slowly to watch, plainly enough to notice.

use bevy::camera::visibility::RenderLayers;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use std::f32::consts::TAU;

use super::god::{DEITY_LAYER, DEITY_STAGE, DeityCamera, GodPanel};

/// Where the apparition hangs: over the dais, at the height the camera
/// regards.
const HEART: Vec3 = Vec3::new(0.0, 1.55, 0.15);

/// The smoke pool. Like the rain, a fixed flock cycling on their own
/// phases - nothing is spawned or despawned while the room is watched.
/// Two in three climb from the dais; the third swirls about the heart.
const MOTES: usize = 96;

/// The most loose stones any roll may set in orbit around the heart.
const MOST_SHARDS: usize = 13;

/// The roll of aspects: the lights a world's god may wear, two tints to
/// each so the color is never still - the apparition shimmers from the
/// first to the second and back. Bright and strange on purpose: the gods
/// are not jewelry. Named for the chronicle's tongue, should the
/// stories ever speak of the light itself.
const ASPECTS: &[(&str, Vec3, Vec3)] = &[
    (
        "teal fire",
        Vec3::new(0.10, 0.95, 0.90),
        Vec3::new(0.45, 1.0, 0.72),
    ),
    (
        "rose lightning",
        Vec3::new(1.0, 0.30, 0.65),
        Vec3::new(1.0, 0.62, 0.85),
    ),
    (
        "violet storm",
        Vec3::new(0.65, 0.35, 1.0),
        Vec3::new(0.92, 0.55, 1.0),
    ),
    (
        "aurora",
        Vec3::new(0.20, 1.0, 0.80),
        Vec3::new(1.0, 0.45, 0.85),
    ),
    (
        "cyan dawn",
        Vec3::new(0.25, 0.80, 1.0),
        Vec3::new(0.72, 0.96, 1.0),
    ),
    (
        "magenta glass",
        Vec3::new(1.0, 0.35, 0.95),
        Vec3::new(0.62, 0.50, 1.0),
    ),
    (
        "emerald",
        Vec3::new(0.20, 1.0, 0.55),
        Vec3::new(0.78, 1.0, 0.50),
    ),
    (
        "electrum",
        Vec3::new(0.95, 0.90, 0.30),
        Vec3::new(0.55, 1.0, 0.65),
    ),
    (
        "old gold",
        Vec3::new(1.0, 0.78, 0.42),
        Vec3::new(1.0, 0.62, 0.26),
    ),
    (
        "pale silver",
        Vec3::new(0.86, 0.92, 1.0),
        Vec3::new(0.68, 0.80, 1.0),
    ),
    (
        "first light",
        Vec3::new(1.0, 0.70, 0.55),
        Vec3::new(1.0, 0.92, 0.50),
    ),
    (
        "white flame",
        Vec3::new(1.0, 0.95, 0.84),
        Vec3::new(1.0, 0.80, 0.45),
    ),
];

/// The manifestation's disposition, eased toward what the world currently
/// says the god is.
#[derive(Resource, Default)]
pub(crate) struct Manifestation {
    /// -1 terrible .. 0 undecided .. +1 benevolent, from the legend.
    lean: f32,
    /// 0 scarcely believed .. 1 fully present, from the flock's trust.
    grandeur: f32,
    /// The apparition's own clock: time as the god feels it, advanced by
    /// the tempo of its temper and the surging of its moods. Every
    /// periodic motion reads this rather than multiplying wall time by a
    /// drifting rate - a change of temper or a burst of speed must change
    /// the pace of the turning, never its place. (Multiplying absolute
    /// time by an eased tempo made every past second jump too: hours in,
    /// one smite would have set the whole room strobing.)
    clock: f32,
    /// When set, the next ease is a snap: a new or freshly-loaded world's
    /// god arrives already being what it is, never caught mid-change.
    snap: bool,
}

impl Manifestation {
    /// Called when a world is founded anew or restored from a save: the
    /// next reading snaps to the truth instead of easing from the old
    /// world's ghost.
    pub(crate) fn arrive(&mut self) {
        self.snap = true;
    }
}

/// The aspect this world's god rolled at its founding.
#[derive(Resource)]
pub(crate) struct ManifestAspect {
    /// For the chronicle, one day.
    #[allow(dead_code)]
    name: &'static str,
    /// The two tints the apparition shimmers between.
    primary: Vec3,
    secondary: Vec3,
    /// How fast the shimmer crosses, radians per god-second.
    shimmer: f32,
}

/// Every entity of the apparition wears this, so a new world's roll can
/// clear the old god from the room in one sweep.
#[derive(Component)]
pub(crate) struct ManifestPiece;

/// One gem of the heart. A god may burn as a single stone, as twins
/// circling one another, or as a triad.
#[derive(Component)]
pub(crate) struct ManifestHeart {
    seed: f32,
    /// How far this gem stands from the center - zero for a lone heart.
    spread: f32,
    /// The gem's own size, split among the gems of the roll.
    girth: f32,
}

/// The brighter core inside the heart.
#[derive(Component)]
pub(crate) struct ManifestCore;

/// A tumbling ring's pivot; the bars are its children. Tilt leans the
/// ring, rate wheels it, and precession swings the lean itself around -
/// so the rings never trace the same path twice.
#[derive(Component)]
pub(crate) struct ManifestRing {
    tilt: f32,
    rate: f32,
    precess: f32,
}

/// One orbiting shard.
#[derive(Component)]
pub(crate) struct ManifestShard {
    seed: f32,
}

/// One mote of the smoke: a climber rising from the dais, or a swirler
/// circling the heart.
#[derive(Component)]
pub(crate) struct ManifestMote {
    seed: f32,
    swirler: bool,
}

/// The light the apparition sheds.
#[derive(Component)]
pub(crate) struct ManifestLight;

/// The paints the animation retunes as the legend leans.
#[derive(Resource)]
pub(crate) struct ManifestPaint {
    veil: Handle<StandardMaterial>,
    core: Handle<StandardMaterial>,
    smoke: Handle<StandardMaterial>,
}

/// The manifestation's color at a given lean: the world's own tint when
/// undecided, burning toward white grace one way and ember dread the
/// other.
fn warmth(lean: f32, tint: Vec3) -> Vec3 {
    const EMBER: Vec3 = Vec3::new(0.95, 0.22, 0.10);
    const GRACE: Vec3 = Vec3::new(1.0, 0.97, 0.9);
    if lean < 0.0 {
        tint.lerp(EMBER, -lean)
    } else {
        tint.lerp(GRACE, lean * 0.8)
    }
}

/// A small deterministic stutter for trembling and flicker: steps at
/// 13Hz, different every step, the same every run.
fn flicker(t: f32) -> f32 {
    let step = (t * 13.0).floor();
    (step * 12.9898).sin() * 43_758.547
}

/// The surging of the god's moods: mostly near one, gathering now and
/// then into a burst of several seconds' sudden speed. Three sines that
/// share no rhythm, multiplied and sharpened, so the bursts arrive like
/// weather - unforeseen but never random. Fed by wall time, so the
/// weather of it is steady however the temper leans.
fn surge(t: f32) -> f32 {
    let s = (t * 0.31).sin() * (t * 0.77 + 1.3).sin() * (t * 0.13 + 4.1).sin();
    s.max(0.0).powi(3)
}

/// Which way a shard orbits: alternating by the shard's index, recovered
/// from its golden-ratio seed. (Parity straight off the scaled seed left
/// every shard turning the same way - the stride never crossed an odd
/// integer until far past six.)
fn shard_sign(seed: f32) -> f32 {
    if (seed / 0.618_034).round() as u32 % 2 == 0 {
        1.0
    } else {
        -1.0
    }
}

/// A shard's orbit, flattened toward the camera: wide enough to swing
/// outside the rings, shallow enough that the far side of every orbit
/// clears the niche wall at z -1.0 with a shard's width to spare.
fn shard_orbit(seed: f32) -> (f32, f32) {
    (1.15 + (seed * 7.3).fract() * 0.35, 0.62)
}

/// The widest any climber's coil may blow: at full dread the smoke hugs
/// the dais wide, but its farthest swing must still clear the niche wall
/// at z -1.0 with better than a mote's width to spare.
const CLIMBER_LEASH: f32 = 1.2;

/// Raises the apparition - and raises it anew whenever the world's seed
/// changes, because a new world is a new god: the aspect, the hearts, the
/// rings and the shard flock are all rolled from the seed, the way a
/// village rolls its banner. The rolls are broad on purpose; two worlds'
/// gods should hardly look like kin.
pub(crate) fn rebuild_manifestation(
    mut commands: Commands,
    seed: Res<crate::WorldSeed>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    pieces: Query<Entity, With<ManifestPiece>>,
    mut built: Local<Option<u32>>,
) {
    if *built == Some(seed.0) {
        return;
    }
    *built = Some(seed.0);
    for piece in &pieces {
        commands.entity(piece).despawn();
    }

    let mut rng = crate::rng::Rng::stream(seed.0 as u64, "manifestation");
    let layer = RenderLayers::layer(DEITY_LAYER);
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));

    // The roll: which light this god wears, and how its court is set.
    let (name, primary, secondary) = *rng.pick(ASPECTS);
    let aspect = ManifestAspect {
        name,
        primary,
        secondary,
        shimmer: rng.range(0.09, 0.18),
    };
    let tint = aspect.primary;
    commands.insert_resource(aspect);

    // The paints. Emissive carries the divinity; the animation retunes all
    // three every frame the room is watched, so the tints here are only
    // the first frame's guess.
    let veil = materials.add(StandardMaterial {
        base_color: Color::srgba(tint.x, tint.y, tint.z, 0.32),
        emissive: LinearRgba::rgb(tint.x * 1.9, tint.y * 1.9, tint.z * 1.9),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.35,
        ..default()
    });
    let core = materials.add(StandardMaterial {
        base_color: Color::srgba(tint.x, tint.y, tint.z, 0.85),
        emissive: LinearRgba::rgb(tint.x * 4.2, tint.y * 4.2, tint.z * 4.2),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.2,
        ..default()
    });
    let smoke = materials.add(StandardMaterial {
        base_color: Color::srgba(tint.x, tint.y, tint.z, 0.18),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    commands.insert_resource(ManifestPaint {
        veil: veil.clone(),
        core: core.clone(),
        smoke: smoke.clone(),
    });

    // The heart: one, two or three gems, splitting the same fire between
    // them; the brighter core burns at the center of whatever they make.
    let gems = if rng.chance(0.5) {
        1
    } else {
        rng.range_i(2, 3) as usize
    };
    let spread = if gems > 1 { rng.range(0.24, 0.36) } else { 0.0 };
    let girth = 0.66 / (gems as f32).powf(0.35);
    for gem in 0..gems {
        commands.spawn((
            Name::new("Manifest Heart"),
            ManifestPiece,
            ManifestHeart {
                seed: gem as f32 / gems as f32,
                spread,
                girth,
            },
            Mesh3d(cube.clone()),
            MeshMaterial3d(veil.clone()),
            Transform::from_translation(DEITY_STAGE + HEART),
            NotShadowCaster,
            layer.clone(),
        ));
    }
    commands.spawn((
        Name::new("Manifest Core"),
        ManifestPiece,
        ManifestCore,
        Mesh3d(cube.clone()),
        MeshMaterial3d(core.clone()),
        Transform::from_translation(DEITY_STAGE + HEART),
        NotShadowCaster,
        layer.clone(),
    ));

    // The rings: one to four, each on a rolled tilt, rate and precession,
    // wheeling against their neighbors. Bars on a pivot, laid along the
    // circle's tangent so they join hands into a segmented ring.
    let ring_count = rng.range_i(1, 4) as usize;
    for ring in 0..ring_count {
        let against = if ring % 2 == 0 { 1.0 } else { -1.0 };
        let tilt = rng.range(0.25, 1.2) * against;
        let rate = rng.range(0.5, 1.1) * against;
        let precess = rng.range(0.06, 0.28) * -against;
        let radius = 0.72 + ring as f32 * 0.14 + rng.range(0.0, 0.16);
        let bars = rng.range_i(9, 16);
        let pivot = commands
            .spawn((
                Name::new("Manifest Ring"),
                ManifestPiece,
                ManifestRing {
                    tilt,
                    rate,
                    precess,
                },
                Transform::from_translation(DEITY_STAGE + HEART),
                Visibility::default(),
            ))
            .id();
        for bar in 0..bars {
            let angle = bar as f32 / bars as f32 * TAU;
            // Laid along the circle's tangent, sized so each ring's bars
            // cover the same share of its circumference.
            commands.spawn((
                Name::new("Manifest Ring Bar"),
                Mesh3d(cube.clone()),
                MeshMaterial3d(veil.clone()),
                Transform::from_translation(Vec3::new(
                    angle.cos() * radius,
                    0.0,
                    angle.sin() * radius,
                ))
                .with_rotation(Quat::from_rotation_y(
                    -(angle + std::f32::consts::FRAC_PI_2),
                ))
                .with_scale(Vec3::new(
                    radius * TAU / bars as f32 * 0.66,
                    0.045,
                    0.045,
                )),
                NotShadowCaster,
                layer.clone(),
                ChildOf(pivot),
            ));
        }
    }

    // The shards: loose stones the heart holds without touching, their
    // number rolled with the world.
    let shard_count = rng.range_i(6, MOST_SHARDS as i32) as usize;
    for shard in 0..shard_count {
        commands.spawn((
            Name::new("Manifest Shard"),
            ManifestPiece,
            ManifestShard {
                seed: shard as f32 * 0.618_034,
            },
            Mesh3d(cube.clone()),
            MeshMaterial3d(veil.clone()),
            Transform::from_translation(DEITY_STAGE + HEART),
            NotShadowCaster,
            layer.clone(),
        ));
    }

    // The smoke: a fixed flock of motes, cycling forever on their own
    // phases - most climbing from the dais, every third swirling about
    // the heart itself. Hidden until the room decides how many the god's
    // presence can hold aloft.
    for mote in 0..MOTES {
        commands.spawn((
            Name::new("Manifest Mote"),
            ManifestPiece,
            ManifestMote {
                seed: mote as f32 * 0.618_034,
                swirler: mote % 3 == 2,
            },
            Mesh3d(cube.clone()),
            MeshMaterial3d(smoke.clone()),
            Transform::from_translation(DEITY_STAGE + HEART).with_scale(Vec3::ZERO),
            Visibility::Hidden,
            NotShadowCaster,
            layer.clone(),
        ));
    }

    // The glow the apparition throws on the stone.
    commands.spawn((
        Name::new("Manifest Light"),
        ManifestPiece,
        ManifestLight,
        PointLight {
            color: Color::srgb(tint.x, tint.y, tint.z),
            intensity: 110_000.0,
            range: 10.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_translation(DEITY_STAGE + HEART),
        layer,
    ));
}

/// Eases the manifestation toward what the world currently says: the lean
/// from the legend's two stories, the grandeur from the flock's trust.
/// Runs whether or not the room is watched, so the codex always opens on
/// a god already become what it has become - never one caught changing.
pub(crate) fn ease_manifestation(
    mut manifest: ResMut<Manifestation>,
    time: Res<Time>,
    legend: Option<Res<crate::villager::belief::Legend>>,
    flock: Query<
        Option<&crate::villager::belief::Faith>,
        (
            With<crate::villager::Villager>,
            Without<crate::creature::Corpse>,
        ),
    >,
    mut forced: Local<Option<(Option<f32>, Option<f32>)>>,
) {
    // DIVUS_FACTUS_LEAN / DIVUS_FACTUS_GRANDEUR pin the apparition for photography.
    let (forced_lean, forced_grandeur) = *forced.get_or_insert_with(|| {
        let read = |name: &str| std::env::var(name).ok().and_then(|v| v.parse::<f32>().ok());
        (read("DIVUS_FACTUS_LEAN"), read("DIVUS_FACTUS_GRANDEUR"))
    });

    let lean_target = forced_lean.unwrap_or_else(|| {
        legend.as_ref().map_or(0.0, |l| {
            let told = l.providence + l.dread;
            // A legend barely told yet holds no opinion of its god.
            let conviction = (told / 6.0).min(1.0);
            if told <= f32::EPSILON {
                0.0
            } else {
                (l.providence - l.dread) / told * conviction
            }
        })
    });
    let grandeur_target = forced_grandeur.unwrap_or_else(|| {
        let mut total = 0usize;
        let mut trust = 0.0f32;
        for faith in &flock {
            total += 1;
            trust += faith.map_or(0.0, |f| f.trust);
        }
        trust / total.max(1) as f32
    });

    // A reputation's pace: most of the way there in ~15 seconds. A pinned
    // reading, or a world just founded or restored, is taken whole - the
    // photograph shows the asked-for extreme, and the new world's god
    // arrives already itself.
    let snap = manifest.snap;
    manifest.snap = false;
    let ease = 1.0 - (-time.delta_secs() / 6.0).exp();
    let lean_ease = if snap || forced_lean.is_some() {
        1.0
    } else {
        ease
    };
    let grandeur_ease = if snap || forced_grandeur.is_some() {
        1.0
    } else {
        ease
    };
    manifest.lean += (lean_target.clamp(-1.0, 1.0) - manifest.lean) * lean_ease;
    manifest.grandeur += (grandeur_target.clamp(0.0, 1.0) - manifest.grandeur) * grandeur_ease;
}

/// The room's camera renders only while the deity page is open; a portrait
/// nobody is looking at costs nothing.
pub(crate) fn gate_deity_camera(
    codex: Res<super::village::Codex>,
    panels: Query<&Visibility, With<GodPanel>>,
    mut cameras: Query<&mut Camera, With<DeityCamera>>,
) {
    let watched = codex.page == super::village::CodexPage::Deity
        && panels.iter().any(|v| *v != Visibility::Hidden);
    for mut camera in &mut cameras {
        if camera.is_active != watched {
            camera.is_active = watched;
        }
    }
}

/// Moves the apparition, while the room is watched: the hearts turn, the
/// rings tumble and precess, the shards keep their inclined orbits, the
/// smoke climbs and swirls, the light breathes - and all of it surges,
/// bursts of speed and trembling that gather and pass like weather, all
/// of it leaning with the legend and shimmering between the aspect's two
/// tints.
#[allow(clippy::type_complexity)]
pub(crate) fn animate_manifestation(
    codex: Res<super::village::Codex>,
    panels: Query<&Visibility, (With<GodPanel>, Without<ManifestMote>)>,
    mut manifest: ResMut<Manifestation>,
    aspect: Option<Res<ManifestAspect>>,
    paint: Option<Res<ManifestPaint>>,
    time: Res<Time>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut lights: Query<&mut PointLight, With<ManifestLight>>,
    mut pieces: ParamSet<(
        Query<(&ManifestHeart, &mut Transform)>,
        Query<&mut Transform, With<ManifestCore>>,
        Query<(&ManifestRing, &mut Transform)>,
        Query<(&ManifestShard, &mut Transform)>,
        Query<(&ManifestMote, &mut Transform, &mut Visibility)>,
    )>,
) {
    if codex.page != super::village::CodexPage::Deity
        || !panels.iter().any(|v| *v != Visibility::Hidden)
    {
        return;
    }
    let (Some(aspect), Some(paint)) = (aspect, paint) else {
        return;
    };
    let t = time.elapsed_secs();
    let lean = manifest.lean;
    let grandeur = manifest.grandeur;
    let grace = lean.max(0.0);
    let dread = (-lean).max(0.0);

    // The tempers, and the weather of the moods: tempo is the standing
    // pace of the temper; the surge throws bursts of speed on top, harder
    // when the god is terrible. Both advance the god's own clock rather
    // than scaling wall time, so a change of pace quickens the turning
    // without ever moving it.
    let tempo = 1.0 + dread * 1.1 - grace * 0.3;
    let burst = surge(t);
    let rush = tempo * (1.0 + burst * (3.2 + dread * 2.3));
    manifest.clock += time.delta_secs() * rush;
    let clock = manifest.clock;
    let stature = 0.55 + 0.45 * grandeur;
    let breath =
        1.0 + (clock * 0.9).sin() * 0.16 + (dread * 0.22 + burst * 0.1) * flicker(t).fract();

    // The burst also shakes the standing pieces: a fine tremor, deeper in
    // dread, gone in the calms.
    let quake = burst * 0.028 + dread * 0.012;
    let tremor = |k: f32| flicker(t * 1.7 + k).fract() * quake;

    // The world's own light, never still: the aspect shimmers between its
    // two tints on its rolled pace, and the legend's lean burns the result
    // toward grace or dread.
    let shimmer = (clock * aspect.shimmer).sin() * 0.5 + 0.5;
    let tint = aspect.primary.lerp(aspect.secondary, shimmer);
    let warm = warmth(lean, tint);

    // The hearts: each gem stood on its corner, turning, breathing,
    // circling the common center when the roll split the fire.
    let corner_up = Quat::from_rotation_x(std::f32::consts::FRAC_PI_4)
        * Quat::from_rotation_z(std::f32::consts::FRAC_PI_4);
    let bob = (t * 0.7).sin() * 0.05;
    for (gem, mut transform) in &mut pieces.p0() {
        let waltz = clock * 0.55 + gem.seed * TAU;
        let offset = Vec3::new(
            waltz.cos() * gem.spread,
            (waltz * 0.7).sin() * gem.spread * 0.3,
            waltz.sin() * gem.spread * 0.6,
        );
        transform.translation = DEITY_STAGE
            + HEART
            + Vec3::Y * bob
            + offset
            + Vec3::new(tremor(gem.seed * 5.0), tremor(gem.seed * 5.0 + 3.0), 0.0);
        transform.rotation = Quat::from_rotation_y(clock * 0.75 + gem.seed * TAU) * corner_up;
        transform.scale = Vec3::splat(gem.girth * stature * (1.0 + (breath - 1.0) * 0.25));
    }
    for mut transform in &mut pieces.p1() {
        transform.translation = DEITY_STAGE + HEART + Vec3::Y * bob;
        transform.rotation = Quat::from_rotation_y(-clock * 1.1) * corner_up;
        transform.scale = Vec3::splat(0.3 * stature);
    }

    // The rings: each tumbling on its own tilt and rate while the tilt
    // itself precesses around, so no ring retraces its path; a bounded
    // sway keeps even one ring from ever looking mechanical, and the
    // terrible god knocks them further off true.
    for (ring, mut transform) in &mut pieces.p2() {
        let sway = (clock * 0.19 + ring.tilt * 7.0).sin() * 0.35;
        let wobble = dread * 0.2 * (t * 2.3 + ring.rate * 40.0).sin();
        transform.translation = DEITY_STAGE
            + HEART
            + Vec3::Y * bob * 0.6
            + Vec3::new(
                tremor(ring.rate * 11.0),
                tremor(ring.rate * 11.0 + 5.0),
                0.0,
            );
        transform.rotation = Quat::from_rotation_y(clock * ring.precess)
            * Quat::from_rotation_z(ring.tilt + wobble)
            * Quat::from_rotation_y(clock * ring.rate + sway);
        transform.scale = Vec3::splat(0.6 + 0.4 * stature);
    }

    // The shards: each on its own inclined orbit and its own tumble, half
    // of them against the stream, each surging on its own private beat.
    for (shard, mut transform) in &mut pieces.p3() {
        let seed = shard.seed;
        let (radius, squash) = shard_orbit(seed);
        let height = -0.25 + (seed * 3.7).fract() * 1.3;
        let incline = (seed * 2.9).fract() * 0.45;
        let sign = shard_sign(seed);
        let hurry = (clock * 0.23 + seed * 9.0).sin() * 1.2;
        let angle = seed * TAU + clock * (0.35 + (seed * 5.1).fract() * 0.4) * sign + hurry;
        transform.translation = DEITY_STAGE
            + HEART
            + Vec3::new(
                angle.cos() * radius,
                height
                    + (angle * 2.0 + seed * 7.0).sin() * incline * 0.4
                    + (t * 0.9 + seed * TAU).sin() * 0.06,
                angle.sin() * radius * squash,
            );
        transform.rotation = Quat::from_euler(
            EulerRot::XYZ,
            clock * 0.5 * sign + seed,
            clock * 0.35 + seed * 2.0,
            0.0,
        );
        transform.scale =
            Vec3::splat((0.055 + (seed * 9.7).fract() * 0.055) * (0.6 + 0.4 * stature));
    }

    // The smoke: how many motes the god can hold aloft is presence; where
    // they go is temperament. Climbers rise from the dais - high and thin
    // for a gracious god, coiling low and wide for a terrible one - while
    // the swirlers weave a quick band around the heart itself.
    let aloft = (24.0 + 72.0 * grandeur) as usize;
    let ceiling = 1.6 + grace * 1.1 - dread * 0.9;
    for (index, (mote, mut transform, mut visibility)) in pieces.p4().iter_mut().enumerate() {
        let live = index < aloft;
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
        let seed = mote.seed;
        let spin = if (seed * 100.0) as u32 % 2 == 0 {
            1.0
        } else {
            -1.0
        };
        let shiver = (dread * 0.035 + burst * 0.02) * flicker(t + seed * 9.0).fract();
        if mote.swirler {
            // A band about the heart: each swirler keeps its own lane and
            // weaves quickly up and down through it, breathing in size.
            let angle = seed * TAU + clock * (0.5 + (seed * 4.7).fract() * 0.4) * spin;
            let radius = 0.72
                + (seed * 6.1).fract() * 0.5
                + dread * 0.35
                + (clock * 0.6 + seed * TAU).sin() * 0.08;
            let weave = (clock * 0.45 + seed * TAU).sin() * (0.42 + grace * 0.25) - dread * 0.5;
            transform.translation = DEITY_STAGE
                + HEART
                + Vec3::new(
                    angle.cos() * radius + shiver,
                    weave,
                    angle.sin() * radius * 0.62,
                );
            transform.rotation = Quat::from_rotation_y(angle);
            transform.scale = Vec3::splat(
                (0.05 + (seed * 8.9).fract() * 0.05)
                    * (0.75 + 0.25 * (clock * 0.9 + seed * TAU).sin())
                    * (0.6 + 0.4 * stature),
            );
        } else {
            // A climber: born at the dais, spiralling up and thinning as
            // it goes, snuffed at its ceiling to be born again below.
            let phase = (clock * (0.055 + (seed * 3.3).fract() * 0.05) + seed).fract();
            let climb = phase.powf(0.85);
            let swirl = seed * TAU + clock * 0.85 * spin;
            // Capped at the smoke's leash: the widest dread-blown coil must
            // still clear the niche wall with a mote's width to spare.
            let radius = ((0.55 + (seed * 6.1).fract() * 0.3) * (1.0 - 0.5 * climb)
                + dread * 0.55 * (1.0 - climb))
                .min(CLIMBER_LEASH);
            transform.translation = DEITY_STAGE
                + Vec3::new(
                    swirl.cos() * radius + shiver,
                    0.34 + climb * ceiling,
                    0.15 + swirl.sin() * radius * 0.8,
                );
            transform.rotation = Quat::from_rotation_y(swirl);
            transform.scale =
                Vec3::splat((0.04 + (seed * 8.9).fract() * 0.06) * (1.0 - phase) + 0.012);
        }
    }

    // The light: the apparition's color, breathing with it, bright as the
    // god is present, flaring with the bursts.
    for mut light in &mut lights {
        light.color = Color::srgb(warm.x, warm.y, warm.z);
        light.intensity = (80_000.0 + 380_000.0 * grandeur) * breath * (1.0 + burst * 0.5);
    }

    // The paints, retuned to the lean: the veil and core take the warmth in
    // their glow, the smoke in its tint.
    if let Some(mut material) = materials.get_mut(&paint.veil) {
        material.base_color = Color::srgba(warm.x, warm.y, warm.z, 0.30 + 0.1 * grandeur);
        material.emissive =
            LinearRgba::rgb(warm.x * 1.9, warm.y * 1.9, warm.z * 1.9) * (0.7 + 0.6 * breath);
    }
    if let Some(mut material) = materials.get_mut(&paint.core) {
        material.base_color = Color::srgba(
            (warm.x * 1.15).min(1.0),
            (warm.y * 1.15).min(1.0),
            (warm.z * 1.15).min(1.0),
            0.85,
        );
        material.emissive =
            LinearRgba::rgb(warm.x * 4.2, warm.y * 4.2, warm.z * 4.2) * (0.7 + 0.5 * breath);
    }
    if let Some(mut material) = materials.get_mut(&paint.smoke) {
        material.base_color = Color::srgba(warm.x, warm.y, warm.z, 0.15 + 0.06 * grandeur);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shards_orbit_both_ways() {
        let signs: Vec<f32> = (0..MOST_SHARDS)
            .map(|i| shard_sign(i as f32 * 0.618_034))
            .collect();
        assert!(
            signs.contains(&1.0) && signs.contains(&-1.0),
            "half the shards should swim against the stream"
        );
    }

    #[test]
    fn the_shards_never_enter_the_wall() {
        // The niche wall's face stands at z -1.0 (see god.rs); the deepest
        // point of every orbit must clear it by more than a tumbling
        // shard's half-width.
        for i in 0..MOST_SHARDS {
            let (radius, squash) = shard_orbit(i as f32 * 0.618_034);
            let deepest = HEART.z - radius * squash;
            assert!(
                deepest > -1.0 + 0.1,
                "shard {i} orbits to z {deepest}, into the wall"
            );
        }
    }

    #[test]
    fn the_smoke_never_enters_the_wall() {
        // The climbers' dais-relative z is 0.15 - radius * 0.8 at the back
        // of a coil; even the widest dread-blown mote, at its largest
        // birth size, must clear the niche wall face at z -1.0.
        let deepest = 0.15 - CLIMBER_LEASH * 0.8;
        assert!(
            deepest - 0.09 > -1.0 + 0.01,
            "the leash lets the smoke into the wall: z {deepest}"
        );
    }

    #[test]
    fn warmth_holds_no_seam_at_neutral() {
        for (_, primary, secondary) in ASPECTS {
            for tint in [*primary, *secondary] {
                let below = warmth(-1e-4, tint);
                let above = warmth(1e-4, tint);
                assert!(
                    (below - above).length() < 1e-3,
                    "the color must cross neutral without a jump"
                );
            }
        }
    }

    #[test]
    fn every_aspect_stays_within_the_light() {
        for (name, primary, secondary) in ASPECTS {
            for tint in [*primary, *secondary] {
                assert!(
                    tint.min_element() >= 0.0 && tint.max_element() <= 1.0,
                    "{name} strays outside srgb"
                );
            }
        }
    }

    #[test]
    fn the_surge_is_bounded_and_mostly_calm() {
        let mut peak = 0.0f32;
        let mut restful = 0usize;
        const STEPS: usize = 60_000;
        for i in 0..STEPS {
            let s = surge(i as f32 * 0.1);
            peak = peak.max(s);
            if s < 0.05 {
                restful += 1;
            }
        }
        assert!(peak <= 1.0, "the surge must never exceed its bound");
        assert!(
            restful > STEPS / 2,
            "the room should rest between bursts, not boil"
        );
    }
}
