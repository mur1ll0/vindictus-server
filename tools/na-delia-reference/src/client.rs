//! `C_HeroesPlayerDelia` from the NA x64 `client.dll` (vtable `0x1af6f10`,
//! constructor `0xaf1220`, factory `player_delia`, 0x8ae0 bytes).

use crate::action_event::{
    ActionEvent, ActionParam, DASH_TO_TARGET, DISABLE_RAGDOLL_LAUNCH, ENABLE_RAGDOLL_LAUNCH,
    END_SPECIAL_HOLD_SKILL, KIND_RAGDOLL_LAUNCH, KIND_STATE_COMMAND, KIND_STATE_END_COMMAND,
    SET_GUARD_LATCH_ATTEN, START_SPECIAL_HOLD_SKILL, command_is,
};
use crate::protected::ProtectedValue;

pub const CLASS_NAME: &str = "C_HeroesPlayerDelia";
pub const BODY_SHAPE_FIG: &str = "csf/bodyshape_delia_default.fig";
/// `GameStateManager001` state in which the body shape figure is always loaded.
pub const GAME_STATE_CHARACTER_CREATION: i32 = 0x25;
/// Activities that drive the `run_yaw`/`sprint_yaw` pose parameters.
pub const ACTIVITY_RUN: i32 = 0x465;
pub const ACTIVITY_SPRINT: i32 = 0x468;
pub const POSE_PARAMETERS: [&str; 2] = ["run_yaw", "sprint_yaw"];
/// Sign applied to the yaw change per pose parameter (`0x1af6d68`).
const POSE_DIRECTION: [f32; 2] = [1.0, -1.0];
/// Bit of `DamageInfo::flags` (`+0xbc`) that marks a guarded hit.
pub const DAMAGE_GUARDED: u32 = 2;
/// Seconds between two "skill failed" guides (`0xaf2500` compares with 1.0).
const SKILL_FAIL_GUIDE_INTERVAL: f32 = 1.0;

/// ConVars registered by the Delia translation unit, with their defaults.
#[derive(Debug, Clone, PartialEq)]
pub struct DeliaCvars {
    pub cl_delia_debug: i32,
    pub delia_guard_atten_reset_always: i32,
    pub delia_run_direct_stabilize_count: i32,
    pub plr_poseparam_run_yaw_return_speed: f32,
    pub plr_poseparam_run_yaw_speed: f32,
    pub plr_poseparam_sprint_yaw_return_speed: f32,
    pub plr_poseparam_sprint_yaw_speed: f32,
}

impl Default for DeliaCvars {
    fn default() -> Self {
        Self {
            cl_delia_debug: 0,
            delia_guard_atten_reset_always: 1,
            delia_run_direct_stabilize_count: 5,
            plr_poseparam_run_yaw_return_speed: 55.0,
            plr_poseparam_run_yaw_speed: 180.0,
            plr_poseparam_sprint_yaw_return_speed: 20.0,
            plr_poseparam_sprint_yaw_speed: 40.0,
        }
    }
}

/// Damage record passed to slot 416; only the fields Delia touches.
#[derive(Debug, Clone, PartialEq)]
pub struct DamageInfo {
    /// `+0x38/+0x40/+0x48`: protected damage amount.
    pub damage: ProtectedValue,
    /// `+0x78`, cleared on a guarded hit.
    pub field_78: i32,
    /// `+0xbc`.
    pub flags: u32,
    /// `+0xc1`, cleared on a guarded hit.
    pub field_c1: u8,
}

