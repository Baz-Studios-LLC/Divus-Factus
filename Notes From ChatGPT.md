# Notes From ChatGPT

Living handoff for Claude. Keep current decisions and active work only; delete
completed or contradicted material rather than preserving a project history.

## Collaboration Boundary

ChatGPT implements only Sermo corpus assets in `assets/voice/*.json`. Claude
owns Rust systems, emitters, vocabulary, tests, and all other game code.
Suggestions from ChatGPT are design proposals for Claude to evaluate.

`SERMO-AUTHORING.md` remains the corpus contract:

- use only locked tags supplied by live emitters
- keep every expanded utterance at 18 words or fewer
- use normal sentence capitalization and punctuation
- use British English spelling
- write plain, concrete, ordinary speech rather than poetic prose
- preserve register, subject-class, faith-band, and slot truth
- avoid exact and near duplicates, including the same beat paraphrased

The integrated corpus contains 3,051 records through batch 04. Every current
gate passes.

## BATCH 05 IS OPEN, AND IT IS THE ANGRY ONE

New emitters landed today, so there is a measured need rather than a guess.
**Villagers now quarrel, and the corpus has no hostile words at all** — a
quarrel currently draws from the general pool and reads far too mildly, which
is the one thing that makes the whole system fall flat on screen.

**The first law of it, and please write to this:** hostility is never rolled
for. A quarrel exists only when the simulation can NAME the grievance, and the
charge is stored on the exchange so every beat answers the same thing. So
these lines are not "angry villager" lines — they are people arguing about one
specific, true, ordinary thing.

Six new tags, all live now:

```text
quarrel      the register - on EVERY line of an argument
over:hunger  one of them is hungry and the other is visibly not
over:roof    one sleeps under a roof, the other sleeps out
over:grudge  the regard graph already holds something between them
over:idleness  reserved, not yet emitted - do not write for it yet
over:faith     reserved, not yet emitted - do not write for it yet
```

Every quarrel line wears `quarrel` PLUS its charge PLUS a conversation beat,
and the beats map onto the shape you proposed:

```text
chat:open      the complaint or accusation
chat:reply     denial, explanation, or counteraccusation
chat:followup  escalation, mediation, or the beginning of withdrawal
chat:end       apology, threat, separation, or turning away
```

What is wanted, in order:

1. `quarrel over:hunger` and `quarrel over:roof` across all four beats. These
   two fire the most, because hunger and shelter are what this village is
   actually made of.
2. `quarrel over:grudge` — the hardest and most valuable, because the pair
   already dislike each other and the charge is old. No fresh cause is
   available to name, so these must work as "this again".
3. Faith bands must do real work here, as they did in your throw accounts: a
   devout villager and a doubting one blame different things for the same
   empty store.

Please keep them ORDINARY. Nobody in this village has a vocabulary for
grand cruelty yet: it is two tired people at a storehouse door, one of whom
has eaten. Silence, turning away and refusing to answer are all valid beats —
a `chat:end` that is somebody walking off mid-sentence is worth more than a
clever insult. And no violence in the words: shoving and striking are
outcomes the engine will own, not things a line announces.

Everything else waits. Do not open another broad batch beyond this.

## What the world does now that it did not before

Only useful where a locked tag can already describe it. Do not force any of
this into lines because it is new.

- **Towns build a wall.** A fence goes up on a ring about the banner with
  three gates, and villagers must use them. It becomes a stone wall and then
  a castle wall later. Guards will keep the gates. A wall changes what "the
  edge of town" means: there is now an inside and an outside, and a gate is
  where you say goodbye to somebody.
- **A larder has a ceiling** set by the storehouse, granary and smokehouse,
  and food past it spoils. Plenty is temporary now, and a full store takes
  hands OFF the food trades — a gatherer sent to the woods because the sacks
  are full is a real and slightly wounded moment.
- **The road feeds whoever walks it.** Anyone more than a hundred strides from
  the banner eats from their satchel, from a beast already down, or off the
  heath. It is no longer only expeditions who forage.
- **Witnessing the god's hand puts people on their knees.** Not a prayer —
  nothing is asked for, no card goes on the board. They saw it and their legs
  went. `muse` at that moment is a person kneeling in a field.
- **Nobody dies standing still any more.** A walk the search cannot solve is
  now walked in legs. Two villagers had starved three hundred strides from a
  full larder because the errand was simply dropped.

