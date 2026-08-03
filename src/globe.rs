//! The planet, seen whole: zoom out far enough and the ground becomes a globe.
//!
//! The terrain has been a genuine sphere since the field moved into a volume —
//! this is the first time the game shows it. Past a zoom threshold the flat
//! world hands the frame to a single planet mesh sampled from the same height
//! and climate fields the chunks are built from, so the globe is not an
//! illustration of the world: it IS the world, coarsely. Grab it with the left
//! mouse and drag to turn it; zoom back in and the ground comes back under you
//! wherever you were looking.
//!
//! The handover is a swap, not a blend, and the craft is in making the two
//! frames agree at the moment of the swap: the view steepens to straight-down
//! on the way out, the globe is framed so the ground you left fills the view
//! the same way, and the palette is the chunk painter's own. Full continuity —
//! one mesh refining all the way from orbit to a villager's doorstep — is the
//! quadtree stage, and everything here (the sampling, the layer, the camera
//! bookkeeping) is the ground that stage builds on.
//!
//! Mechanically the swap is one component: the god camera's `RenderLayers`.
//! The globe lives alone on its own layer, so pointing the camera at that
//! layer hides every chunk, tree, villager and fog cloth in a single write,
//! and pointing it back restores them untouched.

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::pbr::DistanceFog;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use std::f32::consts::FRAC_PI_2;

use crate::camera::{CameraRig, GodCamera};
use crate::palette;
use crate::terrain::{PLANET_RADIUS, Terrain, WATER_LEVEL};

/// The globe's own render layer. Nothing else lives on it, which is what
/// makes showing the planet a single component write on the camera.
pub const GLOBE_LAYER: usize = 2;

/// Going out through this camera distance, the flat world hands over to the
/// planet.
///
/// Low, on purpose, and lower than it first shipped. The loaded ground is a
/// disc about thirteen hundred units wide, and from five thousand up that
/// disc was a shrinking island in a void of clear-colour grey — the handover
/// arrived long after the world had already dissolved, and the space between
/// was nothing at all. From here the ground still fills most of the frame,
/// so the planet takes over from GROUND, not from grey.
pub const CURTAIN: f32 = 3_200.0;

/// Coming back in through this, the ground returns. Inside the curtain so the
/// two thresholds cannot chatter when the wheel hesitates between them.
const RESURFACE: f32 = 2_850.0;

/// How far out the wheel will take the god. From here the whole planet sits
/// in the frame with room to spare, like a held apple.
pub const CEILING: f32 = 42_000.0;

/// Terrain relief on the globe, multiplied. At true scale the tallest
/// mountain is two percent of the radius and the planet reads as a billiard
/// ball; three times is the classroom-globe exaggeration that lets a thumb
/// find the ranges.
const EXAGGERATION: f32 = 3.0;

/// Grid cells along each edge of each cube face. Six faces of this give a
/// vertex roughly every hundred and fifty units of ground — coarse up close,
/// but the globe is never seen closer than the curtain, and from there a cell
/// subtends less than two degrees.
const FACE_CELLS: usize = 160;

/// The rotation that stands the planet up in the world: the terrain scaffold
/// maps local ground onto the unit sphere with the reference point at +Z and
/// the poles at ±Y, and this turns that frame so the reference point is UP —
/// putting the globe's surface tangent under the flat world — with north kept
/// as the game's north (-Z).
fn planet_stance() -> Quat {
    Quat::from_rotation_x(-FRAC_PI_2)
}

/// Where the planet's centre sits: far enough below the flat world that the
/// sea-level sphere grazes the ground the game is played on.
fn planet_centre() -> Vec3 {
    Vec3::new(0.0, -(PLANET_RADIUS + WATER_LEVEL), 0.0)
}

/// The state of the orbital view.
#[derive(Resource)]
pub struct GlobeView {
    /// Whether the planet currently owns the frame.
    pub shown: bool,
    /// Unit direction from the planet's centre to the ground under view, in
    /// world space. Dragging rotates this; leaving orbit turns it back into
    /// the `(x, z)` the flat world speaks.
    look: Vec3,
    /// Present height above the sea-level sphere, smoothed toward `sought`.
    gaze: f32,
    /// Where the wheel has asked the height to go.
    sought: f32,
    /// The seed the current globe mesh was carved for, so a new world gets a
    /// new planet and an old one is never rebuilt mid-play.
    carved_for: Option<u32>,
}

