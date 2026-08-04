//! One switch per layer of the world, so a cost can be attributed rather than
//! guessed at.
//!
//! Brett's idea, and it replaces how the frame-time work had been going: a
//! temporary env dial written for one measurement, deleted, then written again
//! from scratch the next time the same question came up. Three of those got
//! added and removed in a single afternoon. A permanent set of toggles is the
//! same work done once — a sweep script can drive each one from the environment
//! and take the layers away in turn, and the same switches sit in the settings
//! so the god can see what each layer is worth to the PICTURE, which is the half
//! of the trade a stopwatch cannot tell you.
//!
//! Note what this is NOT: a quality setting. A layer is either there or it is
//! not, and several of these would make the world wrong if a player left them
//! off. They live under the view settings as instruments.

use bevy::prelude::*;

use crate::matter::Boulder;
use crate::scatter::{Foliage, GroveMesh};

/// A layer of the world that can be taken away whole.
///
/// Only things heavy enough to be worth attributing. Two more layers exist as
/// switches but keep their state elsewhere, because they already had an owner
/// before this module: the weather deck in [`crate::clouds::TheSkyIsClear`] and
/// the veil in [`crate::fog::FogMode`]. Duplicating those here would give each
/// of them two truths, and a switch that reads one while the world reads the
/// other is worse than no switch at all.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Layer {
    /// Trees, bushes and boulders: the merged grove meshes and everything that
    /// sways or can be pushed over.
    Scenery,
    /// The planet's own quadtree surface, under and beyond the streamed chunks.
    Patches,
    /// The sea.
    Water,
    /// Everything the village has raised.
    Buildings,
    /// The villagers themselves.
    Folk,
    /// The sun's shadows. Not a visibility layer — it reaches the light itself,
    /// through [`crate::calendar`].
    Shadows,
}

impl Layer {
    pub const ALL: [Layer; 6] = [
        Layer::Scenery,
        Layer::Patches,
        Layer::Water,
        Layer::Buildings,
        Layer::Folk,
        Layer::Shadows,
    ];

    /// What the switch is called.
    pub fn label(self) -> &'static str {
        match self {
            Layer::Scenery => "scenery",
            Layer::Patches => "planet surface",
            Layer::Water => "water",
            Layer::Buildings => "buildings",
            Layer::Folk => "villagers",
            Layer::Shadows => "shadows",
        }
    }

    /// What it says when it is on: the state, not the name of the flag.
    pub fn note(self) -> &'static str {
        match self {
            Layer::Scenery => "trees, brush and stone",
            Layer::Patches => "the world beyond the near ground",
            Layer::Water => "seas and rivers",
            Layer::Buildings => "what the village has raised",
            Layer::Folk => "the people, and their bodies",
            Layer::Shadows => "cast by the sun",
        }
    }

    /// The environment dial that starts this layer hidden, without its prefix.
    ///
    /// `DIVUS_FACTUS_LAYER_SCENERY=0` and so on, one spelling for all of them,
    /// so a sweep script can build the name from the label rather than carrying
    /// a table of special cases.
    pub fn dial(self) -> &'static str {
        match self {
            Layer::Scenery => "SCENERY",
            Layer::Patches => "PATCHES",
            Layer::Water => "WATER",
            Layer::Buildings => "BUILDINGS",
            Layer::Folk => "FOLK",
            Layer::Shadows => "SHADOWS",
        }
    }
}

/// How far the god may pull back before the scenery stops being drawn.
///
/// Trees are the single largest cost in the frame at the altitude where frames
/// drop — measured at five to eight milliseconds of twenty-five, and taking them
/// away was the only change all day that reached the sixty-frame cap. Unlike the
/// shadows they are not invisible up there: an eight-unit tree at fourteen
/// hundred units still stands about eight pixels tall, and removing the scenery
/// changes 1.00% of the frame against a 0.13% noise floor. So this is not a free
/// win being collected — it is a judgement that a forest reading as green ground
/// from a great height is worth the frames, and Brett's judgement to make with
/// his eyes rather than mine with a pixel count.
///
/// An ALTITUDE, deliberately, and not a distance from each tree. Culling by the
/// tree's own distance sounds more principled and looks far worse: at four
/// hundred units up the ground below is four hundred away and the horizon is two
/// thousand, so any per-tree radius draws a bald ring around the middle of the
/// view. Cutting on how far the god has pulled back has no boundary in it at
/// all — the scenery is either there or it is not, and the altitude is chosen so
/// that when it goes it was only ever a green tint.
///
/// `DIVUS_FACTUS_SCENERY_CEILING` moves it for eyeballing.
fn scenery_ceiling() -> f32 {
    std::env::var("DIVUS_FACTUS_SCENERY_CEILING")
        .ok()
        .and_then(|dial| dial.parse().ok())
        .unwrap_or(1000.0)
}

