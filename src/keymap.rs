//! The keymap: every deed the keyboard can ask of the god, the key each
//! deed answers to, and the player's own choices, written beside the saves
//! so they survive a restart.
//!
//! The settings page reads and rewrites this; every system that listens to
//! the keyboard asks here instead of naming keys of its own. The escape
//! key, the mouse, and the workbench's function keys stay fixed — they are
//! the house's own fittings, not furniture to be moved.

use bevy::prelude::*;

/// A deed the keyboard can do. The order is the order the keys file is
/// written in, and the index into the map's table.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Deed {
    PanNorth,
    PanSouth,
    PanWest,
    PanEast,
    TurnLeft,
    TurnRight,
    Pause,
    Slower,
    Faster,
    Flourish,
    Smite,
    Bounty,
    MendOrQuake,
    Codex,
    Markers,
    Survey,
    Roofs,
    Doings,
    Trades,
    Fog,
}

impl Deed {
    pub const ALL: [Deed; 20] = [
        Deed::PanNorth,
        Deed::PanSouth,
        Deed::PanWest,
        Deed::PanEast,
        Deed::TurnLeft,
        Deed::TurnRight,
        Deed::Pause,
        Deed::Slower,
        Deed::Faster,
        Deed::Flourish,
        Deed::Smite,
        Deed::Bounty,
        Deed::MendOrQuake,
        Deed::Codex,
        Deed::Markers,
        Deed::Survey,
        Deed::Roofs,
        Deed::Doings,
        Deed::Trades,
        Deed::Fog,
    ];

    /// The name the deed goes by in the keys file.
    fn written(self) -> &'static str {
        match self {
            Deed::PanNorth => "pan-north",
            Deed::PanSouth => "pan-south",
            Deed::PanWest => "pan-west",
            Deed::PanEast => "pan-east",
            Deed::TurnLeft => "turn-left",
            Deed::TurnRight => "turn-right",
            Deed::Pause => "pause",
            Deed::Slower => "slower",
            Deed::Faster => "faster",
            Deed::Flourish => "flourish",
            Deed::Smite => "smite",
            Deed::Bounty => "bounty",
            Deed::MendOrQuake => "mend-or-quake",
            Deed::Codex => "codex",
            Deed::Markers => "markers",
            Deed::Survey => "survey",
            Deed::Roofs => "roofs",
            Deed::Doings => "doings",
            Deed::Trades => "trades",
            Deed::Fog => "fog",
        }
    }

    fn default_key(self) -> KeyCode {
        match self {
            Deed::PanNorth => KeyCode::KeyW,
            Deed::PanSouth => KeyCode::KeyS,
            Deed::PanWest => KeyCode::KeyA,
            Deed::PanEast => KeyCode::KeyD,
            Deed::TurnLeft => KeyCode::KeyQ,
            Deed::TurnRight => KeyCode::KeyE,
            Deed::Pause => KeyCode::Space,
            Deed::Slower => KeyCode::Minus,
            Deed::Faster => KeyCode::Equal,
            Deed::Flourish => KeyCode::Digit1,
            Deed::Smite => KeyCode::Digit2,
            Deed::Bounty => KeyCode::Digit3,
            Deed::MendOrQuake => KeyCode::Digit4,
            Deed::Codex => KeyCode::Tab,
            Deed::Markers => KeyCode::KeyP,
            Deed::Survey => KeyCode::KeyR,
            Deed::Roofs => KeyCode::KeyH,
            Deed::Doings => KeyCode::KeyL,
            Deed::Trades => KeyCode::KeyK,
            Deed::Fog => KeyCode::KeyF,
        }
    }
}

/// Which key answers to which deed. One key per deed, always: binding a
/// key that already serves another deed trades the two, so nothing is ever
/// left unbound.
#[derive(Resource)]
pub struct Keymap {
    binds: [KeyCode; Deed::ALL.len()],
}