impl Default for GlobeView {
    fn default() -> Self {
        GlobeView {
            shown: false,
            look: Vec3::Y,
            gaze: CURTAIN,
            sought: CURTAIN,
            carved_for: None,
        }
    }
}

/// The planet mesh entity.
#[derive(Component)]
struct ThePlanet;

pub struct GlobePlugin;

impl Plugin for GlobePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GlobeView>()
            .add_systems(Update, carve_the_globe.run_if(resource_exists::<Terrain>))
            // After the camera set: the rig has settled and written its
            // transform, so orbit can read the final distance and, while the
            // planet owns the frame, have the last word on where the camera
            // actually is.
            .add_systems(
                Update,
                behold_the_world
                    .after(crate::camera::CameraSet)
                    .run_if(crate::world_is_afoot),
            )
            .add_systems(
                Update,
                dress_for_space.after(crate::render::track_fog_to_zoom),
            );
    }
}

// --------------------------------------------------------------- the carving

/// Builds the planet from the same fields the chunks are built from — once
/// per world, during the loading screen's shadow.
fn carve_the_globe(
    mut commands: Commands,
    terrain: Res<Terrain>,
    mut view: ResMut<GlobeView>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    old: Query<Entity, With<ThePlanet>>,
) {
    if view.carved_for == Some(terrain.seed) {
        return;
    }
    view.carved_for = Some(terrain.seed);
    for stale in &old {
        commands.entity(stale).despawn();
    }

    let mesh = carve(&terrain, FACE_CELLS);
    commands.spawn((
        Name::new("The Planet"),
        ThePlanet,
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 1.0,
            ..default()
        })),
        Transform::from_translation(planet_centre()).with_rotation(planet_stance()),
        Visibility::Hidden,
        RenderLayers::layer(GLOBE_LAYER),
        // Three hundred thousand triangles have no business in a shadow map,
        // and cascades sized for a village would only paint acne on a world.
        NotShadowCaster,
        NotShadowReceiver,
    ));

    // The planet's own sun. The world's lights are confined to the layers the
    // world lives on, so without this the globe would hang in space unlit.
    // Same colour and strength as the sun the ground knows, from the same
    // direction, so the day side of the planet is the day the player left.
    commands.spawn((
        Name::new("The Planet's Sun"),
        ThePlanet,
        DirectionalLight {
            color: palette::shade(&palette::BONE, 1.0),
            illuminance: 17_000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_translation(crate::SUN_DIRECTION * 140.0).looking_at(Vec3::ZERO, Vec3::Y),
        RenderLayers::layer(GLOBE_LAYER),
    ));

    info!(
        "the planet was carved: {} vertices of a world {:.0} units around",
        6 * (FACE_CELLS + 1) * (FACE_CELLS + 1),
        crate::terrain::planet_circumference()
    );
}

