//! `C_HeroesPlayerDelia` from the NA x64 `client.dll` (vtable `0x1af6f10`,
//! constructor `0xaf1220`, factory `player_delia`, 0x8ae0 bytes).

use crate::action_event::{
    ActionEvent, ActionParam, DASH_TO_TARGET, DISABLE_RAGDOLL_LAUNCH, ENABLE_RAGDOLL_LAUNCH,
    END_SPECIAL_HOLD_SKILL, KIND_RAGDOLL_LAUNCH, KIND_STATE_COMMAND, KIND_STATE_END_COMMAND,
    SET_GUARD_LATCH_ATTEN, START_SPECIAL_HOLD_SKILL, command_is,
};
use crate::condition::na;
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
    /// `0x2683e0(this, id)`: action-state condition `id` is raised.
    fn has_condition(&self, id: u32) -> bool;
    /// `0x26b660(this, id)`.
    fn add_condition(&mut self, id: u32);
    /// `0x2646b0(this, id)`.
    fn remove_condition(&mut self, id: u32);
    /// `0x2696c0(info, this + 0x7748)`: the hit ignores the guard.
    fn sub_2696c0(&self, info: &DamageInfo) -> bool;
    /// `0x269370(info)`: the hit ignores the guard.
    fn sub_269370(&self, info: &DamageInfo) -> bool;
    /// Owner handle of a component resolves to a live entity.
    fn owner_alive(&self) -> bool;
    /// `0x3e4430`: the entity is the local player.
    fn is_local_player(&self) -> bool;
    /// `0x3b0b00(owner, layer 0x1f9eea0)` (with `0x148b40` resolving the model):
    /// the "hold in range" gesture is playing.
    fn hold_gesture_playing(&self) -> bool;
    /// `0x3b65c0(owner, layer 0x1f9eea0)`.
    fn stop_hold_gesture(&mut self);
    /// `0x3a40e0(owner, layer 0x1f9eea0, sequence 0x1f9eea8, true, -1, …)`.
    fn play_hold_gesture(&mut self);
    /// Static flag `0x1f9eeb0` that allows the gesture.
    fn hold_gesture_enabled(&self) -> bool;
    /// `0x3b39b0(owner, "battle_idle_lower") > -1`.
    fn has_battle_idle_lower(&self) -> bool;
    /// `0x19c340(owner + 0x7748, "BattleIdle", 0)`.
    fn set_battle_idle(&mut self);
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
    pub slots: [LaunchSlot; RAGDOLL_TARGETS],
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

    /// `0x2350d0`: first slot whose target handle does not resolve, or -1.
    pub fn free_slot(&self, resolves: impl Fn(u32) -> bool) -> i32 {
        self.slots
            .iter()
            .position(|slot| slot.target == INVALID_HANDLE || !resolves(slot.target))
            .map_or(-1, |index| index as i32)
    }

    /// Body of the `0xaf39f0` loop (slot 83) when slot `index` sees a new target:
    /// the release time resets, the target is remembered and its launch time starts.
    pub fn update_slot(
        &mut self,
        index: usize,
        target_alive: bool,
        now: f32,
    ) -> Option<LaunchChange> {
        let slot = self.slots.get_mut(index)?;
        if slot.target == slot.previous {
            return None;
        }
        slot.release_requested_at = -1.0;
        slot.previous = if target_alive {
            slot.target
        } else {
            INVALID_HANDLE
        };
        slot.launched_at = now;
        Some(if target_alive {
            LaunchChange::Started
        } else {
            LaunchChange::Cleared
        })
    }

    /// `0xaf2a90`: half a second after the launch, the launching local player asks
    /// the server to release the ragdoll once a hull trace below it is clear.
    /// Returns true when the request is sent; the slot then waits for the server.
    pub fn take_release_request(
        &mut self,
        index: usize,
        owner_is_local: bool,
        ground_clear: bool,
        now: f32,
    ) -> bool {
        let Some(slot) = self.slots.get_mut(index) else {
            return false;
        };
        let due = slot.launched_at != -1.0
            && f64::from(slot.launched_at) + RAGDOLL_RELEASE_DELAY <= f64::from(now);
        if !due || !owner_is_local || slot.release_requested_at > -1.0 || !ground_clear {
            return false;
        }
        slot.release_requested_at = now;
        slot.launched_at = -1.0;
        true
    }

    /// `0xaf3be0`: before a release is requested, the client ragdoll moves to the
    /// networked position (keeping its own angles, physics slot 37) when the jump
    /// is finite and at most 1024 units.
    pub fn should_snap(
        &self,
        index: usize,
        ragdoll_position: [f32; 3],
        ragdoll_angles: [f32; 3],
    ) -> bool {
        let Some(slot) = self.slots.get(index) else {
            return false;
        };
        if slot.release_requested_at > -1.0 {
            return false;
        }
        let distance = ragdoll_position
            .iter()
            .zip(slot.position)
            .map(|(a, b)| (a - b) * (a - b))
            .sum::<f32>()
            .sqrt();
        distance.is_finite()
            && (0.0..=RAGDOLL_SNAP_LIMIT).contains(&distance)
            && slot
                .position
                .iter()
                .chain(&ragdoll_angles)
                .all(|v| v.is_finite())
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
                // `active` is only set when the in-range gesture was showing.
                if host.is_local_player() && host.hold_gesture_playing() {
                    host.stop_hold_gesture();
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

    /// `0xaf3670`, from slot 380: leaves the dash, then raises
    /// `AS_Cond_Delia_Special_Hold_Skill` while a target is in reach.
    pub fn think(&mut self, host: &mut impl ClientHost, reach: Option<&HoldReach>) {
        if !host.owner_alive() {
            return;
        }
        if self.dash_to_target && self.pending_notify && host.has_battle_idle_lower() {
            host.set_battle_idle();
            host.special_hold_notify();
            self.pending_notify = false;
            self.dash_to_target = false;
        }
        let in_reach = match reach {
            Some(reach) if !self.active => reach.in_reach(),
            _ => {
                host.remove_condition(na::DELIA_SPECIAL_HOLD_SKILL);
                return;
            }
        };
        if in_reach {
            host.add_condition(na::DELIA_SPECIAL_HOLD_SKILL);
            if host.is_local_player() && !host.hold_gesture_playing() && host.hold_gesture_enabled()
            {
                host.stop_hold_gesture();
                host.play_hold_gesture();
            }
        } else {
            host.remove_condition(na::DELIA_SPECIAL_HOLD_SKILL);
            if host.is_local_player() && host.hold_gesture_playing() {
                host.stop_hold_gesture();
            }
        }
    }
}

/// Squared reach added to the target's size (`0x1af8598`: 450²).
pub const SPECIAL_HOLD_REACH_SQ: f32 = 202_500.0;

/// Owner and target (`+0x0c` handle) geometry for [`SpecialHoldSkill::think`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HoldReach {
    /// Squared distance between the two origins (slot 9, `+0x48`).
    pub distance_sq: f32,
    /// Target collision bounds (`ICollideable` slot 20): squared X/Y extent.
    pub target_extent_xy_sq: f32,
}

