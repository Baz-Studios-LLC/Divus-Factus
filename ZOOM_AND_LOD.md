# Continuous zoom from a longhouse to a planet

How *Divus Factus* renders one world from two metres to orbit with no map screen,
no orbit mode and no loading threshold anywhere in the climb. Written for someone
(or something) working on a different game and deciding what to borrow.

Numbers here are the real constants from the shipping code, not illustrations.
Engine is Bevy 0.19 / Rust, but almost nothing below is Bevy-specific.

---

## 1. The claim being satisfied

One camera, one scene graph, one continuous scroll wheel. The wheel that frames a
single building keeps turning until the whole globe sits in the window, then turns
back and lands on whatever you were looking at. There is no point in that range
where the game switches representation *visibly*, and no point where it stops
simulating.

That constraint is what makes the design interesting. If you are willing to have
a "world map mode" this is all much easier and you should do that instead.

---

## 2. The central trick: a flat simulation, a spherical picture

**The simulation never knows the world is round.** Every villager, building,
tree, pathfinding query and collision check works in flat `(x, z)` with `y` up, on
an infinite plane, exactly as it did before the planet existed. Terrain height is
a function `height_at(x, z)`. Nothing in the game logic does spherical maths.

**The renderer wraps that plane onto a sphere as the last step before drawing.**
A system (`bend_the_world`) runs in `PostUpdate`, *after* transform propagation,
and rewrites every drawable's `GlobalTransform`:

```
bend_frame(flat_xyz) -> (position_on_sphere, rotation)
unbend(sphere_pos)   -> flat_xyz
```

The flat point's distance from the world origin becomes an arc length on a sphere
of radius `PLANET_RADIUS = 6000`; its height becomes radial displacement. The
returned rotation stands the object up along the local surface normal, so a
longhouse a kilometre away leans away from you by the right angle and its
foundations stay in the ground.

Consequences worth knowing before copying this:

- **It is a rendering lie, and lies need a boundary.** Anything drawn but not
  simulated in flat space must be excluded from the bend: UI, the sky, the planet
  patches themselves, the camera rig, cloud shell, starfield. The exclusion list
  is explicit, and every new kind of drawable has to decide which side it is on.
  We got this wrong repeatedly — a hand model whose root was excluded but whose
  fingers were not, rivers that inherited an excluded parent's identity transform
  and sank 28 units into the ground.
