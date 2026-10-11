//! ISAAC variants recovered from Devcat Core.dll and Vindictus ServiceCore.dll.
//!
//! This is legacy transport compatibility code, not a modern security primitive.
//! It does not implement the transport handshake or packet salt negotiation.

const SIZE: usize = 256;
const GOLDEN_RATIO: u32 = 0x9e37_79b9;

/// Legacy ISAAC state with the original reverse block output order.
/// Source: Core.dll SHA256 8942a0c26a9abf4c5f0d1f7ffff41452a3714d099e3b76012cebdee97293c2f0,
/// Devcat.Core.Net.Message.Isaac. All additions intentionally wrap at 32 bits.
pub struct LegacyIsaac {
    results: [u32; SIZE],
    memory: [u32; SIZE],
    count: usize,
    a: u32,
    b: u32,
    c: u32,
}

impl LegacyIsaac {
    /// Initialize using the legacy signed seed constructor.
    pub fn seeded(seed: i32) -> Self {
        let mut results = [0; SIZE];
        results[0] = seed as u32;
        Self::from_results(results)
    }

    /// Initialize the actual Vindictus frontend variant from ServiceCore.Isaac.
    /// It multiplies the salt by four with wrapping and writes two seed slots.
    pub fn heroes_seeded(salt: u32) -> Self {
        let seed = salt.wrapping_mul(4);
        let mut results = [0; SIZE];
        results[0] = seed;
        results[(seed % 64) as usize] = seed;
        Self::from_results(results)
    }

    fn from_results(results: [u32; SIZE]) -> Self {
        let mut generator = Self {
            results,
            memory: [0; SIZE],
            count: 0,
            a: 0,
            b: 0,
            c: 0,
        };
        let mut state = [GOLDEN_RATIO; 8];
        for _ in 0..4 {
            mix(&mut state);
        }
        for index in (0..SIZE).step_by(8) {
            for (offset, word) in state.iter_mut().enumerate() {
                *word = word.wrapping_add(generator.results[index + offset]);
            }
            mix(&mut state);
            generator.memory[index..index + 8].copy_from_slice(&state);
        }
        for index in (0..SIZE).step_by(8) {
            for (offset, word) in state.iter_mut().enumerate() {
                *word = word.wrapping_add(generator.memory[index + offset]);
            }
            mix(&mut state);
            generator.memory[index..index + 8].copy_from_slice(&state);
        }
        generator.generate();
        generator.count = SIZE;
        generator
    }

    /// Return the next signed 32-bit word, preserving C# int bit patterns.
    pub fn next_word(&mut self) -> i32 {
        if self.count == 0 {
            self.generate();
            self.count = SIZE;
        }
        self.count -= 1;
        self.results[self.count] as i32
    }

    /// Apply the recovered CryptoTransform byte operation in place.
    /// Each byte consumes an entire word and uses its low eight bits.
    /// Repeated calls maintain stream position; this does not negotiate a salt.
    pub fn xor_bytes(&mut self, bytes: &mut [u8]) {
        for byte in bytes {
            *byte ^= self.next_word() as u8;
        }
    }

    fn generate(&mut self) {
        self.c = self.c.wrapping_add(1);
        self.b = self.b.wrapping_add(self.c);
        for index in 0..SIZE {
            let x = self.memory[index];
            self.a ^= match index % 4 {
                0 => self.a << 13,
                1 => self.a >> 6,
                2 => self.a << 2,
                _ => self.a >> 16,
            };
            self.a = self.a.wrapping_add(self.memory[(index + 128) % SIZE]);
            let y = self.memory[((x & 0x3fc) >> 2) as usize]
                .wrapping_add(self.a)
                .wrapping_add(self.b);
            self.memory[index] = y;
            self.b = self.memory[(((y >> 8) & 0x3fc) >> 2) as usize].wrapping_add(x);
            self.results[index] = self.b;
        }
    }
}

fn mix(s: &mut [u32; 8]) {
    s[0] ^= s[1] << 11;
    s[3] = s[3].wrapping_add(s[0]);
    s[1] = s[1].wrapping_add(s[2]);
    s[1] ^= s[2] >> 2;
    s[4] = s[4].wrapping_add(s[1]);
    s[2] = s[2].wrapping_add(s[3]);
    s[2] ^= s[3] << 8;
    s[5] = s[5].wrapping_add(s[2]);
    s[3] = s[3].wrapping_add(s[4]);
    s[3] ^= s[4] >> 16;
    s[6] = s[6].wrapping_add(s[3]);
    s[4] = s[4].wrapping_add(s[5]);
    s[4] ^= s[5] << 10;
    s[7] = s[7].wrapping_add(s[4]);
    s[5] = s[5].wrapping_add(s[6]);
    s[5] ^= s[6] >> 4;
    s[0] = s[0].wrapping_add(s[5]);
    s[6] = s[6].wrapping_add(s[7]);
    s[6] ^= s[7] << 8;
    s[1] = s[1].wrapping_add(s[6]);
    s[7] = s[7].wrapping_add(s[0]);
    s[7] ^= s[0] >> 9;
    s[2] = s[2].wrapping_add(s[7]);
    s[0] = s[0].wrapping_add(s[1]);
}