/// Engine and base-class calls that are not reconstructed here.
pub trait ClientHost {
    /// `0xacf5b0`: `C_HeroesPlayer` slot 419.
    fn base_handle_action_event(&mut self, event: &ActionEvent) -> bool;
    /// `0xada5a0`: `C_HeroesPlayer` slot 416.
    fn base_on_take_damage(&mut self, info: &DamageInfo) -> i32;
    /// `0x2683e0(this, id)`.
    fn has_condition(&self, id: u32) -> bool;
    /// `0x26b660(this, id)`.
    fn sub_26b660(&mut self, id: u32);
    /// `0x2696c0(info, this + 0x7748)`: the hit ignores the guard.
    fn sub_2696c0(&self, info: &DamageInfo) -> bool;
    /// `0x269370(info)`: the hit ignores the guard.
    fn sub_269370(&self, info: &DamageInfo) -> bool;
    /// Owner handle of a component resolves to a live entity.
    fn owner_alive(&self) -> bool;
    /// `0x3e4430`: the entity is the local player.
    fn is_local_player(&self) -> bool;
    /// `0x148b40` and `0x3b0b00` on the owner: the hold skill can start.
    fn can_start_special_hold(&self) -> bool;
    /// `0x3b65c0` on the owner.
    fn start_special_hold(&mut self);
    /// Slot 1 of the special-hold component vtable (`0xaf1dc0`).
    fn special_hold_notify(&mut self);
}

/// `+0x89b0`: ragdoll launch state, vtable `0x1af6ed0`, 0xf0 bytes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RagdollLaunch {
    /// `+0x08`, set on spawn (slot 29).
    pub owner: u32,
    /// `+0x0c`.
    pub enabled: bool,
    /// `+0x10`, from the second parameter of `ENABLE_RAGDOLL_LAUNCH`.
    pub power: i32,
}

/// Facts about the entity hit by a Delia attack (`0xaf3540`).
#[derive(Debug, Clone, PartialEq)]
pub struct LaunchTarget<'a> {
    /// `__RTDynamicCast` to the NPC type succeeded.
    pub is_npc: bool,
    /// NPC vtable slots 138 (`+0x450`) and 130 (`+0x410`).
    pub slot_138: bool,
    pub slot_130: bool,
    /// `0x3ae6c0(npc)` returned an object.
    pub has_3ae6c0: bool,
    /// `npc + 0x1bc` handle resolves to an entity.
    pub has_owner: bool,
    /// `0x1b2500(npc)`, NPC slots 153 (`+0x4c8`) and 360 (`+0xb40`), `0xa9baa0(npc)`.
    pub blocked: [bool; 4],
    /// NPC slot 16 (`+0x80`): class name.
    pub class_name: &'a str,
}

impl RagdollLaunch {
    /// `0x235150`: `ENABLE_RAGDOLL_LAUNCH <power>` / `DISABLE_RAGDOLL_LAUNCH`.
    pub fn handle_command(&mut self, params: &[ActionParam]) -> bool {
        if params.is_empty() {
            return true;
        }
        if params.len() == 2 && command_is(params, ENABLE_RAGDOLL_LAUNCH) {
            self.enabled = true;
            // cvttss2si: truncation toward zero.
            self.power = params[1].value as i32;
            return true;
        }
        if command_is(params, DISABLE_RAGDOLL_LAUNCH) {
            self.enabled = false;
        }
        true
    }

    /// `0xaf3540`: whether the hit marks `npc + 0x4ab0` for a ragdoll launch.
    /// `free_slot` is `0x2350d0(this)`, an index into the five tracked targets.
    pub fn should_launch(&self, free_slot: i32, target: Option<&LaunchTarget<'_>>) -> bool {
        let Some(target) = target else { return false };
        self.enabled
            && free_slot >= 0
            && target.is_npc
            && target.slot_138
            && target.slot_130
            && !target.has_3ae6c0
            && !target.has_owner
            && !target.blocked.iter().any(|blocked| *blocked)
            && !target.class_name.starts_with("c_npc_seiren")
    }
}

/// `+0x8aa0`: special hold skill state, vtable `0x1af6ef0`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SpecialHoldSkill {
    /// `+0x08`.
    pub owner: u32,
    /// `+0x14`.
    pub active: bool,
    /// `+0x15`.
    pub dash_to_target: bool,
    /// `+0x16`: notify through vtable slot 1 on the next start.
    pub pending_notify: bool,
}

