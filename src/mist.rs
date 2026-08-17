//! Ground fog: the mist that lies in the low ground of damp and cold country.
//!
//! WHAT DECIDES WHERE MIST IS. Not the `Biome` enum. `biome_for` is a
//! CLASSIFICATION - four thresholds on two continuous fields - and a fog driven
//! by it would change abruptly along a line no eye can see a reason for,
//! wherever moisture crossed 0.58. The climate underneath is smooth, so the
//! mist is read from the climate directly and inherits that smoothness. Three
//! things decide it, and they are genuinely three different questions:
//!
//! 1. WHAT THE AIR CARRIES - damp country, or cold country ([`mist_climate`]).
//! 2. WHETHER IT COMES IN BANKS - noise at a couple of hundred meters, remapped
//!    hard enough to leave real holes, because a smooth field over the gentle
//!    coastal ground a village is founded on comes out as one flat sheet from
//!    edge to edge, which is the gray soup every fog effect fails into.
//! 3. WHERE IT SETTLES - how low the ground lies compared to WHAT IS AROUND IT
//!    ([`pooling`], applied in [`settle_the_mist`] once the neighbors exist).
//!    Measured against sea level instead, a hollow in a plateau got nothing
//!    while a flat coastal plain got everything: fog on the dull ground and
//!    none in the valley, which is exactly backwards.
//!
//! Which is also why it agrees with the country it stands in without being told
//! to. Wetland is the biome you get from high moisture and low ground; mist is
//! the fog you get from the same two things.
//!
//! MEASURED, not guessed (`probe_mist_cover` in `terrain`, three worlds). The
//! first law had mist on 86% of all land at a mean weight of 0.57 - fog
//! everywhere, which is fog nowhere.
//!
//! HOW THE SHADER LEARNS IT. It cannot ask. `moisture_at` and `temperature_for`
//! are three octaves of seeded fbm apiece, and a WGSL copy of them would be one
//! silent divergence away from mist that disagrees with the ground it lies on.
//! So a field is baked on the processor into a small texture around the camera,
//! and the pass samples it. The bake is spread over several frames, and it asks
//! `base_height_at` rather than `height_at` - see the note in [`bake_the_field`]
//! for the hundred and fifty milliseconds that saves.
//!
//! AND IT IS A CLOCK, NOT A SUN ANGLE. See [`burn_off`]: a day is not
//! symmetric, and the one thing everybody knows about fog is that mornings have
//! it and afternoons do not.

use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::terrain::{Terrain, WATER_LEVEL};

/// How wide a square of world the field covers, in meters.
///
/// SMALL ON PURPOSE. The first field was six kilometers across, which at 128
/// cells is forty-seven meters a cell - and a river channel is twelve meters
/// wide, so every river in the world fell between two samples. Mist lying along
/// a river, tracing it white through the country, is the best thing ground fog
/// ever does, and at that resolution it was not available at all. The bake
/// costs per CELL and not per meter, so tightening the span costs nothing and
/// buys the whole picture.
pub const FIELD_SPAN: f32 = 3_072.0;

/// Cells to a side: 16384 samples at twenty-four meters apiece.
pub const FIELD_SIDE: u32 = 128;

/// How many frames one bake is spread across. Even a cheap bake is a hitch if
/// it lands in one frame, and this game watches its frame time.
const BAKE_FRAMES: u32 = 8;

/// How far the camera may wander from the field's center before a new bake
/// starts. A third of the span, so there is always most of a field's worth of
/// good ground ahead of the player.
const WANDER: f32 = FIELD_SPAN / 3.0;

/// How far above the LOCAL floor mist can still find air to fill. Past this it
/// has spilled out of whatever hollow it was lying in.
const POOLS_BELOW: f32 = 90.0;

/// How wide a neighborhood decides what "the local floor" is, in cells.
///
/// This is the difference between mist that pools and mist that does not.
/// Measured against sea level, a hollow in a plateau at two hundred meters got
/// no mist at all while a flat coastal plain got all of it - fog on the boring
/// ground and none in the valley the player is standing over, which is exactly
/// backwards. What matters is not how high a place is but how much lower it is
/// than what surrounds it.
const FLOOR_REACH: i32 = 3;

