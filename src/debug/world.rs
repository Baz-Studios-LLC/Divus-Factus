//! THE LAND window: the world's own numbers.

use crate::ui;
use bevy::prelude::*;
/// The toolbar button that opens the world panel.
#[derive(Component)]
pub(crate) struct WorldButton;

/// The world panel: the state of the sky, the season to come, the land.
#[derive(Component)]
pub(crate) struct WorldPanel;

/// Which world reading a row shows.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorldValue {
    Date,
    SkyState,
    Temperature,
    Country,
}

pub(crate) fn spawn_world_panel(mut commands: Commands) {
    let window = ui::window(&mut commands, "THE WORLD", 240.0);
    commands
        .entity(window.root)
        .insert((Name::new("World Panel"), WorldPanel, Visibility::Hidden));
    for (value, label) in [
        (WorldValue::Date, "date"),
        (WorldValue::SkyState, "sky"),
        (WorldValue::Temperature, "warmth"),
        (WorldValue::Country, "country"),
    ] {
        let row = ui::stat_row(&mut commands, window.body, label, None);
        commands.entity(row.value).insert(value);
    }
}

/// Fills the world panel while it is open.
pub(crate) fn update_world_panel(
    clock: Option<Res<crate::calendar::WorldClock>>,
    sky: Option<Res<crate::calendar::Sky>>,
    terrain: Option<Res<crate::terrain::Terrain>>,
    site: Option<Res<crate::villager::SettlementSite>>,
    panels: Query<&Visibility, With<WorldPanel>>,
    mut values: Query<(&WorldValue, &mut Text)>,
    weather: Option<Res<crate::weather::Weather>>,
) {
    if !panels.iter().any(|v| *v != Visibility::Hidden) {
        return;
    }

    for (value, mut text) in &mut values {
        let fresh = match value {
            WorldValue::Date => clock
                .as_ref()
                .map_or_else(|| "-".into(), |c| c.date_phrase()),
            WorldValue::SkyState => weather
                .as_ref()
                .map_or_else(|| "-".to_string(), |w| w.kind().describe().to_string()),
            WorldValue::Temperature => match (&weather, &sky) {
                (Some(weather), Some(sky)) => weather.temperature_word(sky.daylight).to_string(),
                _ => "-".to_string(),
            },
            WorldValue::Country => match (&terrain, &site) {
                (Some(terrain), Some(site)) => {
                    match terrain.biome_at(site.centre.x, site.centre.z) {
                        crate::terrain::Biome::Temperate => "temperate country".into(),
                        crate::terrain::Biome::Boreal => "cold forest country".into(),
                        crate::terrain::Biome::Arid => "dry country".into(),
                        crate::terrain::Biome::Wetland => "wet country".into(),
                        crate::terrain::Biome::Alpine => "high country".into(),
                    }
                }
                _ => "-".to_string(),
            },
        };
        if text.0 != fresh {
            *text = Text::new(fresh);
        }
    }
}
