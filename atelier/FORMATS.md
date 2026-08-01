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

**The lattice**: every coordinate and dimension is a whole multiple of
1/16 m - the universal quantum. Coarse work steps 4/16, joints land on
2/16, fine work on 1/16. Binary fractions carry exactly in floats, so
two parts that should meet, meet.

Local space: +Y up, metres. Position and orientation on the bench are
FREE: the import rebases the building on its own bounds, and the door
widget defines the front - the whole blueprint is turned so the (first)
door faces the village. The gold sill (+X) is a working aid, not a law.
The bench saves to `out/buildings/workbench.json` and reloads it on launch;
rename a finished piece in Finder to keep it.

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
  `floor`, `roof`, `prop:<name>` (bed, table, stool, hearth, chair, bench,
  chest, barrel, crate, shelves, cupboard, pot, basket, rug, woodpile,
  candle, sack, trough — the shelf grows on request), or `widget:<kind>`.
  `prop:mannequin` is the scale reference and is SKIPPED on import.
  `prop:bed` and `prop:bed-double` carry their own sleep semantics when
  imported: a double becomes the household's marriage bed - the wedded
  pair claims it together, each to their own side, and children never
  do, whatever the shortage.
- EXPORT A COPY writes `out/buildings/build-<n>.json`, never overwriting;
  the SAVED WORK drawer lists everything in that folder on launch.
- `yaw` turns about the part's centre; `tilt` pitches roof panels.
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

## Versioning

Every file carries `"format": 1`. When a format grows, the number moves and
this page says what changed; the game keeps reading old numbers.
