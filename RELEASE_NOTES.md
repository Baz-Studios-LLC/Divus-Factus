## The shipped game could not find its own files

**Every building drawn in the Atelier was missing from the released game.** The
village raised its old hand-built houses instead, in every world, on every
machine — while the same code run from the source tree raised the drawn ones.
Everything the game reads with its own hands looked in two places, and both of
them are only right in a source tree: a bundled app launched from Finder has no
asset root and a working directory of `/`, so the search missed quietly and the
village fell back to what it has always been able to build. Which looks exactly
like nothing being wrong.

**The voice lines were missing too, and nobody could have seen it** — a village
with no lines simply says less. It reads all 1140 of them now.

Both are fixed at the root: the places a data folder might be are written down
once, and every reader takes all of them, beginning with the folder beside the
program. The bench had it twice over — its palette was falling back to guesses at
the game's own colours in every launcher build.

## A building can be carried into the game by hand

**Draw it, press BAKE, start the game.** The Atelier's work used to reach the
village only through a developer's command line, which meant the bench in the
launcher build was a sketchpad with nowhere to send anything. There is a BAKE
glyph beside the save now: it writes the building where the game reads, alongside
the ones that shipped, and the next world raises it. A drawing of your own with
the same name as a shipped one replaces it.

**Every kind of building can take a drawing.** A work saved as `tavern-corner`
becomes the tavern the moment it is baked — no list to add it to. Once a kind has
one drawing, the village never falls back to its own hand for that kind again.

## Under the hood

- The game says which way each building went up: *the longhouse rises from the
  drawing longhouse1-10people*, or *the mine rises by the village's own hand — no
  drawing carried in*. There was no way to tell from the outside, which is how
  the fault above hid for a release.
- Drawing names are claimed by the longest kind-word that begins them, so `mill`
  no longer threatens to swallow `sawmill` and `smokehouse` the way `house` once
  swallowed `longhouse`.
- The rig bench: the head no longer flips when it is touched, limbs no longer
  shiver or swell under the hand, and a press on the scrub bar lands where the
  cursor is.
