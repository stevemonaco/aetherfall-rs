//! The scene stack: the world at the bottom, with menus and dialogue pushed over it as overlays.

use std::any::Any;

use crate::core::gfx::Batch;

use super::world::World;
use super::world_scene::WorldScene;

pub enum SceneCmd {
    Push(Box<dyn Scene>),
    Pop,
    /// Clears the whole stack and starts over with this scene.
    Replace(Box<dyn Scene>),
}

/// What a scene can reach while it updates: the scenes beneath it and the command queue.
pub struct SceneCtx<'a> {
    pub below: &'a mut [Box<dyn Scene>],
    pub cmds: &'a mut Vec<SceneCmd>,
}

impl SceneCtx<'_> {
    pub fn push(&mut self, scene: impl Scene) {
        self.cmds.push(SceneCmd::Push(Box::new(scene)));
    }

    pub fn push_boxed(&mut self, scene: Box<dyn Scene>) {
        self.cmds.push(SceneCmd::Push(scene));
    }

    pub fn pop(&mut self) {
        self.cmds.push(SceneCmd::Pop);
    }

    pub fn replace(&mut self, scene: impl Scene) {
        self.cmds.push(SceneCmd::Replace(Box::new(scene)));
    }

    /// The world beneath this overlay, if any.
    pub fn world(&mut self) -> Option<&mut World> {
        self.below.iter_mut().rev().find_map(|s| s.as_any_mut().downcast_mut::<WorldScene>()).map(|w| &mut w.world)
    }
}

pub trait Scene: Any {
    /// Overlays (menus, dialogue) let the scene beneath them keep drawing.
    fn is_overlay(&self) -> bool {
        false
    }

    fn enter(&mut self, _ctx: &mut SceneCtx) {}
    fn exit(&mut self) {}
    fn update(&mut self, ctx: &mut SceneCtx, dt: f32);
    fn draw(&self, below: &[Box<dyn Scene>], batch: &mut Batch);
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// Implements the downcasting accessors for a scene.
#[macro_export]
macro_rules! scene_any {
    () => {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    };
}

/// The world beneath an overlay, for drawing.
pub fn world_below(below: &[Box<dyn Scene>]) -> Option<&World> {
    below.iter().rev().find_map(|s| s.as_any().downcast_ref::<WorldScene>()).map(|w| &w.world)
}

#[derive(Default)]
pub struct SceneManager {
    stack: Vec<Box<dyn Scene>>,
    cmds: Vec<SceneCmd>,
}

impl SceneManager {
    pub fn push(&mut self, scene: Box<dyn Scene>) {
        self.cmds.push(SceneCmd::Push(scene));
    }

    pub fn is_empty(&self) -> bool {
        self.stack.is_empty() && self.cmds.is_empty()
    }

    /// The world at the bottom of the stack, if a game is running.
    pub fn world(&self) -> Option<&World> {
        world_below(&self.stack)
    }

    pub fn update(&mut self, dt: f32) {
        self.apply_pending();
        if let Some((top, below)) = self.stack.split_last_mut() {
            let mut ctx = SceneCtx { below, cmds: &mut self.cmds };
            top.update(&mut ctx, dt);
        }
        self.apply_pending();
    }

    pub fn draw(&self, batch: &mut Batch) {
        let mut first = self.stack.len().saturating_sub(1);
        while first > 0 && self.stack[first].is_overlay() {
            first -= 1;
        }
        for i in first..self.stack.len() {
            self.stack[i].draw(&self.stack[..i], batch);
        }
    }

    fn apply_pending(&mut self) {
        while !self.cmds.is_empty() {
            for cmd in std::mem::take(&mut self.cmds) {
                match cmd {
                    SceneCmd::Push(mut scene) => {
                        let mut ctx = SceneCtx { below: &mut self.stack, cmds: &mut self.cmds };
                        scene.enter(&mut ctx);
                        self.stack.push(scene);
                    }
                    SceneCmd::Pop => {
                        if let Some(mut top) = self.stack.pop() {
                            top.exit();
                        }
                    }
                    SceneCmd::Replace(mut scene) => {
                        for s in self.stack.iter_mut() {
                            s.exit();
                        }
                        self.stack.clear();
                        let mut ctx = SceneCtx { below: &mut self.stack, cmds: &mut self.cmds };
                        scene.enter(&mut ctx);
                        self.stack.push(scene);
                    }
                }
            }
        }
    }
}