/// Whether the scenery should still be drawn, given how far back the eye is.
///
/// Hysteresis for the same reason the shadows have it (see
/// [`crate::calendar::shadows_can_land`]): a god hovering exactly on the line
/// would otherwise plant and fell every forest in the world once a frame.
pub fn scenery_within_reach(distance: f32, currently_drawn: bool) -> bool {
    let ceiling = scenery_ceiling();
    if currently_drawn {
        distance < ceiling * 1.08
    } else {
        distance < ceiling
    }
}

/// Which layers are currently taken away.
#[derive(Resource)]
pub struct ViewLayers {
    hidden: [bool; Layer::ALL.len()],
}

impl Default for ViewLayers {
    /// Every layer on, unless its dial says otherwise.
    ///
    /// `DIVUS_FACTUS_SHADOWS=0` is honoured as well as the newer
    /// `DIVUS_FACTUS_LAYER_SHADOWS=0`, because the older spelling is written
    /// into a good deal of this project's measurement history and silently
    /// dropping it would make those numbers unreproducible.
    fn default() -> Self {
        let mut hidden = [false; Layer::ALL.len()];
        for (slot, layer) in hidden.iter_mut().zip(Layer::ALL) {
            let off = |name: String| std::env::var(name).is_ok_and(|dial| dial == "0");
            *slot = off(format!("DIVUS_FACTUS_LAYER_{}", layer.dial()))
                || (layer == Layer::Shadows && off("DIVUS_FACTUS_SHADOWS".to_string()));
        }
        ViewLayers { hidden }
    }
}

impl ViewLayers {
    fn slot(layer: Layer) -> usize {
        Layer::ALL.iter().position(|&l| l == layer).unwrap_or(0)
    }

    pub fn shown(&self, layer: Layer) -> bool {
        !self.hidden[Self::slot(layer)]
    }

    pub fn hidden(&self, layer: Layer) -> bool {
        self.hidden[Self::slot(layer)]
    }

    pub fn toggle(&mut self, layer: Layer) {
        let slot = Self::slot(layer);
        self.hidden[slot] = !self.hidden[slot];
    }
}

pub struct LayerPlugin;

impl Plugin for LayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ViewLayers>()
            // PostUpdate, and that placement is the point: the systems that
            // legitimately govern these entities' visibility run in Update -
            // the quadtree hiding patches it has refined away, the veil
            // revealing chunks as they are dressed - and a toggle has to be the
            // LAST word or the two fight and the layer flickers.
            .add_systems(PostUpdate, apply_layers);
    }
}

