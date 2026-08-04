## A new game is a new world

**The Title door starts a real new game.** Leaving a world from the pause menu razed it and grew another on a fresh seed — and then handed you back the old one's village, standing in the new one's ground. Begin walked straight past the founding into a settlement you had not founded. It is what a cold launch is now: an empty world, a flag in your hand, ground nobody has named, and an unnamed god. The opening dive comes down on land, too. The survey that finds the likeliest country in a world was only ever run once, so a new world was never surveyed at all and the descent aimed at the old world's good ground — which, in a different world, is open ocean.

**And the walk back to the title is covered.** The world darkens, the swap happens in the black, and the curtain lifts only once the new planet has finished building — no more watching the ground assemble itself behind the lettering. The title then comes up out of the dark rather than arriving whole.

**Half water, and continents you can stand in the middle of.** The world had been three quarters ocean, and its land came in crumbs: two thirds of all of it was within sight of the sea. The shape of land cannot be tuned after the fact — no curve applied to a finished world moves a coastline, only the spectrum that generates it decides between continents and archipelagos — so eighteen of them were sampled over nine thousand directions apiece and scored on how much of their land is interior. The one chosen nearly doubles it, 36% to 67%. Half the planet is sea now, taken from the field's own median rather than guessed at.

**Snow belongs to the poles, not to height alone.** A three-hundred-unit peak at the equator is bare rock and grass; the same height near a pole is under snow, and at the poles the snow comes down to the water.

**Frames, and where they were going.** Every layer of the world was measured with a baseline run either side of it, and two things came out worth taking. Shadows stop being drawn once the eye is further back than they can possibly reach — past that distance nothing casts anything, and the picture measured identical with them gone. And the forests thin out as you climb, each stand at its own altitude, so the world empties gradually instead of switching off. Together they take a fifth off the frame at the height where it used to drop hardest.

**One settings panel, and switches instead of hotkeys.** There had been two, one in the codex and another behind the pause menu and the title. There is one now, and the title shows that same page rather than a copy of it. Inside it, a row of switches for what the world shows you — the weather, the veil, the scenery, the planet's surface, the water, the buildings, the people, the shadows — because most of what a game can turn on and off does not deserve a letter of the alphabet.

## Under the hood

- A felled tree falls at once. It used to stand on in the forest for two full seconds after it had finished falling: a whole stand of trees is one mesh, and the wait that keeps a spreading fire from rebuilding that mesh once per burning tree was being charged to a single axe stroke as well.
- The veil is one law now, not merely one colour. It thickens when you look through it edge-on — which the near ground did and the planet did not, so the two disagreed by up to thirteen shades wherever they met at an angle, visible as a line exactly where the near ground ends.
- Letting go of a possessed villager no longer leaves two cursors on the screen.
- Leaving a world no longer flashes the terrain white: turning any layer back on had been handing every hidden thing back at once, the planet's unpainted patches included.
- Shadow quality was measured and then left alone. A smaller shadow map is worth nothing whatsoever here, and halving the cascades buys a millisecond at the cost of 3% of the frame in visible quality at midday — where the same test at dawn had called it free.
- Bigger tree clusters were measured and rejected too. Merging four stands into one cuts nine thousand meshes and costs four milliseconds: what limits this game is vertices rather than draw calls, and a small mesh is culled where a big one is not.
- Said plainly, because it is still true: rivers step where two courses meet at different levels, and the near ground is still visibly sharper than the planet it sits on. Both are next.
