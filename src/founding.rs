//! The first act: an empty world, a flag in the god's hand, and the
//! choosing of ground.
//!
//! Nothing has been founded when this phase begins. The player sweeps the
//! land, the flag reads what is under it — how wooded, whether there is
//! rock in a working walk, whether it will take a village at all — and
//! where they put it, the village is born.
//!
//! Every instrument is locked here without a line of code to lock it:
//! the codex, the miracles, the markers, the survey and the debug hud are
//! all gated on [`GameState::Playing`], and this is not it. There is no
//! chronicle to read and nothing to survey until somebody plants.

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::text::FontSize;
use std::f32::consts::FRAC_PI_2;

use crate::GameState;
use crate::palette;
use crate::terrain::Terrain;
use crate::villager::{ChosenGround, reckon_ground, will_take_a_village};

/// The likeliest ground in the world, found once at startup.
///
/// The old site search still runs — it just no longer FOUNDS anything.
/// It picks where the opening dive comes down, so the god always arrives
/// standing over good land instead of somewhere at random (which, on the
/// first world tried, was open water). From there the flag can go
/// straight in, or the player can go looking for somewhere they like
/// better. The machine offers; it no longer decides.
#[derive(Resource)]
pub struct OpeningVantage(pub Vec3);

/// What the ground under the flag is, refreshed as it moves.
#[derive(Resource, Default)]
pub struct GroundUnderTheFlag {
    /// Where the cursor meets the land, if it does.
    pub at: Option<Vec3>,
    /// Why this ground will not take a village, if it will not.
    pub refusal: Option<&'static str>,
    /// How much of a working walk bears wood, and rock, 0 to 1.
    pub timberland: f32,
    pub stoneland: f32,
    /// Whether there is shore within a working walk.
    pub shore: bool,
    /// Where the reckoning above was taken. The survey walks a hundred
    /// and fifty terrain samples; it is not worth retaking for a cursor
    /// that has shifted a handspan.
    reckoned_at: Option<Vec3>,
}

impl GroundUnderTheFlag {
    /// Whether the flag would go in here.
    pub fn will_take_it(&self) -> bool {
        self.at.is_some() && self.refusal.is_none()
    }
}

/// The flag the god carries, and the parts of it that take a colour.
#[derive(Component)]
struct TheFlag {
    cloth: Vec<Entity>,
}

/// The line of text under the flag, saying what the ground is.
#[derive(Component)]
struct GroundReadout;

/// How far the cursor must move before the reckoning is retaken.
const RESURVEY: f32 = 4.0;

pub struct FoundingPlugin;

impl Plugin for FoundingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GroundUnderTheFlag>()
            // Not at Startup: the land itself is inserted by another
            // startup system, and a survey that runs first finds no
            // terrain, says nothing, and drops the opening dive on the
            // world origin - which on the first world tried was open
            // ocean. It runs until it succeeds instead.
            .add_systems(
                Update,
                survey_the_land.run_if(not(resource_exists::<OpeningVantage>)),
            )
            .add_systems(
                Update,
                (
                    read_the_ground,
                    carry_the_flag,
                    say_the_ground,
                    plant_the_flag,
                )
                    .chain()
                    .run_if(in_state(GameState::Choosing)),
            )
            .add_systems(
                OnEnter(GameState::Choosing),
                (raise_the_flag, plant_it_unattended).chain(),
            )
            .add_systems(OnExit(GameState::Choosing), put_the_flag_away);
    }
}

/// Finds the likeliest ground in the world, so the opening dive has
/// somewhere worth coming down to.
fn survey_the_land(
    mut commands: Commands,
    terrain: Option<Res<Terrain>>,
    seed: Option<Res<crate::WorldSeed>>,
    mut rigs: Query<&mut crate::camera::CameraRig>,
) {
    let (Some(terrain), Some(seed)) = (terrain, seed) else {
        return;
    };
    let mut rng = crate::rng::Rng::stream(seed.0 as u64, "settlement");
    let at = crate::villager::choose_settlement_site(&terrain, &mut rng);
    info!(
        "the land was surveyed: the likeliest ground lies at {:.0}, {:.0}",
        at.x, at.z
    );
    commands.insert_resource(OpeningVantage(at));

    // And the title drifts over THAT ground, not over the world origin.
    // The opening framing used to be a PostStartup pass aimed at the
    // settlement, so the title always hung above the village and Begin
    // was a short drop straight down onto it. With the founding moved
    // into the player's hands there is no village to aim at - and
    // without this the title sat over open ocean and Begin flew the
    // whole map sideways to get anywhere worth standing.
    if let Ok(mut rig) = rigs.single_mut() {
        rig.focus = at;
        rig.target_focus = at;
    }
}

/// A pole, a crossarm and a drop of cloth — the town banner, before there
/// is a town to raise it over.
/// Where the pole sits in the closed fist, in the hand's own space.
///
/// The fingers curl about the hand's local X, so the tube a fist makes has
/// its axis along X and a held shaft runs down it. With the carrying roll on,
/// local -X is the way up, which is why the flag is turned to lie along it.
const GRIP: Vec3 = Vec3::new(0.0, -0.30, -0.62);

