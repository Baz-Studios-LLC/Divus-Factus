## It boots

**v0.3.0 crashed on startup.** Two faults, both introduced by the settlement rewrite and both invisible to the test suite, because the tests assemble small worlds rather than the real one:

- Two queries in the working day both reached for a building's position, one of them mutably. Bevy reasons about which components a query *could* touch rather than which ones ever actually co-occur, so a build site and a boulder had to be told apart explicitly even though nothing is ever both.
- Each town's graveyard became a thing the burial system writes to rather than reads, and it was never registered.

Neither could be caught by a unit test. Both are caught by starting the game once, which is now what happens before a release goes out.

## Divus Factus

**A new name** — the game was called Egregore until now. A game called Egregor shipped on Steam in 2024, so the name had to go. Everything follows it: the window, the save folder, the bundle, the launcher entry. The word still appears in the design notes where it means the occult idea — a being sustained by collective belief — rather than the title.

**Your old villages will not be found.** Saves lived under the old name's folder and are not migrated. Sorry — this is the price of a rename, and better paid now than later.

## Two ways a village spreads

**THE LONGHOUSE** — a house is a family's: four beds under one roof. Everyone else now has a longhouse, eight beds and none of them kin. A child who comes of age leaves the family house for it; a marriage moves the pair back out into a house of their own. Neither move is scripted anywhere — both fall out of the village asking, once a day, whether anyone is sleeping under the wrong roof. And strangers no longer bunk in with a family, not even at the founding.

**COLONIES** — a town pressed for room and food, whose explorers have found somewhere better, will send a party out to found a town of its own. They walk there. It takes days, you can watch them go, and a god who objects has that long to intervene. On arrival they raise a banner, a fire, a woodpile and a name — and from that moment they are their own people, with their own stores, their own harvest and their own famine.

**HOMESTEADS** — not every family wants a neighbour through the wall. Some houses are now raised out past the town's rings on their own ground, with a plot of field turned beside them. They are still the town's: they walk in to its square, eat from its stores, answer its famine watch. They simply live out.

## The people

**FAMILY NAMES** — every villager carries a house name now, and it is a genuinely separate system from given names: each village's tongue reserves a sound that appears in no first name, so a surname can never be mistaken for one. A wife takes her husband's house at the wedding, and the house she was born into is remembered — which is what a family tree will need when it comes.

**FEWER CHILDREN, LATER** — a couple's chance of another child falls with each they already have. Large families are still possible. They are simply remarkable now.

## Fixes

**Roofs now stand over their own walls** — two faults in the shared roof geometry, so both were wrong on every gabled building in the game: the ridge beam hung in the air above where the roof actually met, and the wider a building was the further its eaves sank below the wall top, letting walls poke up through their own roof. Shed roofs had the same class of fault, which studded every sawmill and storehouse with little tabs. All of it is derived from one pitch and one span now, so a fix cannot mend one roof and leave the rest.

**Every town simulates fully** — the groundwork under the colonies. Stores, work, hearths, prayer, fields, burials and the famine watch all belong to a particular town now rather than to *the* town. Found along the way: children were being born as citizens of whichever town the camera happened to be on, one town's blacksmith was sharpening every town's tools, and every settlement would have buried its dead in the first one's graveyard.
