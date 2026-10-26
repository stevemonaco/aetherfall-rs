# Combat rework: skills, the Weave, supports and the Aether

Design notes for the skill system rework. Saves from the earlier rework (save version 3) load and are migrated;
the Weave changed in versions 4 to 6, so older saves get every Weave point back.

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
- Each mode has its own HUD icon (a gust, a flame, a drop and a bolt), and switching plays a flourish around the hero:
  a whirl of wind, a pillar of flame, a water spout or a lightning strike with crackling arcs.

## Mastery

- Each element has mastery 0-5, earned from kill EXP while fighting in that mode. A kill landed on a weakness counts
  1.5x. The Mentor's Ribbon accessory adds 50%.
- Thresholds (cumulative): 800, 4500, 15000, 45000, 100000.
- Mastery raises that mode's blade power, teaches the element's second skill at mastery 1 and third at mastery 3,
  opens the second support socket of each group at mastery 3, and makes Aether pierce that element's weakness more
  often.

## The Aether

Overheat is gone. Its meter now builds the Aether instead.

- Landing hits fills the Aether meter (melee 1.2, finisher 3, bolt 0.6, charged bolt 2, skill hit 1.5), 1.75x on a
  weakness. Ranged builds it more slowly to balance the safer distance. Accessories (Resonant Stone), Weave nodes, the
  Prism Band and gems scale it.
- The meter's size grows with the blade: 60 with one sigil, +20 per further sigil, 140 once awakened. Each Aether
  Conduit (a key item, five in the world: Gullwing Shoals, Rimewater Gorge, Stormbreak Ridge, the Ashen Barrens and
  Cloudstep Ascent) makes it 8% smaller.
- Progress locks in after 4 seconds without being hit, and at once when a combo finisher lands. A hit drains up to
  15% of the meter, but never below the locked part. The HUD shows the locked part solid and the loose part pale. The
  Lodestone accessory (dug up in the Drowned Causeway) cuts the delay to 2 seconds; the Anchor notable cuts a second
  and makes charged bolts lock it too; the Unbroken keystone locks everything at once but fills 25% slower.
- When the meter is full, Aether wakes by itself for 6 seconds (8 once awakened). The equipped Aether form fires
  once as it wakes. While it is awake, hits never shorten it: the hero takes 30% less damage, isn't staggered and
  builds ailments at half the rate. Mode switching stays open.
- While awake, every hit may pierce to the target's weakness instead of landing in the current mode's element:
  25% + 15% per mastery level of that weakness element, +25% once awakened, +15% with the Prism Band, +20% with the
  Prism Lens, plus Weave nodes, capped at 100%. Hits that pierce a shield's emblem shatter it.
- The meter can't fill while Aether is awake, and empties when it fades; it starts filling again at once. The
  Afterglow notable leaves a quarter of it filled and locked.
- Aether hits open Concord seals once the blade has awakened, and are the only hits that reach Aether-sealed foes
  (the First Concord's Unbound phase, Sovereign Vael's aether form). Striking those foes with other hits feeds the
  meter instead.

| Form (Aether page) | Learned | Level 1 | Level 2 | Level 3 |
|---|---|---|---|---|
| Prism Edge | First sigil | 3 blades at the nearest foes | 4 blades | 6 blades and an Aether burst |
| Prism Ray | Third sigil | One beam along the aim | A wider, stronger beam | Three beams |
| Sanctum | Awakening | Heal 6%; aura for all of Aether: blows at 70%, no ailments or blight, cleansing pulse every 2s | Heal 10%; blows at 60% | Heal 15%; blows at 50% |

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

- A lattice on a 17x17 grid: threads run along every even row and column inside a diamond, 177 nodes. A node can be
  woven once an orthogonal neighbour is; the cursor walks the threads cell by cell, Q/E jump between the keystones and
  the skill button zooms out to the whole Weave.
- Regions are themed by playstyle, not element: Might (north: ATK, FOC, finishers, kill rewards), Ward (south: life,
  DEF, regeneration, guard), Swiftness (east: speed, dodges), Spirit (west: SP, ailments, skills) and Aether (the
  diagonal bands: meter, pierce, duration).
- Minor nodes rotate three stats per region and grow with distance from the hub: +1% ATK near it, +2% in the middle
  ring, +3% at the rim. Core stats also appear at half value in other regions (ATK in Swiftness and Aether, life in
  Spirit), so no stat sits in a cheap cluster. Might's west half leans ranged (FOC, bolt surge) and its east half
  melee (reach, finisher damage).
