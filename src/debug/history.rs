//! THE CHRONICLE window: what the village remembers happening.

use crate::ui;
use bevy::prelude::*;
/// The toolbar button that opens the history.
#[derive(Component)]
pub(crate) struct HistoryButton;

/// The history panel: everything that has ever happened, stamped.
#[derive(Component)]
pub(crate) struct HistoryPanel;

/// The history text block.
#[derive(Component)]
pub(crate) struct HistoryText;

pub(crate) fn spawn_history_panel(mut commands: Commands) {
    let window = ui::window(&mut commands, "THE CHRONICLE", 320.0);
    commands.entity(window.root).insert((
        Name::new("History Panel"),
        HistoryPanel,
        Visibility::Hidden,
    ));
    let text = commands.spawn((HistoryText, ui::dim(""))).id();
    commands.entity(text).insert(ChildOf(window.body));
}

/// Fills the history panel with the tail of the world's chronicle.
pub(crate) fn update_history_panel(
    history: Option<Res<crate::villager::WorldChronicle>>,
    panels: Query<&Visibility, With<HistoryPanel>>,
    mut texts: Query<&mut Text, With<HistoryText>>,
) {
    if !panels.iter().any(|v| *v != Visibility::Hidden) {
        return;
    }
    let (Some(history), Ok(mut text)) = (history, texts.single_mut()) else {
        return;
    };

    let events = &history.events;
    let shown = 16usize;
    let tail = events.len().saturating_sub(shown);
    let mut lines: Vec<String> = events[tail..]
        .iter()
        .map(|event| format!("{}  {}", event.stamp, event.text))
        .collect();
    if tail > 0 {
        lines.insert(0, format!("({tail} earlier entries)"));
    }
    if lines.is_empty() {
        lines.push("nothing has happened yet".into());
    }
    let fresh = lines.join("\n");
    if text.0 != fresh {
        *text = Text::new(fresh);
    }
}