impl HoldReach {
    pub fn in_reach(&self) -> bool {
        self.target_extent_xy_sq + SPECIAL_HOLD_REACH_SQ >= self.distance_sq
    }
}

/// Networked Delia fields read by `OnDataChanged` (secondary vtable `+0x10`, slot 5).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DeliaNetworkState {
    /// `+0x8a90`, from server `+0xaea0`.
    pub armor_purge_available: bool,
    /// `+0x8a98`, from server `+0xaeac`.
    pub sp_skill3_active: bool,
    /// `+0x8abc`: guarded hits counted by the server.
    pub guard_hit_count: i32,
}

/// Slots tracked by the ragdoll launch state (`0x2350d0` loops to 5).
pub const RAGDOLL_TARGETS: usize = 5;
/// Seconds a launched target flies before the client asks to release it
/// (`0x192d1d8`, a double).
pub const RAGDOLL_RELEASE_DELAY: f64 = 0.5;
/// Largest jump the client accepts when snapping a ragdoll to the server pose
/// (`0x19525e4`).
pub const RAGDOLL_SNAP_LIMIT: f32 = 1024.0;
/// Server command sent by `0xaf2a90` through `IVEngineClient` slot 101.
pub const RELEASE_COMMAND: &str = "request_release_launch_ragdoll";

