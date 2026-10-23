//! A small seeded random generator standing in for System.Random.

use std::cell::Cell;

#[derive(Clone)]
pub struct Rng {
    state: Cell<u64>,
}

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng { state: Cell::new(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1) }
    }

    pub fn from_time() -> Rng {
        #[cfg(not(target_arch = "wasm32"))]
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(1);
        #[cfg(target_arch = "wasm32")]
        let seed = (js_sys::Date::now() * 1000.0) as u64;
        Rng::new(seed)
    }

    fn next_u64(&self) -> u64 {
        // xorshift64*
        let mut x = self.state.get();
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state.set(x);
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A float in [0, 1), like Random.NextSingle.
    pub fn next_f32(&self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// An integer in [min, max), like Random.Next(min, max).
    pub fn range(&self, min: i32, max: i32) -> i32 {
        if max <= min {
            return min;
        }
        min + (self.next_u64() % (max - min) as u64) as i32
    }

    /// An integer in [0, max), like Random.Next(max).
    pub fn below(&self, max: i32) -> i32 {
        self.range(0, max)
    }

    pub fn shuffle<T>(&self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = (self.next_u64() % (i as u64 + 1)) as usize;
            items.swap(i, j);
        }
    }
}
