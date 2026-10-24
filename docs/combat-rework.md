# Combat rework: skills, the Weave, supports and the Aether

Design notes for the skill system rework. Saves from the earlier rework (save version 3) load and are migrated.

## The blade and the sigils

- The hero wakes holding the Dormant Aether Blade, a key item. It cannot be swung: until the first sigil the hero can
  talk, open things and dodge, but not attack.
- The four elements come from Sigils set into the blade's hilt: Sigil of Wind (Elder Orin), Sigil of Fire (Warlock
  chest), Sigil of Water (Sunken Temple), Sigil of Thunder (Storm Spire). Each sigil unlocks its mode; the first one
  wakes the blade. There is no neutral mode.
- Forgemaster Corvin no longer fuses four swords; with all four sigils set, his forge awakens the blade
  (flag `aether_awakened`), which strengthens the Aether.
- Blade power is per mode: `(6 + 1.2 * level) * (1 + 0.08 * mastery(mode))`, times the smith bonus. No sword outranks
  another.

## Switching modes

- Tap the mode key (L1, or Q/E) to cycle; the mouse wheel and 1-4 also switch.
- Hold it for 0.2s to open the quick-change wheel: the world slows nearly to a stop, and a direction picks the mode.
  Wind is up, Fire right, Water down and Thunder left, so opposite elements face each other. Release to switch.

## Mastery

- Each element has mastery 0-5, earned from kill EXP while fighting in that mode. A kill landed on a weakness counts
  1.5x. The Mentor's Ribbon accessory adds 50%.
- Thresholds (cumulative): 800, 4500, 15000, 45000, 100000.
- Mastery raises that mode's blade power, teaches the element's second skill at mastery 1 and third at mastery 3,
  opens the second support socket of each group at mastery 3, and makes Aether pierce that element's weakness more
  often.

## The Aether

Overheat is gone. Its meter now builds the Aether instead.

- Landing hits fills the Aether meter (melee 1.2, finisher 2.5, bolt 0.8, charged bolt 2.5, skill hit 1.5), 1.75x on
  a weakness. Taking a hit drains 20. Accessories (Resonant Stone), Weave nodes, the Prism Band and gems scale it.
- When the meter is full, Aether wakes by itself for 6 seconds (8 once awakened). The equipped Aether form fires
  once as it wakes. Taking a hit while it is awake costs a fifth of its duration. Mode switching stays open.
- While awake, every hit may pierce to the target's weakness instead of landing in the current mode's element:
  25% + 15% per mastery level of that weakness element, +25% once awakened, +15% with the Prism Band, +20% with the
  Prism Lens, plus Weave nodes, capped at 100%. Hits that pierce a shield's emblem shatter it.
- Afterwards the meter rests for 10 seconds before it fills again.
- Aether hits open Concord seals once the blade has awakened, and are the only hits that reach Aether-sealed foes
  (the First Concord's Unbound phase, Sovereign Vael's aether form). Striking those foes with other hits feeds the
  meter instead.

| Form (Aether page) | Learned | Level 1 | Level 2 | Level 3 |
|---|---|---|---|---|
| Prism Edge | First sigil | 3 blades at the nearest foes | 4 blades | 6 blades and an Aether burst |
| Prism Ray | Third sigil | One beam along the aim | A wider, stronger beam | Three beams |
| Sanctum | Awakening | Pulse, cleanse, brief ward | Longer ward | Long ward and 6% heal |

The form's level is 1 with one or two sigils, 2 with three or four, 3 once awakened.

## Skills

- One skill button. Each mode equips one skill from that element's pool of three.
- Tap: level 1 (1 SP pip). Hold: the whole world slows to 30%; level 2 at 0.4s, level 3 at 0.9s (real time),
  auto-release at 1.5s. Level N costs N pips, paid on release, capped by unlocked level and SP. A hit cancels.
- An element's skills come with its sigil and mastery 1 and 3. Its charge levels come from its orb (level 2) and
  bracer (level 3), found well away from that element's sigil.