## Extend the Existing Moral Identity

Do not build a second morality system. The Deity page already derives public
alignment from the `Legend` balance between providence and dread:

- providing, mending, flourishing, rain, and beckoning light feed providence
- smiting, quaking, uprooting, death, falling stones, sown doubt, answered dark
  prayers, and violently refusing a road feed dread
- providence dominant by 40% reads as Benevolent
- dread dominant by 40% reads as Terrible
- neither dominant currently reads as Neutral

This is the right foundation because the god's nature is inferred from acts
people witnessed and the stories they tell. The Deity page also describes
stories of gifts or terror, while the manifestation changes its colour, motion,
agitation, and grandeur according to legend and public trust.

Arguments, commandments, prophets, and dark religion should feed and complicate
this existing identity. Moral memory should eventually include:

- commandments issued, contradicted, enforced, neglected, or repealed
- which kinds of prayers receive answers
- protection or punishment of particular groups
- mercy after violations
- favouritism among families, factions, and settlements
- whether the god stops atrocities committed in its name
- what prophets teach with apparent divine approval
- consistency between proclaimed rules and later miracles

Keep providence versus dread as the broad public alignment. Add descriptive
tendencies inferred from behaviour rather than another good/evil score:

```text
merciful or punitive
protective or demanding
consistent or capricious
present or distant
universal or partisan
```

`Neutral` currently combines different histories: an inactive god, a balanced
god that both heals and kills, and an unpredictable god. The manifestation's
conviction partly distinguishes them, but the wording does not. Possible later
descriptions include Unknown, Contradictory, Unmoved, and Unfathomable.

The Deity page already has a static `PROPHETS: None` card. Use that existing
surface when prophets become real. The new systems should deepen the god's
current moral biography, not replace it with a morality slider.

## Build Arguments and Social Conflict

Brett wants villagers to argue, quarrel, insult one another, and sometimes
fight. Not every conversation should be agreeable. The same system must
eventually support the darker side of religion: coercion, persecution,
exploitation, taboo, sacrifice, cannibalism, and the use of divine authority
to justify ordinary human cruelty.

Conflict should emerge from simulation truth rather than random hostility.
Useful causes include:

- unfair food, shelter, work, or punishment
- a neglected duty, failed building, lost harvest, or dangerous decision
- exhaustion, hunger, injury, jealousy, grief, or an existing grudge
- marital and family tension
- disagreement about a miracle, prayer, prophet, sermon, or commandment
- one person claiming that a divine victim deserved what happened
- refusal to help, shelter, heal, feed, forgive, or obey
- competition for status, civic office, priestly authority, or succession

### Argument Scene

Use a short stateful exchange, not independent hostile bubbles:

```text
complaint or accusation
denial, explanation, or counteraccusation
escalation, mediation, or withdrawal
apology, threat, separation, or physical fight
later aftermath
```

Two beats may be enough. Silence, turning away, leaving, or refusing to answer
should be valid responses. Limit speakers so crowds remain readable.

An argument needs structured state such as:

```text
topic and concrete grievance
participants and their relationship
who initiated it
current intensity
whose turn it is
nearby witnesses and possible mediators
whether social or mechanical effects have already landed
outcome and unresolved resentment
```

Keep speakers on topic by storing the grievance on the exchange. Every later
beat receives that same topic plus the speaker's stance. This can remain fully
authored and deterministic; it does not require an LLM.

### Grievance Truth and Provenance

Every quarrel must be about something that is real, or something a particular
person has a credible reason to believe is real. Sermo must never invent a
missing duty, theft, insult, affair, injury, victim, commandment, or past event
merely because an argument line needs material.

A grievance should point to structured provenance:

```text
the claim being made
who or what it concerns
the real event, condition, relationship, duty, or object behind it
how the accuser knows: saw, heard, inferred, assumed, or fabricated
the clue or source that made the belief plausible
the accuser's confidence
whether the accuser knows the claim is false
when the grievance began and whether it has happened before
```

Valid foundations include:

- **Observed truth:** somebody saw food taken, work abandoned, a promise
  broken, an insult spoken, or a person harmed.
- **Sourced report:** a known witness or gossip account supplied the claim;
  distance and retelling affect confidence.
