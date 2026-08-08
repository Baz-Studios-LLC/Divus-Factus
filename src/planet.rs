//! The planet, with nothing living on it.
//!
//! `--planet` boots the world and stops there: ground, sea, sky and the
//! quadtree, and not one villager, building or economy. The point is to get
//! the SPHERE right before anything is asked to stand on it, because every
//! coordinate bug this game has had - a fog that would not lift, a village
//! that would not build - came from the seam between a flat simulation and a
//! round picture, and that seam is invisible while a village is busy in front
//! of it.
//!
//! It is built from the game's own plugins, not a copy of them. Whatever gets
//! fixed here is fixed in the game, and whatever cannot be booted without the
//! villagers is a coupling worth knowing about.
//!
//! What it is for:
//!
//! - **Going to the places the flat scaffold gets wrong.** The poles, where
//!   east-west arc length collapses; the antipode, half a world from the
//!   origin, where a flat position and its seat have nothing to do with each
//!   other. The keys jump straight there. In the game those are a two hour
//!   walk, which is why nothing was ever tested at them.
//! - **Reading a position out loud.** The panel says where the camera is in
//!   both dialects at once - flat `(x, z)` and a `Place` - so the moment they
//!   stop agreeing you can see it happen.

use bevy::prelude::*;

use crate::place::Place;
use crate::terrain::PLANET_RADIUS;

/// Boot the planet on its own.
pub fn run() {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(bevy::render::RenderPlugin {
                render_creation: bevy::render::settings::WgpuSettings {
                    backends: Some(
                        bevy::render::settings::Backends::from_env().unwrap_or_default(),
                    ),
                    ..default()
                }
                .into(),
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Divus Factus - the planet".into(),
                    resolution: (1600u32, 900u32).into(),
                    ..default()
                }),
                ..default()
            }),
    )
    .init_state::<crate::GameState>()
    .init_resource::<crate::WorldSeed>()
    .add_plugins((
        crate::camera::CameraPlugin,
        crate::terrain::TerrainPlugin,
        crate::globe::GlobePlugin,
        crate::render::RenderPlugin,
        crate::keymap::KeymapPlugin,
        // Trees, rocks and the rest of what grows on the ground. Not villagers
        // - nothing here is alive - but a planet with bare ground on it is not
        // the planet the game is played on, and half of what the eye judges
        // ground BY is what is standing on it.
        crate::scatter::ScatterPlugin,
        crate::grass::GrassPlugin,
        // Not for the veil - that is off. Chunks are born HIDDEN and this owns
        // the only line in the game that shows one, so without it every
        // streamed chunk stays invisible for the life of the run, and with it
        // every tree, grove and stretch of near water standing on one. What
        // was on screen all this time was the planet's own patches.
        crate::fog::FogPlugin,
        crate::debug::timings::TimingsPlugin,
    ))
    .add_plugins((
        crate::calendar::CalendarPlugin,
        crate::sky::SkyPlugin,
        crate::starfield::StarfieldPlugin,
        crate::clouds::CloudPlugin,
        // Not for the game's sake: the camera wants `PointerContext` and the
        // seasons want somewhere to post a notice, both of which the UI owns.
        crate::ui::UiPlugin,
    ))
    // No fog of war. There is nobody here to have walked anywhere, so there is
    // nothing a veil could honestly be hiding - and this is a bench for looking
    // at the planet. It is already inert without a `KnownWorld` to draw from;
    // this says so out loud, so it cannot come back by some other door.
    .insert_resource(crate::fog::FogMode(false))
    // Which layers are drawn. The whole debug plugin would bring the faith
    // chart, the chronicle and the village panels with it, and every one of
    // them wants a village.
    .init_resource::<crate::debug::layers::ViewLayers>()
    // Where the flag went, which is nowhere - the villagers own this and there
    // are none here, but the veil asks after it.
    .init_resource::<crate::villager::ChosenGround>()
    // The sun, the sky fill and the moon. These live in `main` rather than in
    // any plugin, so an app assembled from the plugin list alone gets no light
    // at all - which is exactly what happened: the planet hung in the dark
    // with its cloud shell the only bright thing on it, at midday.
    .add_systems(Startup, crate::spawn_lighting)
    // Straight past the splash and the title: there is no game to open.
    .add_systems(Startup, open_the_world)
    .add_systems(
        Update,
        (jump_about, read_the_position, are_the_trees_upright),
    )
    .run();
}

/// No splash, no title, no flag - just stand on the ground.
fn open_the_world(mut next: ResMut<NextState<crate::GameState>>) {
    next.set(crate::GameState::Playing);
}

/// Somewhere worth standing, and what makes it awkward.
struct Landmark {
    key: KeyCode,
    name: &'static str,
    flat: Vec3,
}