- **Objects already seated on the sphere need a marker** (`BentInPlace`: "my
  vertices are already in world space, leave my transform alone").
- **Writing `GlobalTransform` directly means bypassing change detection**, which
  is fine, but it also means nothing downstream may re-propagate afterwards.
- **The plane's flatness is visible at scale.** A tangent plate 2,500 units wide
  on a 6,000-radius sphere lifts ~500 units off the curve at its edge — from orbit
  it reads as a square sticker jutting into space. This is the single biggest
  constraint on how much near ground you can draw at altitude (see §4).

The payoff is enormous and worth the boundary problems: no spherical pathfinding,
no pole singularities in gameplay, no re-derivation of any existing system, and
the whole sim can be developed and tested flat.

---

## 3. Two representations of ground

| | near ground ("chunks") | the planet ("patches") |
|---|---|---|
| geometry | flat grid, bent by the renderer | true sphere, vertices baked in world space |
| tile size | `CHUNK_SIZE = 64` units | varies with depth, 92 units to a sixth of the planet |
| grid | `CHUNK_CELLS = 32` per side | `PATCH_CELLS = 32` per side, at *every* depth |
| lifetime | streamed in a radius around the camera | quadtree, cached, partly permanent |
| simulation | yes — this is what everything walks on | no, purely visual |
| collision | yes | never |

Both are drawn **at the same time, by the same camera**, on different render
layers. The planet is not a view you switch to; it is the far ground of the only
view there is. At play height the patches are what you see past the edge of the
streamed ring; at orbit the ring has tapered away and the patches are the world.

The handover between them is the one seam in this design, and §8 is honest about
it.

---

## 4. Chunk streaming: radius from altitude, and back down again

```rust
pub fn stream_radius(camera_distance: f32) -> i32 {
    let chunks = (camera_distance / CHUNK_SIZE) * 1.6 + MIN_VIEW_CHUNKS as f32;
    let wanted = (chunks.round() as i32).clamp(MIN_VIEW_CHUNKS, VIEW_CHUNKS);
    // taper back DOWN above 1,400 units
    let receding = ((camera_distance - 1_400.0) / 4_000.0).clamp(0.0, 1.0);
    let ceiling = VIEW_CHUNKS as f32 + (MIN_VIEW_CHUNKS as f32 - VIEW_CHUNKS as f32) * receding;
    wanted.min(ceiling.round() as i32)
}
```

`MIN_VIEW_CHUNKS = 6`, `VIEW_CHUNKS = 20`. The radius grows with altitude, peaks
at about 1,400 units, then **shrinks again** — because of the tangent-plate
problem in §2 and because from high up the planet's own patches are a better
picture of that ground anyway. A thousand chunks that are smaller than the pixels
they cost are a pure loss.

That peak is also, by construction, the worst frame in the game: 1,257 chunks,
~30,000 meshes, ~26,000 shadow casters. Every performance number below is measured
there.

Build budgets: `CHUNKS_PER_FRAME = 3` normally, more when a lot is due. Rationing
exists to *hide* arriving ground; a budget so small that it stops hiding anything
is worse than no budget.

---

## 5. The planet: a cube-sphere quadtree split on screen-space error

Six root patches, one per cube face (`face_axes` gives each an outward axis and
two spanning axes). Each patch is a `32×32` grid of cells covering a square of
its face, projected onto the sphere and displaced by the same `height_at` field
the chunks use — so the two representations agree about where mountains are, at
different resolutions.

**Depth is carried by the tree, not the grid.** A child patch covers a quarter of
the ground with the same 32×32 cells. That *is* the sharpening. `cell_arc()` is a
patch's cell size in world units:

```
cell_arc = (π/2 · PLANET_RADIUS) / (PATCH_CELLS · 2^level)
```

**The split test is in pixels, not distance:**

```rust
let px_per_radian = window_height / fov;              // fov = 0.62 rad
let distance      = (camera_to_patch_centre - reach).max(REFINE_FLOOR);
let sharp_px      = key.cell_arc() / distance * px_per_radian;
if sharp_px > SPLIT_PX && key.level < MAX_LEVEL { split }
```

`SPLIT_PX = 7.0`, `MAX_LEVEL = 8`, `REFINE_FLOOR = 300.0`.

This is the part most worth copying. A screen-space threshold means the tree is
automatically correct for any field of view, any window size, and any distance —
you never tune per-altitude LOD bands, because "how big is this on screen" is the
only question that matters. `SPLIT_PX` is a single knob trading sharpness against
patch count; 7 pixels deliberately keeps a faceted low-poly look rather than
dissolving into smoothness.

`REFINE_FLOOR` clamps how close the camera may count as being, so patches just
past the loaded ring stay sharp instead of refining to absurd depth underfoot.

### Retention, which matters more than generation

- `EVERGREEN = 4`: every patch at depth ≤ 4 is **built at world creation and
  never freed**. At that depth the whole planet is sharp from orbit, so zooming
  out builds *nothing* — the far view is prebaked. Level 5 would be 6,000
  patches, most of a gigabyte of vertex buffers, for tiles only wanted below
  ~3,000 units where the chunks are drawing the detail anyway.
- Deeper patches are kept in a map and **felled after `KEPT_FOR = 1800` frames
  off-screen**, entity and mesh both.
- `BUILDS_PER_FRAME = 6`, `BUILDS_PER_HURRIED_FRAME = 26`. Measured: on a descent
  1,800 patches fall due at once; 6/frame took five seconds of watching the
  planet sharpen, 26 brings it under two.
- A `paint_beat` counter invalidates *painting* separately from geometry, so the
  fog of war can sweep across a planet that already stands without rebuilding it.

### Depth buffer

`near` scales with zoom, `far = 70_000`, and **reverse-Z**. A 2-to-70,000 unit
range is not survivable with a conventional depth buffer; reverse-Z plus a
zoom-scaled near plane is what keeps it honest. If your engine does not do
reverse-Z by default, do this first — before any of the LOD work, because
z-fighting at scale will otherwise be misdiagnosed as an LOD bug.

---

## 6. Everything else that is gated on altitude

The same principle throughout: **stop paying for what cannot be seen, and let the
measurement decide where.**

**Shadows.** Cascades reach `SHADOW_REACH = 900` units from the eye. Pull further
back than that and the ground in view is beyond the last cascade, so nothing can
be cast onto it — but the shadow passes still ran. Disabling them past the reach
is worth **2.8 ms of a 27 ms frame** for a picture that measures *identical*
(0.00–0.01% of pixels differ, against a 0.03–0.06% noise floor). The threshold is
derived from the reach rather than written as an altitude, so moving one moves the
other. Hysteresis, or a camera hovering on the line flickers every shadow in the
world.

Measured visibility ladder — share of the frame that changes with shadows removed:

| altitude | 60 | 200 | 400 | 700 | 1000 | 1400 |
|---|---|---|---|---|---|---|
| frame changed | 0.74% | 1.47% | 0.46% | 0.10% | 0.01% | 0.00% |

**Scenery (trees, brush, boulders).** The single largest cost at the worst
altitude: ~5 ms, and removing it was the only change that reached the 60 fps cap.
Unlike shadows it is *not* invisible up there — an 8-unit tree at 1,400 units is
still ~8 pixels tall, and removing all scenery changes 1.00% of the frame. So this
one is a judgement, not a free win:

| altitude | 200 | 400 | 700 | 1000 | 1400 |
|---|---|---|---|---|---|
| scenery is this much of the frame | 14.03% | 13.64% | 4.12% | 1.79% | 1.00% |

The knee is between 700 and 1,000; the ceiling sits at 1,000.

Two design points here, both learned the hard way:

- **Cull on camera altitude, not on per-object distance.** A per-tree radius
  sounds more principled and looks far worse: at 400 units up the ground below is
  400 away and the horizon is 2,200, so any radius draws a bald ring around the
  middle of the view. An altitude cut has no boundary in it at all.
- **Dissolve, don't switch.** Each stand has its own threshold, spread over the
  band from 72% of the ceiling to the ceiling, keyed to a hash of the entity so
  its place in the queue is *fixed*. Hash the altitude instead and the same forest
  flickers while the camera hangs still. Quantise the distance before comparing or
  a hand resting on the scroll wheel strobes one stand. Measured in the picture:
  4.12% → 2.36% → 1.58% → 0.43% → 0.01% across the band.

A true alpha fade was rejected: the scenery shares the ground's opaque material
(see-through trees look worse than absent ones), and the alternative is a
per-frame transform on 30,000 entities, which re-propagates and re-bends all of
them — buying smoothness with a stutter is the wrong trade.

---

## 7. The fog of war at two scales, with one law

Worth a section because it is the most instructive bug in the whole system.

Unknown ground is hidden. Near the camera it is hidden by **cloths** — unlit
sheets over each chunk. On the planet there is no cloth small enough, so
veiled-ness is baked into patch **vertex alpha** and applied by the patch shader.

Three separate things had to be unified, and each was a visible defect until it
was:

1. **The same colour constant** (`VEIL_TINT`), read by both. Obvious.
2. **Applied after lighting.** Painting the veil colour into a *lit* surface does
   not give the veil colour — the sun's diffuse and specular both add to it, so
   the planet's veil came out half again as far toward white and shifted with the
   time of day while the cloths never moved. The patch shader mixes to the tint
   *after* `apply_pbr_lighting`, which makes it exactly the cloth's colour under
   any sun.
3. **The same law, not just the same density.** The cloths are a Beer-Lambert
   slab: looked at squarely a sheet takes its 0.9, and a grazing look travels
   further through it and comes out heavier, capped at 6× thickness. The patches
   took a flat 0.9 at every angle. Straight down the two measure identical; at an
   oblique view — which is most of the time from orbit — they differed by up to
   13/255, visible as a line exactly where the near ground ends. Both reduce to
   the same depth-independent expression:

```
k       = min(1 / |dot(surface_normal, to_eye)|, 6)
density = 1 - (1 - a)^k
```

which needs no sheet and no thickness, only the angle, and returns exactly `a`
looked at straight on.

---

## 8. What this design does badly

Stated plainly, because these are the parts a borrower needs to plan for.

- **The detail seam.** The chunk ring is visibly sharper than the patches it abuts.
  Measured by hiding the ring and diffing: the ring accounts for **15.96% of the
  frame at 1,000 units, 19.97% at 700, 53.44% at 400** (noise floor 0.01%). So the
  patches cannot stand in for the near ground at any altitude, and the ring cannot
  shrink further until this is fixed. This is the one job that would unlock the
  remaining frame time.
- **No geomorphing between patch levels.** A patch splitting is a visible pop.
  The screen-space threshold decides *when* to split correctly; it does nothing
  about making the transition smooth. Vertex morphing between parent and child
  heights is the standard answer and is not implemented here.
- **Skirts, not stitching.** Neighbouring patches at different depths crack at the
  join; a downward skirt around each patch curtains the gap rather than the
  meshes agreeing. Cheap and effective, but it is a curtain.
- **The tangent-plate lift** (§2) is a hard ceiling on how much flat ground can be
  drawn at altitude. It is why the streamed radius has to taper.

---

## 9. If you are porting this

In rough order of value:

1. **Reverse-Z and a zoom-scaled near plane, first.** Everything else is
   unmeasurable until depth precision is sound across the range.
2. **Screen-space error for LOD selection** (`cell_arc / distance * px_per_radian
   > threshold`). One knob, automatically right for every FOV and resolution.
3. **Flat simulation, bent at render time**, if your world is small enough that a
   tangent frame is workable and you have an existing flat codebase. Budget real
   time for the exclusion-list boundary; it is where all the bugs live.
4. **Constant grid, varying coverage** for tiles. Depth in the tree, not the grid.
5. **Prebake and never free the shallow levels.** Zooming out should allocate
   nothing.
6. **Invalidate painting separately from geometry** if anything view-dependent
   (fog of war, ownership, seasons) is baked into vertices.
7. **Gate expensive layers on camera altitude, with a hashed dissolve band**, and
   set the threshold from a measured pixel-difference ladder rather than by eye.

### On measuring any of this

Two habits did more for this system than any optimisation:

- **Diff pixels, not impressions.** Capture the same scene with a feature on and
  off and count the share of the frame that changed. Establish the noise floor
  first, by diffing two runs that differ in *nothing* — here it is 0.03–0.06%,
  because villagers walk and clouds drift between runs. A change under the floor
  is free by definition. This is what turned "shadows probably don't matter up
  there" into a number.
- **Interleave A and B inside one batch.** Frame time on this machine drifts 3–6
  ms *between* batches of runs — larger than most effects worth hunting. One run
  held a flat 16.7 ms across four windows and then measured 22.6 ms three times
  from a byte-identical command. A four-way comparison once had every config in
  pass 1 at 27.3 ms and every config in pass 2 at 24.4 — the pass, not the
  setting. Alternate, take the gap between adjacent pairs, three alternations
  minimum.

Also worth building early: a **per-layer toggle set** (scenery, near ground,
planet surface, water, buildings, people, shadows, fog, weather), each reachable
from both a settings switch and an environment variable. It replaces the habit of
writing a throwaway measurement dial, deleting it, and writing it again next
week — and a sweep script can then take each layer away in turn with a baseline
run either side.

A measurement that surprised us, as an argument for building that: taking the
planet's patches away made the frame **slower** by 0.9 ms in both passes. They
were earning their keep as an occluder. That killed a day of planned work before
it started.