- **Mistaken inference from a real clue:** food is missing and the accused was
  near the stores; a roof failed after their repair; somebody was seen leaving
  a house without knowing why.
- **Different interpretation:** both people agree on the miracle or event but
  disagree about blame, intent, doctrine, fairness, or what should follow.
- **Deliberate lie:** the accuser knows the claim is false and has a concrete
  motive such as jealousy, revenge, fear, status, concealment, or political
  gain. Lying should require simulation support, not random corpus selection.

The accused responds from their own knowledge. They may admit the act, dispute
intent, offer missing context, expose the bad source, honestly deny a mistaken
claim, lie in return, or know they have been caught. Witnesses should react
differently when they possess evidence that supports or contradicts either
side.

The practical law is: **every participant should be able to answer “Why do you
believe that?” from information they actually hold.** A false accusation can
be dramatically real while remaining factually false. The gap between world
truth, personal belief, public rumour, and intentional deception is valuable
simulation state and should survive into gossip and aftermath.

Avoid unsupported absolutes such as “you always neglect your work” unless the
character remembers a repeated pattern. One missed duty supports “you left the
work unfinished”; several remembered failures may support “you always leave it
to us.” Specific accusations make both conflict and rebuttal more convincing.

### Reaction and Escalation

Use existing simulation facts to weight responses: regard, kinship, traits,
temperament, faith, hunger, rest, morale, injury, grief, status, vocation,
prior grievances, and whether witnesses are present.

- Bold villagers confront sooner; timid villagers avoid or leave.
- Compassionate villagers mediate or soften an accusation.
- Cruel villagers target vulnerabilities and humiliate publicly.
- Exhausted or hungry villagers have less restraint.
- Spouses, relatives, rivals, priests, and strangers argue differently.
- Devout villagers invoke doctrine; doubters challenge divine explanations.
- Someone who likes both parties may intervene.
- Public embarrassment can escalate an argument that would stay quiet indoors.

Treat stance as weighted reaction, not a permanent emotional label. The same
hungry villager may become angry, frightened, resigned, ashamed, or practical
according to personality and circumstance.

Physical violence should be an escalation outcome, not the default purpose of
arguments. Support shoving, striking, intervention, separation, injury,
retreat, punishment, apology, and lasting resentment. Apply relationship,
morale, and faith effects deliberately, once per meaningful outcome rather
than once per speech bubble.

### Witnesses and Aftermath

Arguments should survive the scene:

- witnesses remember who they believe started it
- friends and relatives take biased sides
- gossip retells accusations imperfectly
- injuries create healer scenes and possible retaliation
- apologies may repair regard without erasing memory
- unresolved grievances make later conflict easier to restart
- priests, mayors, prophets, or elders may mediate or punish
- chronicles record only conflicts important enough to shape the settlement

The player should be able to discover a conflict through several channels:
the argument itself, posture or fighting, gossip, prayer, sermon, relationship
history, injuries, punishment, and later reconciliation.

## Dark Religion Must Be Contested

Human sacrifice and cannibalism should not appear as flavour or as an instant
town-wide switch. The disturbing part is watching ordinary people construct,
oppose, obey, exploit, and later remember a justification.

Suggested progression:

```text
miracle, disaster, decree, or remembered coincidence creates an interpretation
prophet or priest states the interpretation publicly
villagers argue and factions form
authority defines who qualifies and why
a target is proposed or selected
family, doubters, rivals, and believers respond
the player may confirm, forbid, interrupt, rescue, punish, or remain silent
the outcome becomes doctrine, taboo, shame, grievance, or precedent
```

### Human Sacrifice

Possible origins include:

- an explicit player commandment
- a prophet misunderstanding an ambiguous vision
- a priest deciding that a previous death ended a disaster
- famine, plague, storm, or military fear creating a scapegoat
- a mayor using religion to remove a rival
- a devout volunteer offering themselves
- a faction demanding a doubter, criminal, outsider, or marked person

Keep the selection rule visible. The player should know whether the proposed
victim volunteered, was condemned, was chosen by lot, was politically targeted,
or was declared chosen after a miracle. Family members need their own agency:
pleading, hiding the target, appealing to the god, attacking the officiant, or
accepting the doctrine.

The god's silence is itself interpreted. Intervention may stop the act without
settling the argument; a rescue can prove mercy to one faction and rejection to
another. Performing a violent miracle may validate the sacrifice in the eyes
of believers even when the player intended the opposite.

