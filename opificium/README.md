# Opificium

This folder is a **project**: one game's own authored work for
[Opificium](https://github.com/Baz-Studios-LLC/Opificium), a maker's bench for
buildings and animations. Opificium made this folder and this note.

The bench and the game **share no code**. Everything that passes between them is a
file described here.

## What is in here

| path                | who writes it | what it is                                        |
| ------------------- | ------------- | ------------------------------------------------- |
| `opificium.json`   | you           | which folders this project uses. Every path has a default, so it may be nearly empty |
| `data/palette.json` | the game      | the colour ramps the bench paints with            |
| `data/kinds.json`   | either        | what a finished drawing may be baked AS           |
| `data/widgets.json` | either        | the marks the bench may place, and their colours  |
| `data/bodies/`      | the game      | bodies for the animation bench to pose            |
| `templates/`        | you           | starting shapes to draw from                      |
| `out/buildings/`    | the bench     | saved drawings, `.baz` - **the source of truth**  |
| `out/baked/`        | the bench     | baked output, when no `install` is set            |

A `.baz` is JSON. It is the editable drawing and the thing worth keeping.

## The palette, the kinds and the marks

`data/palette.json` is the one file the game really must provide, or the bench
paints in its own colours instead of the game's:

```json
{ "ramps": [ { "name": "wood", "steps": [[28,19,16], "...5 RGB steps..."] } ] }
```

`kinds.json` and `widgets.json` are **vocabulary the game understands**. The bench
offers only what is listed, and a word it is given is passed through untouched:

```json
{ "format": 1, "kinds": [ { "word": "house" }, { "word": "townhall", "label": "TOWN HALL" } ] }
{ "format": 1, "marks": [ { "mark": "door", "ramp": "cloth-green", "shade": 0.6 } ] }
```

**These are contracts, not data.** The game matches these words against its own
code. A word the game does not understand costs whatever it was attached to, and
nothing in the bench can catch that - it cannot see the game's source. Keep them
true, or have the game generate these two files the way it generates the palette.

## What the bench hands the game

Baking resolves a drawing into plain boxes with colours already looked up, and
writes it where `install` in the manifest points - otherwise `out/baked/`. That
output is **generated**: bake it again rather than editing it.

```json
{
  "format": 2,
  "name": "...", "kind": "...",
  "half_w": 3.6, "half_d": 4.2, "high": 7.6,
  "boxes": [ { "at": [0,1.25,0], "size": [4,2.5,0.25], "turn": [0,0,0,1],
               "form": "box", "rgb": [110,92,70], "alpha": 1.0, "stage": "walls" } ],
  "marks": [ { "mark": "door", "at": [3.6,0.4,0.0], "yaw": 0.0 } ],
  "levels": [ { "name": "", "half_w": 3.6, "half_d": 4.2, "high": 7.6,
                "phases": [ { "boxes": ["..."] } ], "marks": ["..."] } ]
}
```

- `boxes` and `marks` are the **base building, finished**. A reader that wants
  nothing else can read only these and ignore the rest.
- `levels` is the building's whole life: the original, then each upgrade. Every
  level carries its own `phases` - one COMPLETE set of boxes per step of raising
  it - and its own footprint and marks, all measured from one shared origin, so an
  upgrade lands on the building it upgrades.
- `stage` on a box says what it IS - `footing`, `frame`, `walls`, `roof`,
  `furnishing` - which is useful for cutaways and for raising a level without
  reading its phases.
- `form` is the box's shape: `box`, `wedge`, `ridge`, `hip:<x>x<z>`, or
  `cut:<low>x<high>`. Both programs build each shape from their own code, so a new
  form must be written twice.
- Local space is +Y up, metres, and every measurement is a whole number of
  sixteenths of a metre.

Re-bake without opening a window:

```sh
opificium <this folder> --bake
```

## What to commit, and what to ignore

Commit everything a person authored:

```
opificium.json
data/
templates/
out/buildings/
```

Ignore what is generated, since it is rebuilt from the drawings on demand:

```gitignore
out/baked/
out/buildings/workbench.baz
```

`workbench.baz` is the bench's scratch pad - whatever was standing when it last
had to keep something. Renaming a drawing is how it becomes worth committing.

The baked output under the game's own assets folder is generated too. Whether to
commit it is the game's call: ignoring it means the game cannot be built without
running the bake first, and committing it means reviewing generated JSON.
