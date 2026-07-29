//! THE GOD window: name, epithet, miracles, and how they feel about you.

use crate::ui;
use crate::villager::Villager;
use crate::witness::Witnessed;
use bevy::prelude::*;
/// THE GOD panel and its live pieces.
#[derive(Component)]
pub(crate) struct GodPanel;

#[derive(Component)]
pub(crate) struct GodButton;

#[derive(Component)]
pub(crate) struct GodName;

#[derive(Component)]
pub(crate) struct GodEpithet;

#[derive(Component)]
pub(crate) struct MiracleTile(crate::miracles::Miracle);

#[derive(Component)]
pub(crate) struct MiracleTileLabel(crate::miracles::Miracle);

#[derive(Component)]
pub(crate) struct FeelingsText;

pub(crate) fn spawn_god_panel(mut commands: Commands) {
    let window = ui::big_window(&mut commands, "THE GOD", 560.0);
    commands
        .entity(window.root)
        .insert((Name::new("God Panel"), GodPanel, Visibility::Hidden));

    // The masthead: the name they gave you, writ large, on the warm card.
    let masthead = ui::detail_card(&mut commands, window.body);
    commands.spawn((
        GodName,
        Text::new("..."),
        TextFont {
            font_size: FontSize::Px(26.0),
            ..default()
        },
        TextColor(ui::theme::accent()),
        ChildOf(masthead),
    ));
    commands.spawn((GodEpithet, ui::dim("not yet named"), ChildOf(masthead)));

    // MIRACLES: a tile per power, earned or waiting.
    ui::section_header(&mut commands, window.body, "MIRACLES");
    let grid = commands
        .spawn((
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Row,
                column_gap: px(10),
                ..default()
            },
            ChildOf(window.body),
        ))
        .id();
    use crate::miracles::Miracle;
    for miracle in [
        Miracle::Flourish,
        Miracle::Smite,
        Miracle::Bounty,
        Miracle::Mend,
        Miracle::Quake,
    ] {
        let cell = ui::tile(&mut commands, grid, 86.0, false);
        commands.entity(cell).insert(MiracleTile(miracle));
        commands.spawn((
            Text::new(miracle.name()),
            TextFont {
                font_size: FontSize::Px(13.0),
                ..default()
            },
            TextColor(ui::theme::text()),
            ChildOf(cell),
        ));
        commands.spawn((MiracleTileLabel(miracle), ui::dim(""), ChildOf(cell)));
    }
    commands.spawn((
        ui::dim("locked powers are earned: a legend of providence crystallises Mend; a legend of dread, Quake"),
        ChildOf(window.body),
    ));

    // HOW THEY FEEL: the congregation read as one voice.
    ui::section_header(&mut commands, window.body, "HOW THEY FEEL ABOUT YOU");
    let feelings = ui::inset_well(&mut commands, window.body);
    commands.spawn((FeelingsText, ui::body(""), ChildOf(feelings)));
}

/// Fills THE GOD panel while it is open.
#[allow(clippy::type_complexity)]
pub(crate) fn update_god_panel(
    panels: Query<&Visibility, With<GodPanel>>,
    name: Option<Res<crate::villager::DivineName>>,
    legend: Option<Res<crate::villager::belief::Legend>>,
    belief: Option<Res<crate::villager::belief::Belief>>,
    flock: Query<
        (Option<&crate::villager::belief::Faith>, Option<&Witnessed>),
        (With<Villager>, Without<crate::creature::Corpse>),
    >,
    mut tiles: Query<(&MiracleTile, &mut BackgroundColor, &mut BorderColor)>,
    mut texts: ParamSet<(
        Query<&mut Text, With<GodName>>,
        Query<&mut Text, With<GodEpithet>>,
        Query<(&MiracleTileLabel, &mut Text)>,
        Query<&mut Text, With<FeelingsText>>,
    )>,
) {
    if !panels.iter().any(|v| *v != Visibility::Hidden) {
        return;
    }
    use crate::miracles::Miracle;

    if let Ok(mut text) = texts.p0().single_mut() {
        let fresh = name.as_ref().map_or("...".to_string(), |n| n.0.clone());
        if text.0 != fresh {
            *text = Text::new(fresh);
        }
    }
    if let Ok(mut text) = texts.p1().single_mut() {
        let fresh = match legend.as_ref().and_then(|l| l.epithet) {
            Some(epithet) => format!("{epithet} - so the people say"),
            None => "named by the people; no epithet yet earned".to_string(),
        };
        if text.0 != fresh {
            *text = Text::new(fresh);
        }
    }

    let unlocked = |miracle: Miracle| match miracle {
        Miracle::Flourish | Miracle::Smite => true,
        m => legend.as_ref().is_some_and(|l| l.unlocked == Some(m)),
    };
    for (tile, mut bg, mut border) in &mut tiles {
        let lit = unlocked(tile.0);
        bg.0 = if lit {
            ui::theme::title_bg()
        } else {
            Color::BLACK.with_alpha(0.3)
        };
        *border = BorderColor::all(if lit {
            ui::theme::card_border()
        } else {
            ui::theme::panel_border().with_alpha(0.2)
        });
    }
    for (label, mut text) in &mut texts.p2() {
        let fresh = if unlocked(label.0) {
            format!("{:.0} belief", label.0.cost())
        } else {
            "locked".to_string()
        };
        if text.0 != fresh {
            *text = Text::new(fresh);
        }
    }

    // The congregation, read as one voice.
    let mut total = 0usize;
    let mut believers = 0usize;
    let mut trust_sum = 0.0f32;
    let mut eyewitnesses = 0usize;
    let mut heard = 0usize;
    for (faith, witnessed) in &flock {
        total += 1;
        if let Some(faith) = faith {
            trust_sum += faith.trust;
            if faith.is_believer() {
                believers += 1;
            }
        }
        if let Some(w) = witnessed {
            if w.total > 0 {
                eyewitnesses += 1;
            } else if w.secondhand > 0 {
                heard += 1;
            }
        }
    }
    let avg = trust_sum / total.max(1) as f32;
    let mood = match avg {
        a if a > 0.6 => "they are sure of you",
        a if a > 0.45 => "they believe, on the whole",
        a if a > 0.25 => "they waver",
        _ => "they doubt you",
    };
    let lean = legend.as_ref().map_or("", |l| {
        if l.providence > l.dread * 1.4 {
            "the stories they tell are of gifts"
        } else if l.dread > l.providence * 1.4 {
            "the stories they tell are of terror"
        } else {
            "their stories cannot decide what you are"
        }
    });
    let power = belief.as_ref().map_or(0.0, |b| b.available());
    let fresh = format!(
        "{believers} of {total} believe - {mood}
{eyewitnesses} have seen you with their own eyes; {heard} know you only from stories
{lean}
{power:.0} belief stands ready to spend",
    );
    if let Ok(mut text) = texts.p3().single_mut()
        && text.0 != fresh
    {
        *text = Text::new(fresh);
    }
}
