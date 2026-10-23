//! Builds entities from map file entries.

use crate::entities::blight_hazards::{BlightGate, Cannon, StormGrid};
use crate::entities::blocks::{AetherSeal, IceBlock, LensShard, PressurePlate, TetherLever, TetherPost};
use crate::entities::boss;
use crate::entities::companion::DigSpot;
use crate::entities::enemy::Enemy;
use crate::entities::entity::Entity;
use crate::entities::hazards::GasVent;
use crate::entities::obstacles::{Brazier, Crate, ObstacleKind, TileObstacle, Turbine};
use crate::entities::props::{Building, Chest, Decor, Npc, Sign, Warp};
use crate::entities::puzzles::{FloorSwitch, Gate, LockedDoor, SaveCrystal, SpanBridge, Trigger};
use crate::entities::rift::Rift;
use crate::entities::tide::Tide;
use crate::entities::trial::TrialGate;
use crate::gameplay::element::Element;
use crate::gameplay::game_data::data;
use crate::scenes::world::World;
use crate::world::map::{EntityData, tile_feet};

pub fn create(world: &mut World, entry: &EntityData) -> Option<Box<dyn Entity>> {
    let map_id = world.map_id().to_string();
    let entity: Box<dyn Entity> = match entry.kind.as_str() {
        "npc" => Box::new(Npc::new(entry)),
        "sign" => Box::new(Sign::new(entry)),
        "chest" => Box::new(Chest::new(entry, &map_id)),
        "warp" => Box::new(Warp::new(entry)),
        "decor" => Box::new(Decor::new(entry)),
        "barrier" => Box::new(legacy_barrier(entry, &map_id)),
        "thicket" => Box::new(TileObstacle::new(entry, &map_id, ObstacleKind::Thicket)),
        "icewall" => Box::new(TileObstacle::new(entry, &map_id, ObstacleKind::IceWall)),
        "flamewall" => Box::new(TileObstacle::new(entry, &map_id, ObstacleKind::FlameWall)),
        "shocklock" => Box::new(TileObstacle::new(entry, &map_id, ObstacleKind::ShockLock)),
        "rock" => Box::new(TileObstacle::new(entry, &map_id, ObstacleKind::Rock)),
        "secret" => Box::new(TileObstacle::new(entry, &map_id, ObstacleKind::Secret)),
        "rift" => Box::new(Rift::new(entry)),
        "dig" => Box::new(DigSpot::new(entry, &map_id)),
        "span" => Box::new(SpanBridge::new(entry)),
        "turbine" => Box::new(Turbine::new(entry)),
        "tide" => Box::new(Tide::new(entry)),
        "storm" => Box::new(StormGrid::new(entry)),
        "cannon" => Box::new(Cannon::new(entry)),
        "blightgate" => Box::new(BlightGate::new(entry)),
        "trial" => Box::new(TrialGate::new(entry)),
        "brazier" => Box::new(Brazier::new(entry)),
        "crate" => Box::new(Crate::new(entry)),
        "vent" => Box::new(GasVent::new(entry)),
        "gate" => Box::new(Gate::new(entry)),
        "switch" => Box::new(FloorSwitch::new(entry)),
        "door" => Box::new(LockedDoor::new(entry, &map_id)),
        "save" => Box::new(SaveCrystal::new(entry)),
        "trigger" => Box::new(Trigger::new(entry, &map_id)),
        "iceblock" => Box::new(IceBlock::new(entry, &map_id, false)),
        "stoneblock" => Box::new(IceBlock::new(entry, &map_id, true)),
        "plate" => Box::new(PressurePlate::new(entry)),
        "post" => Box::new(TetherPost::new(entry)),
        "lever" => Box::new(TetherLever::new(entry)),
        "shard" => Box::new(LensShard::new(entry)),
        "seal" => Box::new(AetherSeal::new(entry, &map_id)),
        "building" => {
            let building = Building::new(entry);
            if entry.str("to").is_some() {
                let mut warp = EntityData {
                    kind: "warp".into(),
                    x: building.door_tile.0 as f32,
                    y: building.door_tile.1 as f32,
                    ..EntityData::default()
                };
                for key in ["to", "tx", "ty", "facing", "blocked"] {
                    if let Some(v) = entry.extra.get(key) {
                        warp.extra.insert(key.into(), v.clone());
                    }
                }
                if let Some(cond) = entry.extra.get("doorIf").or(entry.extra.get("restore")) {
                    warp.extra.insert("if".into(), cond.clone());
                }
                warp.extra.insert("sound".into(), "door".into());
                world.add(Warp::new(&warp));
            }
            Box::new(building)
        }
        "enemy" | "boss" => {
            let kind = entry.str_or("kind", "slime");
            let def = data().enemies.get(kind)?;
            let pos = tile_feet(entry.x as i32, entry.y as i32);
            let level = entry.int("level", 0);
            let level = (level > 0).then_some(level);
            if entry.kind != "boss" {
                let map_level = world.data.level;
                return Some(Box::new(Enemy::new(
                    def,
                    pos,
                    Some(level.unwrap_or(if map_level > 1 { map_level } else { def.level })),
                )));
            }
            let flag = entry.str("flag").map(str::to_string).unwrap_or_else(|| format!("boss_{kind}"));
            Box::new(boss::create(def, pos, flag, entry.strings("defeat"), entry.bool("echo", false)))
        }
        _ => return None,
    };
    Some(entity)
}

// Maps from 1.0 describe obstacles as elemental barriers; translate them into the chemistry objects.
fn legacy_barrier(entry: &EntityData, map_id: &str) -> TileObstacle {
    let kind = match (entry.str("art"), Element::parse(entry.str("element"))) {
        (Some("ice"), _) => ObstacleKind::IceWall,
        (_, Element::Fire) => ObstacleKind::Thicket,
        (_, Element::Water) => ObstacleKind::FlameWall,
        (_, Element::Thunder) => ObstacleKind::ShockLock,
        _ => ObstacleKind::Rock,
    };
    TileObstacle::new(entry, map_id, kind)
}