- 50 notables carry the triggers and passives (Riposte, Phantom Step, Wildfire, Bloodlust, Battle Trance, Elite
  Hunter, Last Stand, Iron Will, Acrobat, Slipstream, Echoing Arts, Anchor, Afterglow, Surging Shots, Long Reach...). Ten keystones need a Weave Knot:
  Overcharge, Glass Edge, Stillwater, Bulwark, Tailwind, Blood Art, Aether Bond, Unbroken, Iron Vanguard and Crescendo.
- One point per level plus Tomes of Insight: about 105 points by the end of the Hollow, so roughly 60% of the Weave.
  Free unpicking at any save crystal.

## Melee, guarding and shots

- Iron Vanguard (keystone, Might's melee side): melee reaches 10% further, contact hits deal 75% less, and contact
  hits and light blows (non-projectile attacks under 1.3 power) no longer knock you back or cancel your actions.
  Slams, charges, blasts and shots still do. In exchange, plain and charged shots deal 90% less damage and build
  ailments at half rate, but still carry their element, break obstacles and hit switches. Skills are unaffected.
- Crescendo (keystone, Aether band): only finishers and charged shots fill the Aether meter, at twice their usual
  rate (6 and 4). Other sources, such as triggers that feed the meter, still work.
- Swings reach further: 26 pixels (34 for finishers), before the Cleave gem and the Weave's reach nodes (Long Reach,
  Whirling Edge and Reach minors).
- Guarding cancels a swing or a charging throw. Only a perfect guard (parry) or one raised a moment before it (a clean
  block, within two perfect windows) stops a hit entirely. Any other block lets 15-50% of the damage through, more as
  the guard weakens. Guard strength drops with each block and slowly while held, and recovers once the guard is
  lowered; at zero the guard breaks, stunning the hero and letting most of the hit through. A bar under the hero
  shows it.
- Bolts come in three tiers: plain, charged and tempest (bigger, piercing, 2.3 power); every bolt bounces off a wall at most once. A bolt can
  surge one tier stronger: Surging Shots (15%), Tempest Quiver (10%), Surge minors (1-3% each) and the Surging gem
  (25%) all stack.

## Items

- Healing comes only from towns: shops, and two early gifts (the Elder's herbs and the Brinewatch mayor's elixir).
  No chest, dig spot or foe outside a town holds a remedy. Stacks are small: three herbs, two draughts, two spirit
  tonics, one elixir. A remedy that doesn't fit is used on the spot instead of being lost. Remedies are dear:
  herbs 150 G, draughts 450 G, spirit tonics 400 G, cooling salves 120 G and elixirs 2000 G.
- The Scholar's Lens grants 20% more EXP and the Fortune Coin 50% more gold. Both are sold in Highcrag (act 3) for
  2400 G and 1500 G; the Lens is also at Ashford's tinker.

## Progression

- Gold is scarcer: kills drop 1.1 + 0.9 per level (down from 2 + 1.6), and most maps keep at most one chest of plain
  gold or materials (puzzle rewards excepted). Chests show what they hold: plain wood for gold and materials, red for
  equipment and gems, purple-gold for key items and permanent upgrades.
- Monsters have 25% more life and 12% more attack. The hero's base life is 25% lower (82 + 10.5 per level).
- Kills give half the EXP they did. Fighting above your level gives up to 25% more (5% per level); fighting below it
  costs 15% per level, so falling behind matters and farming easy areas does not pay.

## Elites

- Elites have three times a normal foe's life (down from four), two levels more and a break gauge. They roll
  affixes: one below level 20, two below 50, three after. Their affixes show above the life bar, which stays up while
  they are alert, and a golden outline marks them.
- Affixes: Swift (faster movement and attacks), Blinking (teleports behind the hero), Frenzied (enrages at half
  life), Vampiric (heals when it lands a hit), Shielded (a shield that regrows 8 seconds after breaking), Volatile
  (bursts a moment after death), Noxious (trails poison clouds), Thorned (plain bolts glance off), Rallying (calls up
  to four lesser kin), Aura-bound (the old elemental aura that only its opposite strips) and Hulking (40% more life,
  never flinches).
- An elite kill gives three times the EXP and gold and a guaranteed heart.
- Roaming packs: entering a combat map (not towns, boss rooms or Blighted expeditions) has a 30% chance (60%
  under the Swarm pact) to hold an elite of one of the map's kinds with two to four of its foes, somewhere reachable
  but well away from the entrance and the exits.
- Rifts are breaches: walking near one tears it open, a purple circle spreads, and its whole quota swarms out of the
  edge within a couple of seconds. Elite rifts send the elite first, then escorts (3 by default). Once every foe it
  sent has fallen the breach stills; stand in the rift for a second to seal it. Leaving the breach for 4 seconds
  while it is open collapses it: its foes are drawn back in and it resets.
