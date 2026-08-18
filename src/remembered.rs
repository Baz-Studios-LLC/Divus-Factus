//! The player's own choices, written beside the saves so they survive a
//! restart.
//!
//! Brett: "can you make the toggles in The View save for the next boot." Worth
//! a file because half of those switches are how this game is MEASURED — you
//! turn the fog off to argue about a framerate, and an argument that reset
//! itself every launch is an argument nobody can finish. The other half are
//! taste, and taste is exactly the thing a player should only have to say once.
//!
//! One file of `name word` lines, the same shape as the keys file. Deliberately
//! dull about everything else: a name this build does not know is left alone, a
//! word it cannot read falls back to the default, and nothing here can fail
//! loudly. A settings file is not worth a crash.

use std::collections::HashMap;

/// The file, beside the keys and the saves.
///
/// `None` on a machine with nowhere to write, which simply means nothing is
/// remembered — never an error worth reporting.
fn path() -> Option<std::path::PathBuf> {
    // Asked of the one place that already knows where a player's own things
    // live, rather than a sixth copy of the platform paths.
    crate::carried::made_by_hand("settings.txt")
}

/// The two words a switch can be written as.
const ON: &str = "on";
const OFF: &str = "off";

/// What the last launch wrote down.
#[derive(Default)]
pub struct Kept(HashMap<String, String>);

/// Reads the file. Empty on the first launch, and that is the normal case.
pub fn kept() -> Kept {
    Kept(parse(&text()))
}

impl Kept {
    /// Which way a switch was left, or `None` if it was never touched.
    ///
    /// `None` and `Some(false)` are different answers and the caller must keep
    /// them apart: never touched means whatever the default says today, which
    /// is how a new switch can change its own default without overriding
    /// anybody who had already made up their mind about it.
    pub fn switch(&self, name: &str) -> Option<bool> {
        match self.0.get(name)?.as_str() {
            ON => Some(true),
            OFF => Some(false),
            // A hand-edit, or a newer build's idea of this line. The default is
            // a better answer than a guess at what was meant.
            _ => None,
        }
    }
}

/// Writes these switches down, leaving every other line in the file alone.
///
/// A MERGE, not a rewrite: this file is meant to grow — the voice, the hand's
/// color, whatever settles next — and a writer that stamped the whole thing
/// would quietly forget everything it did not happen to know about.
pub fn keep(switches: impl IntoIterator<Item = (String, bool)>) {
    let Some(path) = path() else {
        return;
    };
    let written = merged(
        &text(),
        switches
            .into_iter()
            .map(|(name, on)| (name, if on { ON } else { OFF }.to_string())),
    );
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, written);
}

/// The file as it stands, or nothing at all.
fn text() -> String {
    path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .unwrap_or_default()
}

/// The name a line speaks about.
fn named(line: &str) -> &str {
    line.split_once(' ')
        .map_or(line, |(name, _)| name)
        .trim()
}

fn parse(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter_map(|line| line.split_once(' '))
        .map(|(name, word)| (name.trim().to_string(), word.trim().to_string()))
        .collect()
}

/// The file with these names set to these words.
///
/// Lines already there keep their places, so the file stays readable by a
/// person; names it has never held are appended. Split out from [`keep`] with
/// no filesystem in it so the merge itself can be tested, which is the only
/// part of this module that has a rule worth getting wrong.
fn merged(text: &str, changes: impl IntoIterator<Item = (String, String)>) -> String {
    let mut changes: Vec<(String, String)> = changes.into_iter().collect();
    let mut said: Vec<String> = Vec::new();
    let mut out = String::new();
    for line in text.lines() {
        let name = named(line);
        // A hand-edited file can hold the same name twice; once it has been
        // answered, later copies go, or the file would say two things and the
        // reader would believe the second.
        if said.iter().any(|done| done == name) {
            continue;
        }
        if let Some(at) = changes.iter().position(|(named, _)| named == name) {
            let (name, word) = changes.remove(at);
            out.push_str(&format!("{name} {word}\n"));
            said.push(name);
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    for (name, word) in changes {
        out.push_str(&format!("{name} {word}\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(pairs: &[(&str, bool)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(name, on)| {
                (
                    (*name).to_string(),
                    if *on { ON } else { OFF }.to_string(),
                )
            })
            .collect()
    }

    #[test]
    fn a_switch_comes_back_the_way_it_was_left() {
        let file = merged("", set(&[("fog", false), ("clouds", true)]));
        let kept = Kept(parse(&file));
        assert_eq!(kept.switch("fog"), Some(false));
        assert_eq!(kept.switch("clouds"), Some(true));
        // Never written is not the same as written off.
        assert_eq!(kept.switch("veil"), None);
    }

    #[test]
    fn writing_one_switch_leaves_the_rest_of_the_file_alone() {
        let before = merged("", set(&[("fog", false), ("clouds", true)]));
        let after = merged(&before, set(&[("fog", true)]));
        let kept = Kept(parse(&after));
        assert_eq!(kept.switch("fog"), Some(true));
        assert_eq!(
            kept.switch("clouds"),
            Some(true),
            "a write about the fog forgot what was said about the clouds"
        );
    }

    #[test]
    fn a_line_this_build_does_not_know_survives() {
        // What an older build must do with a newer one's file: leave it be.
        // Otherwise going back a version silently resets settings.
        let after = merged("voice vault\nfog on\n", set(&[("fog", false)]));
        assert!(
            after.contains("voice vault"),
            "a setting this build knows nothing about was thrown away: {after}"
        );
    }

    #[test]
    fn a_switch_is_written_once_however_often_it_is_written() {
        let mut file = String::new();
        for _ in 0..5 {
            file = merged(&file, set(&[("fog", false)]));
        }
        assert_eq!(file.matches("fog ").count(), 1, "the file grew: {file}");
    }

    #[test]
    fn a_hand_edited_double_is_settled() {
        let after = merged("fog on\nfog off\n", set(&[("fog", true)]));
        assert_eq!(Kept(parse(&after)).switch("fog"), Some(true));
        assert_eq!(after.matches("fog ").count(), 1);
    }

    #[test]
    fn a_word_that_is_neither_falls_back_to_the_default() {
        assert_eq!(Kept(parse("fog maybe\n")).switch("fog"), None);
    }
}