/// Samples the world onto a cube-sphere and paints it with the chunk
/// painter's own palette.
///
/// A cube-sphere rather than a latitude/longitude ball for the same reason
/// the noise lives in a volume: no crowding at the poles. Each face is a grid
/// of directions; every direction is turned back into the `(x, z)` the
/// terrain speaks — the mapping is exact for the whole sphere — and asked the
/// same questions a chunk vertex asks: height, moisture, biome, mottling.
fn carve(terrain: &Terrain, cells: usize) -> Mesh {
    let n = cells;
    let stride = n + 1;
    // One cell of skirt each side, for normals and slope at the face edges.
    let padded = n + 3;

    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(6 * stride * stride);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(6 * stride * stride);
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(6 * stride * stride);
    let mut indices: Vec<u32> = Vec::with_capacity(6 * n * n * 6);

    // The six faces of the cube, each spanned by two axes around a fixed one.
    let faces: [(Vec3, Vec3, Vec3); 6] = [
        (Vec3::X, Vec3::Y, Vec3::Z),
        (Vec3::NEG_X, Vec3::Y, Vec3::NEG_Z),
        (Vec3::Y, Vec3::Z, Vec3::X),
        (Vec3::NEG_Y, Vec3::NEG_Z, Vec3::X),
        (Vec3::Z, Vec3::Y, Vec3::NEG_X),
        (Vec3::NEG_Z, Vec3::Y, Vec3::X),
    ];

    for (outward, along_v, along_u) in faces {
        let base = positions.len() as u32;

        // The padded position grid, skirt included: drawn radial by drawn
        // height, so slopes and normals come from the geometry that will
        // actually be seen.
        let mut grid: Vec<Vec3> = Vec::with_capacity(padded * padded);
        let mut ground: Vec<(f32, f32, f32)> = Vec::with_capacity(padded * padded);
        for gj in 0..padded {
            for gi in 0..padded {
                let u = (gi as f32 - 1.0) / n as f32 * 2.0 - 1.0;
                let v = (gj as f32 - 1.0) / n as f32 * 2.0 - 1.0;
                let dir = (outward + along_u * u + along_v * v).normalize();
                let (x, z) = ground_coordinates(dir);
                let h = terrain.base_height_at(x, z);
                grid.push(dir * drawn_radial(h));
                ground.push((x, z, h));
            }
        }
        let at = |gi: usize, gj: usize| grid[gj * padded + gi];

        for gj in 0..stride {
            for gi in 0..stride {
                // Offset past the skirt.
                let (pi, pj) = (gi + 1, gj + 1);
                let here = at(pi, pj);
                let dir = here.normalize();
                let (x, z, h) = ground[pj * padded + pi];

                // Central differences across the drawn surface: one cross
                // product gives the normal, and how far it leans off the
                // radial gives the slope the painter wants.
                let across = at(pi + 1, pj) - at(pi - 1, pj);
                let down = at(pi, pj + 1) - at(pi, pj - 1);
                let mut normal = across.cross(down).normalize_or(dir);
                if normal.dot(dir) < 0.0 {
                    normal = -normal;
                }
                let slope = (1.0 - normal.dot(dir)).clamp(0.0, 1.0);

                positions.push(here.to_array());
                normals.push(normal.to_array());
                colors.push(paint(terrain, x, z, h, slope));
            }
        }

        // Two triangles per cell, wound so the face looks outward. The test
        // is done on geometry rather than trusted to the face table — and the
        // emission below is matched to the test's SIGN, which the first
        // version got backwards on both branches at once. Every face came out
        // wound inward, the near hemisphere culled away, and the view through
        // the open shell landed on the INSIDE of the far side — which reads
        // exactly like a planet, but sits a whole planet further off, which
        // is how a fog band aimed at the limb swallowed what looked like the
        // ground. `(pb - pa) × (pc - pa) · outward > 0` means `[a, b, c]` is
        // counter-clockwise seen from outside, which is Bevy's front face.
        let a = base;
        let b = base + 1;
        let c = base + stride as u32;
        let outward_as_abc = {
            let pa = Vec3::from(positions[a as usize]);
            let pb = Vec3::from(positions[b as usize]);
            let pc = Vec3::from(positions[c as usize]);
            (pb - pa).cross(pc - pa).dot(pa) > 0.0
        };
        for row in 0..n as u32 {
            for column in 0..n as u32 {
                let top_left = base + row * stride as u32 + column;
                let top_right = top_left + 1;
                let bottom_left = top_left + stride as u32;
                let bottom_right = bottom_left + 1;
                if outward_as_abc {
                    indices.extend([top_left, top_right, bottom_left]);
                    indices.extend([top_right, bottom_right, bottom_left]);
                } else {
                    indices.extend([top_left, bottom_left, top_right]);
                    indices.extend([top_right, bottom_left, bottom_right]);
                }
            }
        }
    }

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(indices))
}

/// The `(x, z)` the terrain speaks, for a direction on the unit sphere in the
/// scaffold's own frame. Exact everywhere: latitude and longitude recovered
/// from the direction reproduce it through `direction_at` to the last bit
/// that matters.
fn ground_coordinates(dir: Vec3) -> (f32, f32) {
    let lat = dir.y.clamp(-1.0, 1.0).asin();
    let lon = dir.x.atan2(dir.z);
    // The minus mirrors `direction_at`'s: the game's north is -z.
    (lon * PLANET_RADIUS, -lat * PLANET_RADIUS)
}

/// Radial distance the surface is drawn at, for ground of height `h`. The sea
/// is a smooth ball at water level — a globe shows its oceans, not its ocean
/// floors — and land rises off it, exaggerated.
fn drawn_radial(h: f32) -> f32 {
    PLANET_RADIUS + WATER_LEVEL + (h - WATER_LEVEL).max(0.0) * EXAGGERATION
}