/// Takes away the layers that are switched off, and gives back the ones that
/// have just been switched on.
///
/// One query over everything governed, with `Has` telling each entity which
/// layer it belongs to. The obvious shape - a query per layer - cannot be
/// written without a matrix of `Without` filters to prove to Bevy that no
/// entity is in two of them at once, and that matrix grows as the square of the
/// layer count and breaks the moment a boulder becomes something else too.
#[allow(clippy::type_complexity)]
fn apply_layers(
    layers: Res<ViewLayers>,
    eyes: Query<&crate::camera::CameraRig, With<crate::camera::GodCamera>>,
    mut scenery_drawn: Local<Option<bool>>,
    mut governed: Query<
        (
            &mut Visibility,
            Has<GroveMesh>,
            Has<Foliage>,
            Has<Boulder>,
            Has<crate::globe::Patch>,
            Has<crate::terrain::WaterPlane>,
            Has<crate::villager::work::buildings::Building>,
            Has<crate::villager::Villager>,
        ),
        Or<(
            With<GroveMesh>,
            With<Foliage>,
            With<Boulder>,
            With<crate::globe::Patch>,
            With<crate::terrain::WaterPlane>,
            With<crate::villager::work::buildings::Building>,
            With<crate::villager::Villager>,
        )>,
    >,
) {
    // Hiding is enforced EVERY frame, because the systems that own these
    // entities keep setting them visible again. Showing happens only on the
    // frame a switch flips: handing everything back to `Inherited` every frame
    // would override the quadtree's own culling and put the whole planet's
    // refined-away surface back on screen.
    // The scenery answers to the altitude as well as to its switch. Held in a
    // `Local` because the answer depends on the last answer - see
    // [`scenery_within_reach`].
    let was_drawn = scenery_drawn.unwrap_or(true);
    let draw_scenery = eyes
        .iter()
        .next()
        .map(|rig| scenery_within_reach(rig.distance, was_drawn))
        .unwrap_or(true);
    let scenery_changed = *scenery_drawn != Some(draw_scenery);
    *scenery_drawn = Some(draw_scenery);

    let restoring = layers.is_changed() || scenery_changed;
    for (mut visibility, grove, foliage, boulder, patch, water, building, folk) in &mut governed {
        let layer = if grove || foliage || boulder {
            Layer::Scenery
        } else if patch {
            Layer::Patches
        } else if water {
            Layer::Water
        } else if building {
            Layer::Buildings
        } else if folk {
            Layer::Folk
        } else {
            continue;
        };
        let hidden = layers.hidden(layer) || (layer == Layer::Scenery && !draw_scenery);
        if hidden {
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
        } else if restoring && *visibility == Visibility::Hidden {
            *visibility = Visibility::Inherited;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_layer_has_its_own_slot() {
        // The slot is found by searching ALL, so a duplicate entry would give
        // two layers one switch and make the second unreachable.
        for layer in Layer::ALL {
            let mut layers = ViewLayers {
                hidden: [false; Layer::ALL.len()],
            };
            layers.toggle(layer);
            for other in Layer::ALL {
                assert_eq!(
                    layers.hidden(other),
                    other == layer,
                    "toggling {layer:?} moved {other:?} as well - two layers \
                     are sharing a slot"
                );
            }
        }
    }

    #[test]
    fn every_layer_has_a_distinct_dial_and_label() {
        for layer in Layer::ALL {
            for other in Layer::ALL {
                if layer == other {
                    continue;
                }
                assert_ne!(layer.dial(), other.dial(), "{layer:?} and {other:?}");
                assert_ne!(layer.label(), other.label(), "{layer:?} and {other:?}");
            }
        }
    }

    /// How much of the frame the scenery IS, by how far back the eye has pulled:
    /// the share of pixels that changed when it was taken away, each against a
    /// noise floor measured the same way in the same batch.
    ///
    ///   alt  200: 14.03% (noise 0.34%)      alt  700:  4.12% (noise 0.01%)
    ///   alt  400: 13.64% (noise 1.23%)      alt 1000:  1.79% (noise 0.01%)
    ///                                       alt 1400:  1.00% (noise 0.13%)
    ///
    /// A seventh of the frame at play distances and a fortieth high up. The knee
    /// is between seven hundred and a thousand, which is where the ceiling sits.
    const MEASURED: [(f32, f32); 5] = [
        (200.0, 14.03),
        (400.0, 13.64),
        (700.0, 4.12),
        (1000.0, 1.79),
        (1400.0, 1.00),
    ];

    /// Where a forest stops being the picture and becomes a tint on the ground.
    ///
    /// A judgement and not a measurement — the numbers say 1.79% at a thousand
    /// units and 4.12% at seven hundred, and nothing in them says which of those
    /// a player would miss. Two percent is where Brett drew it by eye, and
    /// `DIVUS_FACTUS_SCENERY_CEILING` is there to move it.
    const A_TINT: f32 = 2.0;

    #[test]
    fn the_scenery_is_never_cut_where_it_is_the_picture() {
        for (distance, share) in MEASURED {
            if share <= A_TINT {
                continue;
            }
            assert!(
                scenery_within_reach(distance, false),
                "the scenery is {share}% of the frame at distance {distance} - \
                 cutting it there would be cutting the world, not an optimisation"
            );
        }
    }

    #[test]
    fn the_scenery_is_cut_where_it_is_only_a_tint() {
        for (distance, share) in MEASURED {
            if share > A_TINT {
                continue;
            }
            assert!(
                !scenery_within_reach(distance, false),
                "the scenery is only {share}% of the frame at distance \
                 {distance} and costs two milliseconds of twenty-four, yet is \
                 still being drawn"
            );
        }
    }

    #[test]
    fn the_ceiling_does_not_flicker_when_hovering_on_it() {
        let on_the_line = scenery_ceiling() * 1.04;
        assert!(
            scenery_within_reach(on_the_line, true),
            "scenery already drawn must survive a small drift outward"
        );
        assert!(
            !scenery_within_reach(on_the_line, false),
            "scenery already cut must not return on the same drift, or every \
             forest in the world is planted and felled once a frame"
        );
    }

    #[test]
    fn layers_start_shown() {
        let layers = ViewLayers {
            hidden: [false; Layer::ALL.len()],
        };
        for layer in Layer::ALL {
            assert!(layers.shown(layer), "{layer:?} should start visible");
        }
    }
}
