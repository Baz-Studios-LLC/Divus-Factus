//! The music: three airs by the other machine, played by this one.
//!
//! The title carries the pastoral theme; the working day hums the
//! settlers' hearth; and when night comes down and the hand begins to
//! glow, the divine presence rises with it. One track stands at a time,
//! and changes of heart are crossfades, never cuts.

use bevy::audio::{
    AudioPlayer, AudioSink, AudioSinkPlayback, PlaybackMode, PlaybackSettings, Volume,
};
use bevy::prelude::*;

/// Seconds a fade takes, in or out.
const FADE: f32 = 3.5;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Air {
    PastoralDeity,
    SettlersHearth,
    DivinePresence,
    WinterHymn,
    GatheringStorm,
    WildernessTrail,
}

impl Air {
    fn path(self) -> &'static str {
        match self {
            Air::PastoralDeity => "audio/pastoral_deity_theme.wav",
            Air::SettlersHearth => "audio/settlers_hearth.wav",
            Air::DivinePresence => "audio/divine_presence.wav",
            Air::WinterHymn => "audio/winter_hymn.wav",
            Air::GatheringStorm => "audio/gathering_storm.wav",
            Air::WildernessTrail => "audio/wilderness_trail.wav",
        }
    }
}

/// The player's chosen loudness, 0 to 1, kept beside the saves.
#[derive(Resource)]
pub struct MusicVolume(pub f32);

impl Default for MusicVolume {
    fn default() -> Self {
        MusicVolume(0.6)
    }
}

/// One playing track and the direction its volume is moving.
#[derive(Component)]
struct Channel {
    air: Air,
    /// 0 to 1: the fade envelope, before the player's volume is applied.
    level: f32,
    /// Fading in toward 1, or out toward the despawn.
    rising: bool,
}

pub struct MusicPlugin;

impl Plugin for MusicPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(load_volume())
            .add_systems(Update, (conduct, fade).chain());
    }
}

/// Decides which air the moment calls for.
fn wanted(
    state: &crate::GameState,
    clock: Option<&crate::calendar::WorldClock>,
    weather: Option<&crate::weather::Weather>,
) -> Air {
    match state {
        crate::GameState::Playing => {
            // Heavy rain or storm triggers the dramatic storm theme.
            if let Some(w) = weather {
                if w.intensity > 0.55 {
                    return Air::GatheringStorm;
                }
            }

            let frac = clock.map(|c| c.time_of_day()).unwrap_or(0.3);
            // The hand starts to glow around 0.72; the presence rises with it
            // and hands back to the hearth a little after dawn breaks.
            if frac >= 0.72 || frac < 0.05 {
                Air::DivinePresence
            } else if let Some(c) = clock {
                match c.season() {
                    crate::calendar::Season::Winter => Air::WinterHymn,
                    crate::calendar::Season::Autumn => Air::WildernessTrail,
                    _ => Air::SettlersHearth,
                }
            } else {
                Air::SettlersHearth
            }
        }
        _ => Air::PastoralDeity,
    }
}

/// Keeps the right track standing: the newcomer fades in while the old
/// fades out, and nothing ever cuts.
fn conduct(
    mut commands: Commands,
    assets: Res<AssetServer>,
    state: Res<State<crate::GameState>>,
    clock: Option<Res<crate::calendar::WorldClock>>,
    weather: Option<Res<crate::weather::Weather>>,
    mut channels: Query<&mut Channel>,
) {
    let want = wanted(state.get(), clock.as_deref(), weather.as_deref());
    let mut standing = false;
    for mut channel in &mut channels {
        if channel.air == want {
            standing = true;
            if !channel.rising {
                channel.rising = true;
            }
        } else if channel.rising {
            channel.rising = false;
        }
    }
    if !standing {
        commands.spawn((
            Channel {
                air: want,
                level: 0.0,
                rising: true,
            },
            AudioPlayer::new(assets.load(want.path())),
            PlaybackSettings {
                mode: PlaybackMode::Loop,
                volume: Volume::Linear(0.0),
                ..default()
            },
        ));
    }
}

/// Walks every channel's envelope and writes it to the sink; a channel
/// faded to silence is put away.
fn fade(
    mut commands: Commands,
    time: Res<Time<Real>>,
    volume: Res<MusicVolume>,
    mut channels: Query<(Entity, &mut Channel, Option<&mut AudioSink>)>,
) {
    let step = time.delta_secs() / FADE;
    for (entity, mut channel, sink) in &mut channels {
        // A freshly spawned track has no sink until its file finishes
        // loading; the envelope holds at silence until then, or the fade
        // would already be spent when the first note lands.
        if sink.is_none() && channel.rising {
            continue;
        }
        channel.level = if channel.rising {
            (channel.level + step).min(1.0)
        } else {
            channel.level - step
        };
        if channel.level <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        if let Some(mut sink) = sink {
            // A gentle curve: linear fades sound like they arrive late.
            let heard = channel.level * channel.level * volume.0;
            sink.set_volume(Volume::Linear(heard));
        }
    }
}

// ------------------------------------------------------------- the setting

fn volume_path() -> Option<std::path::PathBuf> {
    let home = std::env::var("HOME").ok();
    let base = if cfg!(target_os = "macos") {
        std::path::PathBuf::from(home?).join("Library/Application Support/Divus Factus")
    } else if cfg!(target_os = "windows") {
        std::path::PathBuf::from(std::env::var("APPDATA").ok()?).join("Divus Factus")
    } else {
        std::path::PathBuf::from(home?).join(".local/share/divus-factus")
    };
    Some(base.join("sound.txt"))
}

fn load_volume() -> MusicVolume {
    volume_path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| text.trim().parse::<f32>().ok())
        .map(|v| MusicVolume(v.clamp(0.0, 1.0)))
        .unwrap_or_default()
}

/// Writes the chosen loudness down so it survives a restart.
pub fn save_volume(volume: &MusicVolume) {
    let Some(path) = volume_path() else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, format!("{:.2}\n", volume.0));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_air_has_a_file_on_disk() {
        for air in [
            Air::PastoralDeity,
            Air::SettlersHearth,
            Air::DivinePresence,
            Air::WinterHymn,
            Air::GatheringStorm,
            Air::WildernessTrail,
        ] {
            let path = std::path::Path::new("assets").join(air.path());
            assert!(
                path.exists(),
                "{air:?} wants {} and it is not there",
                path.display()
            );
        }
    }
}