/// The places a flat scaffold is known to get wrong, one key each.
fn landmarks() -> [Landmark; 5] {
    let quarter = PLANET_RADIUS * std::f32::consts::FRAC_PI_2;
    [
        Landmark {
            key: KeyCode::Digit1,
            name: "the origin - where flat and round agree exactly",
            flat: Vec3::ZERO,
        },
        Landmark {
            key: KeyCode::Digit2,
            name: "a long walk out - two thousand units",
            flat: Vec3::new(2_000.0, 0.0, -1_400.0),
        },
        Landmark {
            key: KeyCode::Digit3,
            name: "the north pole - east and west collapse here",
            flat: Vec3::new(0.0, 0.0, -quarter),
        },
        Landmark {
            key: KeyCode::Digit4,
            name: "the antipode - half a world from the origin",
            flat: Vec3::new(PLANET_RADIUS * std::f32::consts::PI, 0.0, 0.0),
        },
        Landmark {
            key: KeyCode::Digit5,
            name: "the seam - where longitude wraps round",
            flat: Vec3::new(PLANET_RADIUS * std::f32::consts::PI - 300.0, 0.0, -200.0),
        },
    ]
}

/// Jump the camera to a landmark. Walking there takes hours; that is exactly
/// why none of these were ever looked at.
fn jump_about(keys: Res<ButtonInput<KeyCode>>, mut rig: Query<&mut crate::camera::CameraRig>) {
    let Ok(mut rig) = rig.single_mut() else {
        return;
    };
    for spot in landmarks() {
        if keys.just_pressed(spot.key) {
            info!("planet: to {}", spot.name);
            rig.focus = spot.flat;
            rig.target_focus = spot.flat;
        }
    }
}

/// Say where we are in both dialects at once.
///
/// The whole diagnostic in one line: the flat `(x, z)` the simulation would
/// use, the same point as a `Place`, and the gap between where the flat map
/// says it is and where it actually seats. At the origin that gap is zero. If
/// it is ever anything else, the two spaces have come apart, and this is where
/// you find out - not four hundred metres into a soak.
fn read_the_position(
    rig: Query<&crate::camera::CameraRig>,
    keys: Res<ButtonInput<KeyCode>>,
    tree: Option<Res<crate::globe::PlanetTree>>,
) {
    if !keys.just_pressed(KeyCode::KeyP) {
        return;
    }
    let Ok(rig) = rig.single() else {
        return;
    };
    let flat = rig.focus;
    let place = Place::from_flat(flat);
    let back = place.flat();
    let home = Place::from_flat(Vec3::ZERO);
    info!(
        "planet: flat ({:.1}, {:.1}) | lat {:.2} lon {:.2} | round trip off by {:.3} | \
         {:.0} units from home along the ground, {:.0} through the rock",
        flat.x,
        flat.z,
        place.direction().y.asin().to_degrees(),
        place.direction().x.atan2(place.direction().z).to_degrees(),
        (back.x - flat.x).abs().max((back.z - flat.z).abs()),
        home.apart(place),
        home.seat().distance(place.seat()),
    );
    if let Some(tree) = tree {
        info!("planet: {} patches standing", tree.standing());
    }
}

/// Are the trees standing up?
///
/// On a ball "upright" is a different direction at every point, so a tree that
/// looks fine at the origin can be lying on its side a continent away. This
/// measures it rather than judging it from the picture: the angle between each
/// tree's own up and the planet's outward where it stands.
fn are_the_trees_upright(
    time: Res<Time>,
    mut said: Local<bool>,
    keys: Res<ButtonInput<KeyCode>>,
    trees: Query<&GlobalTransform, With<crate::scatter::FellableTree>>,
    groves: Query<(&Mesh3d, &ViewVisibility), With<crate::scatter::GroveMesh>>,
    meshes: Res<Assets<Mesh>>,
) {
    // Once the world has had time to settle, and again on T.
    let due = !*said && time.elapsed_secs() > 12.0;
    if !due && !keys.just_pressed(KeyCode::KeyT) {
        return;
    }
    *said = true;
    let centre = crate::globe::planet_centre();
    let mut counted = 0usize;
    let mut worst: f32 = 0.0;
    let mut total = 0.0;
    for at in &trees {
        let outward = (at.translation() - centre).normalize_or(Vec3::Y);
        let mine = (at.rotation() * Vec3::Y).normalize_or(Vec3::Y);
        let off = mine.dot(outward).clamp(-1.0, 1.0).acos().to_degrees();
        worst = worst.max(off);
        total += off;
        counted += 1;
    }
    if counted == 0 {
        info!("planet: no trees standing at all");
        return;
    }
    info!(
        "planet: {counted} trees, leaning {:.1} degrees off the outward on          average, worst {worst:.1}",
        total / counted as f32,
    );

    // A tree is only an entity; what is DRAWN is its grove's merged mesh. If
    // every tree is standing and nothing is on screen, this is where it went.
    let mut stands = 0usize;
    let mut seen = 0usize;
    let mut vertices = 0usize;
    for (mesh, visible) in &groves {
        stands += 1;
        if visible.get() {
            seen += 1;
        }
        if let Some(mesh) = meshes.get(&mesh.0) {
            vertices += mesh.count_vertices();
        }
    }
    info!("planet: {stands} groves, {seen} on screen, {vertices} vertices between them");
}
