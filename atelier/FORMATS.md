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

Local space: origin at the plot centre on the ground, **+X faces the
village** (the door side), +Y up. Metres.

```json
{
  "format": 1,
  "name": "house-of-the-long-hearth",
  "kind": "house",
  "boxes": [
    {
      "at": [0.0, 1.25, 0.0],
      "size": [0.24, 2.5, 6.4],
      "ramp": "wood", "shade": 0.7,
      "stage": "walls",
      "roof": false,
      "yaw": 0.0
    }
  ],
  "widgets": [
    { "kind": "sleep", "at": [-2.1, 0.3, 1.4], "yaw": 0.0 },
    { "kind": "door",  "at": [3.0, 0.0, 0.0],  "yaw": 0.0 }
  ]
}
```

- `stage`: `"footing" | "frame" | "walls" | "roof" | "furnishing"` — the
  order villagers raise it. The game maps footing to the mason's stone
  stage and the rest to the carpenter's thirds; `roof: true` marks boxes
  the H-key cutaway lifts (usually everything staged `"roof"`).
- `yaw` on a box turns it about its own centre (radians).

**Widgets** are invisible in the game; each is a place plus a facing
(`yaw`, radians, 0 = facing +X):

| kind    | colour on the bench | the game makes of it                       |
|---------|---------------------|--------------------------------------------|
| `sleep` | blue                | a bed slot; yaw points where the HEAD lies |
| `sit`   | amber               | a seat; yaw is the way the sitter faces    |
| `fire`  | red                 | hearth/camp: light, warmth, shelter draw   |
| `smoke` | grey                | a wisp source (chimneys, roof holes)       |
| `door`  | green               | a doorway in the shell for routing         |
| `work`  | purple              | where a craftsman stands to do the trade   |
| `store` | brown               | where goods pile                           |
| `light` | gold                | a small lamp                               |

Optional `"size"` on `fire`/`light` scales the effect (default 1.0).

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