- Sealing a rift that restores part of a town cuts to that town: the camera settles on what came back, it
  shimmers into being (a house turns solid, a townsperson appears, planks span a gap), then the scene returns to
  where you stood.

## Bestiary

- New attacks: Burst (a quick string of re-aimed shots), Spiral (a turning two-armed stream), Mortar (lobbed shells
  onto marked circles), Eruption (a row of blasts racing along the aim), Hook (a line that drags the hero in),
  Whirl (spins toward the hero, striking on every beat), Blink (teleports behind the hero), Mend (heals hurt allies
  nearby) and Ward (shields allies nearby). Support casts are only chosen when an ally needs them.
- New traits: Volatile (bursts in its element on death), Regen (heals 4% a second once left alone) and Enrage (faster
  movement and attacks below half life).
- 34 new foes across every act and the Hollow, from Field Rats and Glade Tenders to Rime Ogres, Storm Callers, Slag
  Shamans and Void Reapers. Each zone swaps a few of its old foes for new ones, and the new kinds join its rifts and
  the trial arenas.
- Some old foes learned new moves: Cave Spiders hook, Inklings burst, Hex Acolytes and Ember Wraiths blink, Sky
  Magi lob mortars, Void Clerics mend, Void Eyes spiral, and Frost Brutes and Concord Knights enrage.

## Zones

Every act's dungeon now sits a journey away from its town, and new zones fill the level gaps:

| Zone | Level | Where |
|---|---|---|
| Thornback Hills | 6 | Through the Fields' rockfall tunnel; the Mossy Grotto is at the top |
| Drowned Causeway | 13 | Between Mistmoor Marsh and Brinewatch |
| Gullwing Shoals | 15 | The skiff from Brinewatch's pier; the Sunken Temple is at the far end |
| Rimewater Gorge | 19 | Between Brinewatch and Frostpeak Trail |
| Stormbreak Ridge | 23 | North of Highcrag; the Storm Spire is at its end |
| Ashen Barrens | 34 | Between the Ashen Rim and the Fallen Bastion |
| Cloudstep Ascent | 38 | North-west of Highcrag (needs all three crystals); the Sky Altar is at the top |
| The Hollow's Mouth | 68 | Stairs beneath the Sleeper's Vault after the game is cleared |

Each has two plain rifts and an elite rift, side pockets with loot (Tomes of Insight, Weave Knots, Aether Conduits)
and, for dungeon approaches, a save crystal by the entrance.

## Supports

- Three socket groups per mode: attack (melee combo), ranged (bolts) and special (skill). One socket each, two at
  mastery 3. Each gem is a single item, so it can sit in only one socket.
- Multishot's three bolts share one volley: each foe takes at most one bolt from it, even after bounces. Gale Slash,
  the triple Ice Lance and the triple Prism Ray share their hits the same way, and hit harder to make up for it
  (2.6, 2.6 and 3.0 per hit).
- Every support but Surging has a drawback. The Aether page has a single socket for Prism supports: Prism Lens (pierces 20% more
  often, 1.5s shorter) and Everlight (3s longer, meter fills 20% slower).
- New gems: Lunging (swings dash to the nearest foe ahead), Sweeping (every swing sweeps a full circle around you),
  Whirlwind (hold attack to spin while moving, striking on every beat; whirling burns 20 SP a second),
  Deflecting (swings cut down enemy shots; finishers send them back), Sundering (shields wear twice as fast, guards
  break faster), Surging (bolts surge a tier 25% of the time, with no drawback), Seeking (bolts curve toward foes) and Executioner (35%
  more damage to broken or frozen foes, 10% less otherwise). All but Surging carry a drawback.

## Golden rifts

A golden crack in the world leads to a 46x30 arena themed to its host, with its own hazards (bog, ice, pits, metal,
water). At its heart is a golden rift.

- Coming near the rift opens one great breach (radius about nine tiles) that pours out four escalating waves: 8 foes;
  10 foes and an elite; 12 foes and two elites; then the Rift Champion with 6 escorts. The waves are fixed, so there is
  nothing to farm.
- The breach holds you in: outside its gold ring your life drains (5% a second, never below 1), and after 6 seconds
  outside it collapses, sweeps away its foes and must be fought again from the first wave.
- The Rift Champion is one of the trial's foes at 1.5x size with five times a foe's life and an extra affix. Every
  4.5 seconds it alternates a ring of telegraphed blasts around you with a nova of shots from itself.
