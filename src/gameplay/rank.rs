//! The style meter: kills, perfect guards and close calls raise the rank from D to SSS, multiplying experience and
//! drops. Taking a hit costs half a rank; going quiet lets it decay.

const NAMES: [&str; 8] = ["", "D", "C", "B", "A", "S", "SS", "SSS"];
const MULTIPLIERS: [f32; 8] = [1.0, 1.1, 1.2, 1.35, 1.5, 1.7, 1.85, 2.0];

#[derive(Default)]
pub struct CombatRank {
    points: f32,
    timer: f32,
    hurt_grace: f32,
    pub rank: usize,
    pub flash: f32,
}

impl CombatRank {
    pub fn name(&self) -> &'static str {
        NAMES[self.rank]
    }

    pub fn multiplier(&self) -> f32 {
        MULTIPLIERS[self.rank]
    }

    pub fn progress(&self) -> f32 {
        if self.rank >= NAMES.len() - 1 { 1.0 } else { (self.points / self.threshold()).clamp(0.0, 1.0) }
    }

    pub fn timer(&self) -> f32 {
        self.timer
    }

    fn threshold(&self) -> f32 {
        2.0 + self.rank as f32 * 1.2
    }

    pub fn kill(&mut self, weight: f32) {
        self.style(weight);
    }

    pub fn style(&mut self, points: f32) {
        self.points += points;
        self.timer = 10.0;
        while self.rank < NAMES.len() - 1 && self.points >= self.threshold() {
            self.points -= self.threshold();
            self.rank += 1;
            self.flash = 0.5;
        }
    }

    /// Costs half a rank's progress, borrowing from the rank below; hits within a second of the last count once.
    pub fn player_hurt(&mut self) {
        if self.hurt_grace > 0.0 {
            return;
        }
        self.hurt_grace = 1.0;
        self.points -= self.threshold() * 0.5;
        while self.points < 0.0 {
            if self.rank == 0 {
                self.points = 0.0;
                return;
            }
            self.rank -= 1;
            self.points += self.threshold();
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.flash = (self.flash - dt).max(0.0);
        self.hurt_grace = (self.hurt_grace - dt).max(0.0);
        if self.rank == 0 {
            return;
        }
        self.timer -= dt;
        if self.timer > 0.0 {
            return;
        }
        self.rank -= 1;
        self.points = 0.0;
        self.timer = 5.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hurt_costs_half_a_rank_once_per_second() {
        let mut rank = CombatRank::default();
        rank.style(2.0 + 3.2 + 1.0);
        assert_eq!(rank.rank, 2);
        rank.player_hurt();
        assert_eq!(rank.rank, 1);
        assert!((rank.points - (1.0 - 2.2 + 3.2)).abs() < 0.001);
        rank.player_hurt();
        assert_eq!(rank.rank, 1);
        rank.update(1.1);
        rank.player_hurt();
        assert_eq!(rank.rank, 1);
        assert!((rank.points - 0.4).abs() < 0.001);
    }
}