/// How far up the shaft the hand takes it. Low, the way anybody carries a
/// standard - hold it at the middle and the cloth is over your head.
const HELD_UP_THE_POLE: f32 = 1.0;

/// The flag's own size inside the fist. The hand's scale is inherited, so
/// this is a fraction of it rather than a size in metres - which is the point:
/// the flag now grows and shrinks WITH the hand instead of staying
/// human-sized in a god's grip.
const FLAG_IN_HAND: f32 = 0.9;

fn raise_the_flag(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    hands: Query<Entity, With<crate::hand::HandModel>>,
) {
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let wood = materials.add(StandardMaterial {
        base_color: palette::shade(&palette::WOOD, 0.55),
        perceptual_roughness: 0.9,
        ..default()
    });
    let cloth_stuff = materials.add(StandardMaterial {
        base_color: palette::shade(&palette::CLOTH_GOLD, 0.85),
        perceptual_roughness: 0.85,
        cull_mode: None,
        ..default()
    });

    // Hung off the hand itself, not placed in the world beside it. Setting a
    // world position every frame looked attached only while the cursor was
    // still: the hand's own position is SMOOTHED toward the cursor and its
    // rotation carries pitch, bank and sway, none of which a world-placed flag
    // knew anything about. Move the mouse quickly and the two came apart;
    // tilt the hand and the flag stayed stubbornly upright. As a child it
    // inherits every one of those for nothing.
    let held = Transform::from_translation(GRIP + Vec3::X * HELD_UP_THE_POLE)
        .with_rotation(Quat::from_rotation_z(FRAC_PI_2))
        .with_scale(Vec3::splat(FLAG_IN_HAND));
    // `HandPart` because it rides in seated space with the hand: the bend must
    // leave everything hanging off the god's hand exactly where the hand put it.
    let mut flag_bundle = commands.spawn((
        Name::new("The founding flag"),
        crate::hand::HandPart,
        held,
        Visibility::Hidden,
    ));
    if let Ok(hand) = hands.single() {
        flag_bundle.insert(ChildOf(hand));
    }
    let flag = flag_bundle.id();
    // On the HAND's render layer, not the world's. The god's hand is drawn by
    // an overlay camera above everything else so a cursor can never be
    // occluded - which meant the pole, an ordinary world object, was drawn in
    // the pass UNDERNEATH it and vanished behind the fist holding it. The
    // shaft runs through the grip and a metre past it, and not one pixel of it
    // could be seen. Same pass as the hand, and the two sort against each
    // other properly.
    //
    // Nothing is lost by it: this flag exists only while the ground is being
    // chosen, and is put away the moment it is planted, so it never needs to
    // hide behind a hill.
    let mut part = |offset: Vec3, size: Vec3, stuff: &Handle<StandardMaterial>| -> Entity {
        commands
            .spawn((
                Mesh3d(cube.clone()),
                MeshMaterial3d(stuff.clone()),
                Transform::from_translation(offset).with_scale(size),
                RenderLayers::layer(crate::render::HAND_LAYER),
                crate::hand::HandPart,
                ChildOf(flag),
            ))
            .id()
    };
    // The pole stands in the ground; the cloth hangs from a crossarm at
    // the head of it, the way the town's own banner does.
    part(Vec3::new(0.0, 1.6, 0.0), Vec3::new(0.14, 3.2, 0.14), &wood);
    part(Vec3::new(0.45, 3.1, 0.0), Vec3::new(1.0, 0.12, 0.12), &wood);
    let cloth = vec![part(
        Vec3::new(0.5, 2.45, 0.0),
        Vec3::new(0.9, 1.2, 0.06),
        &cloth_stuff,
    )];
    commands.entity(flag).insert(TheFlag { cloth });

    // And the words under it.
    commands.spawn((
        GroundReadout,
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(15.0),
            ..default()
        },
        TextColor(palette::shade(&palette::BONE, 0.95)),
        TextLayout {
            justify: Justify::Center,
            ..default()
        },
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(72.0),
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            ..default()
        },
        GlobalZIndex(6),
    ));
}

fn put_the_flag_away(
    mut commands: Commands,
    flags: Query<Entity, With<TheFlag>>,
    words: Query<Entity, With<GroundReadout>>,
) {
    for gone in flags.iter().chain(words.iter()) {
        commands.entity(gone).despawn();
    }
}

/// The flag reads the ground it is over, every frame it moves.
fn read_the_ground(
    terrain: Option<Res<Terrain>>,
    hand: Option<Res<crate::hand::DivineHand>>,
    mut reading: ResMut<GroundUnderTheFlag>,
) {
    let (Some(terrain), Some(hand)) = (terrain, hand) else {
        return;
    };
    reading.at = hand.cursor_world;
    reading.refusal = hand
        .cursor_world
        .and_then(|at| will_take_a_village(&terrain, at.x, at.z));

    // The materials reckoning is a hundred and fifty terrain samples;
    // retaking it every frame for a cursor that has barely moved is
    // waste, and a few strides is finer than the ground itself varies.
    let Some(at) = hand.cursor_world else {
        return;
    };
    if reading
        .reckoned_at
        .is_some_and(|was| was.distance(at) < RESURVEY)
    {
        return;
    }
    let (timberland, stoneland, shore) = reckon_ground(&terrain, at.x, at.z);
    reading.timberland = timberland;
    reading.stoneland = stoneland;
    reading.shore = shore;
    reading.reckoned_at = Some(at);
}