- Every visit rolls two trial mods (three in the Trial of the Prism), shown at the top of the screen with the wave
  counter: Haste, Volatile, Eruptions, Ironhide, Elite Tide and Bloodthirst. Each adds 25% EXP and gold.
- Clearing the last wave opens two of the gem chests around the rift (three in the Trial of Cinders). A cleared golden
  rift can be rekindled once at Heat (interact with it) for the last and strongest gem.
| Trial | Host | Gems |
|---|---|---|
| Trial of Thorns | Whisperwood | Cleave, Brutal, Lunging |
| Trial of Mire | Mistmoor Marsh | Momentum, Infusion, Sweeping |
| Trial of Salt | Salt Mine | Piercing, Lingering, Seeking |
| Trial of Rime | Frostpeak | Multishot, Leech, Sundering |
| Trial of Tides | Cove | Efficiency, Haste, Surging |
| Trial of Cinders | Ashen Reach | Echo, Overdrive, Deflecting, Whirlwind |
| Trial of the Prism (after the blade awakens) | Cinderfall | Prism Lens, Everlight, Executioner |

## Gemcutter's Prisms

- At its top tier a gem's upside is 40% larger and its drawback two-thirds smaller; gems whose upside is a new
  behaviour (Multishot, Sweeping, Whirlwind...) hit 4% harder per tier instead. Use a Prism from the Items tab.
- A Prism hones a gem one tier (Cut, Polished, Flawless, Perfect); each step costs as many Prisms as the tier it
  reaches, ten for a Perfect gem. Prisms come from puzzle and exploration chests across every act, elites (5%, unless the hero is more than 10 levels above
  them; never cut off near the level cap) and Ashford's tinker (3000 G).
- Heat adds two trial mods and Meagre (half EXP, gold and drops), and its foes are eight levels higher.

## Puzzles

- New pieces: element crystals (open only to one element's hit, optionally on a timer; bolts count, so bank shots off
  walls reach hidden ones) and toggle switches (each hit flips a flag; gates opened by the flag and by its opposite
  take turns).
- Each act dungeon has a signature mechanic: the Grotto wind, the Temple water, the Spire conduction, the Bastion
  past and present, the Citadel all of them together. Fields and side areas hide small optional puzzles in front of
  purple chests.
- The map screen's area view (Confirm to switch; dungeons open on it) charts the current map with the hero, exits,
  rifts, locked doors and unopened chests.
- A Wayfarer by every town's save crystal points at the next step on the road.

## The guardian spirit
- The hero wakes with a guardian spirit that fights beside them by itself: it zaps the nearest foe every couple of
  seconds. Spirit hits scale with the hero's stats and don't trigger support gems.
- Freeing a beast from a rift teaches the spirit its form, each with its own auto-attack and exploration perk:
  - Hound (Windward Fields): bites; sniffs out buried treasure.
  - Hawk (Whisperwood's heart): dives and marks foes to take more damage; reveals hidden crossings (`"type": "veil"`
    tiles that become walkable planks while it flies with you, as over the brook in Whisperwood).
  - Lantern Wisp (Fallen Bastion): flings light; lights the dark and reveals cloaked foes.
  - Ember Sprite (Ashen Reach): spits fireballs; burns away thickets you walk up to, and fire ailments build faster.
- The Soul button cycles the forms the spirit knows; the Spirit tab of the menu also picks a form and sets two charm
  slots. The Guardian (Sunken Temple) and Stormling (Storm Spire) now leave charms instead: the Guardian Charm turns
  aside a projectile every 10 seconds, the Storm Charm arcs each spirit attack on to a second foe.
- Two Weave notables strengthen it: Kindred Spirit (+30% spirit damage) and Pack Leader (+25% attack speed, +10%
  damage). Soul ranks are gone.

## Towns

- Towns start sparse; restorations fill them in. Rifts on the road to a town now clear the way instead: thorns of
  rift-growth seal each exit until that map's rift is sealed (Thornback Hills to the Grotto, Mistmoor Marsh to the
  Causeway, the Causeway to Brinewatch, Rimewater Gorge to Frostpeak, Frostpeak to Highcrag, the Ashen Reach to
  Ashford).
- Towns are restored by rifts past them: Brinewatch's net shed and shipwright from Gullwing Shoals, Highcrag's hearth
  and apprentice from Stormbreak Ridge, Ashford's tents and tinker from the Ashen Barrens.
- Leafhollow's Echo Stone (the training hall) returns with the Weave shrine. Each shrine has its own look now: Echo
  Stone, Weave shrine, Pact Shrine and Corvin's forge no longer borrow the Sky Altar.

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
