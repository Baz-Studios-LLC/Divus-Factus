//! THE MIRACLES: the spellbook.
//!
//! Brett: "The miracles page should show all of the abilities that you
//! can get and have a copy of the toolbar where you can slot them. Like a
//! spell book from WoW" — and, like WoW's book, the shelf is ICON AND
//! NAME alone, three to a row, with everything else in Ordo's floating
//! tooltip that rides the mouse. "We are going to need a lot more room
//! for more miracles." The header carries the belief ladder and a LIVE
//! MIRROR of the ten-slot bar, made of the same `MiracleSlot`s the HUD's
//! apron wears, so the dresser, the styler, the cooldown sweep and the
//! drag all serve it without knowing there are two bars. Entries go to
//! the bar by press (first empty slot) or by drag.
//!
//! THE DEITY page next door is for the god's own standing — and will one
//! day carry the progression tree, when belief-as-XP finds its shape.

use super::*;
use crate::miracles::{Miracle, MiracleCard};

/// The spellbook page's root: every is-the-page-open gate reads this.
#[derive(Component)]
pub(crate) struct SpellbookPanel;

/// An entry's name, dressed by its lock state.
#[derive(Component)]
pub(crate) struct MiracleName(Miracle);

/// Pressing a learned entry sets its miracle in the first empty slot —
/// the quick way; the drag is the precise one.
pub(crate) fn place_from_the_book(
    grimoire: Res<crate::miracles::Grimoire>,
    mut hotbar: ResMut<crate::miracles::Hotbar>,
    mut notices: MessageWriter<ui::Notice>,
    cards: Query<(&Interaction, &MiracleCard), Changed<Interaction>>,
) {
    for (interaction, card) in &cards {
        if *interaction != Interaction::Pressed || !grimoire.knows(card.0) {
            continue;
        }
        if hotbar.slot_of(card.0).is_some() {
            continue;
        }
        let before: usize = hotbar.0.iter().filter(|m| m.is_some()).count();
        hotbar.take_in(card.0);
        let after: usize = hotbar.0.iter().filter(|m| m.is_some()).count();
        if after > before {
            notices.write(ui::Notice::new(format!(
                "{} takes its place on the bar",
                card.0.name()
            )));
        } else {
            notices.write(ui::Notice::new(
                "The bar is full - drag a miracle off it to make room".to_string(),
            ));
        }
    }
}