/// The flag follows the cursor, and goes red over ground that will not
/// have it — so a refusal is something the god SEES while sweeping,
/// rather than something they are told after committing to it.
fn carry_the_flag(
    reading: Res<GroundUnderTheFlag>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut flags: Query<(&TheFlag, &mut Visibility)>,
    cloth: Query<&MeshMaterial3d<StandardMaterial>>,
) {
    let Ok((flag, mut showing)) = flags.single_mut() else {
        return;
    };
    // Where it IS is the hand's business now - it hangs off the fist. All this
    // decides is whether there is a flag to see and what colour it reads.
    if reading.at.is_none() {
        *showing = Visibility::Hidden;
        return;
    }
    *showing = Visibility::Inherited;

    let colour = if reading.refusal.is_some() {
        palette::shade(&palette::CLOTH_RED, 0.55)
    } else {
        palette::shade(&palette::CLOTH_GOLD, 0.85)
    };
    for piece in &flag.cloth {
        if let Ok(stuff) = cloth.get(*piece)
            && let Some(mut stuff) = materials.get_mut(&stuff.0)
            && stuff.base_color != colour
        {
            stuff.base_color = colour;
        }
    }
}

/// What the ground is, in the founders' own terms.
fn say_the_ground(
    reading: Res<GroundUnderTheFlag>,
    mut words: Query<(&mut Text, &mut TextColor), With<GroundReadout>>,
) {
    let Ok((mut text, mut ink)) = words.single_mut() else {
        return;
    };
    let (said, colour) = match (reading.at, reading.refusal) {
        (None, _) => (String::new(), palette::shade(&palette::BONE, 0.95)),
        (Some(_), Some(refusal)) => (
            refusal.to_string(),
            palette::shade(&palette::CLOTH_RED, 0.75),
        ),
        (Some(_), None) => {
            let wood = match reading.timberland {
                t if t > 0.66 => "deep woods",
                t if t > 0.33 => "trees enough",
                t if t > 0.05 => "thin woods",
                _ => "no timber in reach",
            };
            let rock = match reading.stoneland {
                s if s > 0.5 => "stone in the rises",
                s if s > 0.1 => "some rock",
                _ => "no rock in reach",
            };
            let water = if reading.shore { ", water" } else { "" };
            (
                format!("{wood}, {rock}{water}"),
                palette::shade(&palette::BONE, 0.95),
            )
        }
    };
    if text.0 != said {
        *text = Text::new(said);
    }
    if ink.0 != colour {
        *ink = TextColor(colour);
    }
}

/// The flag goes in, and the world begins.
fn plant_the_flag(
    mouse: Res<ButtonInput<MouseButton>>,
    scheme: Res<crate::keymap::MouseScheme>,
    reading: Res<GroundUnderTheFlag>,
    mut chosen: ResMut<ChosenGround>,
    mut next: ResMut<NextState<GameState>>,
) {
    // The ACTION button, like every act of the god's will - planting the
    // flag is the first one. The left button keeps its own job even here:
    // grab the land, turn the world, look for better ground.
    if !mouse.just_pressed(scheme.action()) {
        return;
    }
    // Refused ground simply will not take it. The flag has been reading
    // red the whole time the cursor was over it and the words beneath
    // have been saying why, so there is nothing left to announce.
    let Some(at) = reading.at.filter(|_| reading.will_take_it()) else {
        return;
    };
    found_here(at, &mut chosen, &mut next);
}

/// Plants the flag with nobody at the keyboard, on the best ground the
/// old site search can find.
///
/// Every headless soak in the project presses Begin and expects a village
/// to exist; with the founding moved into the player's hands, that stops
/// being true and the whole verification harness goes dark. `1` takes the
/// search's own answer.
fn plant_it_unattended(
    vantage: Option<Res<OpeningVantage>>,
    mut chosen: ResMut<ChosenGround>,
    mut next: ResMut<NextState<GameState>>,
) {
    info!("the world is made, and nobody is in it: the flag is in your hand");
    if std::env::var("DIVUS_FACTUS_AUTOPLANT").is_err() {
        return;
    }
    let Some(vantage) = vantage else {
        warn!("nothing to plant in: the land was never surveyed");
        return;
    };
    info!("the flag was planted unattended, on the ground the survey liked best");
    found_here(vantage.0, &mut chosen, &mut next);
}

fn found_here(at: Vec3, chosen: &mut ChosenGround, next: &mut NextState<GameState>) {
    info!("the flag goes in at {:.0}, {:.0}", at.x, at.z);
    chosen.0 = Some(at);
    next.set(GameState::Playing);
}
