//! Sound effects: the world, answered in sound.
//!
//! The music owns the mood; these own the MOMENTS — a knuckle on
//! shingles, the sea taking what fell in it, thunder that answers a dark
//! prayer. Every one is synthesized from pure math (see the maker script
//! beside the assets), which keeps them of a piece with a world built
//! entirely from little cubes: warm, soft, and nothing pretending to be
//! a recording of the real world.
//!
//! One channel in: anything in the game plays a sound by writing
//! [`PlaySfx`]. Sounds of the god's own hand play at full volume — the
//! hand is at the player's arm, not in the world — while world sounds
//! carry a position and fade with the distance from where the god is
//! looking. Some moments are heard centrally, off messages that already
//! exist (the knock, the smite, the notice chimes), so their systems
//! never learned sound happened to them.

use bevy::audio::{AudioPlayer, PlaybackMode, PlaybackSettings, Volume};
use bevy::prelude::*;

/// Every effect the kit knows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SfxKind {
    /// Two knuckle raps on a roof.
    Knock,
    /// The water claiming something.
    Splash,
    /// Something heavy meeting the turf.
    Thud,
    /// The hand closing on something.
    Grab,
    /// Something hurled, leaving.
    Whoosh,
    /// Thunder: the crack and the rumble that owns the valley after.
    Smite,
    /// The prayer bell — the pink channel's voice.
    Chime,
    /// Two warm strikes rising: a village remembering a good day.
    Fanfare,
    /// The founding flag driven home.
    Plant,
}

impl SfxKind {
    pub const ALL: [SfxKind; 9] = [
        SfxKind::Knock,
        SfxKind::Splash,
        SfxKind::Thud,
        SfxKind::Grab,
        SfxKind::Whoosh,
        SfxKind::Smite,
        SfxKind::Chime,
        SfxKind::Fanfare,
        SfxKind::Plant,
    ];

    pub fn path(self) -> &'static str {
        match self {
            SfxKind::Knock => "audio/sfx/knock.wav",
            SfxKind::Splash => "audio/sfx/splash.wav",
            SfxKind::Thud => "audio/sfx/thud.wav",
            SfxKind::Grab => "audio/sfx/grab.wav",
            SfxKind::Whoosh => "audio/sfx/whoosh.wav",
            SfxKind::Smite => "audio/sfx/smite.wav",
            SfxKind::Chime => "audio/sfx/chime.wav",
            SfxKind::Fanfare => "audio/sfx/fanfare.wav",
            SfxKind::Plant => "audio/sfx/plant.wav",
        }
    }

    /// How loud this plays before distance has its say.
    fn presence(self) -> f32 {
        match self {
            // Thunder is the one sound allowed to own the room.
            SfxKind::Smite => 1.0,
            SfxKind::Knock | SfxKind::Plant | SfxKind::Thud => 0.8,
            SfxKind::Splash | SfxKind::Whoosh => 0.7,
            SfxKind::Grab => 0.55,
            SfxKind::Chime | SfxKind::Fanfare => 0.6,
        }
    }
}

/// Play a sound. `at` is the FLAT world position for things happening in
/// the world — they fade with distance from the god's regard — and `None`
/// for the god's own acts and the interface, which are always at the ear.
#[derive(Message)]
pub struct PlaySfx {
    pub kind: SfxKind,
    pub at: Option<Vec3>,
}

/// The loaded kit.
#[derive(Resource)]
struct SfxBank {
    handles: [Handle<bevy::audio::AudioSource>; 9],
}

/// The player's effects loudness, kept beside the music's own dial.
#[derive(Resource)]
pub struct SfxVolume(pub f32);

impl Default for SfxVolume {
    fn default() -> Self {
        SfxVolume(0.9)
    }
}

fn volume_path() -> Option<std::path::PathBuf> {
    let home = std::env::var("HOME").ok();
    let base = if cfg!(target_os = "macos") {
        std::path::PathBuf::from(home?).join("Library/Application Support/Divus Factus")
    } else if cfg!(target_os = "windows") {
        std::path::PathBuf::from(std::env::var("APPDATA").ok()?).join("Divus Factus")
    } else {
        std::path::PathBuf::from(home?).join(".local/share/divus-factus")
    };
    Some(base.join("sfx.txt"))
}