impl SpecialHoldSkill {
    /// `0xaf2f50`.
    pub fn handle_event(&mut self, host: &mut impl ClientHost, event: &ActionEvent) -> bool {
        if !host.owner_alive() || event.params.is_empty() {
            return false;
        }
        match event.kind {
            KIND_STATE_COMMAND if event.is_command(START_SPECIAL_HOLD_SKILL) => {
                if host.is_local_player() && host.can_start_special_hold() {
                    host.start_special_hold();
                    self.active = true;
                    if self.pending_notify {
                        host.special_hold_notify();
                        self.pending_notify = false;
                    }
                }
                true
            }
            KIND_STATE_COMMAND if event.is_command(DASH_TO_TARGET) => {
                self.dash_to_target = true;
                true
            }
            KIND_STATE_END_COMMAND if event.is_command(END_SPECIAL_HOLD_SKILL) => {
                self.active = false;
                self.dash_to_target = false;
                true
            }
            _ => false,
        }
    }
}

/// Inputs of one `run_yaw`/`sprint_yaw` update (`0xaf1740`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct YawFrame {
    pub activity: i32,
    /// `0xac6f80(this)`.
    pub can_steer: bool,
    /// Absolute yaw in degrees (slot 10, `[angles + 4]`).
    pub yaw: f32,
    /// Range of the selected pose parameter (`0x3af310`).
    pub pose_min: f32,
    pub pose_max: f32,
    /// `gpGlobals->frametime`.
    pub frame_time: f32,
}

/// `+0x899c/+0x89a0/+0x89a4/+0x8ab8`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RunYawPose {
    pub last_yaw: f32,
    pub target: f32,
    pub pose: f32,
    pub stable_frames: i32,
}

/// Pose parameter writes produced by one update.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PoseWrite {
    /// Both parameters set to 0 outside run and sprint.
    ResetBoth,
    /// Parameter `POSE_PARAMETERS[index]` follows `RunYawPose::pose`.
    Set { index: usize, value: f32 },
}

/// Source `AngleNormalize(AngleDiff(a, b))`: difference folded into `[-180, 180)`.
fn angle_diff(a: f32, b: f32) -> f32 {
    (a - b + 180.0).rem_euclid(360.0) - 180.0
}

/// Moves `current` toward `goal` by `speed * frame_time` without passing it.
fn approach(goal: f32, current: f32, speed: f32, frame_time: f32) -> f32 {
    let delta = goal - current;
    let next = current + delta / (delta.abs() + f32::EPSILON) * frame_time * speed;
    let high = goal.max(current);
    if next > high {
        high
    } else {
        goal.min(current).max(next)
    }
}

impl RunYawPose {
    /// `0xaf1740`, called from slot 380 while the game is not paused.
    pub fn update(&mut self, cvars: &DeliaCvars, frame: &YawFrame) -> Option<PoseWrite> {
        let sprint = match frame.activity {
            ACTIVITY_RUN => false,
            ACTIVITY_SPRINT => true,
            _ => {
                self.target = 0.0;
                self.pose = 0.0;
                self.last_yaw = frame.yaw;
                return Some(PoseWrite::ResetBoth);
            }
        };
        if !frame.can_steer {
            return None;
        }
        let index = usize::from(sprint);
        let target = POSE_DIRECTION[index] * angle_diff(self.last_yaw, frame.yaw) + self.target;
        self.target = if target > frame.pose_max {
            frame.pose_max
        } else {
            frame.pose_min.max(target)
        };

        // Two `comiss`/`ja` tests: a NaN yaw counts as steady, as in the DLL.
        #[allow(clippy::neg_cmp_op_on_partial_ord)]
        let steady = !(self.last_yaw - 1.0 > frame.yaw) && !(frame.yaw > self.last_yaw + 1.0);
        self.stable_frames = if steady { self.stable_frames + 1 } else { 0 };
        if self.stable_frames > cvars.delia_run_direct_stabilize_count {
            let speed = if sprint {
                cvars.plr_poseparam_sprint_yaw_return_speed
            } else {
                cvars.plr_poseparam_run_yaw_return_speed
            };
            self.pose = approach(0.0, self.pose, speed, frame.frame_time);
            self.target = self.pose;
        } else {
            let speed = if sprint {
                cvars.plr_poseparam_sprint_yaw_speed
            } else {
                cvars.plr_poseparam_run_yaw_speed
            };
            self.pose = approach(self.target, self.pose, speed, frame.frame_time);
        }
        self.last_yaw = frame.yaw;
        Some(PoseWrite::Set {
            index,
            value: self.pose,
        })
    }
}

