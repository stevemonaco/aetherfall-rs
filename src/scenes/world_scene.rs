//! The world as the bottom of the scene stack: runs the world and opens whatever it asks for.

use super::game_over::GameOverScene;
use super::menu::MenuScene;
use super::restore::RestoreScene;
use super::scene::{Scene, SceneCtx};
use super::story::{ENDING, StoryScene};
use super::title::TitleScene;
use super::world::{World, WorldRequest};
use crate::core::gfx::Batch;
use crate::gameplay::game_state::GameState;
use crate::scene_any;

pub struct WorldScene {
    pub world: World,
}

impl WorldScene {
    pub fn new(state: GameState, auto_save: bool) -> WorldScene {
        WorldScene { world: World::new(state, auto_save) }
    }
}

impl Scene for WorldScene {
    scene_any!();

    fn enter(&mut self, _ctx: &mut SceneCtx) {
        self.world.enter();
    }

    fn update(&mut self, ctx: &mut SceneCtx, dt: f32) {
        if let Some(overlay) = self.world.next_overlay() {
            ctx.push_boxed(overlay);
            return;
        }
        self.world.update(dt);
        for request in std::mem::take(&mut self.world.requests) {
            match request {
                WorldRequest::Menu => ctx.push(MenuScene::new()),
                WorldRequest::Map => ctx.push(super::map_scene::MapScene::new()),
                WorldRequest::GameOver(state) => ctx.push(GameOverScene::new(*state)),
                WorldRequest::Ending => ctx.replace(StoryScene::new(ENDING, Some(Box::new(TitleScene::new())))),
                WorldRequest::Restoration { town, flag, label } => {
                    ctx.push(RestoreScene::new(self.world.state.clone(), &town, &flag, &label))
                }
            }
        }
    }

    fn draw(&self, _below: &[Box<dyn Scene>], batch: &mut Batch) {
        self.world.draw(batch);
    }
}
