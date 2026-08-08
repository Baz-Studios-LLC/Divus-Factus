//! The first contact between Divus Factus and Ordo.
//!
//! `DIVUS_FACTUS_ORDO=1` puts two panels on screen carrying the SAME content:
//! one built out of Ordo's widgets, one hand-built the way every panel in this
//! game is built today. Side by side, in the same frame, under the same light.
//!
//! The point is not to convert the interface. It is to find out what is wrong
//! with Ordo while it is still cheap to change - it has never been used by a
//! real game, and this is the first one. See `../Ordo/FIRST-PORT.md`.
//!
//! Everything here is scaffolding and should be deleted the day the answer is
//! known, which is why it is one file behind one dial and touches nothing else.

use bevy::ecs::schedule::common_conditions::run_once;
use bevy::prelude::*;
use ordo::prelude::*;

use crate::ui::{Anchor as DfAnchor, theme};

/// Whether the trial is wanted at all.
pub fn asked_for() -> bool {
    std::env::var("DIVUS_FACTUS_ORDO").is_ok_and(|dial| dial != "0")
}

pub struct OrdoTrialPlugin;

impl Plugin for OrdoTrialPlugin {
    fn build(&self, app: &mut App) {
        if !asked_for() {
            return;
        }
        // OrdoPlugin and the ramps are the GAME's now, added in `ui` for
        // every run - the day the kit stopped being a trial. Only the
        // side-by-side panels remain behind the dial.
        //
        // NOT at Startup: `Fonts` is loaded a beat later, so a panel raised
        // there dies asking for it. Ordo's own widgets do not care - they
        // carry a `Face` tag and the repaint pass finds the font whenever it
        // arrives - which is the first thing the port has said in Ordo's
        // favour, and it said it by killing the hand-built twin.
        app.add_systems(
            Update,
            raise_the_pair
                .run_if(resource_exists::<crate::ui::Fonts>)
                .run_if(run_once),
        );
    }
}

/// The same panel twice: Ordo's on the left, the game's own on the right.
fn raise_the_pair(mut commands: Commands, fonts: Res<crate::ui::Fonts>) {
    // ---- Ordo's -------------------------------------------------------
    commands.spawn((
        panel(Anchor::TopLeft, Some(260.0)),
        children![
            heading("The Village"),
            (row(), children![label("Believers"), body("1,204")]),
            (row(), children![label("Timber"), body("86")]),
            (row(), children![label("Faith"), dim("rising")]),
            button("Dismiss"),
        ],
    ));

    // ---- and a WINDOW, which Ordo did not have until now --------------
    //
    // Lifted out of this game, because the rule is that anything the interface
    // needs and Ordo lacks gets built IN ORDO. Drag it by the title bar, shut
    // it with the cross, and clicking any part of it brings it to the front.
    //
    // Note what the caller writes: the body, and nothing else. The title bar
    // and the close button are put on afterwards by a pass, the same way paint
    // is - which is what lets a window be an ordinary `children!` spawn like
    // every other widget here.
    commands.spawn((
        ordo::window::window("The Ledger", Anchor::BottomLeft, 300.0),
        children![
            (row(), children![label("Founded"), body("Spring 1")]),
            (row(), children![label("Souls"), body("14")]),
            (row(), children![label("Mood"), dim("content")]),
        ],
    ));

    // ---- and the game's own, by hand ----------------------------------
    //
    // Written the way every panel in this game is written, so what is being
    // compared is the two ways of saying it rather than two different panels.
    let mine = crate::ui::panel(&mut commands, DfAnchor::TopRight, None, Some(260.0)).root;
    let title = commands
        .spawn((
            Text::new("The Village"),
            TextFont {
                font: fonts.display.clone().into(),
                font_size: bevy::text::FontSize::Px(theme::TITLE_SIZE),
                ..default()
            },
            TextColor(theme::accent()),
            ChildOf(mine),
        ))
        .id();
    let _ = title;
    for (name, value, dimmed) in [
        ("Believers", "1,204", false),
        ("Timber", "86", false),
        ("Faith", "rising", true),
    ] {
        let line = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    min_height: Val::Px(22.0),
                    ..default()
                },
                ChildOf(mine),
            ))
            .id();
        commands.spawn((
            Text::new(name),
            TextFont {
                font: fonts.text.clone().into(),
                font_size: bevy::text::FontSize::Px(theme::BODY_SIZE),
                ..default()
            },
            TextColor(theme::text_dim()),
            Node {
                width: Val::Px(theme::LABEL_WIDTH),
                ..default()
            },
            ChildOf(line),
        ));
        commands.spawn((
            Text::new(value),
            TextFont {
                font: fonts.text.clone().into(),
                font_size: bevy::text::FontSize::Px(if dimmed {
                    theme::SMALL_SIZE
                } else {
                    theme::BODY_SIZE
                }),
                ..default()
            },
            TextColor(if dimmed {
                theme::text_dim()
            } else {
                theme::text()
            }),
            ChildOf(line),
        ));
    }
}
