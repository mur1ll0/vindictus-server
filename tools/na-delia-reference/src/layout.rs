//! Object layout and vtable slots of the Delia entity in the NA x64 build.
//!
//! The 1.69 client is 32-bit and its player base is smaller (452 client slots
//! against 538 here), so these numbers are only valid for the NA x64 DLLs. They
//! are the reference for mapping each override onto the 1.69 base through the
//! Hagie entity, which exists in both versions.

pub mod client {
    pub const OBJECT_SIZE: usize = 0x8ae0;
    pub const VTABLE_SLOTS: usize = 538;
    pub const VTABLE: u32 = 0x1af6f10;
    pub const BASE_VTABLE: u32 = 0x1af1340;
    pub const CONSTRUCTOR: u32 = 0xaf1220;
    pub const BASE_CONSTRUCTOR: u32 = 0xabeb50;

    /// Fields added by `C_HeroesPlayerDelia` (offsets from `this`).
    pub const GUARD_CELL: usize = 0x8980;
    pub const GUARD_SEED: usize = 0x8988;
    pub const GUARD_KEY: usize = 0x8990;
    pub const RUN_YAW_LAST_YAW: usize = 0x899c;
    pub const RUN_YAW_TARGET: usize = 0x89a0;
    pub const RUN_YAW_POSE: usize = 0x89a4;
    pub const TOP_PANEL_HEALTH: usize = 0x89a8;
    pub const RAGDOLL_LAUNCH: usize = 0x89b0;
    pub const SPECIAL_SKILL_ID: usize = 0x8a94;
    pub const SPECIAL_HOLD_SKILL: usize = 0x8aa0;
    pub const RUN_YAW_STABLE_FRAMES: usize = 0x8ab8;
    pub const GUARD_LATCH_ATTEN: usize = 0x8ac8;

    /// Slots that `C_HeroesPlayerDelia` overrides against `C_HeroesPlayer`.
    /// The NA 32-bit build also overrides 477 and 534; here both point to the base.
    pub const OVERRIDES: [usize; 26] = [
        0, 13, 15, 16, 17, 29, 31, 83, 190, 191, 286, 301, 330, 331, 349, 351, 380, 416, 419, 455,
        470, 476, 485, 487, 489, 516,
    ];
}

pub mod server {
    pub const OBJECT_SIZE: usize = 0xaf98;
    pub const VTABLE_SLOTS: usize = 752;
    pub const VTABLE: u32 = 0x10f17b0;
    pub const BASE_VTABLE: u32 = 0x10e89e8;
    pub const CONSTRUCTOR: u32 = 0x6e3ae0;
    pub const BASE_CONSTRUCTOR: u32 = 0x692630;

    pub const WEAPON_STATE: usize = 0xa6c4;
    pub const CURRENT_ACTION_NAME: usize = 0xa828;
    pub const GUARD_CELL: usize = 0xad28;
    pub const GUARD_SEED: usize = 0xad30;
    pub const GUARD_KEY: usize = 0xad38;
    pub const STRONG_ATTACK_SKILL_ID: usize = 0xaeb0;
    pub const PENDING_GRAB_TARGETS: usize = 0xaf70;
    pub const ABILITY_SLOTS_RESET: usize = 0xaf48;
    pub const GRAB_TARGET_CHANGED: usize = 0xaf8d;
    pub const STRONG_ATTACK_DELAY_ACTIVE: usize = 0xaf8e;
    pub const STRONG_ATTACK_DELAY_END: usize = 0xaf90;

    /// Slots that `CHeroesPlayerDelia` overrides against `CHeroesPlayer`.
    pub const OVERRIDES: [usize; 37] = [
        0, 9, 10, 11, 12, 13, 28, 29, 73, 78, 137, 240, 241, 334, 390, 434, 435, 466, 469, 504,
        517, 615, 624, 627, 628, 631, 657, 677, 694, 697, 699, 725, 729, 730, 731, 736, 748,
    ];
}
