## A maker can draw where the goods go

Opificium can now author a **pallet**: not a point but a box, dragged out to
the room a building sets aside for goods to stack in. The game reads them.

- **A storehouse stacks where its drawing says**, and holds what the drawing
  gives it room for. The four corners it used to use, identical for every
  storehouse whatever its shape, are gone.
- **A pallet takes whatever the village needs it to.** The pallets are
  shared out among the goods actually being stored, and shared out again
  when that changes — six pallets over timber, stone and food deal two
  each, and once a granary takes the food away the same six deal three each
  to the two that remain. New goods join the deal the day the village first
  has any.
- **Goods fill one pallet before starting the next**, in rows and then
  layers, each layer laid the other way about. How much a pallet holds is a
  fact about the box that was drawn, not a number in the code.
- **The granary is asked the same way**, so food is authored in both of its
  homes.

A building that draws no pallets keeps the corners it always had. Nothing
already authored changes.

## Buildings stand at the size they were drawn

**A carried-in building is placed, levelled and cleared at its own
footprint.** Only houses ever asked their drawing how big they were; every
other kind used a size from the days the village drew its own buildings. So
an authored storehouse of 6.4 by 10.6 metres was given a pad of 2.8 by 1.6 —
a fifth of the building. On a slope that is one end hanging in the air and
the other buried, with trees left standing inside the porches. **Expect the
ground around larger buildings to look different.**

## A clock tells the hour

A drawn clock face gets hands: the hour hand turns twice a day, the minute
hand twenty-four times, both from the top at dawn. The dial is the maker's
carpentry; what it reads is the world's.

## Under the hood

**Aspectus** — the game's own renderer — takes its first pass. The frosted
glass behind an open book used to wake a second camera and render the entire
world again into a small image. It is now a blur of the frame already drawn:
no second scene, and the real screen rather than a stretched thumbnail of
it.
