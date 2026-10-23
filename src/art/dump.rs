//! The procedural sprites for a contact sheet, for reviewing art outside the game.

#![allow(dead_code)]

use crate::art::{bosses, combat, props};
use crate::core::color::Rgba;
use crate::core::gfx::TexId;
use crate::gameplay::element::Element;

pub fn all() -> Vec<(String, TexId)> {
    let mut sprites = vec![
        s("serpent", bosses::serpent()),
        s("golem", bosses::golem()),
        s("sovereign", bosses::sovereign()),
        s("sovereign_ascended", bosses::sovereign_ascended()),
        s("ice_tile", props::ice_tile()),
        s("ice_wall", props::ice_wall()),
        s("flame_wall", props::flame_wall()),
        s("iron_wall", props::iron_wall()),
        s("ice_shard", props::ice_shard()),
        s("thunder_bolt", props::thunder_bolt()),
        s("barrier_bubble", props::barrier_bubble()),
        s("element_orb", bosses::element_orb(Rgba::rgb(255, 120, 40))),
        s("ripple", bosses::ripple()),
    ];
    for id in ["flame_ring", "rabbit_boots", "tide_crystal", "power_charm", "coral_shield"] {
        sprites.push((format!("icon_{id}"), props::icon(id)));
    }
    sprites.extend([
        s("warlock", props::warlock()),
        s("tree", props::tree()),
        s("bush", props::bush()),
        s("boulder", props::boulder()),
        s("sign", props::sign()),
        s("pot", props::pot()),
        s("well", props::well()),
        s("pod", props::pod()),
        s("bed", props::bed()),
        s("table", props::table()),
        s("shelf", props::shelf()),
        s("counter", props::counter()),
        s("plant", props::plant()),
        s("save_crystal", props::save_crystal()),
        s("cave_mouth", props::cave_mouth()),
        s("stairs_down", props::stairs_down()),
        s("stairs_up", props::stairs_up()),
        s("cracked_rock", props::cracked_rock()),
        s("thorns", props::thorns()),
        s("gate", props::gate()),
        s("locked_door", props::locked_door()),
        s("floor_switch", props::floor_switch(false)),
        s("torch", props::torch()),
        s("blade_wind", combat::blade(Element::Wind)),
        s("blade_fire", combat::blade(Element::Fire)),
        s("slash_fire", combat::slash_arc(Element::Fire, false)),
        s("slash_none_heavy", combat::slash_arc(Element::None, true)),
        s("bolt_water", combat::bolt(Element::Water, false)),
        s("bolt_thunder_charged", combat::bolt(Element::Thunder, true)),
        s("tall_grass", combat::tall_grass()),
        s("flames", combat::flames()),
        s("scorch", combat::scorch()),
        s("ice_block", combat::ice_block()),
        s("turbine_base", combat::turbine_base()),
        s("turbine_blades", combat::turbine_blades()),
        s("brazier", combat::brazier()),
        s("crate", combat::crate_box()),
        s("cloud", combat::cloud()),
        s("tether_post", combat::tether_post()),
        s("lever", combat::lever()),
        s("guard_shield", combat::guard_shield()),
        s("enemy_shield", combat::enemy_shield()),
        s("hook", combat::hook()),
        s("vent", combat::vent()),
    ]);
    for mode in [Element::None, Element::Wind, Element::Fire, Element::Water, Element::Thunder, Element::Aether] {
        sprites.push((format!("emblem_{}", mode.key()), combat::emblem(mode)));
    }
    sprites.extend([
        s("wind_slash", props::wind_slash()),
        s("tornado", props::tornado()),
        s("fireball", props::fireball()),
        s("coin", props::coin()),
        s("heart_pickup", props::heart_pickup()),
        s("puff", props::puff()),
        s("house", props::house(4, 3, 1, "green")),
    ]);
    sprites
}

fn s(name: &str, tex: TexId) -> (String, TexId) {
    (name.to_string(), tex)
}