| Mode | Skills | Orb (level 2) | Bracer (level 3) |
|---|---|---|---|
| Wind | Gale Slash, Cyclone, Updraft | Old Saltmine | Storm Spire (Golem's chest) |
| Fire | Cinder Rush, Meteor, Flame Aegis | Cinderfall Caldera | Fallen Bastion (Warden's chest) |
| Water | Frost Bloom, Glacier Lance, Ice Mirror | Storm Spire | Sky Citadel |
| Thunder | Arc Cleave, Storm Call, Static Field | Ashen Rim | Inverted Spire (Blight) |

The old elemental rings became the orbs (saves convert them). The Prism Band stays in the Citadel as an Aether item.

## The Weave

- A lattice on a 13x13 grid: threads run along every even row and column inside a diamond, about 96 nodes. A node
  can be woven once an orthogonal neighbour is; the cursor walks the threads cell by cell, and Q/E jump between the
  regions' keystones.
- Regions are themed by playstyle, not element: Might (north: ATK, FOC, finishers, kill rewards), Ward (south: life,
  DEF, guard and parry), Swiftness (east: speed, dodges), Spirit (west: SP, ailments, skills) and Aether (the
  diagonals: meter, pierce, duration).
- Minor nodes give small stats (+2% ATK, +3% life and so on). Notables carry the triggers and passives (Riposte,
  Phantom Step, Wildfire, Chain Shock, Flicker, True Sight...). Keystones sit at the tips and need a Weave Knot:
  Overcharge, Stillwater, Tailwind, Blood Art and Aether Bond (Aether always pierces, but lasts half as long).
- One point per level plus Tomes of Insight. Free unpicking at any save crystal.

## Supports

- Three socket groups per mode: attack (melee combo), ranged (bolts) and special (skill). One socket each, two at
  mastery 3. Each gem is a single item, so it can sit in only one socket.
- Every support has a drawback. The Aether page has a single socket for Prism supports: Prism Lens (pierces 20% more
  often, 1.5s shorter) and Everlight (3s longer, meter fills 20% slower).
- Supports come from Trial Rifts: gold rifts in the world that lead to a hard arena with three rifts to seal (one
  holding an elite). Sealing all three reveals two gem chests.

| Trial | Host | Gems |
|---|---|---|
| Trial of Thorns | Whisperwood | Cleave, Brutal |
| Trial of Mire | Mistmoor Marsh | Multishot, Infusion |
| Trial of Salt | Salt Mine | Piercing, Lingering |
| Trial of Rime | Frostpeak | Momentum, Leech |
| Trial of Tides | Cove | Efficiency, Haste |
| Trial of Cinders | Ashen Reach | Echo, Overdrive |
| Trial of the Prism (after the blade awakens) | Cinderfall | Prism Lens, Everlight |

## Shields

Shielded enemies no longer require the Aether Tether. A shield wears down after six blocked melee hits; finishers,
charged shots and skills count twice (about three), and plain shots glance off. Any hit of the element on its emblem
(the enemy's weakness) shatters it at once and stuns the enemy. The tether still rips it off in one pull, but leaves
the enemy enraged: faster on its feet and quicker to attack again.

## Memory crystals and eras

The Memory Lens item and its button (T / L3) are gone; L3 is unbound. Eras are reached through memory crystals
(`"type": "shard"` in map data). The Bastion's crystals are always awake. A crystal with a `"key"` (shared by the
crystals of both eras) sleeps until the Bastion is restored (`rift:ash_forge_anvil`), then an Aether Mote wakes it
for good (flag `crystal:<key>`). Motes are a stackable key item: two in the Bastion vault chest that used to hold
the Lens, and Tinker Varro sells them for 400 G. Which zones get era maps and where else motes turn up is content
still to place.

## Combat rank

A hit costs half a rank (carrying into the lower rank), and hits within one second of each other count once.

## Testing

A training hall with dummies that never die and show damage per second, plus debug switches: `--skills all`
(every skill, all levels, full-strength Aether form), `--weave all`, `--supports all` and `--mastery N`.
