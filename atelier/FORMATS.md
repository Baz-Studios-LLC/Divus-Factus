# The file contract

The Atelier and Divus Factus share no code. Everything that passes between
them is a file described here, and this page is the single word on what
those files mean. The game exports its truth for the Atelier (`data/`);
the maker exports work for the game (`out/`), and the god carries it into
the world by hand.

## Game → Atelier

### `data/palette.json`

Written by the game: `cargo test export_palette_for_atelier -- --ignored`.
Re-run whenever the game's palette changes.

```json
{ "ramps": [ { "name": "wood", "steps": [[26,28,36], ...5 RGB steps...] } ] }
```

Colour in every other file is spoken as `{ "ramp": "wood", "shade": 0.7 }` —
a name and a 0..1 step, never raw RGB — so authored work inherits palette
changes for free.

### `data/rig.json` (to come)

The canonical body: part names, joint pivots, segment lengths as fractions,
exported from the game's builder so the Rig bench animates the true shape.
Parts: `body, head, leg-l, shin-l, leg-r, shin-r, arm-l, forearm-l, arm-r,
forearm-r` (hinges are children of their upper joints; rotations are about
local X: positive carries the free end forward).

## Atelier → game

### Blueprints: `out/buildings/<name>.json`

**The lattice**: the bench speaks in UNITS, and one unit - 1/16 m - is
the smallest measure that exists. Every coordinate and dimension is a
whole number of them. The files below still carry metres (divide by 16)
so the game's world and old exports stay true. Coarse work steps 4/16, joints land on
2/16, fine work on 1/16. Binary fractions carry exactly in floats, so
two parts that should meet, meet. STRUCTURAL DIMENSIONS obey it too:
walls 0.25 x 2.5, floors and roofs 0.125 thick, foundations 0.375,
trim 0.3125 high, door openings 1.25 x 2.125, windows 1.25 x 1.25
with the sill at 0.75. Furniture stays organic - nothing joins
against a stool.

Local space: +Y up, metres. Position and orientation on the bench are
FREE: the import rebases the building on its own bounds, and the door
widget defines the front - the whole blueprint is turned so the (first)
door faces the village. The gold sill (+X) is a working aid, not a law.
The bench saves to `out/buildings/workbench.baz` and reloads it on launch;
rename a finished piece in Finder to keep it. A saved building is a `.baz` —
JSON inside, as it always was, but named for the studio whose bench made it
rather than for the notation it happens to be written in. Files saved as `.json`
before the rename still open, and always will.

The file is a list of PARTS, not raw boxes — every entry is something the
game already understands, so the carrying-in is mechanical:

```json
{
  "format": 1,
  "name": "workbench",
  "parts": [
    { "part": "wall-2",       "at": [3.0, 0.0, 1.0], "yaw": 0.0, "tilt": 0.0,
      "ramp": null, "shade": 0.7, "stage": "walls" },
    { "part": "prop:bed",     "at": [-2.1, 0.0, 1.4], "yaw": 1.5708, "tilt": 0.0,
      "ramp": null, "shade": 0.7, "stage": "furnishing" },
    { "part": "widget:sleep", "at": [-2.1, 0.0, 1.4], "yaw": 1.5708, "tilt": 0.0,
      "ramp": null, "shade": 0.7, "stage": "widget" }
  ]
}
```

- `part`: `wall-<len>` (0.25 thick, 2.4 high — the Atelier's truth; the game conforms),
  `ridge-<len>` (the cap over a ridge line, half a metre across at the
  bench's pitch), `gable-<len>` (a wedge - a real triangular prism, peak at the
  bench's 45 degree pitch, so it stands half as tall as it is wide),
  `floor`, `roof`, `prop:<name>` (bed, table, stool, hearth, chair, bench,
  chest, barrel, crate, shelves, cupboard, pot, basket, rug, woodpile,
  candle, sack, trough — the shelf grows on request), or `widget:<kind>`.
  `prop:mannequin` is the scale reference and is SKIPPED on import.
  `prop:doorway` is an opening with no leaf, for interior walls, and it
  carries NO widget: a gap in a wall run is a portal by itself, found by
  walking the segments. A door widget means a real door - an entrance -
  and the first one defines the building's front.
  `prop:bed` and `prop:bed-double` carry their own sleep semantics when
  imported: a double becomes the household's marriage bed - the wedded
  pair claims it together, each to their own side, and children never
  do, whatever the shortage.
- EXPORT A COPY writes `out/buildings/build-<n>.json`, never overwriting;
  the SAVED WORK drawer lists everything in that folder on launch.
- `yaw` turns about the part's centre; `tilt` pitches roof panels.
- `flip`: mirrored - the body reflected across its own length and the
  tilt leaning the other way (the far half of a gable).
- `ramp`/`shade`: a repaint, or `null` for the part's authored colours.
- `stage`: `footing | frame | walls | roof | furnishing` — the order the
  village raises it; `widget` entries never become boxes at all.

### Clips: `out/anim/<name>.json`

```json
{
  "format": 1,
  "name": "sleeping",
  "seconds": 2.4,
  "looped": true,
  "tracks": [
    { "part": "shin-l", "keys": [ { "t": 0.0, "x": -0.1 }, { "t": 1.2, "x": -0.16 } ] }
  ]
}
```

- `t` in seconds; rotation in radians about the part's local X (`x`), with
  optional `y`/`z` for the head and body. Linear blend between keys; a
  looped clip blends its last key back into its first.
- Parts are the rig names above. Clips carry rotations only — the game
  retargets them across every genome's proportions.

### The baked building: `assets/buildings/<name>.json`

What the bench hands the game, written by `cargo test bake_the_works --
--ignored`. Not parts any more but the plain boxes they resolve to, each with
its colour already looked up, plus the marks that say what the place is FOR.

```json
{
  "format": 1, "name": "longhouse1-10people",
  "half_w": 3.65, "half_d": 6.7, "high": 5.2,
  "boxes": [ { "at": [0,1.25,0], "size": [4,2.5,0.25], "turn": [0,0,0,1],
               "form": "box", "rgb": [110,92,70], "alpha": 1.0, "cloth": "wood",
               "stage": "walls" } ],
  "marks": [ { "mark": "door", "at": [3.65,0.375,0.0], "yaw": 0.0 } ]
}
```

- `form` is the box's shape, and there are four. Both programs draw each one
  from its own code — they share none — so a shape is only the same shape in
  both because it is written out twice and named here:
  - `box`: the plain cuboid, which is most of everything.
  - `wedge`: a GABLE's prism. A triangle with its peak in the middle, standing
    across the part's length. Symmetric.
  - `ridge`: the same prism turned to run lengthwise, apex up — a ridge cap.
  - `mitre`: a right-angle prism, full height at -X and cut clean away to
    nothing at +X. What a saw leaves, and what a beam meeting a roof wants: a
    wedge cannot do it, being symmetric, and a tilted box cannot do it either,
    because the far end of a tilted box is still square.
  - `mitre-back`: the same cut the other way about, full height at +X. Which
    hand is wanted depends on which END of a beam is being capped, and a beam
    running between two slopes wants one of each.
- `half_w`/`half_d` are the FINISHED building's footprint: the plot the village
  clears, the obstacle while it is being raised, and the walkable shell when it
  is done.
- `cloth` names the ramp a box was painted from, so a village can re-dye a
  street of one blueprint into a street of different houses.

## Versioning

Every file carries `"format": 1`. When a format grows, the number moves and
this page says what changed; the game keeps reading old numbers.