/// The full range the field's height channel spans, in meters. One byte over
/// this is about a meter and a half a step, which the sampler's own
/// interpolation smooths out well below anything mist shows.
const HEIGHT_RANGE: f32 = 380.0;

/// How deep the mist lies over its ground, in meters, where it lies thickest.
/// The shader thins it upward over this AND gives it a top, so a ridge can
/// stand out of it like an island.
pub const MIST_DEPTH: f32 = 34.0;

/// How far the mist's cool side is pulled from the sky's own color toward a
/// warm bone white.
///
/// Mist wants to be the sky seen edge-on, and mostly it is - that is what makes
/// it go rose at dawn on its own. But the fog of war is already a cold slate
/// blue laid over unknown ground, and the player reads the edge of what their
/// village knows off exactly that color. Weather that shared the hue would make
/// the two impossible to tell apart, so the sky is taken for its MOOD and most
/// of the way back out for its hue.
pub const MIST_OF_THE_SKY: f32 = 0.7;

/// How much mist the AIR at a place carries, before the land is consulted.
///
/// Damp air, or cold air. Returns 0 to 1. Where it actually settles is a
/// separate question, answered by the local floor during the bake - the two
/// are split because they are genuinely different facts, and mixing them was
/// what put fog on the coastal flats and none in the upland valleys.
///
/// Public and free-standing so the probe can measure it and a test can pin it:
/// the numbers in here were chosen against three whole worlds, not by eye.
pub fn mist_climate(land: &Terrain, x: f32, z: f32, height: f32) -> f32 {
    if height < WATER_LEVEL {
        // Over open water there is no ground to pool on, and the sea has its
        // own color already. Mist rolls off the water onto the shore, which is
        // what the local floor does for it from the land's side.
        return 0.0;
    }
    let moisture = land.moisture_at(x, z);
    let temperature = land.temperature_for(x, z, height);

    // Damp country breathes mist. The threshold sits above the wetland
    // classification's own so that thick mist is a wetland thing without being
    // every wetland.
    let damp = ((moisture - 0.54) / 0.22).clamp(0.0, 1.0);
    // And cold country does too, for a different reason - the air cannot hold
    // what it carries. Weighted a little under the damp so a boreal morning is
    // hazy rather than blind.
    let cold = ((0.30 - temperature) / 0.18).clamp(0.0, 1.0) * 0.75;

    let carried = damp.max(cold);
    if carried <= 0.0 {
        return 0.0;
    }

    // AND IT COMES IN BANKS. Everything above is a smooth field, and a smooth
    // field over the gentle coastal ground a village is founded on comes out as
    // one flat sheet of white from edge to edge - which is the gray soup every
    // fog effect fails into, and is not what anybody has ever walked through. A
    // bank of mist has a shape: it lies thick along one hollow, thins, gives
    // out, and picks up again further on.
    //
    // Two octaves at two hundred meters and a hundred, so the shapes are the
    // size of a field rather than of a leaf, and seeded off the world so the
    // same valley is misty in the same way every morning.
    //
    // THE SCALE IS SET AGAINST THE FIELD'S OWN CELL, which is the thing that
    // is easy to get wrong: the first try ran at thirty meters, which is barely
    // one cell of a twenty-four meter bake, so every bank fell between two
    // samples and averaged back out into the flat sheet it was meant to break
    // up. A feature has to be several cells across to survive being baked.
    let banks = crate::noise::fbm_3d(
        crate::terrain::direction_at(x, z) * (crate::terrain::PLANET_RADIUS * 0.005)
            + Vec3::new(31.0, 7.0, -19.0),
        land.seed ^ 0x_4157,
        2,
        2.1,
        0.5,
    );
    // Remapped hard, so that it genuinely CLEARS in places rather than merely
    // getting thinner. A patch of open air with the sun on it is what makes the
    // bank beside it read as a bank.
    let patch = ((banks - 0.32) / 0.34).clamp(0.0, 1.0);

    carried * patch
}