impl Default for Keymap {
    fn default() -> Self {
        let mut binds = [KeyCode::Space; Deed::ALL.len()];
        for deed in Deed::ALL {
            binds[deed as usize] = deed.default_key();
        }
        Keymap { binds }
    }
}

impl Keymap {
    pub fn key(&self, deed: Deed) -> KeyCode {
        self.binds[deed as usize]
    }

    pub fn pressed(&self, keys: &ButtonInput<KeyCode>, deed: Deed) -> bool {
        keys.pressed(self.key(deed))
    }

    pub fn just_pressed(&self, keys: &ButtonInput<KeyCode>, deed: Deed) -> bool {
        keys.just_pressed(self.key(deed))
    }

    /// Gives the deed a new key. If the key already serves another deed,
    /// the two trade places.
    pub fn bind(&mut self, deed: Deed, key: KeyCode) {
        let old = self.key(deed);
        if let Some(other) = Deed::ALL
            .into_iter()
            .find(|d| *d != deed && self.key(*d) == key)
        {
            self.binds[other as usize] = old;
        }
        self.binds[deed as usize] = key;
    }

    pub fn restore_defaults(&mut self) {
        *self = Keymap::default();
    }
}

/// Every key a deed may be bound to, and the short name it wears on a
/// keycap and in the keys file. What is absent is refused on purpose:
/// escape, the modifiers, the backquote and the function keys keep their
/// fixed offices, and so do the arrows - they are the camera's standing
/// alternates, and a deed bound over them would fire twice.
const NAMED: &[(KeyCode, &str)] = &[
    (KeyCode::KeyA, "A"),
    (KeyCode::KeyB, "B"),
    (KeyCode::KeyC, "C"),
    (KeyCode::KeyD, "D"),
    (KeyCode::KeyE, "E"),
    (KeyCode::KeyF, "F"),
    (KeyCode::KeyG, "G"),
    (KeyCode::KeyH, "H"),
    (KeyCode::KeyI, "I"),
    (KeyCode::KeyJ, "J"),
    (KeyCode::KeyK, "K"),
    (KeyCode::KeyL, "L"),
    (KeyCode::KeyM, "M"),
    (KeyCode::KeyN, "N"),
    (KeyCode::KeyO, "O"),
    (KeyCode::KeyP, "P"),
    (KeyCode::KeyQ, "Q"),
    (KeyCode::KeyR, "R"),
    (KeyCode::KeyS, "S"),
    (KeyCode::KeyT, "T"),
    (KeyCode::KeyU, "U"),
    (KeyCode::KeyV, "V"),
    (KeyCode::KeyW, "W"),
    (KeyCode::KeyX, "X"),
    (KeyCode::KeyY, "Y"),
    (KeyCode::KeyZ, "Z"),
    (KeyCode::Digit0, "0"),
    (KeyCode::Digit1, "1"),
    (KeyCode::Digit2, "2"),
    (KeyCode::Digit3, "3"),
    (KeyCode::Digit4, "4"),
    (KeyCode::Digit5, "5"),
    (KeyCode::Digit6, "6"),
    (KeyCode::Digit7, "7"),
    (KeyCode::Digit8, "8"),
    (KeyCode::Digit9, "9"),
    (KeyCode::Space, "Space"),
    (KeyCode::Tab, "Tab"),
    (KeyCode::Minus, "-"),
    (KeyCode::Equal, "="),
    (KeyCode::Comma, ","),
    (KeyCode::Period, "."),
    (KeyCode::Slash, "/"),
    (KeyCode::Backslash, "\\"),
    (KeyCode::Semicolon, ";"),
    (KeyCode::Quote, "'"),
    (KeyCode::BracketLeft, "["),
    (KeyCode::BracketRight, "]"),
    (KeyCode::Enter, "Enter"),
    (KeyCode::Backspace, "Back"),
    (KeyCode::Insert, "Ins"),
    (KeyCode::Delete, "Del"),
    (KeyCode::Home, "Home"),
    (KeyCode::End, "End"),
    (KeyCode::PageUp, "PgUp"),
    (KeyCode::PageDown, "PgDn"),
    (KeyCode::Numpad0, "Num0"),
    (KeyCode::Numpad1, "Num1"),
    (KeyCode::Numpad2, "Num2"),
    (KeyCode::Numpad3, "Num3"),
    (KeyCode::Numpad4, "Num4"),
    (KeyCode::Numpad5, "Num5"),
    (KeyCode::Numpad6, "Num6"),
    (KeyCode::Numpad7, "Num7"),
    (KeyCode::Numpad8, "Num8"),
    (KeyCode::Numpad9, "Num9"),
    (KeyCode::NumpadAdd, "Num+"),
    (KeyCode::NumpadSubtract, "Num-"),
    (KeyCode::NumpadMultiply, "Num*"),
    (KeyCode::NumpadDivide, "Num/"),
];

