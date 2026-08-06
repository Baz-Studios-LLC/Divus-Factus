## THE RIG: the Atelier learns to animate

**The bench has a second half.** THE RIG stands a villager on a pedestal and lets
you pose them by hand: press any part of the body and drag, and the joint it
hangs from turns to follow. Key the pose at a moment on the timeline, key another
further along, and press PLAY — the body turns from one to the other and loops.
What you save is a clip, and the village can be taught it.

**It is the game's own villagers standing there, not a drawing of one.** Six real
bodies come out of the game's own builder — child, adult and elder, woman and
man, from 0.96m to 1.76m — and you can switch between them mid-clip to see how a
pose reads on the smallest body it will ever play on and the largest. A clip
holds joint rotations and nothing else, which is exactly what lets one clip play
on a child and on their father: an elbow bent ninety degrees is bent ninety
degrees whatever the forearm measures.

**Clips lie over the village's own motion rather than replacing it.** The walk,
the breath and the head-scan stay procedural, because those answer to speed and
ground. A clip writes only the joints it was drawn with, so an arm can chop while
the legs keep walking. Name a clip after what a villager is doing — `praying`,
`chatting`, `mourning` — and everyone doing it plays it. Draw nothing and the
village animates exactly as it did before.

And there are props to hold: an axe, a mining pick, a sword, a fishing pole, a
hoe, a torch. They hang in the hand of whichever body is standing, so a swing can
be judged with the thing being swung.

## The village

**Two new buildings, and the old three are gone.** A longhouse and a house, drawn
over from scratch to look medieval rather than modern — smaller, tighter, and
built of the bench's newer parts. The hall carries a double door, because ten
people live behind it.

**A building keeps the colours it was painted.** The village used to re-dye each
house's walls and roof with a colour of its own, which was right while a drawing
arrived in whatever colours the catalogue happened to hold. There is a brush on
the bench now, so what a maker paints is what the village raises.

**Half of every building stands mirrored.** One drawing makes two buildings for
nothing but a sign change, and a street of one blueprint stops reading as a row
of stamps. The mirror runs along the building's length, so the front door stays
in the front wall.

**The ground under a building is barely disturbed.** A pad was a circle wide
enough to hold a rectangle's corners — half again as much levelled earth as the
floor needed, standing out past every wall as a plateau, bald. Pads are the shape
of the thing standing on them now, the bank comes back at one in four rather than
one in three, and the grass grows right up to the walls and under them.

## The bench

- **Open a work from anywhere.** A picker opens any `.baz` on the disk rather
  than only the ones in the bench's own folder — and the shelf of saved works,
  the ready-made starts and the standing notes are gone with it. The tools that
  act on a whole work are four drawn glyphs at the foot of the rail now, with
  tooltips that follow the cursor.
- **A step can be taken from one stage and put on another.** TAKE holds the step
  you are showing; PUT lays it over the step you are showing, in place of what is
  there. Undo still reaches it, which took noticing.
- **A beam trimmed twice keeps the cuts the first trim made.** It used to be
  handed back square at the same length it already had, which looks exactly like
  the saw work being undone.
- **Buttons that read as one row.** The modes, the stage bar and the shelves all
  share a width, and the walls a window splits now lap rather than meeting on a
  seam you could see through — in the bench and in the world.

## Under the hood

- The bake reads a work drawn in stages. A stage-drawn building baked to nothing
  at all, silently, and would have gone into the game as an empty site.
- The bodies the rig poses are exported by the game's own builder rather than
  copied by hand, so the bench cannot drift from the village it draws for.
- A looping clip turns through its own seam rather than snapping at the end of
  it, in the bench and in the game alike.
