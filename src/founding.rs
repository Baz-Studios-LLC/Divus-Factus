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

use bevy::prelude::*;

use crate::GameState;
use crate::terrain::Terrain;
use crate::villager::{ChosenGround, will_take_a_village};

/// What the ground under the flag is, refreshed as it moves.
#[derive(Resource, Default)]
pub struct GroundUnderTheFlag {
    /// Where the cursor meets the land, if it does.
    pub at: Option<Vec3>,
    /// Why this ground will not take a village, if it will not.
    pub refusal: Option<&'static str>,
}

pub struct FoundingPlugin;

impl Plugin for FoundingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GroundUnderTheFlag>()
            .add_systems(
                Update,
                (read_the_ground, plant_the_flag)
                    .chain()
                    .run_if(in_state(GameState::Choosing)),
            )
            .add_systems(OnEnter(GameState::Choosing), plant_it_unattended);
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
}

/// The flag goes in, and the world begins.
fn plant_the_flag(
    mouse: Res<ButtonInput<MouseButton>>,
    reading: Res<GroundUnderTheFlag>,
    mut chosen: ResMut<ChosenGround>,
    mut next: ResMut<NextState<GameState>>,
    mut notices: MessageWriter<crate::ui::Notice>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(at) = reading.at else {
        // The cursor is off the world - open sky, or over a panel.
        return;
    };
    if let Some(refusal) = reading.refusal {
        // Refused, and told why. The ghost should already have been
        // reading red before the click ever came - this is the backstop,
        // not the message.
        notices.write(crate::ui::Notice::new(format!(
            "The flag will not stand here: {refusal}"
        )));
        return;
    }
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
    terrain: Option<Res<Terrain>>,
    seed: Option<Res<crate::WorldSeed>>,
    mut chosen: ResMut<ChosenGround>,
    mut next: ResMut<NextState<GameState>>,
) {
    info!("the world is made, and nobody is in it: the flag is in your hand");
    if std::env::var("DIVUS_FACTUS_AUTOPLANT").is_err() {
        return;
    }
    let (Some(terrain), Some(seed)) = (terrain, seed) else {
        warn!("nothing to plant in: the land itself is not ready");
        return;
    };
    let mut rng = crate::rng::Rng::stream(seed.0 as u64, "settlement");
    let at = crate::villager::choose_settlement_site(&terrain, &mut rng);
    info!("the flag was planted unattended, on the ground the search liked best");
    found_here(at, &mut chosen, &mut next);
}

fn found_here(at: Vec3, chosen: &mut ChosenGround, next: &mut NextState<GameState>) {
    info!("the flag goes in at {:.0}, {:.0}", at.x, at.z);
    chosen.0 = Some(at);
    next.set(GameState::Playing);
}
