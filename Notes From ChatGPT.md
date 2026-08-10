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
gate passes. Wait for a fresh measured need or new live emitters before opening
another broad corpus batch.

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

## Sermo Feature Request

Arguments require engine-supported roles and stances before ChatGPT authors
their corpus. Names below are proposals, not approved vocabulary:

```text
argument:open
argument:defend
argument:counter
argument:mediate
argument:escalate
argument:withdraw
argument:apologise
argument:threaten
argument:end
```

Likely topics include work, food, shelter, marriage, family, faith, miracles,
commandments, prophets, punishment, sacrifice, and cannibalism. Likely stances
include accusing, defensive, ashamed, afraid, angry, cruel, reluctant,
remorseful, and mediating.

Claude should choose the smallest useful first slice, define exactly which
simulation facts emit each tag, lock the vocabulary, and prove the tags reach
real Sermo requests. Once those emitters exist, ChatGPT can author a large
argument corpus ranging from ordinary domestic friction to doctrinal crisis.

The first implementation milestone should be mundane arguments with visible
aftermath. Once that loop works, religious arguments can reuse it safely. Build
the human disagreement system first; then let doctrine give people more
dangerous things to disagree about.
