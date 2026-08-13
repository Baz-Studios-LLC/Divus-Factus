//! The town strip: who and what the settlement under your eye is made of.
//!
//! Four numbers and a name, across the top of the screen. Brett: "I want a
//! little hud at the top of the screen with some basic information about
//! the town you are looking at. Souls - Timber - Stone - Food ... Maybe
//! with an icon on the left of the number ... Maybe the town name as well".
//!
//! It answers the town you are LOOKING AT, which is a question with two
//! halves: the banner has to be on the screen, and you have to be close
//! enough for the place to be a place rather than a patch of ground. Fail
//! either and the strip goes, rather than reporting on somewhere you
//! cannot see.

use bevy::prelude::*;
use bevy::ui::{
    FlexDirection, GlobalZIndex, JustifyContent, Node, PositionType, Val::Px as px,
};

use super::theme;

/// The strip itself.
#[derive(Component)]
pub(crate) struct TownStrip;

/// The town's name, above the numbers.
#[derive(Component)]
pub(crate) struct TownStripName;

/// One of the four readings, in the order they are built.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TownReading {
    Souls,
    Timber,
    Stone,
    Food,
}

/// How high the eye may be before a town stops being a town.
///
/// The same reasoning as the talk bubbles, and a little more generous:
/// from up here you are looking at country rather than at anybody's home,
/// and a strip of numbers about one of the specks below is noise. Brett:
/// "When you are not looking at the town on the screen it goes away. Or if
/// you are zoomed out too far."
const TOO_HIGH_FOR_A_TOWN: f32 = 220.0;

/// How far outside the window a banner may sit and still count as looked
/// at, as a fraction of the window. A town whose square is a step off the
/// edge is one you are plainly standing in.
const EDGE_GRACE: f32 = 0.15;