/// Why the special skill could not be used (slot 476).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillFailure {
    Location,
    SpGaugeLack,
    Transform,
    Generic,
}

impl SkillFailure {
    pub fn localization_key(self) -> &'static str {
        match self {
            Self::Location => "#GameUI_Heroes_delia_skill_fail_msg_location",
            Self::SpGaugeLack => "#GameUI_Heroes_SPGauge_Lack",
            Self::Transform => "#GameUI_Heroes_delia_skill_fail_msg_transform",
            Self::Generic => "#GameUI_Heroes_delia_skill_fail_msg",
        }
    }
}

/// Skill-manager answers read by slot 476 (`this + 0x7a28`, `this + 0x8398`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkillCheck {
    /// `0x234fb0(this)`.
    pub location_allowed: bool,
    /// `0x273150(skills, id, required)`.
    pub usable: bool,
    /// `0x273280(skills)`.
    pub current_sp: f32,
    /// `0x273350(skills, id)`.
    pub sp_cost: i32,
    /// `0xb18cf0(this + 0x8398)`.
    pub transformed: bool,
}

/// Effects of slot 476 before it calls the base (`0xae2f60`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillFailureFeedback {
    /// Shown through `ActionStateGuide`, at most once per second for all players.
    pub guide: Option<SkillFailure>,
    /// `Effect.Campfire.failed` on the local player.
    pub sound: bool,
}

#[derive(Debug, Clone)]
pub struct DeliaClient {
    pub cvars: DeliaCvars,
    /// Slots 487/489: guard state.
    pub guard: ProtectedValue,
    /// `+0x8ac8`: values of `SET_GUARD_LATCH_ATTEN`.
    pub guard_latch_atten: Vec<f32>,
    pub ragdoll_launch: RagdollLaunch,
    pub special_hold: SpecialHoldSkill,
    pub run_yaw: RunYawPose,
    /// `+0x89a8`.
    pub top_panel_health: i32,
    /// `+0x8a94`.
    pub special_skill_id: i32,
    /// Base field `+0x8591`, set by a guarded hit.
    pub guard_blocked_hit: bool,
    /// Static `0x22f25c4`: time of the last skill-failure guide.
    pub last_skill_fail_guide: f32,
}

impl DeliaClient {
    pub fn new(this_address: u64) -> Self {
        Self {
            cvars: DeliaCvars::default(),
            guard: ProtectedValue::new(this_address + 0x8980),
            guard_latch_atten: Vec::new(),
            ragdoll_launch: RagdollLaunch::default(),
            special_hold: SpecialHoldSkill::default(),
            run_yaw: RunYawPose::default(),
            top_panel_health: 0,
            special_skill_id: 0,
            guard_blocked_hit: false,
            last_skill_fail_guide: 0.0,
        }
    }

