//! Where the game's own data files are, wherever the game is standing.
//!
//! Bevy finds its assets by itself - shaders, the title logo - but everything
//! the game reads with its own hands does not: the baked buildings, the voice
//! corpus, the authored clips. Those all asked the same two questions and got
//! the same two answers: `$BEVY_ASSET_ROOT/assets/...`, and the path the crate
//! was COMPILED at.
//!
//! Which works in the source tree and nowhere else. Launched from Finder a
//! bundled app has no `BEVY_ASSET_ROOT`, and its working directory is `/` - so
//! the first road becomes `/assets/buildings` and the second is a folder on the
//! build machine. Both miss, quietly, and the village falls back to the hand-
//! built defaults it has always had. Brett found it the hard way: "if I boot it
//! from the .command file I get the new house, but if I boot from the launcher
//! 0.3.16 it gives me the procedural house."
//!
//! The voice corpus was silently empty in every shipped build for the same
//! reason, which nobody could have seen: a village with no lines simply says
//! less.
//!
//! So the roads are written once, here, and every reader takes all of them.

use std::path::PathBuf;

/// Every place a folder of game data might be, best first.
///
/// `under` is a path below the assets root, like `assets/buildings`.
pub fn roads(under: &str) -> Vec<PathBuf> {
    let mut roads: Vec<PathBuf> = Vec::new();
    // Told outright: a soak, a capture, a test harness.
    if let Ok(root) = std::env::var("BEVY_ASSET_ROOT") {
        roads.push(PathBuf::from(root).join(under));
    }
    // Beside the program. This is a bundle's own layout - the packaging script
    // puts `assets` next to the binary in Contents/MacOS - and it is also where
    // a Windows zip unpacks to.
    if let Ok(exe) = std::env::current_exe()
        && let Some(beside) = exe.parent()
    {
        roads.push(beside.join(under));
        // And the Mac's other convention, in case the layout ever moves.
        roads.push(beside.join("../Resources").join(under));
    }
    // The working directory, for a run started from the source tree.
    roads.push(PathBuf::from(under));
    // And the tree itself, for `cargo test` and `cargo run` from anywhere.
    roads.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(under));
    roads
}

/// Where a maker's OWN work goes: the drawings and clips they bake out of the
/// Opificium, under the same roof as their saves.
///
/// It is not the bundle. A bundle is replaced whole on the next update and is
/// not writable besides, so anything a player makes has to live where their
/// saves do. This is the folder that makes the shipped pair self-sufficient:
/// the bench bakes into it and the game reads out of it, with no source tree
/// and no cargo anywhere in the story.
pub fn made_by_hand(under: &str) -> Option<PathBuf> {
    let home = std::env::var("HOME").ok();
    let base = if cfg!(target_os = "macos") {
        PathBuf::from(home?).join("Library/Application Support/Divus Factus")
    } else if cfg!(target_os = "windows") {
        PathBuf::from(std::env::var("APPDATA").ok()?).join("Divus Factus")
    } else {
        PathBuf::from(home?).join(".local/share/divus-factus")
    };
    Some(base.join(under))
}

/// The first of them that is a folder worth reading.
#[cfg_attr(not(test), allow(dead_code))]
pub fn folder(under: &str) -> Option<PathBuf> {
    roads(under).into_iter().find(|road| road.is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tree's own copy has to be reachable, or every test that reads a
    /// building, a clip or a line is testing nothing.
    ///
    /// The FOLDER, not its contents. It is where Opificium bakes to, and it
    /// stands empty whenever the buildings are being reauthored - a village
    /// with no drawings falls back to its own hand and plays perfectly well.
    /// Demanding a building in it made an empty bench a broken build.
    ///
    /// And it is the PROJECT'S folder now, not one inside `assets`: one path
    /// every game that uses the bench can agree on, rather than each of them
    /// telling the bench where to carry things.
    #[test]
    fn the_game_can_find_its_own_buildings() {
        let home =
            folder(crate::villager::work::baked::BAKED_UNDER).expect("the buildings are somewhere");
        assert!(
            std::fs::read_dir(&home)
                .expect("the folder opens")
                .flatten()
                .all(
                    |entry| entry.path().extension().is_none_or(|kind| kind == "json"
                        || entry.path().file_name().is_some_and(|f| f == ".gitkeep"))
                ),
            "{} holds a file that is not a baked drawing",
            home.display()
        );
    }

    /// Beside the program comes before the working directory, because a bundle
    /// launched from Finder has a working directory of `/` and would otherwise
    /// look for `/assets` - which is the whole bug this exists to answer.
    #[test]
    fn the_program_looks_beside_itself_before_it_looks_around() {
        let roads = roads("assets/buildings");
        let beside = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|at| at.join("assets/buildings")));
        let (Some(beside), Some(here)) = (
            beside.and_then(|road| roads.iter().position(|had| *had == road)),
            roads
                .iter()
                .position(|road| *road == PathBuf::from("assets/buildings")),
        ) else {
            panic!("the roads no longer include both the program and the cwd");
        };
        assert!(beside < here, "the working directory is asked too early");
    }
}