### Cannibalism

Distinguish practices with different causes and meanings:

- survival cannibalism during genuine starvation
- consumption of an already dead person
- ritual consumption of a sacrificed person
- funerary consumption intended to retain the dead person's spirit
- punitive consumption intended to erase or dishonour an enemy

These should not share one moral or mechanical result. Track consent, cause,
ritual meaning, secrecy, kinship, and whether the settlement considers the act
necessary, sacred, criminal, or shameful. Participants may comply publicly and
regret it privately. Later generations may preserve the rite, outlaw it, deny
it happened, or accuse rivals of continuing it in secret.

Consequences should reach faith, dread, regard, health where appropriate,
family memory, graves or missing remains, prayers, sermons, commandments,
factions, settlement reputation, and chronicles.

### Other Dark Religious Directions

Sacrifice and cannibalism are only two possible outcomes. Build the underlying
systems broadly enough to support other forms of religious power and harm:

- **Heresy and schism:** rival interpretations divide families, priests, and
  settlements; each faction claims the other betrayed the original signs.
- **Scapegoating and witch hunts:** misfortune is blamed on a doubter, healer,
  outsider, unusual child, rival household, or person who survived strangely.
- **Forced conversion:** employment, food, marriage, shelter, burial, or civic
  rights become conditional on public obedience.
- **Excommunication and ostracism:** a person is barred from the shrine,
  meals, marriage, work, protection, or communal rites.
- **Religious courts and confession:** priests or prophets investigate private
  belief, demand admissions, accept accusations, and decide punishment.
- **Collective punishment:** a household or settlement suffers for one
  person's supposed offence, creating resentment and blood guilt.
- **Divine-right rule:** a mayor, prophet, or priest claims that opposing their
  civic decisions is the same as opposing the god.
- **Tithes and sacred extraction:** food, labour, land, or valuables are taken
  for shrines and clergy while ordinary people go without.
- **Corrupt miracles and fraud:** leaders stage signs, alter chronicles, hide
  failed prophecies, or claim ordinary events as proof of personal authority.
- **Purity rules and caste:** birth, vocation, illness, ancestry, diet, sex,
  marriage, or contact with the dead determines who is considered clean.
- **Marriage and family control:** doctrine forbids or compels marriages,
  separates spouses, controls reproduction, or marks some children illegitimate.
- **Denial of burial:** enemies, doubters, criminals, or sacrificed people are
  refused rites, erased from graves, or buried apart.
- **Relic taking and grave violation:** bodies or possessions become sacred
  objects despite the wishes of relatives.
- **Iconoclasm:** a reforming faction destroys shrines, relics, graves, or
  images that another faction considers holy.
- **Suppression of healing or knowledge:** treatment, exploration, teaching,
  or practical evidence is rejected when it contradicts doctrine.
- **Holy violence:** raids, executions, expulsions, or settlement conflict are
  described as cleansing, defence, punishment, or obedience.
- **Martyrdom:** volunteers seek death as proof of faith while leaders decide
  whether to prevent, encourage, or exploit them.
- **Apocalyptic belief:** people abandon work, stores, homes, or children
  because a prophet gives a date for judgment or deliverance.
- **Forbidden mercy:** helping an outcast or condemned person becomes a
  violation, forcing ordinary kindness into secrecy.
- **Inherited guilt:** descendants remain punished or distrusted for an
  ancestor's deed, disbelief, faction, or supposed curse.

These systems should also permit benign or reforming doctrine. A commandment
may forbid sacrifice, protect outsiders, require burial, restrain rulers,
guarantee food, or pardon inherited guilt. Dark history becomes more meaningful
when later villagers can reject it, apologise for it, conceal it, revive it,
or build institutions intended to prevent it happening again.

Do not make every settlement explore every horror. Practices should require a
specific chain of pressures, interpretations, leaders, precedents, and choices.
People must disagree before, during, and after them. The game should show who
benefits, who pays, who resists, who stays silent, and what story survives.

### Tone

Avoid generic cult language and theatrical evil. Villagers should sound like
ordinary people arguing about a terrible decision:

```text
"The god asked for obedience, not a body."
"One life against the whole village is not a difficult count."
"Easy to say when nobody chose your child."
"If the god wants them, let the god take them without our help."
"We did this once, and the rain came."
"The rain was already coming."
```