pub(crate) fn spawn_town_strip(mut commands: Commands) {
    // The strip HANGS from the top edge, so it is centred by a full-width
    // rail rather than by guessing at a left margin.
    let rail = commands
        .spawn((
            Name::new("Town Strip"),
            TownStrip,
            Node {
                position_type: PositionType::Absolute,
                // ABOVE the top edge, by more than the panel's own corner
                // radius. A plate that stops short of the edge reads as a
                // box floating near the top; one whose top corners are off
                // the screen entirely reads as a tab fixed to it, which is
                // what it is. Brett: "have the panel slid up a bit so the
                // top edge is off the screen so that it looks like a tab on
                // the top instead of a floating box".
                top: px(-14.0),
                left: px(0.0),
                right: px(0.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::Center,
                ..default()
            },
            // Above the world, below the book: this is a glance, and it
            // must never sit over a page somebody has opened.
            GlobalZIndex(60),
            Visibility::Hidden,
        ))
        .id();

    // The plate and its cells come from ORDO, not from colours picked
    // here. Brett: "Make sure you use ordo for this... add to ordo if it
    // is missing something" - and it was missing both of these, so they
    // were cut into the kit where the next game can wear them too.
    let strip = commands.spawn((ordo::hanging_rail(), ChildOf(rail))).id();
    // Ordo's padding is cut for a page; a tab wants less air, and its top
    // half is off the screen anyway.
    commands
        .entity(strip)
        .entry::<Node>()
        .and_modify(|mut node| {
            node.padding = bevy::ui::UiRect {
                left: px(8.0),
                right: px(8.0),
                // Fourteen of these are off the screen - the slide above -
                // so what a reader actually sees over the name is the
                // remainder. At sixteen that left two, and the name sat on
                // the edge it hangs from.
                top: px(26.0),
                bottom: px(7.0),
            };
            node.row_gap = px(2.0);
        });

    let name = commands
        .spawn((TownStripName, ordo::heading(""), ChildOf(strip)))
        .id();
    commands.entity(name).insert(super::DisplayFace);

    // `ordo::row` brings its own Node - a second one in the same bundle
    // is a duplicate component, and Bevy refuses those at spawn - so the
    // gap between the cells is set on the row it already has.
    let row = commands.spawn((ordo::row(), ChildOf(strip))).id();
    commands
        .entity(row)
        .entry::<Node>()
        .and_modify(|mut node| {
            node.justify_content = JustifyContent::Center;
            node.column_gap = px(4.0);
        });

    for reading in [
        TownReading::Souls,
        TownReading::Timber,
        TownReading::Stone,
        TownReading::Food,
    ] {
        // One width for all four, the way the bench cuts its mode
        // buttons: readings that step in and out as the eye runs along
        // them read as four unrelated things.
        let cell = commands
            .spawn((ordo::readout(72.0), ChildOf(row)))
            .id();
        let tint = theme::accent().with_alpha(0.85);
        match reading {
            TownReading::Souls => crate::debug::village::person_glyph(&mut commands, cell, tint),
            TownReading::Timber => crate::debug::village::tree_glyph(&mut commands, cell, tint),
            TownReading::Stone => crate::debug::village::stone_glyph(&mut commands, cell, tint),
            TownReading::Food => crate::debug::village::food_glyph(&mut commands, cell, tint),
        }
        commands.spawn((reading, ordo::body("0"), ChildOf(cell)));
    }
}

/// Fills the strip from whichever town is under the eye, and hides it when
/// none is.
#[allow(clippy::too_many_arguments)]
pub(crate) fn update_town_strip(
    time: Res<Time<Real>>,
    mut since: Local<f32>,
    state: Res<State<crate::GameState>>,
    cameras: Query<(&bevy::camera::Camera, &GlobalTransform)>,
    rigs: Query<&crate::camera::CameraRig>,
    towns: Query<(
        Entity,
        &crate::villager::Settlement,
        &crate::villager::SettlementGround,
        &crate::villager::work::Stockpile,
    )>,
    folk: Query<
        &crate::villager::MemberOf,
        (
            With<crate::villager::Villager>,
            Without<crate::creature::Corpse>,
        ),
    >,
    books: Query<&Visibility, With<crate::debug::village::VillagePanel>>,
    mut strip: Query<&mut Visibility, (With<TownStrip>, Without<crate::debug::village::VillagePanel>)>,
    mut name: Query<&mut Text, (With<TownStripName>, Without<TownReading>)>,
    mut readings: Query<(&TownReading, &mut Text)>,
) {
    let Ok(mut showing) = strip.single_mut() else {
        return;
    };
    // The book owns the screen when it is open; a strip peeking over the
    // top of a page is the interface talking over itself.
    let playing = matches!(state.get(), crate::GameState::Playing)
        && books.iter().all(|v| *v == Visibility::Hidden);

    let looked_at = playing
        .then(|| the_town_under_the_eye(&cameras, &rigs, &towns))
        .flatten();

    let fresh = if looked_at.is_some() {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    if *showing != fresh {
        *showing = fresh;
    }
    let Some(town) = looked_at else {
        return;
    };

    // The numbers themselves change slowly; a census every frame is a
    // whole village walked for a line of text nobody watches tick.
    *since += time.delta_secs();
    if *since < 0.4 {
        return;
    }
    *since = 0.0;

    let Ok((entity, settlement, _, store)) = towns.get(town) else {
        return;
    };
    let souls = folk.iter().filter(|member| member.0 == entity).count();

    if let Ok(mut text) = name.single_mut() {
        let called = settlement.name.to_uppercase();
        if text.0 != called {
            *text = Text::new(called);
        }
    }
    for (reading, mut text) in &mut readings {
        let fresh = match reading {
            TownReading::Souls => format!("{souls}"),
            TownReading::Timber => format!("{:.0}", store.timber),
            TownReading::Stone => format!("{:.0}", store.stone),
            TownReading::Food => format!("{:.0}", store.food()),
        };
        if text.0 != fresh {
            *text = Text::new(fresh);
        }
    }
}

/// Which town the eye is actually on, if any.
///
/// Two questions, and both have to answer yes. Is the camera low enough
/// for a town to read as a town, and is the banner on the screen? Where
/// more than one qualifies - villages do crowd, and colonies will crowd
/// harder - the nearest to the middle of the window wins, because that is
/// the one being looked at rather than merely in view.
fn the_town_under_the_eye(
    cameras: &Query<(&bevy::camera::Camera, &GlobalTransform)>,
    rigs: &Query<&crate::camera::CameraRig>,
    towns: &Query<(
        Entity,
        &crate::villager::Settlement,
        &crate::villager::SettlementGround,
        &crate::villager::work::Stockpile,
    )>,
) -> Option<Entity> {
    if rigs
        .iter()
        .next()
        .is_none_or(|rig| rig.distance >= TOO_HIGH_FOR_A_TOWN)
    {
        return None;
    }
    let (camera, camera_at) = cameras
        .iter()
        .find(|(camera, _)| camera.order == 0 && camera.is_active)?;
    let window = camera.logical_viewport_size()?;
    let middle = window * 0.5;
    let grace = window * EDGE_GRACE;

    towns
        .iter()
        .filter_map(|(entity, _, ground, _)| {
            // The banner stands in the WORLD, and the world is bent onto
            // the globe - so it is asked for where it is drawn, not where
            // the simulation keeps it.
            let (seat, _) = crate::globe::bend_frame(ground.centre);
            let at = camera.world_to_viewport(camera_at, seat).ok()?;
            let inside = at.x > -grace.x
                && at.y > -grace.y
                && at.x < window.x + grace.x
                && at.y < window.y + grace.y;
            inside.then(|| (entity, at.distance(middle)))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(entity, _)| entity)
}