/// How readily mist settles on ground that stands `above` its local floor.
///
/// Free-standing so a test can pin the shape: this is the term that makes mist
/// a thing that POOLS. Without it the same damp country carried mist up every
/// hill it had and 86% of all land came out foggy in three measured worlds.
pub fn pooling(above_the_floor: f32) -> f32 {
    let out_of_it = (above_the_floor / POOLS_BELOW).clamp(0.0, 1.0);
    // Squared, so the bottom of a hollow is decisively mistier than its sides
    // rather than fading out linearly like a gradient somebody painted.
    (1.0 - out_of_it) * (1.0 - out_of_it)
}

/// The baked field: mist weight and ground height over a square of world.
///
/// Held in the main world as an image handle the pass's uniform points at, and
/// rebuilt in slices as the camera roams.
#[derive(Resource)]
pub struct MistField {
    pub image: Handle<Image>,
    /// Where the field is centered, in flat sim coordinates.
    pub center: Vec2,
    /// Where the bake in progress is centered - which is not where the finished
    /// field is, until it lands.
    baking_toward: Vec2,
    /// Rows written so far in the bake in progress.
    rows_done: u32,
    /// The bake in progress, written row by row and swapped in whole.
    scratch: Vec<u8>,
    /// Whether a bake is running at all.
    baking: bool,
}

impl MistField {
    /// The world square the field covers, as (min, span).
    pub fn bounds(&self) -> (Vec2, f32) {
        (self.center - Vec2::splat(FIELD_SPAN * 0.5), FIELD_SPAN)
    }
}

/// Whether there is ground fog at all. `DIVUS_FACTUS_MIST=0` clears the air,
/// for photographing the world without weather in the way.
#[derive(Resource)]
pub struct MistMode(pub bool);

impl Default for MistMode {
    fn default() -> Self {
        MistMode(!std::env::var("DIVUS_FACTUS_MIST").is_ok_and(|dial| dial == "0"))
    }
}

pub struct MistPlugin;

impl Plugin for MistPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MistMode>()
            .add_systems(Startup, lay_out_the_field)
            .add_systems(
                Update,
                (bake_the_field, tell_the_pass)
                    .chain()
                    .run_if(resource_exists::<MistField>),
            );
    }
}

/// How thick the mist is at a given hour, where 0 is midnight and 0.5 is noon.
///
/// A DAY IS NOT SYMMETRIC, and this is the whole reason it takes the clock
/// rather than the sun's height. Sun elevation is a perfect mirror about noon,
/// so a mist driven by it is exactly as thick at six in the evening as at six
/// in the morning - which no one has ever seen. Real ground fog forms through
/// the night as the land gives up its heat, stands thickest around dawn, burns
/// off over a couple of hours of morning, and does not simply come back when
/// the sun goes down: the evening gets a little haze, late and thin, and mostly
/// it does not get that.
///
/// Free-standing and pinned by tests, because the shape of this curve is the
/// entire feel of a morning.
pub fn burn_off(time_of_day: f32) -> f32 {
    let hour = time_of_day.rem_euclid(1.0) * 24.0;
    if hour < 5.0 {
        // The small hours: the mist has been gathering all night.
        1.0
    } else if hour < 7.0 {
        // Dawn, and the thickest of it - the land at its coldest just as the
        // light arrives to show it.
        1.0
    } else if hour < 10.0 {
        // Burning off. Three hours, not an instant: a morning that clears
        // between one frame and the next reads as a switch being thrown.
        let through = (hour - 7.0) / 3.0;
        1.0 - through * 0.88
    } else if hour < 17.0 {
        // The long middle of the day. Not nothing - a damp hollow keeps a
        // breath of haze in it - but nothing anybody would call fog.
        0.12
    } else if hour < 21.0 {
        // Evening. It comes back, but late and thin, and never to the morning's
        // depth.
        let through = (hour - 17.0) / 4.0;
        0.12 + through * 0.33
    } else {
        // Night, thickening toward the small hours.
        let through = (hour - 21.0) / 3.0;
        0.45 + through * 0.55
    }
}

