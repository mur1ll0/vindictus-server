//! Action-state conditions: named flags that native code raises and the
//! action-state scripts test (`As_Cond_*`).
//!
//! Each DLL registers `name → id` at start-up (NA x64 client `0x1a5820`,
//! server `0x1a26a0`: 231 names). The ids are positions in that list, so they
//! change between builds: the 1.69 list ends at 120 and has no Delia entry.

/// Ids in the NA x64 build.
pub mod na {
    /// Bound of the NA set/clear functions (`cmp edx, 0xe7`; eight dwords).
    pub const LIMIT: u32 = 0xe7;
    pub const CAN_PERFECT_ENDURABLE: u32 = 79;
    pub const DO_PERFECT_ENDURABLE: u32 = 80;
    pub const CAN_ARMOR_PURGE: u32 = 140;
    pub const DELIA_SP_SKILL3_ACTIVATED: u32 = 141;
    pub const DELIA_SPECIAL_HOLD_SKILL: u32 = 142;
    pub const DELIA_GUARD: u32 = 143;
    pub const DELIA_GUARD_HIT: u32 = 144;
}

/// Ids in the 1.69 build (client `b284ba71…`, server `d7727e18…`).
pub mod v169 {
    pub const CAN_PERFECT_ENDURABLE: u32 = 76;
    pub const DO_PERFECT_ENDURABLE: u32 = 77;
    /// Highest id registered by the 1.69 DLLs (`As_Cond_Maintain_Constant_MotionSpeed`).
    pub const LAST: u32 = 120;
    /// Bound checked by the four set/clear functions of each 1.69 DLL
    /// (`cmp ecx, 0x79`: client `0x7de3fb`, `0x7de44b`, `0x7de55b`, `0x7de5bb`;
    /// server `0x7e63cb`, `0x7e641b`, `0x7e647b`, `0x7e64db`). A 121-bit
    /// Source `CBitVec` takes four dwords, so ids up to 127 should fit without
    /// moving any field (inferred from the bound, not from the object layout).
    pub const LIMIT: u32 = 0x79;
    pub const STORAGE_BITS: u32 = 128;
}

/// Script names of the conditions that only Delia raises, in NA id order.
pub const DELIA_CONDITIONS: [(&str, u32); 5] = [
    ("As_Cond_Can_Armor_Purge", na::CAN_ARMOR_PURGE),
    (
        "As_Cond_Delia_SPSkill3_Activated",
        na::DELIA_SP_SKILL3_ACTIVATED,
    ),
    (
        "AS_Cond_Delia_Special_Hold_Skill",
        na::DELIA_SPECIAL_HOLD_SKILL,
    ),
    ("AS_Cond_Delia_Guard", na::DELIA_GUARD),
    ("AS_Cond_Delia_Guard_Hit", na::DELIA_GUARD_HIT),
];

/// Ids proposed for the Delia conditions in the 1.69 registries: the next free
/// positions after [`v169::LAST`], all inside the existing 128-bit storage.
pub const PROPOSED_V169: [(&str, u32); 5] = [
    (DELIA_CONDITIONS[0].0, v169::LAST + 1),
    (DELIA_CONDITIONS[1].0, v169::LAST + 2),
    (DELIA_CONDITIONS[2].0, v169::LAST + 3),
    (DELIA_CONDITIONS[3].0, v169::LAST + 4),
    (DELIA_CONDITIONS[4].0, v169::LAST + 5),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proposed_ids_fit_the_1_69_storage() {
        assert!(PROPOSED_V169.iter().all(|(_, id)| *id < v169::STORAGE_BITS));
    }
}
