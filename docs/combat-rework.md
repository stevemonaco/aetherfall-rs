# Combat rework: skills, the Circuit web, supports and Aether

Design notes for the skill system rework. Save compatibility is not kept (save version 3).

## The blade and the sigils

- The hero wakes holding the Aether Blade, dormant. Dormant means Neutral mode.
- The four elements come from Sigils set into the blade's hilt: Sigil of Wind (Elder Orin), Sigil of Fire (Warlock
  chest), Sigil of Water (Sunken Temple), Sigil of Thunder (Storm Spire). Owning a sigil unlocks its mode.
- Forgemaster Corvin no longer fuses four swords; with all four sigils set, his forge awakens the blade
  (flag `aether_awakened`), which unlocks the Aether burst.
- Blade power is per mode: `(4 + level) * (1 + 0.08 * mastery(mode))`, times the smith bonus. No sword outranks
  another.

## Mastery

- Each mode (Neutral and the four elements) has mastery 0-5, earned from kill EXP while fighting in that mode.
  A kill landed on a weakness counts 1.5x. The Mentor's Ribbon accessory adds 50%.
- Thresholds (cumulative): 500, 3000, 10000, 30000, 70000.
- Mastery raises that mode's blade power, opens the second support socket of each group at mastery 3, and lets
  Aether strike that element's weakness at mastery 3.

## Aether burst

- The Resonance gauge (0-100) fills when hits land on a weakness (melee 4, bolt 3, skill 3, kill 10).
- With the blade awakened and the gauge full, switching to Aether starts an 8 second burst that drains the gauge;
  when it empties the hero returns to the previous mode. No SP drain.
- During the burst a hit takes the target's weakest element among elements at mastery 3 or more; otherwise it is
  neutral. Auras break only when that element is the aura's opposite.

## Skills

- One skill button. Each mode equips one skill from that element's pool of three.
- Tap: level 1 (1 SP pip). Hold: the whole world slows to 30%; level 2 at 0.4s, level 3 at 0.9s (real time),
  auto-release at 1.5s. Level N costs N pips, paid on release, capped by unlocked level and SP. A hit cancels.
- The first skill of each element comes with the sigil; the other two are web notables. Levels 2 and 3 are
  element notables ("Technique" and "Apex").

| Mode | Skills |
|---|---|
| Neutral | Whirlwind, Triple Shot, Phantom Step |
| Wind | Gale Slash, Cyclone, Updraft |
| Fire | Cinder Rush, Meteor, Flame Aegis |
| Water | Frost Bloom, Glacier Lance, Ice Mirror |
| Thunder | Arc Cleave, Storm Call, Static Field |
| Aether | Prism Edge, Prism Ray, Sanctum |

## Triggers

Web notables can react to events: hit, kill, hurt, dodge, close call, guard, perfect guard, mode switch, overheat,
skill use and rank up. Former guard and dodge Arts live on as triggers (Riposte, Tempest Ward, Blaze Step, Mist
Veil, Arc Step, Flicker).

## The Circuit web

- One connected web: a hub with an arm per mode. Every node costs one point; keystones also need a Circuit Shard.
- Small nodes (stats), notables (triggers, skills, skill levels) and keystones (rule changes with a drawback).
- Free reset at any save crystal.

## Supports

- Three socket groups per mode: attack (melee combo), ranged (bolts) and special (skill). One socket each, two at
  mastery 3. Each gem is a single item, so it can sit in only one socket.
- Every support has a drawback. Prism supports fit only the Aether mode.
- Supports come from Trial Rifts: gold rifts in the world that lead to a hard arena with several rifts to seal.

## Combat rank

A hit costs half a rank (carrying into the lower rank), and hits within one second of each other count once.

## Testing

A training hall with dummies that never die and show damage per second, plus debug switches: `--skills all`,
`--tree all`, `--supports all` and `--mastery N`.