/// One vertex's colour: the sea by depth in the water's own ramp, the land by
/// the chunk painter itself, so the planet from orbit is recognisably the
/// ground the game is played on.
fn paint(terrain: &Terrain, x: f32, z: f32, h: f32, slope: f32) -> [f32; 4] {
    if h <= WATER_LEVEL {
        // The water plane's own two shades, but the ramp runs deeper and
        // saturates sooner: from orbit an ocean is a body, not a shore, and
        // painted at the shoreline's pale teal the whole sea read as shallows.
        let depth = ((WATER_LEVEL - h) / 8.0).clamp(0.0, 1.0);
        let shallow = palette::shade(&palette::WATER, 0.9).to_linear();
        let deep = palette::shade(&palette::WATER, 0.45).to_linear();
        return [
            shallow.red + (deep.red - shallow.red) * depth,
            shallow.green + (deep.green - shallow.green) * depth,
            shallow.blue + (deep.blue - shallow.blue) * depth,
            1.0,
        ];
    }

    let moisture = terrain.moisture_at(x, z);
    let shade_t =
        (0.42 + ((h - WATER_LEVEL) / 200.0) * 0.45 + (moisture - 0.5) * 0.12).clamp(0.0, 1.0);
    let color = crate::terrain::surface_color(
        h,
        slope,
        moisture,
        shade_t,
        terrain.biome_for(x, z, h),
        terrain.line_variation_at(x, z),
        terrain.ground_patch_at(x, z),
        None,
    )
    .to_linear();
    [color.red, color.green, color.blue, 1.0]
}

// --------------------------------------------------------------- the orbit

/// Opens the orbital view when the zoom leaves the world, drives it while it
/// is open — grab and drag to turn the planet, wheel to close or back away —
/// and lands the camera back on the ground when the zoom comes home.
#[allow(clippy::too_many_arguments)]
fn behold_the_world(
    mut commands: Commands,
    time: Res<Time<Real>>,
    dive: Option<Res<crate::camera::CameraDive>>,
    terrain: Option<Res<Terrain>>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse_motion: Res<bevy::input::mouse::AccumulatedMouseMotion>,
    mouse_scroll: Res<bevy::input::mouse::AccumulatedMouseScroll>,
    pointer: Res<crate::ui::PointerContext>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut view: ResMut<GlobeView>,
    mut planets: Query<&mut Visibility, With<ThePlanet>>,
    mut cameras: Query<(Entity, &mut CameraRig, &mut Transform), With<GodCamera>>,
) {
    let Ok((camera, mut rig, mut transform)) = cameras.single_mut() else {
        return;
    };

    if !view.shown {
        // The opening descent owns the rig; the handover waits for it.
        if rig.distance > CURTAIN && dive.is_none() {
            view.shown = true;
            view.look = planet_stance() * crate::terrain::direction_at(rig.focus.x, rig.focus.z);
            view.gaze = rig.distance;
            view.sought = rig.distance;
            commands
                .entity(camera)
                .insert(RenderLayers::layer(GLOBE_LAYER));
            for mut showing in &mut planets {
                *showing = Visibility::Inherited;
            }
        } else {
            return;
        }
    }

    // The wheel, shared with the ground's own zoom handling in shape.
    let scroll = if pointer.over_ui {
        0.0
    } else {
        crate::camera::normalised_scroll(mouse_scroll.delta.y, mouse_scroll.unit)
    };
    if scroll != 0.0 {
        let factor = (1.0 - scroll * rig.zoom_sensitivity).clamp(0.5, 2.0);
        view.sought = (view.sought * factor).clamp(RESURFACE * 0.9, CEILING);
    }
    // Escape is the other way home.
    if keys.just_pressed(KeyCode::Escape) {
        view.sought = RESURFACE * 0.9;
    }
    let ease = 1.0 - (-6.0 * time.delta_secs()).exp();
    view.gaze += (view.sought - view.gaze) * ease;

    // The grab. Dragging turns the planet under the hand: horizontal drags
    // rotate the look about the screen's up axis, vertical about its right,
    // scaled so a pixel of mouse is about a pixel of ground.
    let pole = planet_stance() * Vec3::Y;
    let up_hint = (pole - view.look * pole.dot(view.look)).normalize_or(Vec3::X);
    if buttons.pressed(MouseButton::Left) && !pointer.over_ui {
        let delta = mouse_motion.delta;
        if delta != Vec2::ZERO {
            let height_px = windows
                .single()
                .map(|w| w.resolution.height())
                .unwrap_or(1000.0)
                .max(1.0);
            // Ground covered by one pixel at this height, as an angle at the
            // planet's centre.
            let per_pixel = 2.0 * (0.31f32).tan() * view.gaze / height_px / PLANET_RADIUS;
            let forward = -view.look;
            let right = forward.cross(up_hint).normalize_or(Vec3::X);
            let turned = Quat::from_axis_angle(up_hint, -delta.x * per_pixel)
                * Quat::from_axis_angle(right, -delta.y * per_pixel)
                * view.look;
            // Short of the poles: the scaffold hands out one longitude per
            // point, and a camera exactly over a pole has no north to hang
            // its frame on.
            let leaning = planet_stance().inverse() * turned;
            if leaning.y.abs() < 0.985 {
                view.look = turned.normalize();
            }
        }
    }

    // Coming home: turn the direction under view back into the ground's own
    // words and give the frame back to the world.
    if view.gaze < RESURFACE {
        let ground_dir = planet_stance().inverse() * view.look;
        let (x, z) = ground_coordinates(ground_dir);
        let y = terrain.as_ref().map_or(WATER_LEVEL, |t| t.height_at(x, z));
        rig.focus = Vec3::new(x, y, z);
        rig.target_focus = rig.focus;
        rig.distance = view.gaze;
        rig.target_distance = view.gaze * 0.96;
        // Straight down and north-up, which is exactly how the planet was
        // being looked at — the swap changes the scenery's resolution, not
        // the view.
        rig.pitch = crate::camera::MAX_PITCH;
        rig.target_pitch = rig.pitch;
        rig.yaw = 0.0;
        rig.target_yaw = 0.0;
        rig.zoom_anchor = None;

        commands.entity(camera).remove::<RenderLayers>();
        for mut showing in &mut planets {
            *showing = Visibility::Hidden;
        }
        view.shown = false;
        *transform = Transform::from_translation(rig.eye()).looking_at(rig.focus, Vec3::Y);
        return;
    }

    // The orbital camera: straight down at the point under view, north up.
    let sea = PLANET_RADIUS + WATER_LEVEL;
    let centre = planet_centre();
    let eye = centre + view.look * (sea + view.gaze);
    *transform = Transform::from_translation(eye).looking_at(centre + view.look * sea, up_hint);
}

