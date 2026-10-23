//! Per-creature elemental buildup gauges and active ailments. Buildup decays when not refreshed; a full gauge applies
//! the element's ailment.

use super::combat::CombatStats;
use super::element::Element;
use crate::core::color::Rgba;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ailment {
    None,
    Burn,
    Chill,
    Freeze,
    Shock,
    Gust,
    Conduct,
}

impl Ailment {
    pub fn label(self) -> &'static str {
        match self {
            Ailment::None => "NONE",
            Ailment::Burn => "BURN",
            Ailment::Chill => "CHILL",
            Ailment::Freeze => "FREEZE",
            Ailment::Shock => "SHOCK",
            Ailment::Gust => "GUST",
            Ailment::Conduct => "CONDUCT",
        }
    }
}

const THRESHOLD: f32 = 100.0;

#[derive(Clone, Default)]
pub struct StatusSet {
    buildup: [f32; 6],
    decay_delay: f32,
    burn_tick: f32,
    shock_tick: f32,
    pub burn: f32,
    pub chill: f32,
    pub frozen: f32,
    pub shock: f32,
    pub dazed: f32,
    pub burn_source: CombatStats,
}

impl StatusSet {
    pub fn wet(&self) -> bool {
        self.chill > 0.0 || self.frozen > 0.0
    }

    pub fn any(&self) -> bool {
        self.burn > 0.0 || self.chill > 0.0 || self.frozen > 0.0 || self.shock > 0.0 || self.dazed > 0.0
    }

    pub fn speed_factor(&self) -> f32 {
        if self.frozen > 0.0 {
            0.0
        } else if self.chill > 0.0 {
            0.6
        } else {
            1.0
        }
    }

    pub fn defense_factor(&self) -> f32 {
        if self.dazed > 0.0 { 0.75 } else { 1.0 }
    }

    pub fn buildup(&self, e: Element) -> f32 {
        self.buildup[e.index()] / THRESHOLD
    }

    /// Adds buildup and returns the ailment it triggered, if any.
    pub fn add(&mut self, element: Element, amount: f32, source: CombatStats, can_freeze: bool) -> Ailment {
        if matches!(element, Element::None | Element::Aether) || amount <= 0.0 {
            return Ailment::None;
        }
        self.decay_delay = 1.5;
        self.buildup[element.index()] += amount;
        if self.buildup[element.index()] < THRESHOLD {
            return Ailment::None;
        }
        self.buildup[element.index()] = 0.0;
        self.apply(element, source, can_freeze)
    }

    pub fn apply(&mut self, element: Element, source: CombatStats, can_freeze: bool) -> Ailment {
        match element {
            Element::Fire => {
                if self.frozen > 0.0 || self.chill > 0.0 {
                    self.frozen = 0.0;
                    self.chill = 0.0;
                }
                self.burn = 4.0;
                self.burn_source = source;
                Ailment::Burn
            }
            Element::Water => {
                self.burn = 0.0;
                if self.chill > 0.0 && can_freeze {
                    self.chill = 0.0;
                    self.frozen = 2.5;
                    return Ailment::Freeze;
                }
                self.chill = 5.0;
                Ailment::Chill
            }
            Element::Thunder => {
                let conduct = self.wet();
                self.shock = 4.0;
                self.shock_tick = 1.0;
                if conduct { Ailment::Conduct } else { Ailment::Shock }
            }
            Element::Wind => {
                self.dazed = 2.0;
                Ailment::Gust
            }
            _ => Ailment::None,
        }
    }

    /// Advances timers; returns burn damage to apply this frame and whether a shock twitch fired.
    pub fn update(&mut self, dt: f32) -> (f32, bool) {
        self.decay_delay -= dt;
        if self.decay_delay <= 0.0 {
            for b in &mut self.buildup {
                *b = (*b - 18.0 * dt).max(0.0);
            }
        }
        let mut burn_damage = 0.0;
        if self.burn > 0.0 {
            self.burn -= dt;
            self.burn_tick -= dt;
            if self.burn_tick <= 0.0 {
                self.burn_tick = 0.5;
                burn_damage = self.burn_source.atk * 0.12;
            }
        }
        let mut pulse = false;
        if self.shock > 0.0 {
            self.shock -= dt;
            self.shock_tick -= dt;
            if self.shock_tick <= 0.0 {
                self.shock_tick = 1.0;
                pulse = true;
            }
        }
        self.chill = (self.chill - dt).max(0.0);
        self.frozen = (self.frozen - dt).max(0.0);
        self.dazed = (self.dazed - dt).max(0.0);
        (burn_damage, pulse)
    }

    /// Ends a freeze early (a shattering blow) and returns whether the target was frozen.
    pub fn shatter(&mut self) -> bool {
        if self.frozen <= 0.0 {
            return false;
        }
        self.frozen = 0.0;
        self.chill = 2.0;
        true
    }

    pub fn extend_chill(&mut self, seconds: f32) {
        self.chill += seconds;
    }

    pub fn clear(&mut self) {
        let source = self.burn_source;
        *self = StatusSet { burn_source: source, ..StatusSet::default() };
    }

    /// The colour of the strongest active ailment, for tinting.
    pub fn tint(&self) -> Option<Rgba> {
        if self.frozen > 0.0 {
            Some(Rgba::rgb(170, 220, 255))
        } else if self.burn > 0.0 {
            Some(Rgba::rgb(255, 160, 100))
        } else if self.shock > 0.0 {
            Some(Rgba::rgb(255, 245, 150))
        } else if self.chill > 0.0 {
            Some(Rgba::rgb(200, 230, 255))
        } else {
            None
        }
    }
}
