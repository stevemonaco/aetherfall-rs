use super::boss::{BossBrain, BossCore};
use super::enemy::Enemy;
use crate::core::geom::Box2;
use crate::core::gfx::Batch;
use crate::scenes::world::World;

pub struct SovereignBrain {}

impl SovereignBrain {
    pub fn new(_enemy: &mut Enemy) -> SovereignBrain {
        SovereignBrain {}
    }
}

impl BossBrain for SovereignBrain {
    fn think(&mut self, _enemy: &mut Enemy, _world: &mut World, _dt: f32) {}

    fn draw(&self, _enemy: &Enemy, _boss: &BossCore, _world: &World, _batch: &mut Batch) {}

    fn hurtbox(&self, enemy: &Enemy) -> Box2 {
        Box2::new(enemy.base.pos.x - 12.0, enemy.base.pos.y - 32.0, 24.0, 32.0)
    }
}