## Windows draws through Direct3D 12

**The game asked Windows for the wrong graphics path.** Windows offers two, and
whichever turned up first was taken — which on a Legion with an RTX 4060 was
Vulkan, the path that gets the least attention there. It asks for Direct3D 12
now, which is what a Windows game is expected to speak and what NVIDIA's driver
is tuned hardest for. Setting `WGPU_BACKEND` still overrides it, for anyone whose
card prefers otherwise.

## Stairs

**A proper flight, in timber or stone.** Treads on an even rhythm, newel posts,
balusters standing on the steps, and a handrail that runs at the flight's own
angle rather than stepping up in blocks. It replaces a three-tread stone doorstep
that had no rail at all — and buildings already drawn keep their old steps.

- **Three handles.** Across for its width, along for its run — and a longer run
  is a taller flight, since the treads stay even and only their number changes —
  and a gold one for the height of the rail.
- **The treads and the rail take their own materials**, so a stone stair can
  carry a timber handrail. Right-click a standing flight to change its rail.
- **A flight is a rhythm, not a size.** Ask for any height and it takes the
  number of even treads that comes nearest: uneven steps are the one thing a foot
  notices.

**Foundations can be raised.** A pad has a height now, on its own gold handle,
and it grows upward from where it sits — so a footing can reach the ground on a
slope instead of hovering over the low side of it.

## The bench

- **Parts stop climbing walls.** A part rested on the highest thing under any
  corner of it, so one corner brushing a wall carried the whole piece onto the
  wall and a maker could never set a thing against one. It rests on what most of
  it stands on now, and a tie settles low.
- **Chosen parts are lit**, and stay lit when the cursor wanders off. Hovering
  and choosing were both writing the same glow, so shift-clicking a second thing
  looked exactly like unchoosing the first.
- **A panel says what is in your hand** — how many parts, whether they are a
  group, and what they are, counting alike things rather than repeating them.
- **Shift-click gathers in placement mode too**, so grouping no longer means
  leaving the mode you are drawing in.
- **Pieces**: keep a group as a piece and set it down in any other work. The
  system that puts one in your hand had never been switched on.
- **One step added, one step dropped.** `+ COPY` and `+ BARE` were a question
  nobody wanted asked; a new step copies the one showing and lands beside it.
- **The bench clears when the rig comes out.** The roof cutaway was putting the
  whole building back on the floor the instant the rig had put it away.

## Under the hood

- Parts no longer wander sideways when a handle sizes something other than their
  own width — a chimney's drop, a flight's rise. A part moves by half of what it
  truly grew, measured from the boxes it is made of.
- The stairs keep every face out of every other face's plane, which is what the
  shimmer along a newel was.