/// Hands the pass the field it baked and the sky it must be the color of.
fn tell_the_pass(
    mut commands: Commands,
    mode: Res<MistMode>,
    state: Res<State<crate::GameState>>,
    field: Res<MistField>,
    sky: Option<Res<crate::calendar::Sky>>,
    clock: Option<Res<crate::calendar::WorldClock>>,
    rigs: Query<&crate::camera::CameraRig>,
    cameras: Query<Entity, With<crate::camera::GodCamera>>,
) {
    let (Some(sky), Some(clock)) = (sky, clock) else {
        return;
    };
    let Ok(rig) = rigs.single() else {
        return;
    };

    // FROM ORBIT THERE IS NO MIST. Ground fog is a thing you stand in; seen
    // from above it is a white film over the whole world, and the planet has
    // its own clouds for that. It also has an edge - the baked field is three
    // kilometers across on a world thirty-seven around - and climbing high
    // enough to see that edge must not be possible. Gone well before the world
    // becomes a globe.
    let close = 1.0 - (rig.distance / (crate::globe::ASCENT * 0.6)).clamp(0.0, 1.0);

    let playing = *state.get() == crate::GameState::Playing;
    let strength = if mode.0 && playing {
        burn_off(clock.time_of_day()) * close * STRENGTH
    } else {
        0.0
    };

    // How low the sun stands over the ground the god is looking at. `Sky` works
    // this out for its own colors and does not keep it, so it is worked out
    // again here from the one field of it that is global.
    let (_, turn) = crate::globe::bend_frame(rig.focus.with_y(0.0));
    let up = turn * Vec3::Y;
    let sun_height = sky.sun_direction.dot(up);
    // Only a LOW sun does this. Overhead light passes through mist without
    // being thrown back at anybody, and a noon glow would look like a bug.
    let low_sun = (1.0 - (sun_height / 0.45).clamp(0.0, 1.0)) * sun_height.max(0.0).min(1.0).ceil();

    // THE COOL SIDE IS DELIBERATELY NOT THE SKY'S BLUE. Mist wants to be the
    // sky seen edge-on, and mostly it is - but the fog of war is already a cold
    // slate blue laid over unknown ground, and the player reads the edge of
    // their knowledge off that color. So the horizon is pulled most of the way
    // to a warm bone white: still the sky's mood, never the veil's hue.
    let horizon = sky.horizon.to_linear();
    let sky_color = Vec3::new(horizon.red, horizon.green, horizon.blue);
    let cool = sky_color.lerp(BONE, MIST_OF_THE_SKY);

    // And the warm side is the sun's own color, which already bends to gold as
    // it gets low - so dawn mist goes gold without anybody deciding it should.
    let sunlight = sky.sun_color.to_linear();
    let warm = Vec3::new(sunlight.red, sunlight.green, sunlight.blue).lerp(Vec3::ONE, 0.25);

    let (min, span) = field.bounds();
    let carried = crate::render::aspectus::MistView {
        tint: cool.extend(strength),
        sunward: warm.extend(low_sun),
        planet: crate::globe::planet_center().extend(crate::terrain::PLANET_RADIUS),
        field: Vec4::new(min.x, min.y, span, MIST_DEPTH),
        dials: Vec4::new(
            REACH,
            HEIGHT_RANGE,
            if std::env::var("DIVUS_FACTUS_MIST_DEBUG").is_ok_and(|dial| dial == "1") {
                1.0
            } else {
                0.0
            },
            THICKEST,
        ),
        sun: sky.sun_direction.extend(0.0),
    };
    for camera in &cameras {
        commands.entity(camera).insert((
            carried.clone(),
            crate::render::aspectus::MistFieldImage(field.image.clone()),
        ));
    }
}

