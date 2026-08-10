//! THE MIRACLES: the spellbook.
//!
//! Brett: "The miracles page should show all of the abilities that you
//! can get and have a copy of the toolbar where you can slot them. Like a
//! spell book from WoW." Every power the god may ever learn hangs here as
//! a card — face, name, what it does, what it costs in days, and what
//! earns it — and the header carries the belief ladder and a LIVE MIRROR
//! of the ten-slot bar. The mirror is made of the same `MiracleSlot`s the
//! HUD's apron wears, so the dresser, the styler, the cooldown sweep and
//! the drag all serve it without knowing there are two bars. Cards go to
//! the bar by press (first empty slot) or by drag, WoW-style.
//!
//! THE DEITY page next door is for the god's own standing — and will one
//! day carry the progression tree, when belief-as-XP finds its shape.

use super::*;
use crate::miracles::{Miracle, MiracleCard};

/// The spellbook page's root: every is-the-page-open gate reads this.
#[derive(Component)]
pub(crate) struct SpellbookPanel;

/// One live text on a spellbook card.
#[derive(Component)]
pub(crate) struct MiracleText {
    miracle: Miracle,
    /// 0 cost, 1 state, 2 reason.
    field: u8,
}

/// Pressing a learned card sets its miracle in the first empty slot —
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

pub(crate) fn spawn_spellbook_page(mut commands: Commands, codex: Res<super::village::Codex>) {
    let page = codex.miracles_page;
    commands
        .entity(page)
        .insert((Name::new("Miracles Page"), SpellbookPanel));

    let leaf = ordo::page(&mut commands, page, super::village::RHYTHM);

    // The header: the ladder that earns the powers, and the bar that
    // carries them — always in reach while the shelf below scrolls.
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
    crate::miracles::raise_bar_slots(&mut commands, bar);
    commands.spawn((
        ui::dim("press a card for the first empty slot - or drag it onto the bar"),
        ChildOf(head),
    ));

    // The shelf: every miracle there is, three to a row on the page
    // grid, learned or still to earn.
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
            // squares off a short row - a border or padding on the column
            // itself joins the flex arithmetic and staggers part-rows off
            // the tracts. The card is a full-width child instead.
            let seat = commands
                .spawn((ordo::col(1, super::village::RHYTHM), ChildOf(grid_line)))
                .id();
            let card = commands
                .spawn((
                    MiracleCard(miracle),
                    Interaction::default(),
                    ui::HoverHint::new(
                        miracle.name(),
                        "press for the first empty slot - or drag it onto the bar",
                    ),
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Row,
                        column_gap: px(12),
                        padding: UiRect::all(px(12)),
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
            // The face: the same engraving the bar's slots wear, on its
            // own permanent plate.
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
                    ChildOf(card),
                ))
                .id();
            crate::miracles::paint_miracle_face(&mut commands, plate, miracle);
            let words = commands
                .spawn((
                    Node {
                        flex_grow: 1.0,
                        min_width: px(0),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(2),
                        ..default()
                    },
                    ChildOf(card),
                ))
                .id();
            commands.spawn((
                Text::new(miracle.name().to_uppercase()),
                ui::DisplayFace,
                TextFont {
                    font_size: FontSize::Px(14.0),
                    ..default()
                },
                TextColor(ui::theme::accent()),
                ChildOf(words),
            ));
            commands.spawn((ui::dim(miracle.blurb()), ChildOf(words)));
            for field in [0u8, 1, 2] {
                commands.spawn((
                    MiracleText { miracle, field },
                    ui::dim(""),
                    Node {
                        max_width: percent(100),
                        ..default()
                    },
                    ChildOf(words),
                ));
            }
        }
        // A short last row keeps its columns' width.
        for _ in third.len()..3 {
            commands.spawn((ordo::col(1, super::village::RHYTHM), ChildOf(grid_line)));
        }
    }
}

/// Keeps the shelf honest while the page shows: which powers are held,
/// what each costs, and what still earns the rest.
pub(crate) fn update_spellbook(
    codex: Res<super::village::Codex>,
    panels: Query<&Visibility, With<SpellbookPanel>>,
    grimoire: Res<crate::miracles::Grimoire>,
    mut cards: Query<(&MiracleCard, &mut BackgroundColor, &mut BorderColor)>,
    mut texts: Query<(&MiracleText, &mut Text)>,
) {
    if codex.page != super::village::CodexPage::Miracles
        || !panels.iter().any(|v| *v != Visibility::Hidden)
    {
        return;
    }
    let unlocked = |miracle: Miracle| grimoire.knows(miracle);
    for (card, mut fill, mut border) in &mut cards {
        let lit = unlocked(card.0);
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
    }
    for (label, mut text) in &mut texts {
        let lit = unlocked(label.miracle);
        let fresh = match label.field {
            0 => {
                if lit {
                    format!("every {} days", label.miracle.cooldown_days())
                } else {
                    match label.miracle.unlock() {
                        crate::miracles::Unlock::Founding => "from the founding".to_string(),
                        crate::miracles::Unlock::Belief(rung) => {
                            format!("at {rung:.0} belief")
                        }
                        crate::miracles::Unlock::Legend => "by legend".to_string(),
                        crate::miracles::Unlock::Dread(depth) => {
                            format!("by dread, {depth:.0} deep")
                        }
                    }
                }
            }
            1 => if lit { "Active" } else { "Locked" }.to_string(),
            _ => {
                if lit {
                    String::new()
                } else if label.miracle == Miracle::Mend {
                    "a legend of providence unlocks it".to_string()
                } else if label.miracle == Miracle::Quake {
                    "a legend of dread unlocks it".to_string()
                } else {
                    String::new()
                }
            }
        };
        if text.0 != fresh {
            *text = Text::new(fresh);
        }
    }
}
