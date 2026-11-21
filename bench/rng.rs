// The Rust twins' copy of CobaltC's `std::Rng` (impl/src/prelude.rs),
// written the same way: xoshiro256** seeded by splitmix64, and `below`
// by rejection, so every build draws the same sequence. Not a workload:
// `mod rng;` in strings.rs, hashcount.rs and tree.rs.
pub struct Rng {
    s0: u64,
    s1: u64,
    s2: u64,
    s3: u64,
}

fn rotl(x: u64, k: u32) -> u64 {
    (x << k) | (x >> (64 - k))
}

fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

impl Rng {
    pub fn new(seed: u64) -> Rng {
        let mut state = seed;
        let a = splitmix(&mut state);
        let b = splitmix(&mut state);
        let c = splitmix(&mut state);
        let d = splitmix(&mut state);
        Rng { s0: a, s1: b, s2: c, s3: d }
    }

    pub fn next_u64(&mut self) -> u64 {
        let result = rotl(self.s1.wrapping_mul(5), 7).wrapping_mul(9);
        let t = self.s1 << 17;
        self.s2 ^= self.s0;
        self.s3 ^= self.s1;
        self.s1 ^= self.s2;
        self.s0 ^= self.s3;
        self.s2 ^= t;
        self.s3 = rotl(self.s3, 45);
        result
    }

    pub fn below(&mut self, n: u64) -> u64 {
        let reject_under = 0u64.wrapping_sub(n) % n;
        let mut x = self.next_u64();
        while x < reject_under {
            x = self.next_u64();
        }
        x % n
    }
}
