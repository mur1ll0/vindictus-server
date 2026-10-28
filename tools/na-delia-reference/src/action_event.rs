//! Commands that action-state scripts send to the player entity.
//!
//! In memory an event is `{ i32 kind; std::vector<Param>* params }` and each
//! parameter is 0x28 bytes: a `std::string` (0x20) followed by an `f32` at
//! `+0x20`. The first parameter names the command; the others carry numbers.
//! Commands are matched with `strncmp(name, literal, strlen(literal))`, so a
//! name only has to start with the literal.

/// Kind 0x7a: command sent while a state runs (starts the special hold skill).
pub const KIND_STATE_COMMAND: i32 = 0x7a;
/// Kind 0x7b: command sent when a state ends (ends the special hold skill).
pub const KIND_STATE_END_COMMAND: i32 = 0x7b;
/// Kind 0x14a: ragdoll launch switch (client `0xaf1b8c`: `cmp ecx, 0xcf` after `sub 0x7b`).
pub const KIND_RAGDOLL_LAUNCH: i32 = 0x14a;

pub const SET_GUARD_LATCH_ATTEN: &str = "SET_GUARD_LATCH_ATTEN";
pub const ENABLE_RAGDOLL_LAUNCH: &str = "ENABLE_RAGDOLL_LAUNCH";
pub const DISABLE_RAGDOLL_LAUNCH: &str = "DISABLE_RAGDOLL_LAUNCH";
pub const START_SPECIAL_HOLD_SKILL: &str = "START_SPECIAL_HOLD_SKILL";
pub const END_SPECIAL_HOLD_SKILL: &str = "END_SPECIAL_HOLD_SKILL";
pub const DASH_TO_TARGET: &str = "DASH_TO_TARGET";

#[derive(Debug, Clone, PartialEq)]
pub struct ActionParam {
    pub name: String,
    pub value: f32,
}

impl ActionParam {
    pub fn command(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            value: 0.0,
        }
    }

    pub fn number(value: f32) -> Self {
        Self {
            name: String::new(),
            value,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActionEvent {
    pub kind: i32,
    pub params: Vec<ActionParam>,
}

impl ActionEvent {
    /// True when the first parameter starts with `literal`.
    pub fn is_command(&self, literal: &str) -> bool {
        command_is(&self.params, literal)
    }
}

pub(crate) fn command_is(params: &[ActionParam], literal: &str) -> bool {
    params
        .first()
        .is_some_and(|param| param.name.starts_with(literal))
}
