# Divus Factus — Technical Architecture

Rust + Bevy 0.19. One dependency: `bevy`. Everything else is written here on purpose —
see [Dependencies](#dependencies).

## Module map

Modules exist only where they earn their place. This is what is actually built:

| Module | Responsibility |
|---|---|
| `main` | App assembly, `WorldSeed`, lighting, capture mode |
| `calendar` | The world clock: day/night, the `Sky` state every light and shader reads |
| `rng` | Deterministic PCG32 with named sub-streams |
| `noise` | Value noise, fBm, ridged, domain-warped |
| `palette` | The master palette: 16 ramps × 5 steps |
| `terrain` | The terrain function, chunk streaming, raycasting, water |
| `camera` | God camera rig (focus + yaw/pitch/distance) |
| `creature` | Genome, body assembly, procedural animation, locomotion, ballistics |
| `scatter` | Trees, rocks, food-bearing bushes, wind sway |
| `villager` | Needs, utility scoring, settlement placement, families, births, coming of age; `work` holds the eleven trades, the stockpile, and staged mason+carpenter construction |
| `hand` | The Divine Hand: picking, carrying, throwing, UI-cursor mode |
| `render` | Post stack: tonemapping, AA, bloom, vignette, depth of field, grading; the hand overlay camera |
| `ui` | The interface kit: theme, panels, text roles, pointer context |
| `debug` | Live tuning HUD, inspector panel, screenshots |

Planned but not yet created: `belief`, `religion`, `divinity`, `miracles`, `history`,
`memory`, `construction`, `save`. They will be added when a milestone needs them, not
before.

## Load-bearing decisions

### The terrain function is authoritative; meshes are derived

`Terrain` exposes `height_at`, `normal_at`, `slope_at`, `is_walkable`. Gameplay never
raycasts render geometry to find out how high the dirt is. That keeps simulation
independent of how — or whether — the world is drawn there.

### Flat shading, not smooth

Terrain triangles do not share vertices; each carries its own face normal. Faceted shading
is what gives the low-poly landscape its form under a single directional light — smooth
normals would flatten a million triangles into an undifferentiated wash.

### One mesh, eighty materials, any number of creatures

Every body part of every creature is the same unit cube, scaled by its `Transform` and
tinted from a shared material cache indexed by palette entry. A hundred creatures cost one
mesh and eighty materials. It also enforces the palette by construction: a creature
*cannot* be coloured off-palette.

### Limbs sit at their joints

A limb entity is positioned at the shoulder or hip, with the visible box parented beneath
it and offset by half its length. Rotating the limb entity swings the box about the joint.
This is what makes procedural animation possible without a skinned mesh or an animation
asset.

### Animation is computed, never authored

A creature whose proportions are decided at runtime cannot have hand-authored animation.
Gait is derived from the genome: stride frequency from leg length, swing amplitude from
speed, bob at twice stride frequency. Generate a creature with longer legs and its walk
adapts on its own — which is the only reason procedurally generated wildlife is tractable
at all.

### Rendering: full resolution, sharp

```
God camera (HDR) ──► tonemap ──► FXAA ──► bloom ──► vignette ──► depth of field ──► window
```

An earlier version rendered to a quarter-resolution target and snapped every pixel to the
master palette, chasing a pixel-art look. It was abandoned: the geometry is genuinely 3D,
and forcing it through a low-resolution buffer cost sharpness without buying the authority
of real pixel art. Sharp low-poly is its own look and suits these models better.

The palette remains as the *authoring* source for every colour, which is what keeps
procedurally generated art coherent. What went away is the post-process that re-quantised
the finished image back onto it.

Style is now carried by depth of field. Tilt-shift blur makes real scenes read as miniature
models on a table, which is the game's premise rendered literally. The focal plane tracks
the camera's own focus point, so the subject stays sharp through a zoom.

Lighting is deliberately high-contrast: a bright warm key against a dim cool fill. A strong
ambient fill lights every surface roughly equally and erases the shading that gives
low-poly geometry its form — the "flat and dull" failure mode.

### Terrain is a function, not a heightmap

There is no stored heightmap. Elevation is a pure function of world position and seed, so
the ground can be sampled anywhere without anything having been generated there. That is
what makes the world unbounded: chunks are a *view* of the terrain function, built around
the camera and discarded when it leaves, and the simulation can ask `height_at` about a
place no chunk has ever covered.

Shape comes from three layers — continents (kilometre-scale, deciding sea from land),
domain-warped hills, and ridges faded in only on high ground so valleys stay walkable. The
land curve is deliberately steepened either side of sea level; left linear, the band where
height is barely above water spans hundreds of units and every shoreline reads as one vast
beach.

Streaming loads a disc of chunks around the camera, nearest first, a few per frame. Order
matters: ground under the player has to appear before the horizon, or the world visibly
assembles itself from the outside in.

Chunk meshes are smooth-shaded and indexed. Normals and colours derive from *world*
position, never from anything chunk-local, so neighbours agree exactly along their shared
edge and no seam appears — a test asserts this vertex by vertex. Heights are sampled once
into a local grid with a one-cell skirt and normals taken from that, which is the
difference between one noise evaluation per vertex and five.

Scatter is parented to its chunk, so unloading takes the trees with it, and its RNG is
seeded from the chunk coordinate — a chunk that unloads and reloads comes back identical
rather than rearranging its forest every time the player looks away.

Climate is a second, independent pair of fields (temperature and moisture) resolved into
five biomes, which choose both the ground ramp and which of the five tree shapes grow
there. Mountains come from their own low-frequency belt mask rather than from wherever the
continent noise happens to peak; without it the world comes out uniformly rolling.

### Scenery is baked, gameplay objects are not

Scenery used to be one entity per box — a tree was a trunk plus canopy slabs, each its own
entity. Across a streamed view that reached **186,000 entities and 30 fps**. The cost was
not generating them; it was transform propagation and visibility culling over that many,
every frame.

Trees and rocks are now baked into a single mesh per chunk. Bushes stay real entities
because they bear food and can be picked up. That split — *does the simulation touch it?* —
took the same view to 15,000 entities and 147 fps.

The cost is that baked scenery cannot be individually animated, so per-tree wind sway is
gone until it can be done in a vertex shader.

### Rivers are hydrology, run lazily

Rivers were once the zero-crossings of a noise field: they meandered convincingly and were
physically absurd — perched on hillsides, sagging across valleys. Water obeys one law, it
seeks level, and a river that ignores it reads as wrong to anyone watching.

The replacement is a hydrology pass restructured for a world with no "after generation":
it runs lazily, per region, the first time anything asks. Springs are chosen
deterministically in high country; each course walks the terrain's steepest descent with a
little momentum until it reaches the sea or a basin it cannot leave; the water level along
the course is clamped to never rise. **A river cannot flow uphill by construction** — the
law is enforced at generation time, so no fluid is simulated at run time and none is
needed. Courses are memoised behind a lock and filed into a spatial hash; the cache
changes when rivers are computed, never what they are, so determinism survives.

Channels are carved into `height_at` as a parabolic bed under the course's water level.
Deep flowing water is unwalkable, so pathfinding routes around rivers; the shallow edge is
fordable. Known limits, documented rather than hidden: courses do not branch or merge, a
basin ending is a crude pond, and the first query in a new region traces its whole
neighbourhood at once — a one-off cost that can stutter.

### Grass is baked geometry with shader wind

Blades are baked a few thousand at a time into one mesh per chunk — the same trade that
fixed the 186k-entity scenery — and streamed in a tight radius, because a blade at three
hundred units is smaller than a pixel. All motion lives in a vertex shader: each vertex
carries its bend weight in `uv.x` (0 at the root, 1 at the tip), and two travelling gusts
plus a per-blade flutter bend the tips while roots stay pinned. The material is an
*extension* of `StandardMaterial` overriding only the vertex stage, so lighting, shadows
and fog stay stock.

Two lessons worth keeping. `double_sided: true` exists to flip normals on back faces; for
blades with authored up-normals that lit half the meadow black — the setting wanted is
`cull_mode: None` with `double_sided` off. And **whatever bends in the main pass must bend
identically in the depth prepass**: the prepass renders with its own vertex stage, and
every pixel where the two disagree becomes a blade-shaped hole showing the sky — which
masquerades convincingly as "the grass is white". The wind therefore lives in a uniform
(the prepass binds no global clock), both vertex shaders carry byte-identical wind code,
and the material is alpha-masked because Bevy strips material bindings from depth-only
opaque prepasses.

### Fog hides the streaming, and is derived from it

Distance fog is computed from the radius actually loaded, never hard-coded, so it can't
fall behind the last chunk. Three things it has to get right, each learned by looking at a
frame and finding it wrong:

- **It is measured from the camera, but chunks load around the focus.** Zoomed out, the
  camera already sits hundreds of units back, so fog sized from the view radius alone
  swallows the entire scene. Offsetting by the orbit distance keeps the subject clear at
  any zoom.
- **It has to start late.** Beginning a third of the way out puts most of the screen behind
  haze. The job is to hide the edge of the world, not to veil the world.
- **The sky must be the same colour as the fog.** Geometry past the fog's end is drawn in
  the fog colour; if the sky behind it differs, the boundary draws a hard line. That is what
  made the square edge of the sea appear as a grey band with a visible corner across the
  horizon. `horizon_color` is the single source for both.

The sea is sized well past the furthest the fog can reach, for the same reason: two
triangles cost nothing, and a stingy quad puts a straight edge across the view.

### The stream radius follows the zoom

A god camera has to pull back far enough to survey a region, but streaming that radius
constantly means paying for a thousand chunks while looking at one village. The radius is
derived from camera distance — six chunks close in, twenty at full zoom-out — and the fog
reads the same figure, so the two can never disagree.

### Utility AI from the first need

`villager` scores its options rather than switching on a state enum, even though there are
currently only two. The shape it must grow into is a dozen competing needs, and retrofitting
scoring onto a state machine later would mean rewriting every behaviour. Adding a need means
adding a scorer.

### No physics engine

The only bodies that ever leave the ground are ones the player threw. A ballistic arc plus
a heightmap lookup covers that completely. Rapier or Avian would cost more in dependency
churn than they return.

### Families are proximity, and time is one number

Sex lives in the genome (beard and build make it readable at god height).
Marriages form between unwed adults who are *near* each other — no global
matchmaking roll — so the village's family tree emerges from movement, food
placement, and everywhere the player's hand has meddled. Births come from
couples; `Parentage` is recorded at birth; children carry a ticking `Childhood`
and have their bodies rebuilt as adults from the same genome. Death stays
survivable at the population scale because the breeding pool replenishes.

The settlement is an entity, not a resource: founded on a day, named in its
people's language, with `MemberOf` membership — built for the day there is more
than one. Every person carries a `Chronicle` of life events (birth always kept;
the middle of a long life is dropped before its beginning), written to by
births, weddings, coming of age, the god's touch, bereavement and death. Gossip
spreads witness accounts to neighbours in earshot; hearing increments
`Witnessed::secondhand`, never `total` — a faith built on rumour and a faith
built on witness are different faiths, and doctrine will care which one it
inherited.

All time derives from one `WorldClock` (600 s/day). The visual consequences are
funnelled through a single `Sky` resource — sun bearing, light colours, horizon,
daylight fraction — and lights, fog, `ClearColor` and the water shader all read
`Sky` rather than the clock. Seasons and weather, when they come, bend `Sky` in
one place instead of touching every consumer. `DIVUS_FACTUS_CLOCK=0.85` starts the
game at any hour, which is how dusk gets art-directed without waiting for it.

### One interface kit, and the hand is the only cursor

Every panel goes through `ui`: one panel builder (anchored frame, title strip), text
*roles* rather than ad-hoc styles (`heading` / `body` / `dim`), and a theme whose colours
are derived from the master palette — the interface is art-directed by the same ramps as
the terrain. Feature modules decide what words go in a panel, never what a panel looks
like. `PointerContext` is the boundary: one resource that knows whether the cursor is over
the interface or the world, asked by every input system before it acts.

The OS cursor stays hidden everywhere, because the Divine Hand *is* the cursor. Over the
world it behaves as before; over a panel it eases into a pointing pose (index extended,
the rest folded), pulls up to a fixed depth in front of the camera, and stops grabbing.
One eased scalar (`HandRig::point`) drives pose, position, scale and attitude together,
so crossing a panel edge is a gesture, not a swap.

To glide *over* panels instead of being hidden behind them, the hand lives on render
layer 1, drawn by a second camera (`HandCamera`, order 1) that is a child of the god
camera — child transform identity keeps the two views pixel-aligned with no sync system.
Two hard-won constraints: stacked cameras must agree on HDR or the writeback between
passes breaks entirely (magenta screen), and the overlay must run `Tonemapping::None`
because the image it composites onto has already been tonemapped once.

## Determinism

One `WorldSeed` drives terrain, scatter, creature bodies and settlement placement. Each
subsystem draws from a *named sub-stream* (`Rng::stream(seed, "scatter")`), so adding a die
roll in the terrain generator cannot shift the numbers the creature generator sees. Worlds
are reproducible from a single `u32`.

## Dependencies

Only `bevy`. Deliberately hand-rolled instead of pulled in:

- **RNG** — `rand`'s thread-local generators are not reproducible; a PCG32 is 60 lines.
- **Noise** — the whole procedural pipeline must be bit-reproducible; value noise and fBm
  are ~80 lines we fully control.
- **Physics** — see above.
- **egui** — the tuning HUD is currently keyboard-driven Bevy UI. `bevy_egui` is the
  obvious upgrade when the panel outgrows function keys; it is version-coupled to Bevy, so
  it is worth deferring until it actually pays for itself.

## Testing

85 tests, all pure-logic, no Bevy app required. They cover the parts where being wrong is
silent: noise range and continuity, RNG stream independence, palette ramp monotonicity,
terrain invariants (rim submerged, a usable fraction walkable, chunks tiling exactly,
sampling off-slab does not panic), genome bounds across 500 generations of inheritance,
population variety, rig topology and non-coplanar geometry, gait timing, raycast accuracy,
throw-velocity estimation, scroll-unit normalisation, and settlement placement.

Bugs caught this way: an integer overflow in the noise octave seed, and a settlement scorer
that put the whole population on a sandbar. Several others were found by looking at frames
rather than by tests — heads z-fighting with hair, quadrupeds staring at the sky because
the animator overwrote their rest pose, and every creature walking backwards because the
facing angle aimed +Z along travel instead of -Z. Each now has a regression test.

## Running

```bash
cargo run
```

There are no asset files — every mesh, colour and animation is generated at runtime.

Unattended screenshot (renders one frame to a file and exits):

```bash
DIVUS_FACTUS_CAPTURE=/tmp/shot.png cargo run
```

Capture mode routes the final image through an offscreen target rather than reading back
the window. Window readback depends on the compositor actually drawing the window, which
macOS will not do when it is unfocused — an unattended run otherwise returns solid black.