/// One of the five tracked launches (`+0x14`, `+0x28`, `+0x3c`, `+0x50`,
/// `+0x8c` and `+0xc8` arrays of the ragdoll launch state).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LaunchSlot {
    /// Networked target handle.
    pub target: u32,
    /// Time the release was requested, or -1.
    pub release_requested_at: f32,
    /// Time the target was seen launched, or -1.
    pub launched_at: f32,
    /// Networked server pose of the ragdoll.
    pub position: [f32; 3],
    pub angles: [f32; 3],
    /// Target handle seen on the previous update.
    pub previous: u32,
}

impl Default for LaunchSlot {
    fn default() -> Self {
        Self {
            target: INVALID_HANDLE,
            release_requested_at: -1.0,
            launched_at: -1.0,
            position: [0.0; 3],
            angles: [0.0; 3],
            previous: INVALID_HANDLE,
        }
    }
}

pub const INVALID_HANDLE: u32 = u32::MAX;

/// What [`RagdollLaunch::update_slot`] did for a slot whose target changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchChange {
    /// `0x235200`: a new target started flying.
    Started,
    /// The previous NPC lost its launch mark (`npc + 0x4ab0 = 0`).
    Cleared,
}

/// Text of the release request (`0xaf2d76`: `"%s %d %d %f %f %f %f %f %f"`).
pub fn release_command(
    slot: usize,
    entity_index: i32,
    origin: [f32; 3],
    angles: [f32; 3],
) -> String {
    format!(
        "{RELEASE_COMMAND} {slot} {entity_index} {:.6} {:.6} {:.6} {:.6} {:.6} {:.6}",
        origin[0], origin[1], origin[2], angles[0], angles[1], angles[2]
    )
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
    pub network: DeliaNetworkState,
    /// `+0x8ac0`: last `guard_hit_count` turned into conditions.
    pub guard_hit_seen: i32,
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
            network: DeliaNetworkState::default(),
            guard_hit_seen: 0,
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

    /// Secondary vtable `+0x10` slot 5 (`0xaf1f40`, `OnDataChanged`): after the
    /// base, networked fields become action-state conditions. A new guarded hit
    /// raises both guard conditions; the scripts lower them.
    pub fn on_data_changed(&mut self, host: &mut impl ClientHost) {
        let toggles = [
            (na::CAN_ARMOR_PURGE, self.network.armor_purge_available),
            (na::DELIA_SP_SKILL3_ACTIVATED, self.network.sp_skill3_active),
        ];
        for (condition, raised) in toggles {
            if raised {
                host.add_condition(condition);
            } else {
                host.remove_condition(condition);
            }
        }
        let count = self.network.guard_hit_count;
        if count > 0 && count != self.guard_hit_seen {
            host.add_condition(na::DELIA_GUARD);
            host.add_condition(na::DELIA_GUARD_HIT);
        }
        self.guard_hit_seen = count;
        // Then 0xaf3380 on the special hold state (not read).
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
            if host.has_condition(na::CAN_PERFECT_ENDURABLE) {
                host.add_condition(na::DO_PERFECT_ENDURABLE);
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
        added: Vec<u32>,
        removed: Vec<u32>,
        notified: bool,
        gesture_playing: bool,
        gesture_started: bool,
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
        fn add_condition(&mut self, id: u32) {
            self.added.push(id);
        }
        fn remove_condition(&mut self, id: u32) {
            self.removed.push(id);
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
        fn hold_gesture_playing(&self) -> bool {
            self.gesture_playing
        }
        fn stop_hold_gesture(&mut self) {
            self.gesture_playing = false;
        }
        fn play_hold_gesture(&mut self) {
            self.gesture_playing = true;
            self.gesture_started = true;
        }
        fn hold_gesture_enabled(&self) -> bool {
            true
        }
        fn has_battle_idle_lower(&self) -> bool {
            true
        }
        fn set_battle_idle(&mut self) {}
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
            slots: Default::default(),
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
        let mut host = Host {
            gesture_playing: true,
            ..Host::default()
        };
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
            (host.last_damage, host.added.as_slice()),
            (Some(0), &[na::DO_PERFECT_ENDURABLE][..])
        );
    }

    #[test]
    fn new_guard_hit_raises_both_guard_conditions() {
        let mut delia = DeliaClient::new(0x1000);
        delia.network.guard_hit_count = 3;
        let mut host = Host::default();
        delia.on_data_changed(&mut host);
        delia.on_data_changed(&mut host);
        assert_eq!(host.added, vec![na::DELIA_GUARD, na::DELIA_GUARD_HIT]);
    }

    #[test]
    fn armor_purge_flag_follows_network() {
        let mut delia = DeliaClient::new(0x1000);
        delia.network.armor_purge_available = true;
        let mut host = Host::default();
        delia.on_data_changed(&mut host);
        assert_eq!(
            (host.added.as_slice(), host.removed.as_slice()),
            (
                &[na::CAN_ARMOR_PURGE][..],
                &[na::DELIA_SP_SKILL3_ACTIVATED][..]
            )
        );
    }

    #[test]
    fn target_in_reach_raises_hold_condition_and_gesture() {
        let mut hold = SpecialHoldSkill::default();
        let mut host = Host::default();
        let reach = HoldReach {
            distance_sq: 400.0 * 400.0,
            target_extent_xy_sq: 0.0,
        };
        hold.think(&mut host, Some(&reach));
        assert!(host.added == [na::DELIA_SPECIAL_HOLD_SKILL] && host.gesture_started);
    }

    #[test]
    fn hold_start_needs_the_reach_gesture() {
        let mut delia = DeliaClient::new(0x1000);
        let mut host = Host::default();
        delia.handle_action_event(&mut host, Some(&event(0x7a, START_SPECIAL_HOLD_SKILL, &[])));
        assert!(!delia.special_hold.active);
    }

    #[test]
    fn release_is_requested_half_a_second_after_launch() {
        let mut launch = RagdollLaunch::default();
        launch.slots[2].target = 77;
        launch.update_slot(2, true, 10.0);
        let early = launch.take_release_request(2, true, true, 10.4);
        let due = launch.take_release_request(2, true, true, 10.5);
        assert_eq!(
            (early, due, launch.slots[2].launched_at),
            (false, true, -1.0)
        );
    }

    #[test]
    fn release_command_matches_printf_format() {
        let text = release_command(1, 42, [1.0, 2.5, -3.0], [0.0, 90.0, 0.0]);
        assert_eq!(
            text,
            "request_release_launch_ragdoll 1 42 1.000000 2.500000 -3.000000 0.000000 90.000000 0.000000"
        );
    }

    #[test]
    fn snap_rejects_far_jump() {
        let mut launch = RagdollLaunch::default();
        launch.slots[0].position = [0.0, 0.0, 0.0];
        assert!(!launch.should_snap(0, [2000.0, 0.0, 0.0], [0.0; 3]));
    }

    #[test]
    fn free_slot_skips_live_targets() {
        let mut launch = RagdollLaunch::default();
        launch.slots[0].target = 5;
        assert_eq!(launch.free_slot(|handle| handle == 5), 1);
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