/// The keycap name of a key, if it is one a deed may wear.
pub fn key_name(key: KeyCode) -> Option<&'static str> {
    NAMED
        .iter()
        .find(|(named, _)| *named == key)
        .map(|(_, name)| *name)
}

fn key_from_name(name: &str) -> Option<KeyCode> {
    NAMED
        .iter()
        .find(|(_, named)| *named == name)
        .map(|(key, _)| *key)
}

/// The keys file, beside the saves and the models.
fn keys_path() -> Option<std::path::PathBuf> {
    let home = std::env::var("HOME").ok();
    let base = if cfg!(target_os = "macos") {
        std::path::PathBuf::from(home?).join("Library/Application Support/Divus Factus")
    } else if cfg!(target_os = "windows") {
        std::path::PathBuf::from(std::env::var("APPDATA").ok()?).join("Divus Factus")
    } else {
        std::path::PathBuf::from(home?).join(".local/share/divus-factus")
    };
    Some(base.join("keys.txt"))
}

/// The defaults, with whatever the player has written over them. Lines are
/// applied through [`Keymap::bind`], so a hand-edited file can never leave
/// two deeds on one key.
fn load() -> Keymap {
    let mut map = Keymap::default();
    let Some(text) = keys_path().and_then(|path| std::fs::read_to_string(path).ok()) else {
        return map;
    };
    for line in text.lines() {
        let Some((deed_name, key_name)) = line.split_once(' ') else {
            continue;
        };
        let deed = Deed::ALL.into_iter().find(|d| d.written() == deed_name);
        if let (Some(deed), Some(key)) = (deed, key_from_name(key_name.trim())) {
            map.bind(deed, key);
        }
    }
    map
}

/// Writes the whole map down. Called after every change; seventeen lines
/// is not a cost worth batching.
pub fn save(map: &Keymap) {
    let Some(path) = keys_path() else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let text: String = Deed::ALL
        .into_iter()
        .filter_map(|deed| Some(format!("{} {}\n", deed.written(), key_name(map.key(deed))?)))
        .collect();
    let _ = std::fs::write(path, text);
}

pub struct KeymapPlugin;

impl Plugin for KeymapPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(load());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binding_a_taken_key_trades_places() {
        let mut map = Keymap::default();
        map.bind(Deed::Smite, KeyCode::Digit1);
        assert_eq!(map.key(Deed::Smite), KeyCode::Digit1);
        assert_eq!(map.key(Deed::Flourish), KeyCode::Digit2);
    }

    #[test]
    fn every_deed_has_a_nameable_default() {
        for deed in Deed::ALL {
            assert!(
                key_name(deed.default_key()).is_some(),
                "{deed:?} defaults to a key the keys file cannot write"
            );
        }
    }

    #[test]
    fn no_two_defaults_share_a_key() {
        for a in Deed::ALL {
            for b in Deed::ALL {
                if a != b {
                    assert_ne!(a.default_key(), b.default_key(), "{a:?} and {b:?}");
                }
            }
        }
    }
}