    /// Slots 13 and 16.
    pub fn class_name(&self) -> &'static str {
        CLASS_NAME
    }

    /// Slot 29 (`0xaf2380`): components learn the owner handle, then the base spawns.
    pub fn on_spawn(&mut self, handle: u32) {
        self.ragdoll_launch.owner = handle;
        self.special_hold.owner = handle;
    }

    /// Slots 190 and 191 (`0x235140`, `0x235130`); same values on the server (240, 241).
    pub fn slot_190(&self) -> f32 {
        0.88
    }

    pub fn slot_191(&self) -> f32 {
        0.96
    }

    /// Slot 349 (`0xaf1b00`); server slot 466.
    pub fn slot_349(&self) -> f32 {
        92.0
    }

    /// Slot 485 (`0xaf1b10`): `89 / slot_349()`; server slot 694.
    pub fn slot_485(&self) -> f32 {
        89.0 / self.slot_349()
    }

    /// Slot 301 (`0xaf1a60`): body shape figure, or none when `BodyShapeModify`
    /// is enabled outside character creation.
    pub fn body_shape_fig(&self, body_shape_modify: bool, game_state: i32) -> Option<&'static str> {
        if body_shape_modify && game_state != GAME_STATE_CHARACTER_CREATION {
            None
        } else {
            Some(BODY_SHAPE_FIG)
        }
    }

    /// Slot 487 (`0x235420`).
    pub fn set_guard_state(&mut self, state: u8) {
        self.guard.set(u32::from(state));
    }

    /// Slot 489 (`0x2353e0`).
    pub fn is_guarding(&self) -> bool {
        self.guard.get() == 1
    }

    /// Slot 330 (`0xaf1d30`); slot 331 forwards to it.
    pub fn can_guard(&self, host: &impl ClientHost, info: &DamageInfo) -> bool {
        self.is_guarding() && !host.sub_2696c0(info) && !host.sub_269370(info)
    }

    /// Slot 416 (`0xaf1ff0`): a guarded hit reaches the base with no damage.
    pub fn on_take_damage(&mut self, host: &mut impl ClientHost, info: &DamageInfo) -> i32 {
        let mut local = info.clone();
        if self.can_guard(host, info) {
            self.guard_blocked_hit = true;
            local.damage.set(0);
            local.flags |= DAMAGE_GUARDED;
            local.field_c1 = 0;
            local.field_78 = 0;
            if host.has_condition(0x4f) {
                host.sub_26b660(0x50);
            }
        }
        host.base_on_take_damage(&local)
    }

    /// Slot 470 (`0xaf2270`): the base runs only while not guarding.
    pub fn slot_470_runs_base(&self) -> bool {
        !self.is_guarding()
    }

    /// Slot 419 (`0xaf1b50`).
    pub fn handle_action_event(
        &mut self,
        host: &mut impl ClientHost,
        event: Option<&ActionEvent>,
    ) -> bool {
        let Some(event) = event else { return false };
        match event.kind {
            KIND_STATE_COMMAND | KIND_STATE_END_COMMAND => {
                if event.is_command(SET_GUARD_LATCH_ATTEN) {
                    if self.guard_latch_atten.is_empty() {
                        self.guard_latch_atten
                            .extend(event.params.iter().skip(1).map(|p| p.value));
                    }
                    return true;
                }
                if self.special_hold.handle_event(host, event) {
                    return true;
                }
                host.base_handle_action_event(event)
            }
            KIND_RAGDOLL_LAUNCH => self.ragdoll_launch.handle_command(&event.params),
            _ => host.base_handle_action_event(event),
        }
    }

    /// Slot 476 (`0xaf2410`) for the special skill; other skills go straight to the base.
    pub fn on_skill_use_failed(
        &mut self,
        skill_id: i32,
        blocked_by_condition_140: bool,
        check: &SkillCheck,
        local_player: bool,
        now: f32,
    ) -> SkillFailureFeedback {
        let silent = SkillFailureFeedback {
            guide: None,
            sound: false,
        };
        if skill_id != self.special_skill_id || blocked_by_condition_140 {
            return silent;
        }
        let failure = if !check.location_allowed {
            SkillFailure::Location
        } else if !check.usable {
            return silent;
        } else if check.sp_cost as f32 > check.current_sp {
            SkillFailure::SpGaugeLack
        } else if check.transformed {
            SkillFailure::Transform
        } else {
            SkillFailure::Generic
        };
        let guide = if now - self.last_skill_fail_guide > SKILL_FAIL_GUIDE_INTERVAL {
            self.last_skill_fail_guide = now;
            Some(failure)
        } else {
            None
        };
        SkillFailureFeedback {
            guide,
            sound: local_player,
        }
    }

    /// Slot 516 (`0xaf1de0`): health added to the top panel, capped at `max`,
    /// sent as `BaseUI.setTopPanelHealth(double)`.
    pub fn top_panel_health_delta(&self, delta: i32, max: i32) -> i32 {
        if self.top_panel_health + delta > max {
            max - self.top_panel_health
        } else {
            delta
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Host {
        base_events: usize,
        last_damage: Option<u32>,
        ignores_guard: bool,
        condition_4f: bool,
        cleared: Vec<u32>,
        notified: bool,
    }

    impl ClientHost for Host {
        fn base_handle_action_event(&mut self, _: &ActionEvent) -> bool {
            self.base_events += 1;
            false
        }
        fn base_on_take_damage(&mut self, info: &DamageInfo) -> i32 {
            self.last_damage = Some(info.damage.get());
            0
        }
        fn has_condition(&self, id: u32) -> bool {
            id == 0x4f && self.condition_4f
        }
        fn sub_26b660(&mut self, id: u32) {
            self.cleared.push(id);
        }
        fn sub_2696c0(&self, _: &DamageInfo) -> bool {
            self.ignores_guard
        }
        fn sub_269370(&self, _: &DamageInfo) -> bool {
            false
        }
        fn owner_alive(&self) -> bool {
            true
        }
        fn is_local_player(&self) -> bool {
            true
        }
        fn can_start_special_hold(&self) -> bool {
            true
        }
        fn start_special_hold(&mut self) {}
        fn special_hold_notify(&mut self) {
            self.notified = true;
        }
    }

    fn event(kind: i32, command: &str, numbers: &[f32]) -> ActionEvent {
        let mut params = vec![ActionParam::command(command)];
        params.extend(numbers.iter().map(|n| ActionParam::number(*n)));
        ActionEvent { kind, params }
    }

    fn damage(amount: u32) -> DamageInfo {
        let mut value = ProtectedValue::new(0x40);
        value.set(amount);
        DamageInfo {
            damage: value,
            field_78: 7,
            flags: 0,
            field_c1: 1,
        }
    }

    #[test]
    fn guard_latch_atten_is_latched_once() {
        let mut delia = DeliaClient::new(0x1000);
        let mut host = Host::default();
        delia.handle_action_event(
            &mut host,
            Some(&event(0x7a, SET_GUARD_LATCH_ATTEN, &[0.5, 0.25])),
        );
        delia.handle_action_event(&mut host, Some(&event(0x7b, SET_GUARD_LATCH_ATTEN, &[9.0])));
        assert_eq!(delia.guard_latch_atten, vec![0.5, 0.25]);
    }

    #[test]
    fn ragdoll_launch_takes_truncated_power() {
        let mut delia = DeliaClient::new(0x1000);
        let mut host = Host::default();
        delia.handle_action_event(
            &mut host,
            Some(&event(0x14a, ENABLE_RAGDOLL_LAUNCH, &[3.9])),
        );
        assert_eq!(
            (delia.ragdoll_launch.enabled, delia.ragdoll_launch.power),
            (true, 3)
        );
    }

    #[test]
    fn ragdoll_launch_skips_seiren() {
        let launch = RagdollLaunch {
            owner: 1,
            enabled: true,
            power: 1,
        };
        let target = LaunchTarget {
            is_npc: true,
            slot_138: true,
            slot_130: true,
            has_3ae6c0: false,
            has_owner: false,
            blocked: [false; 4],
            class_name: "c_npc_seiren_boss",
        };
        assert!(!launch.should_launch(0, Some(&target)));
    }

    #[test]
    fn special_hold_start_flushes_pending_notify() {
        let mut delia = DeliaClient::new(0x1000);
        delia.special_hold.pending_notify = true;
        let mut host = Host::default();
        delia.handle_action_event(&mut host, Some(&event(0x7a, START_SPECIAL_HOLD_SKILL, &[])));
        assert!(delia.special_hold.active && host.notified && !delia.special_hold.pending_notify);
    }

    #[test]
    fn unknown_command_falls_back_to_base() {
        let mut delia = DeliaClient::new(0x1000);
        let mut host = Host::default();
        delia.handle_action_event(&mut host, Some(&event(0x7a, "OTHER", &[])));
        assert_eq!(host.base_events, 1);
    }

    #[test]
    fn guarded_hit_reaches_base_with_zero_damage() {
        let mut delia = DeliaClient::new(0x1000);
        delia.set_guard_state(1);
        let mut host = Host {
            condition_4f: true,
            ..Host::default()
        };
        delia.on_take_damage(&mut host, &damage(120));
        assert_eq!(
            (host.last_damage, host.cleared.as_slice()),
            (Some(0), &[0x50][..])
        );
    }

    #[test]
    fn unblockable_hit_keeps_damage() {
        let mut delia = DeliaClient::new(0x1000);
        delia.set_guard_state(1);
        let mut host = Host {
            ignores_guard: true,
            ..Host::default()
        };
        delia.on_take_damage(&mut host, &damage(120));
        assert_eq!(host.last_damage, Some(120));
    }

    #[test]
    fn body_shape_is_skipped_outside_creation_when_modify_is_on() {
        let delia = DeliaClient::new(0x1000);
        assert_eq!(delia.body_shape_fig(true, 3), None);
    }

    #[test]
    fn body_shape_is_loaded_in_character_creation() {
        let delia = DeliaClient::new(0x1000);
        assert_eq!(
            delia.body_shape_fig(true, GAME_STATE_CHARACTER_CREATION),
            Some(BODY_SHAPE_FIG)
        );
    }

    #[test]
    fn slot_485_is_89_over_92() {
        assert_eq!(DeliaClient::new(0).slot_485(), 89.0 / 92.0);
    }

    #[test]
    fn sp_lack_guide_is_throttled_to_one_second() {
        let mut delia = DeliaClient::new(0x1000);
        delia.special_skill_id = 7;
        let check = SkillCheck {
            location_allowed: true,
            usable: true,
            current_sp: 10.0,
            sp_cost: 50,
            transformed: false,
        };
        let first = delia.on_skill_use_failed(7, false, &check, true, 5.0);
        let second = delia.on_skill_use_failed(7, false, &check, true, 5.5);
        assert_eq!(
            (first.guide, second.guide),
            (Some(SkillFailure::SpGaugeLack), None)
        );
    }

    #[test]
    fn top_panel_health_is_capped() {
        let mut delia = DeliaClient::new(0);
        delia.top_panel_health = 90;
        assert_eq!(delia.top_panel_health_delta(30, 100), 10);
    }

    #[test]
    fn run_yaw_follows_turn_and_clamps() {
        let mut pose = RunYawPose::default();
        let frame = YawFrame {
            activity: ACTIVITY_RUN,
            can_steer: true,
            yaw: -30.0,
            pose_min: -20.0,
            pose_max: 20.0,
            frame_time: 0.1,
        };
        pose.update(&DeliaCvars::default(), &frame);
        assert_eq!((pose.target, pose.pose), (20.0, 18.0));
    }

    #[test]
    fn run_yaw_returns_to_center_after_stable_frames() {
        let cvars = DeliaCvars::default();
        let mut pose = RunYawPose {
            last_yaw: 0.0,
            target: 10.0,
            pose: 10.0,
            stable_frames: 5,
        };
        let frame = YawFrame {
            activity: ACTIVITY_RUN,
            can_steer: true,
            yaw: 0.0,
            pose_min: -45.0,
            pose_max: 45.0,
            frame_time: 0.1,
        };
        pose.update(&cvars, &frame);
        assert_eq!((pose.target, pose.pose), (4.5, 4.5));
    }

    #[test]
    fn leaving_run_resets_pose() {
        let mut pose = RunYawPose {
            last_yaw: 3.0,
            target: 5.0,
            pose: 5.0,
            stable_frames: 2,
        };
        let frame = YawFrame {
            activity: 0,
            can_steer: true,
            yaw: 40.0,
            pose_min: 0.0,
            pose_max: 0.0,
            frame_time: 0.1,
        };
        assert_eq!(
            pose.update(&DeliaCvars::default(), &frame),
            Some(PoseWrite::ResetBoth)
        );
    }
}