pub(crate) fn spawn_spellbook_page(
    mut commands: Commands,
    codex: Res<super::village::Codex>,
    mut sweeps: ResMut<Assets<crate::miracles::CooldownSweep>>,
) {
    let page = codex.miracles_page;
    commands
        .entity(page)
        .insert((Name::new("Miracles Page"), SpellbookPanel));

    let leaf = ordo::page(&mut commands, page, super::village::RHYTHM);

    // The header: the ladder that earns the powers, and the bar that
    // carries them — always in reach while the shelf below grows.
    let head = commands
        .spawn((
            Node {
                width: percent(100),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(8),
                padding: UiRect::axes(px(0), px(4)),
                ..default()
            },
            ChildOf(leaf.header),
        ))
        .id();
    let meter_seat = commands
        .spawn((
            Node {
                width: px(460),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            ChildOf(head),
        ))
        .id();
    crate::miracles::raise_belief_meter(&mut commands, meter_seat);
    let bar = commands
        .spawn((
            Name::new("Spellbook Bar Mirror"),
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: px(6),
                padding: UiRect::all(px(5)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(6)),
                ..default()
            },
            BackgroundColor(ui::theme::panel_bg()),
            BorderColor::all(ui::theme::panel_border()),
            ChildOf(head),
        ))
        .id();
    crate::miracles::raise_bar_slots(&mut commands, &mut sweeps, bar);
    commands.spawn((
        ui::dim("press a miracle for the first empty slot - or drag it onto the bar"),
        ChildOf(head),
    ));

    // The shelf: every miracle there is, icon and name alone, three to a
    // row on the page grid. The rest of each entry's story lives in the
    // tooltip that rides the mouse.
    let all = Miracle::ALL;
    for third in all.chunks(3) {
        let grid_line = commands
            .spawn((ordo::grid_row(super::village::RHYTHM), ChildOf(leaf.body)))
            .id();
        commands
            .entity(grid_line)
            .entry::<Node>()
            .and_modify(|mut node| {
                node.flex_shrink = 0.0;
            });
        for &miracle in third {
            // The tract column stays BARE, exactly like the filler that
            // squares off a short row; the entry is a full-width child.
            let seat = commands
                .spawn((ordo::col(1, super::village::RHYTHM), ChildOf(grid_line)))
                .id();
            let entry = commands
                .spawn((
                    MiracleCard(miracle),
                    Interaction::default(),
                    ordo::Tooltip::new(miracle.name(), ""),
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: px(10),
                        padding: UiRect::all(px(6)),
                        border: UiRect::all(px(1)),
                        border_radius: BorderRadius::all(px(6)),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BackgroundColor(Color::BLACK.with_alpha(0.3)),
                    BorderColor::all(ui::theme::panel_border().with_alpha(0.2)),
                    ChildOf(seat),
                ))
                .id();
            let plate = commands
                .spawn((
                    Node {
                        width: px(42),
                        height: px(42),
                        flex_shrink: 0.0,
                        border: UiRect::all(px(1)),
                        border_radius: BorderRadius::all(px(5)),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BackgroundColor(ui::theme::panel_bg().with_alpha(0.6)),
                    BorderColor::all(ui::theme::panel_border()),
                    ChildOf(entry),
                ))
                .id();
            crate::miracles::paint_miracle_face(&mut commands, plate, miracle);
            commands.spawn((
                MiracleName(miracle),
                Text::new(miracle.name().to_uppercase()),
                ui::DisplayFace,
                TextFont {
                    font_size: FontSize::Px(14.0),
                    ..default()
                },
                TextColor(ui::theme::text_dim()),
                TextLayout::linebreak(LineBreak::NoWrap),
                ChildOf(entry),
            ));
        }
        // A short last row keeps its columns' width.
        for _ in third.len()..3 {
            commands.spawn((ordo::col(1, super::village::RHYTHM), ChildOf(grid_line)));
        }
    }
}

/// One line saying what a miracle asks and where it stands, for the
/// tooltip's body.
fn story_of(miracle: Miracle, lit: bool) -> String {
    if lit {
        return format!(
            "{}. Ready every {} days.",
            miracle.blurb(),
            miracle.cooldown_days()
        );
    }
    let earns = match miracle.unlock() {
        crate::miracles::Unlock::Founding => "yours from the founding".to_string(),
        crate::miracles::Unlock::Belief(rung) => {
            format!("earned at {rung:.0} belief")
        }
        crate::miracles::Unlock::Legend => {
            if miracle == Miracle::Mend {
                "a legend of providence unlocks it".to_string()
            } else {
                "a legend of dread unlocks it".to_string()
            }
        }
        crate::miracles::Unlock::Dread(depth) => {
            format!("earned by dread, {depth:.0} deep")
        }
    };
    format!("{}. Locked - {}.", miracle.blurb(), earns)
}

/// Keeps the shelf honest while the page shows: which powers are held,
/// and each entry's tooltip telling its story.
pub(crate) fn update_spellbook(
    codex: Res<super::village::Codex>,
    panels: Query<&Visibility, With<SpellbookPanel>>,
    grimoire: Res<crate::miracles::Grimoire>,
    mut entries: Query<(
        &MiracleCard,
        &mut BackgroundColor,
        &mut BorderColor,
        &mut ordo::Tooltip,
    )>,
    mut names: Query<(&MiracleName, &mut TextColor)>,
) {
    if codex.page != super::village::CodexPage::Miracles
        || !panels.iter().any(|v| *v != Visibility::Hidden)
    {
        return;
    }
    for (card, mut fill, mut border, mut tip) in &mut entries {
        let lit = grimoire.knows(card.0);
        fill.0 = if lit {
            ui::theme::title_bg()
        } else {
            Color::BLACK.with_alpha(0.3)
        };
        *border = BorderColor::all(if lit {
            ui::theme::accent().with_alpha(0.55)
        } else {
            ui::theme::panel_border().with_alpha(0.2)
        });
        let fresh = story_of(card.0, lit);
        if tip.line != fresh {
            tip.line = fresh;
        }
    }
    for (name, mut ink) in &mut names {
        let lit = grimoire.knows(name.0);
        let fresh = if lit {
            ui::theme::accent()
        } else {
            ui::theme::text_dim()
        };
        if ink.0 != fresh {
            ink.0 = fresh;
        }
    }
}
