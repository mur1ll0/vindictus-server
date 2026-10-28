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

/// How well a NA x64 slot was paired with a 1.69 slot by
/// `scripts/reverse/na_x64_to_169_vtables.py`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confidence {
    /// Matching strings or constants in the overrides and the base.
    High,
    /// Same set of classes overriding the slot.
    Medium,
    /// Position between two anchors only.
    Low,
}

/// Delia slot in the NA x64 vtable and its 1.69 counterpart; `None` means the
/// method does not exist in the 1.69 base.
pub type PortSlot = (usize, Option<(usize, Confidence)>);

pub mod port {
    use super::Confidence::{High, Low, Medium};
    use super::PortSlot;

    /// `C_HeroesPlayer`: 538 x64 slots against 452 in 1.69.
    pub const CLIENT: [PortSlot; 26] = [
        (0, Some((0, Medium))),
        (13, Some((13, High))),
        (15, Some((15, Medium))),
        (16, Some((16, High))),
        (17, Some((17, Medium))),
        (29, Some((29, High))),
        (31, Some((31, High))),
        (83, Some((84, High))),
        (190, Some((177, High))),
        (191, Some((178, High))),
        (286, Some((249, Medium))),
        (301, Some((264, Medium))),
        (330, None),
        (331, None),
        (349, None),
        (351, Some((308, Medium))),
        (380, Some((334, High))),
        (416, Some((367, Medium))),
        (419, Some((368, High))),
        (455, Some((401, High))),
        (470, None),
        (476, Some((419, Medium))),
        (485, None),
        (487, Some((429, High))),
        (489, Some((430, High))),
        (516, None),
    ];

    /// `CHeroesPlayer`: 752 x64 slots against 646 in 1.69.
    pub const SERVER: [PortSlot; 37] = [
        (0, Some((0, Medium))),
        (9, Some((9, High))),
        (10, Some((10, Medium))),
        (11, Some((11, High))),
        (12, Some((12, Medium))),
        (13, Some((13, Medium))),
        (28, Some((26, High))),
        (29, Some((27, High))),
        (73, Some((70, Medium))),
        (78, Some((75, High))),
        (137, Some((125, High))),
        (240, Some((221, High))),
        (241, Some((222, High))),
        (334, Some((293, Medium))),
        (390, Some((346, Low))),
        (434, None),
        (435, None),
        (466, Some((414, High))),
        (469, Some((417, Medium))),
        (504, None),
        (517, Some((460, High))),
        (615, Some((555, High))),
        (624, Some((563, High))),
        (627, None),
        (628, None),
        (631, None),
        (657, Some((586, High))),
        (677, None),
        (694, None),
        (697, Some((618, High))),
        (699, Some((619, High))),
        (725, Some((638, High))),
        (729, None),
        (730, Some((641, Low))),
        (731, None),
        (736, None),
        (748, None),
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

/// Base methods of the NA x64 build that call a Delia method missing from 1.69:
/// `(NA-only slot, calling base slot in x64, same calling slot in 1.69)`.
/// These 1.69 slots are where the port has to reach the missing behaviour.
pub mod na_only_callers {
    pub const CLIENT: [(usize, usize, usize); 2] = [
        // Slot 470 (base call skipped while guarding) is called by base 469.
        (470, 469, 413),
        // Slot 485 (89 / slot 349) is called by base 484 (low-confidence pair).
        (485, 484, 427),
    ];

    pub const SERVER: [(usize, usize, usize); 6] = [
        (677, 453, 400),
        (694, 693, 614),
        (729, 624, 563),
        (731, 624, 563),
        (729, 730, 641),
        (731, 730, 641),
    ];
}
