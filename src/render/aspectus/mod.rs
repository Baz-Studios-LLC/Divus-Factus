//! ASPECTUS: the passes this game owns.
//!
//! Everything else the game draws goes through Bevy's `core_3d` pipeline with
//! our materials hung on it - the ground and its veil, the planet's skin, the
//! grass, the clouds, the sky. Those are SURFACES. What lives here are PASSES:
//! work done over the whole frame, in the render graph, on our terms.
//!
//! A renderer is not a thing you declare. It is a thing you take over one job
//! at a time, and each job has to earn its place by doing something the
//! pipeline was doing worse for this particular game.
//!
//! - **[`frost`]** blurs the frame behind an open book. It earned its place by
//!   deleting a camera: the book used to wake a second `Camera3d` that
//!   rendered the whole world again into a 480x270 image.
//! - **[`veil`]** reads the depth the world just wrote and veils everything
//!   standing in unwalked country - villagers, animals, buildings, anything
//!   that will ever be added - where the material veil could only cover what
//!   opted in, one material at a time.
//!
//! What Bevy keeps, because replacing it would buy this game nothing: PBR
//! lighting, shadow maps, transparency sorting, bloom.
//!
//! What is still worth taking, when the frame wants it: our own tonemapping
//! and grading (Bevy's is a good FILMIC curve, and this is a painted diorama,
//! not a photograph), an outline pass over depth and normals (the world is low
//! poly boxes, which is exactly what edge detection flatters, and it would let
//! the hover highlight stop being real geometry), and a tilt-shift depth of
//! field keyed to height in frame rather than to a virtual lens.

/// Whether a pass may run at all, for measuring what it costs.
///
/// `DIVUS_FACTUS_NO=mist,veil,frost` turns them off, comma separated. This is
/// the only way to price a full-screen pass in this game: the main-world
/// stopwatch says the SIMULATION is innocent - zero slow frames in fourteen
/// minutes - while frames were taking 96ms, so seventy of those milliseconds
/// are being spent past Update, where nothing was measuring.
///
/// A/B is the only measurement that counts here, because frame times in this
/// game drift several milliseconds between runs for reasons that have nothing
/// to do with the change being tested.
/// Passes switched off from the settings screen, by name.
///
/// AN ATOMIC, because a render pass runs in the render world and cannot read a
/// game resource. Brett wanted the fog off while he watched the framerate -
/// "Can we get a toggle in The View for the fog? I would like to test the game
/// with the fog completely turned off" - and a shared bit is the whole
/// mechanism that needs.
pub static MIST_IS_OFF: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Turns the mist off, or on.
pub fn set_mist_off(off: bool) {
    MIST_IS_OFF.store(off, std::sync::atomic::Ordering::Relaxed);
}

/// Whether the mist is off right now — by the switch, or by the dial that
/// forbade it before the window opened.
///
/// The dial is folded in HERE rather than only in [`pass_is_wanted`] so that
/// there is one truth: the settings screen reads this to draw its switch, and a
/// switch reading on over a pass that a command line had already forbidden
/// would be a switch that lies.
pub fn mist_is_off() -> bool {
    MIST_IS_OFF.load(std::sync::atomic::Ordering::Relaxed) || pass_is_forbidden("mist")
}

/// Whether THIS RUN's command line forbade a pass outright.
///
/// Stronger than the switch and unanswerable by it, which is the point: a dial
/// is an instruction about one launch, and a measurement run that quietly obeyed
/// a settings file clicked days ago would be worthless.
pub fn pass_is_forbidden(name: &str) -> bool {
    static OFF: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    let off = OFF.get_or_init(|| {
        std::env::var("DIVUS_FACTUS_NO")
            .unwrap_or_default()
            .split(',')
            .map(|word| word.trim().to_ascii_lowercase())
            .filter(|word| !word.is_empty())
            .collect()
    });
    off.iter().any(|word| word == name)
}

pub fn pass_is_wanted(name: &str) -> bool {
    !(pass_is_forbidden(name) || (name == "mist" && mist_is_off()))
}

use bevy::prelude::*;

pub mod frost;
pub mod mist;
pub mod veil;

pub use frost::Frost;
pub use mist::{MistFieldImage, MistView};
pub use veil::VeilView;

pub struct AspectusPlugin;

impl Plugin for AspectusPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((frost::FrostPass, veil::VeilPass, mist::MistPass));
    }
}
