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
//! A second pass - the veil, reading depth so that villagers and animals are
//! hidden by it too - is written but not landed: it runs without crashing now
//! and paints the whole world black, which is a bug in the pass rather than in
//! the plumbing. It waits on the branch `aspectus-veil-pass`.
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

use bevy::prelude::*;

pub mod frost;

pub use frost::Frost;

pub struct AspectusPlugin;

impl Plugin for AspectusPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(frost::FrostPass);
    }
}