/// A warm-neutral white, well away from the veil's slate. Not pure white:
/// nothing else in this game is, and a pure white mist read as a hole.
const BONE: Vec3 = Vec3::new(0.86, 0.83, 0.76);

/// How thick the mist gets at its very thickest, before the hour and the
/// altitude take their share.
const STRENGTH: f32 = 0.06;

/// The most the mist may ever hide.
///
/// A CEILING, and a low one, because this is a low-poly world whose whole read
/// depends on neighboring palette steps staying apart on flat facets. Two
/// steps of the grass ramp are close enough that a heavy wash merges them, and
/// a misty valley that has lost its faceting has stopped looking like this
/// game. Depth still stacks up to here; it simply never gets to erase the
/// drawing.
const THICKEST: f32 = 0.62;

/// How far down the ray the march looks, in meters.
///
/// Not to the horizon: mist is a local thing, and a march that reached the far
/// distance would be both slow and the distance fog this game deliberately
/// deleted. Far enough to fill the valley in front of the player and no
/// further.
const REACH: f32 = 850.0;

fn lay_out_the_field(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    // Rgba8: mist weight in R, ground height in G (normalized over the terrain's
    // own range), and two channels spare. Unorm rather than Srgb - these are
    // NUMBERS, and an sRGB curve on them would bend the mist's falloff.
    let blank = vec![0u8; (FIELD_SIDE * FIELD_SIDE * 4) as usize];
    let mut image = Image::new(
        Extent3d {
            width: FIELD_SIDE,
            height: FIELD_SIDE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        blank,
        TextureFormat::Rgba8Unorm,
        bevy::asset::RenderAssetUsages::MAIN_WORLD | bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    // Sampled with a linear filter and clamped: the field has an edge, and
    // wrapping it would put the far side of the world in the near distance.
    image.sampler = bevy::image::ImageSampler::Descriptor(bevy::image::ImageSamplerDescriptor {
        address_mode_u: bevy::image::ImageAddressMode::ClampToEdge,
        address_mode_v: bevy::image::ImageAddressMode::ClampToEdge,
        mag_filter: bevy::image::ImageFilterMode::Linear,
        min_filter: bevy::image::ImageFilterMode::Linear,
        ..default()
    });

    commands.insert_resource(MistField {
        image: images.add(image),
        // Deliberately far from anywhere the player will start, so the first
        // bake is triggered by the wander check on the first frame that has a
        // camera rather than by a special case.
        center: Vec2::splat(f32::MAX / 4.0),
        baking_toward: Vec2::ZERO,
        rows_done: 0,
        scratch: Vec::new(),
        baking: false,
    });
}

/// Bakes the field a slice at a time, following the camera.
fn bake_the_field(
    mut field: ResMut<MistField>,
    mut images: ResMut<Assets<Image>>,
    land: Option<Res<Terrain>>,
    rigs: Query<&crate::camera::CameraRig>,
) {
    let Some(land) = land else {
        return;
    };
    let Ok(rig) = rigs.single() else {
        return;
    };
    let looking_at = Vec2::new(rig.focus.x, rig.focus.z);

    if !field.baking {
        // Nothing to do until the player has walked far enough that the field
        // no longer covers what they can see.
        if field.center.distance(looking_at) < WANDER {
            return;
        }
        field.baking = true;
        field.baking_toward = looking_at;
        field.rows_done = 0;
        field.scratch = vec![0u8; (FIELD_SIDE * FIELD_SIDE * 4) as usize];
    }

    let step = FIELD_SPAN / FIELD_SIDE as f32;
    let min = field.baking_toward - Vec2::splat(FIELD_SPAN * 0.5);
    let rows_per_frame = FIELD_SIDE.div_ceil(BAKE_FRAMES);
    let until = (field.rows_done + rows_per_frame).min(FIELD_SIDE);

    for row in field.rows_done..until {
        let z = min.y + row as f32 * step;
        for column in 0..FIELD_SIDE {
            let x = min.x + column as f32 * step;
            // BASE HEIGHT, NOT `height_at`, AND THIS IS LOAD-BEARING.
            // `height_at` asks the river index for a channel, and the index
            // SOLVES any region it has not seen - taking a write lock while it
            // does, which every chunk mesh and every walking villager in the
            // game then waits on. A bake triggered by the camera reaching new
            // country is a bake over exactly the ground the index has never
            // seen: measured at 38 microseconds a cell cold against half a
            // microsecond warm, which is a hundred and fifty milliseconds of
            // frozen game at the worst possible moment. `base_height_at` is
            // noise and nothing else - no lock, no rivers, no walking the list
            // of every building pad in the world, and the same cost on the
            // hundredth call as the first.
            let height = land.base_height_at(x, z);
            let climate = mist_climate(&land, x, z, height);
            let carried = (height / HEIGHT_RANGE).clamp(0.0, 1.0);
            let at = ((row * FIELD_SIDE + column) * 4) as usize;
            // R holds the climate for now. The pooling cannot be worked out
            // until the neighbors exist, so it is applied in a second pass
            // once every row is down.
            field.scratch[at] = (climate * 255.0) as u8;
            field.scratch[at + 1] = (carried * 255.0) as u8;
        }
    }
    field.rows_done = until;

    if field.rows_done >= FIELD_SIDE {
        settle_the_mist(&mut field.scratch);
        // Swapped in whole. A field written into the live image row by row
        // would show the player a seam between the country they were in and
        // the country they are in, traveling up the screen.
        let baked = std::mem::take(&mut field.scratch);
        if let Some(mut image) = images.get_mut(&field.image) {
            image.data = Some(baked);
        }
        field.center = field.baking_toward;
        field.baking = false;
    }
}

/// Second pass over a finished bake: lets the mist find the low ground.
///
/// Each cell looks around itself for the lowest ground in reach and keeps only
/// as much mist as its height above THAT allows. This is what turns a map of
/// damp country into a map of mist: the climate says which valleys could have
/// fog, and this says where in them it lies.
///
/// Done here rather than per-cell during the bake because a cell cannot know
/// its neighbors until they exist - and done on the scratch copy, so the field
/// the shader is reading never holds a half-settled state.
fn settle_the_mist(scratch: &mut [u8]) {
    let side = FIELD_SIDE as i32;
    // The heights are read from the same buffer that is being written, so the
    // floors are all found first against untouched data.
    let mut floors = vec![0u8; (FIELD_SIDE * FIELD_SIDE) as usize];
    for row in 0..side {
        for column in 0..side {
            let mut lowest = u8::MAX;
            for dz in -FLOOR_REACH..=FLOOR_REACH {
                for dx in -FLOOR_REACH..=FLOOR_REACH {
                    let nz = (row + dz).clamp(0, side - 1);
                    let nx = (column + dx).clamp(0, side - 1);
                    let at = ((nz * side + nx) * 4 + 1) as usize;
                    lowest = lowest.min(scratch[at]);
                }
            }
            floors[(row * side + column) as usize] = lowest;
        }
    }

    for cell in 0..(FIELD_SIDE * FIELD_SIDE) as usize {
        let height = scratch[cell * 4 + 1] as f32 / 255.0 * HEIGHT_RANGE;
        let floor = floors[cell] as f32 / 255.0 * HEIGHT_RANGE;
        let settled = scratch[cell * 4] as f32 / 255.0 * pooling(height - floor);
        scratch[cell * 4] = (settled * 255.0) as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mist is a thing that POOLS, and this is the term that makes it one.
    ///
    /// Removed, the same damp country carried mist up every hill it had, and
    /// 86% of all land in three measured worlds came out foggy. What matters is
    /// not that high ground has less - it is that ground standing well clear of
    /// its own surroundings has NONE.
    #[test]
    fn mist_lies_in_the_low_ground() {
        assert_eq!(pooling(0.0), 1.0, "the floor of a hollow is full of it");
        assert_eq!(
            pooling(POOLS_BELOW),
            0.0,
            "clear of the hollow entirely, there is none - not a little, none",
        );
        assert_eq!(pooling(POOLS_BELOW * 4.0), 0.0, "and none further up");
        let low = pooling(POOLS_BELOW * 0.25);
        let high = pooling(POOLS_BELOW * 0.75);
        assert!(
            low > high,
            "it must thin all the way up, not step: {low} then {high}",
        );
    }

    /// The measured law: mist belongs to particular country, not to everywhere.
    ///
    /// The first law put mist on 86% of land at a mean weight of 0.57, which is
    /// fog everywhere, which is fog nowhere. This walks a whole world and holds
    /// the line: most land is clear, and the mist that exists is somewhere.
    #[test]
    fn most_of_the_world_is_clear_air() {
        let land = Terrain::new(7);
        let (mut land_cells, mut damp_air) = (0, 0);
        for step in 0..10_000 {
            let x = (step % 100) as f32 * 180.0 - 9000.0;
            let z = (step / 100) as f32 * 180.0 - 9000.0;
            let height = land.base_height_at(x, z);
            if height < WATER_LEVEL {
                continue;
            }
            land_cells += 1;
            if mist_climate(&land, x, z, height) > 0.5 {
                damp_air += 1;
            }
        }
        assert!(land_cells > 500, "a world should have land in it");
        let share = damp_air as f32 / land_cells as f32;
        assert!(
            share < 0.55,
            "mist-bearing air must be particular country, not the default: {:.0}% of land",
            share * 100.0,
        );
    }

    /// The desert does not steam.
    ///
    /// Asked of the REAL desert - ground the game itself calls `Arid` - rather
    /// than of thresholds picked to make the point, which is what the first
    /// version of this did, and it found no such ground in a whole world.
    #[test]
    fn the_desert_does_not_steam() {
        let land = Terrain::new(7);
        let mut checked = 0;
        for step in 0..14_400 {
            let x = (step % 120) as f32 * 150.0 - 9000.0;
            let z = (step / 120) as f32 * 150.0 - 9000.0;
            let height = land.base_height_at(x, z);
            if height < WATER_LEVEL {
                continue;
            }
            if land.biome_for(x, z, height) != crate::terrain::Biome::Arid {
                continue;
            }
            checked += 1;
            assert_eq!(
                mist_climate(&land, x, z, height),
                0.0,
                "desert ground at ({x}, {z}) should be clear air",
            );
        }
        assert!(checked > 0, "a whole world should hold some desert");
    }

    /// Under the water there is nothing to pool on.
    #[test]
    fn the_open_sea_carries_no_mist() {
        let land = Terrain::new(7);
        assert_eq!(mist_climate(&land, 0.0, 0.0, WATER_LEVEL - 30.0), 0.0);
    }

    /// A DAY IS NOT A MIRROR. The whole reason `burn_off` reads the clock
    /// rather than the sun's height: elevation is symmetric about noon, so a
    /// mist driven by it stands as thick at six in the evening as at six in the
    /// morning, which nobody has ever seen.
    #[test]
    fn the_morning_is_mistier_than_the_evening() {
        let dawn = burn_off(6.0 / 24.0);
        let dusk = burn_off(18.0 / 24.0);
        assert!(
            dawn > dusk * 2.0,
            "dawn must be decisively the mistier hour: {dawn} against {dusk}",
        );
        assert!(
            burn_off(13.0 / 24.0) < 0.2,
            "the middle of the day is all but clear",
        );
        assert!(
            burn_off(3.0 / 24.0) > 0.9,
            "the small hours gather it",
        );
    }

    /// The curve must not jump. A morning that clears between one frame and the
    /// next reads as a switch being thrown rather than as the sun coming up.
    #[test]
    fn the_mist_never_snaps() {
        let mut worst: f32 = 0.0;
        let mut last = burn_off(0.0);
        for tick in 1..=2_400 {
            let now = burn_off(tick as f32 / 2_400.0);
            worst = worst.max((now - last).abs());
            last = now;
        }
        assert!(
            worst < 0.02,
            "the thickest step across a whole day was {worst}, which the eye would catch",
        );
    }
}