fn load_volume() -> SfxVolume {
    volume_path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| text.trim().parse::<f32>().ok())
        .map(|v| SfxVolume(v.clamp(0.0, 1.0)))
        .unwrap_or_default()
}

/// Writes the chosen loudness down so it survives a restart.
#[allow(dead_code)] // the settings dial arrives with the next sound pass
pub fn save_volume(volume: &SfxVolume) {
    let Some(path) = volume_path() else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, format!("{:.2}\n", volume.0));
}

fn load_bank(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(SfxBank {
        handles: SfxKind::ALL.map(|kind| assets.load(kind.path())),
    });
}

/// Hears the moments that already announce themselves — the knock, the
/// smite, the notice chimes — so those systems never learn about sound.
fn hear_the_world(
    mut play: MessageWriter<PlaySfx>,
    mut knocks: MessageReader<crate::villager::home::Knock>,
    mut acts: MessageReader<crate::witness::DivineEvent>,
    mut notices: MessageReader<crate::ui::Notice>,
) {
    for _knock in knocks.read() {
        // The knock is the god's own knuckle: full voice, because the ACT
        // is at the player's arm however far the roof is.
        play.write(PlaySfx {
            kind: SfxKind::Knock,
            at: None,
        });
    }
    for act in acts.read() {
        if matches!(act.kind, crate::witness::DivineEventKind::Smote) {
            play.write(PlaySfx {
                kind: SfxKind::Smite,
                at: Some(act.position),
            });
        }
    }
    // One chime and one fanfare per frame, however the notices burst.
    let mut chimed = false;
    let mut hailed = false;
    for notice in notices.read() {
        if notice.prayer && !chimed {
            chimed = true;
            play.write(PlaySfx {
                kind: SfxKind::Chime,
                at: None,
            });
        } else if notice.fanfare && !hailed {
            hailed = true;
            play.write(PlaySfx {
                kind: SfxKind::Fanfare,
                at: None,
            });
        }
    }
}

/// Beyond this many world units from the god's regard, a sound is not
/// worth the air it moves.
const EARSHOT: f32 = 520.0;

/// Plays everything asked for this frame, faded by distance from where
/// the god is looking. A handful per frame at most — a burst of twenty
/// landings is a landslide, and a landslide is ONE sound.
fn play_all(
    mut commands: Commands,
    mut asked: MessageReader<PlaySfx>,
    bank: Option<Res<SfxBank>>,
    volume: Res<SfxVolume>,
    rigs: Query<&crate::camera::CameraRig>,
) {
    let Some(bank) = bank else {
        return;
    };
    if volume.0 <= 0.0 {
        asked.clear();
        return;
    }
    let looking_at = rigs.single().map(|rig| rig.target_focus).ok();
    let mut played = 0;
    for ask in asked.read() {
        if played >= 4 {
            break;
        }
        let carry = match (ask.at, looking_at) {
            (Some(at), Some(focus)) => {
                let d = at.distance(focus);
                if d > EARSHOT {
                    continue;
                }
                // Full voice up close, fading off toward earshot's edge.
                (1.0 - ((d - 40.0) / (EARSHOT - 40.0)).clamp(0.0, 1.0) * 0.9).max(0.1)
            }
            _ => 1.0,
        };
        let heard = ask.kind.presence() * carry * volume.0;
        if heard <= 0.01 {
            continue;
        }
        let slot = SfxKind::ALL
            .iter()
            .position(|k| *k == ask.kind)
            .unwrap_or(0);
        commands.spawn((
            AudioPlayer::new(bank.handles[slot].clone()),
            PlaybackSettings {
                mode: PlaybackMode::Despawn,
                volume: Volume::Linear(heard),
                ..default()
            },
        ));
        played += 1;
    }
}

pub struct SfxPlugin;

impl Plugin for SfxPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(load_volume())
            .add_message::<PlaySfx>()
            .add_systems(Startup, load_bank)
            .add_systems(Update, (hear_the_world, play_all).chain());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_sound_has_a_file_on_disk() {
        for kind in SfxKind::ALL {
            let path = std::path::Path::new("assets").join(kind.path());
            assert!(
                path.exists(),
                "{kind:?} wants {} and it is not there",
                path.display()
            );
        }
    }
}