// --------------------------------------------------------------- the sky

/// Space is dark, and the climb to it starts before the handover.
///
/// The fog tracker paints the clear colour with the horizon every frame; this
/// runs after it and pulls the sky down toward black as the god rises — and
/// it starts INSIDE the play zoom's last stretch, not at the handover. The
/// void beyond the loaded ground is drawn in the clear colour, and left at
/// horizon-grey it read as a dead frame: the world shrank into featureless
/// nothing, then a planet appeared out of the same nothing. Darkening on the
/// way up recasts that void as the sky thinning into space, which is a thing
/// the eye already knows the meaning of.
///
/// Past the curtain it also stretches the distance fog so it stops eating a
/// planet thirty thousand units deep — left at the play band it swallowed
/// everything past the first thousand, and the whole ball came back as one
/// blank fog-coloured disc.
fn dress_for_space(
    view: Option<Res<GlobeView>>,
    mut clear: ResMut<ClearColor>,
    mut cameras: Query<(&CameraRig, &mut DistanceFog), With<GodCamera>>,
) {
    let Some(view) = view else {
        return;
    };
    let Ok((rig, mut fog)) = cameras.single_mut() else {
        return;
    };

    // The height of the climb: the rig's own distance on the way up, the
    // orbit's once the planet has the frame. One number, so the darkening is
    // continuous through the handover.
    let height = if view.shown { view.gaze } else { rig.distance };

    // From the last stretch of play zoom out to high orbit, 0 to 1.
    let out = ((height - 1_500.0) / (CEILING * 0.45 - 1_500.0)).clamp(0.0, 1.0);
    if out <= 0.0 {
        return;
    }

    let horizon = clear.0.to_linear();
    let space = Color::srgb(0.004, 0.005, 0.012).to_linear();
    // Eased, so the first stretch of the climb only dusks the sky and the
    // true black is kept for genuine altitude.
    let toward = out * out;
    clear.0 = Color::LinearRgba(bevy::color::LinearRgba {
        red: horizon.red + (space.red - horizon.red) * toward,
        green: horizon.green + (space.green - horizon.green) * toward,
        blue: horizon.blue + (space.blue - horizon.blue) * toward,
        alpha: 1.0,
    });

    if !view.shown {
        return;
    }

    // The fog's remnant becomes the atmosphere. The ball's fragments run from
    // the nadir (`gaze` away) out to the limb, and the haze must touch ONLY
    // the last stretch before the limb or it eats the planet whole — which is
    // exactly what it did at first: the play fog's band is a thousand units
    // wide, the ball is thirty thousand deep, and every fragment past the
    // band came back as flat horizon-grey. The whole planet rendered
    // perfectly and displayed as a blank ball, and nothing looked wrong in
    // the code because the code was looking at the wrong band.
    let eye_out = PLANET_RADIUS + WATER_LEVEL + view.gaze;
    let limb = (eye_out * eye_out - PLANET_RADIUS * PLANET_RADIUS)
        .max(0.0)
        .sqrt();
    fog.falloff = bevy::pbr::FogFalloff::Linear {
        start: limb - PLANET_RADIUS * 0.18,
        end: limb + PLANET_RADIUS * 0.22,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Leaving orbit must land where the orbit was looking: the direction
    /// under view, turned into ground words, turned back into a direction,
    /// is the same direction. This is the exact path the camera takes on the
    /// way home, sign conventions and all — the mirror bug this build caught
    /// lived precisely here.
    #[test]
    fn the_way_down_lands_where_you_looked() {
        for i in 0..500 {
            let golden = 2.399963_f32;
            let y = 0.995 - 1.99 * (i as f32 + 0.5) / 500.0;
            let r = (1.0 - y * y).sqrt();
            let a = golden * i as f32;
            let dir = Vec3::new(r * a.cos(), y, r * a.sin());
            let (x, z) = ground_coordinates(dir);
            let back = crate::terrain::direction_at(x, z);
            assert!(
                dir.dot(back) > 0.999_99,
                "direction {dir} came back as {back}"
            );
        }
    }

    /// And the compass agrees between the ground and the globe: a point the
    /// game calls north of another (smaller z — pan-north walks along -z)
    /// must sit at HIGHER latitude, or the orbital view is a mirror of the
    /// world it claims to show.
    #[test]
    fn north_from_orbit_is_north_on_the_ground() {
        let here = crate::terrain::direction_at(0.0, 0.0);
        let north_of_here = crate::terrain::direction_at(0.0, -600.0);
        assert!(
            north_of_here.y > here.y,
            "walking north lowered the latitude: the globe is a mirror"
        );
    }

    /// The sea is painted as a body of water, darker than any land is green,
    /// and the land is greener than the sea. A planet that fails this is a
    /// planet whose paint has quietly broken — it happened once as a fog band
    /// a thirtieth the depth of the ball.
    #[test]
    fn the_planet_is_blue_where_it_is_wet_and_green_where_it_is_not() {
        let t = Terrain::new(4242);
        let mut sea = (0.0f32, 0);
        let mut land = (0.0f32, 0);
        for i in 0..2000 {
            let golden = 2.399963_f32;
            let y = 1.0 - 2.0 * (i as f32 + 0.5) / 2000.0;
            let r = (1.0 - y * y).sqrt();
            let a = golden * i as f32;
            let dir = Vec3::new(r * a.cos(), y, r * a.sin());
            let (x, z) = ground_coordinates(dir);
            let h = t.base_height_at(x, z);
            let c = paint(&t, x, z, h, 0.05);
            if h <= WATER_LEVEL {
                sea = (sea.0 + c[2] - c[1].max(c[0]), sea.1 + 1);
            } else if h < WATER_LEVEL + 70.0 {
                land = (land.0 + c[1] - c[0].max(c[2]), land.1 + 1);
            }
        }
        assert!(sea.1 > 100 && land.1 > 100, "world too uniform to judge");
        assert!(sea.0 / sea.1 as f32 > 0.0, "the sea is not blue");
        assert!(land.0 / land.1 as f32 > 0.0, "the lowland is not green");
    }
}