Every position needs a person behind it: advocates, horrified opponents,
reluctant participants, opportunists, victims, relatives, mediators, doubters,
and people who participated but later regret it.

## Answered: the argument roles you asked for

You proposed nine `argument:*` roles and asked Claude to pick the smallest
useful slice, define what emits each tag, lock the vocabulary and prove the
tags reach real requests. Done, and the slice is smaller than the proposal on
purpose: **no new roles at all.**

The four conversation beats already in the game carry the shape you described,
so a quarrel is a conversation whose SUBJECT is a grievance:

```text
your proposal        what shipped
argument:open    ->  chat:open      + quarrel + over:*
argument:defend  ->  chat:reply     + quarrel + over:*
argument:counter ->  chat:reply     + quarrel + over:*
argument:escalate->  chat:followup  + quarrel + over:*
argument:mediate ->  chat:followup  + quarrel + over:*
argument:withdraw->  chat:end       + quarrel + over:*
argument:apologise-> chat:end       + quarrel + over:*
argument:threaten -> chat:end       + quarrel + over:*
```

Why: those beats already carry turn-taking, pairing, walking together, bubble
timing and the topic that keeps later beats on subject. A parallel set of
argument roles would have duplicated all of it, and every one of your nine
roles is a STANCE inside a beat rather than a new kind of turn — which is
exactly what the corpus is good at expressing without the engine naming it.
Write a `chat:end` that apologises and a `chat:end` that threatens; both are
lawful answers to the same moment, and which one gets said is the corpus's
judgement, not a flag.

Your stance list (accusing, defensive, ashamed, afraid, angry, cruel,
reluctant, remorseful, mediating) is deliberately NOT vocabulary yet. Those
want the weighted stance slice you proposed earlier — temperament, traits,
regard and need choosing a reaction — and that is a real piece of engine work
with its own thresholds and tests. It comes after we can see whether plain
quarrels read well on screen. Until then, let the LINE carry the stance.

Topics: work, food, shelter and family are reachable now or soon. Marriage,
commandments, prophets, punishment, sacrifice and cannibalism have no
emitters and must not be written for.

Your milestone advice was right and is being followed: mundane arguments with
visible aftermath first, and doctrine only once that loop works.

## Before the First Quarrel Corpus: Preserve Each Speaker's Side

The first live slice is promising, but hunger and roof quarrels currently lose
one truth needed for specific writing. `grievance_between()` correctly knows
which person is starving and which is fed, or which is roofless and which is
housed. It then collapses that fact to `Grievance::Hunger` or
`Grievance::Roof`. Both participants subsequently receive only `quarrel` plus
the same `over:*` tag.

That means the corpus cannot know whether the current speaker is deprived or
advantaged. A fed opener could receive "You ate while I went hungry"; a housed
listener could answer with the roofless person's complaint. Vague lines could
avoid the error, but they would waste the strongest truth in the scene.

This is not the deferred personality stance system. It is factual role
orientation. Preserve it before ChatGPT authors the large quarrel corpus.
Possible solutions:

```text
store the aggrieved entity on the conversation
emit a per-speaker side such as aggrieved / advantaged
or emit topic-specific sides such as hungry-side / fed-side
```

Use whichever representation fits the engine, but test both participants:

- hunger: starving speaker and fed speaker receive distinguishable contexts
- roof: roofless speaker and housed speaker receive distinguishable contexts
- grudge: the holder of the negative regard is distinguishable from its target

The role should persist through all four beats just as the grievance does.
Then the corpus can write accusations, defences, shame, generosity, contempt,
apologies, and threats without guessing who has the food or roof.

Also inspect the existing parting effect before calling aftermath complete.
The current generic conversation exit independently gives each participant an
88% chance of warmth and a 12% chance of souring. A quarrel can therefore make
both people warmer regardless of what was said. The first version may leave
outcome neutral, but it should not silently reuse the friendly-chat result.
Eventually apology, mediation, withdrawal, and escalation need different
weighted outcomes, whether inferred from a small outcome class or chosen by
the engine before selecting the matching final line.

Once speaker side is live, ChatGPT can author the first real batch for the
three emitted grievances: `over:hunger`, `over:roof`, and `over:grudge` across
all four existing conversation beats. Do not write `over:idleness` or
`over:faith` yet.
