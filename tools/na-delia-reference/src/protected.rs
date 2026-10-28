//! Obfuscated integer used for gameplay state that cheats would target.
//!
//! The entity keeps a heap cell, a 64-bit seed and a one-byte key
//! (client `+0x8980/+0x8988/+0x8990`, server `+0xad28/+0xad30/+0xad38`). Every
//! write advances the MSVC `rand()` LCG, derives a key in `1..=255` and stores
//! `value ^ (key * 0x01010101)`. Read: client `0x2353e0`, server `0x266fd0`.
//! Write: client `0x235420`, server `0x267010`.

const LCG_MULTIPLIER: u64 = 0x343fd;
const LCG_INCREMENT: u64 = 0x269ec3;
const KEY_SPREAD: u32 = 0x0101_0101;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtectedValue {
    cell: u32,
    seed: u64,
    key: u8,
}

impl ProtectedValue {
    /// Constructor path: the seed starts as the address of the member
    /// (client `0xaf12a1`: `imul rcx, rbx, 0x343fd` with `rbx = this + 0x8980`)
    /// and the initial value is 0.
    pub fn new(member_address: u64) -> Self {
        let mut value = Self {
            cell: 0,
            seed: member_address,
            key: 0,
        };
        value.set(0);
        value
    }

    fn next_key(&mut self) -> u8 {
        self.seed = self
            .seed
            .wrapping_mul(LCG_MULTIPLIER)
            .wrapping_add(LCG_INCREMENT);
        let random = (self.seed >> 16) & 0x7fff;
        // random % 255 is at most 254, so the key always fits in a byte.
        (random % 255) as u8 + 1
    }

    pub fn set(&mut self, value: u32) {
        self.key = self.next_key();
        self.cell = value ^ (u32::from(self.key) * KEY_SPREAD);
    }

    pub fn get(&self) -> u32 {
        self.cell ^ (u32::from(self.key) * KEY_SPREAD)
    }

    /// Stored cell and key, as another process would see them in memory.
    pub fn raw(&self) -> (u32, u8) {
        (self.cell, self.key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_then_get_returns_value() {
        let mut value = ProtectedValue::new(0x1_0000_8980);
        value.set(1);
        assert_eq!(value.get(), 1);
    }

    #[test]
    fn key_follows_msvc_rand_sequence() {
        // srand(1): rand() yields 41, 18467, 6334 → keys 41 % 255 + 1, 18467 % 255 + 1, …
        let mut value = ProtectedValue {
            cell: 0,
            seed: 1,
            key: 0,
        };
        let keys: Vec<u8> = (0..3)
            .map(|_| {
                value.set(0);
                value.raw().1
            })
            .collect();
        assert_eq!(keys, vec![42, 108, 215]);
    }

    #[test]
    fn stored_cell_is_value_xor_spread_key() {
        let mut value = ProtectedValue {
            cell: 0,
            seed: 1,
            key: 0,
        };
        value.set(1);
        assert_eq!(value.raw(), (1 ^ 0x2a2a_2a2a, 42));
    }
}
