//! `CHeroesPlayerDelia` from the NA x64 `server.dll`, the local battle
//! simulation (vtable `0x10f17b0`, constructor `0x6e3ae0`, factory
//! `player_delia`, 0xaf98 bytes).

use crate::condition::na;
use crate::protected::ProtectedValue;

pub const CLASS_NAME: &str = "CHeroesPlayerDelia";
pub const WEAPON_CLASS: &str = "weapon_bastardsword";
pub const STRONG_ATTACK_DELAY_BUFF: &str = "skill_delia_strong_attack_delay";
/// Grace period after a strong attack lands (`0xf5c1d4`).
pub const STRONG_ATTACK_DELAY: f32 = 1.5;
/// Push applied to the hit NPC (`0xf503a4`).
pub const STRONG_ATTACK_PUSH: f32 = 10.0;
/// Bits of the server damage record.
pub const DAMAGE_GUARDED: u32 = 2;
pub const DAMAGE_FLAG_IGNORE_GUARD: u32 = 0x0800_0000;
pub const DAMAGE_FLAG_GUARD_DECIDED: u32 = 0x1000_0000;

/// Server damage record: the two flag words Delia reads.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DamageFlags {
    /// `+0x6c`.
    pub kind: u32,
    /// `+0xbc`.
    pub result: u32,
}

/// What the hit NPC answers when a strong attack lands (slot 730).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StrongAttackTarget {
    pub handle: u32,
    /// Protected HP at `npc + 0xe8/+0xf8`.
    pub health: i32,
    /// `0x467290(npc, info)`.
    pub immune: bool,
    /// `0x6e9920(npc, this, …)`.
    pub launchable: bool,
    /// `0x6e4380(this, npc)` and NPC slot 936 (`+0x1d40`).
    pub knockable: bool,
    /// `0x6eb590(npc)`.
    pub queueable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrongAttackOutcome {
    Ignored,
    /// `0x5a6020` + `0x5aed90(npc, 10.0)`, then the grab and the delay buff.
    Launched,
    /// `0x5994a0(npc, this, 10.0, …)` + `0x5acda0` + `0x6eaa30`, then the grab and the delay buff.
    KnockedDown,
    /// Handle stored in `+0xaf70` for slot 731 to resolve later.
    Queued,
}

/// Inputs of slot 730 that come from the base class.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StrongAttackContext<'a> {
    /// `0x6aafb0(this)`.
    pub attacking: bool,
    /// Skill level of `+0xaeb0` (`0x6a2cf0`).
    pub skill_level: i32,
    /// Slot 247 on the result of slot 245 for the delay buff.
    pub has_delay_buff: bool,
    /// `+0xa828`: current action name.
    pub current_action: &'a str,
    /// `info + 0x90`: name of the attack that hit.
    pub attack_name: &'a str,
    /// `0x6a1b80(this)`: an entity is already held.
    pub holding: bool,
    /// `gpGlobals->curtime`.
    pub now: f32,
}

#[derive(Debug, Clone)]
pub struct DeliaServer {
    /// Slots 697/699: guard state.
    pub guard: ProtectedValue,
    /// `+0xa6c4`: 1 while `weapon_bastardsword` is equipped.
    pub weapon_state: i32,
    /// `+0xad20`.
    pub field_ad20: bool,
    /// `+0xaf48`: ability slots 4..9 were reset (slots 137, 748).
    pub ability_slots_reset: bool,
    /// `+0xaf50`, set by slot 628.
    pub field_af50: bool,
    /// `+0xaf70`: strong-attack targets waiting for slot 731.
    pub pending_targets: Vec<u32>,
    /// `+0xaf8d`: the held entity changed (slot 729).
    pub grab_target_changed: bool,
    /// `+0xaf8e/+0xaf90`.
    pub strong_attack_delay_active: bool,
    pub strong_attack_delay_end: f32,
    /// Entity held through slot 729 (`+0xa818` handle).
    pub held: Option<u32>,
    /// `+0xaea0`, networked to client `+0x8a90`.
    pub armor_purge_available: bool,
    /// `+0xaeac`, networked to client `+0x8a98`.
    pub sp_skill3_active: bool,
}

/// Inputs of the condition update (`0x6e4d30`, inlined in slot 517).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConditionInput {
    /// `0x266ba0(this)`.
    pub can_update_armor: bool,
    /// Some equipment slot 4..=8 holds an item (`0x6a24c0 > 0`), or `0x187850()`.
    pub armor_equipped: bool,
    /// `0x7370d0(this + 0xa040)`.
    pub purge_blocked: bool,
    /// State of skill `+0xaea8` in the skill list `+0x96d8` (`0x29bf70`), or
    /// `None` when the skill is not learned (`0x29c800` returns -1).
    pub sp_skill3: Option<bool>,
}

impl DeliaServer {
    pub fn new(this_address: u64) -> Self {
        Self {
            guard: ProtectedValue::new(this_address + 0xad28),
            weapon_state: 0,
            field_ad20: false,
            ability_slots_reset: false,
            field_af50: false,
            pending_targets: Vec::new(),
            grab_target_changed: false,
            strong_attack_delay_active: false,
            strong_attack_delay_end: -1.0,
            held: None,
            armor_purge_available: false,
            sp_skill3_active: false,
        }
    }

    /// `0x6e4d30`: refreshes the two networked flags and returns the conditions
    /// to raise (`true`) or lower: `As_Cond_Can_Armor_Purge` and
    /// `As_Cond_Delia_SPSkill3_Activated` (`0x28f4b0` raises, `0x288090` lowers).
    pub fn update_conditions(&mut self, input: &ConditionInput) -> [(u32, bool); 2] {
        if input.can_update_armor {
            self.armor_purge_available = input.armor_equipped && !input.purge_blocked;
        }
        self.sp_skill3_active = input.sp_skill3.unwrap_or(false);
        [
            (na::CAN_ARMOR_PURGE, self.armor_purge_available),
            (na::DELIA_SP_SKILL3_ACTIVATED, self.sp_skill3_active),
        ]
    }

    /// Slots 11 and 12.
    pub fn class_name(&self) -> &'static str {
        CLASS_NAME
    }

    /// Slots 240 and 241 (`0x266d30`, `0x266d20`).
    pub fn slot_240(&self) -> f32 {
        0.88
    }

    pub fn slot_241(&self) -> f32 {
        0.96
    }

    /// Slot 466 (`0x6e4eb0`).
    pub fn slot_466(&self) -> f32 {
        92.0
    }

    /// Slot 694 (`0x6e4ec0`).
    pub fn slot_694(&self) -> f32 {
        89.0 / self.slot_466()
    }

    /// Slot 697 (`0x267010`): `forced_off` is `0x7dd5e0(this)`.
    pub fn set_guard_state(&mut self, state: u8, forced_off: bool) {
        self.guard
            .set(if forced_off { 0 } else { u32::from(state) });
    }

    /// Slot 699 (`0x266fd0`).
    pub fn is_guarding(&self) -> bool {
        self.guard.get() == 1
    }

    /// Slot 434 (`0x6e5750`); slot 435 forwards to it. `ignored_by_attacker` and
    /// `ignored_by_hit` are `0x28d410(info, attacker)` and `0x28d0b0(info)`.
    pub fn can_guard(
        &self,
        info: &DamageFlags,
        ignored_by_attacker: bool,
        ignored_by_hit: bool,
    ) -> bool {
        if info.kind & DAMAGE_FLAG_GUARD_DECIDED != 0 {
            return info.result & DAMAGE_GUARDED != 0;
        }
        self.is_guarding()
            && info.kind & DAMAGE_FLAG_IGNORE_GUARD == 0
            && !ignored_by_attacker
            && !ignored_by_hit
    }

    /// Slot 615 (`0x6e6320`), first part: records the guard decision in the
    /// damage record and says whether `0x6e56e0`/`0x6e5d50` run before the base.
    pub fn mark_guard(&self, info: &mut DamageFlags, guarded: bool) -> bool {
        if guarded {
            info.result |= DAMAGE_GUARDED;
        } else {
            info.result &= !DAMAGE_GUARDED;
        }
        !self.field_ad20 && !guarded && info.kind & DAMAGE_FLAG_IGNORE_GUARD == 0
    }

    /// Slot 657 (`0x6e5cb0`), after the base: true when the networked weapon
    /// state changed (`0xfc390` marks it dirty).
    pub fn on_weapon_changed(&mut self, weapon_class: &str) -> bool {
        let state = i32::from(weapon_class.starts_with(WEAPON_CLASS));
        let changed = self.weapon_state != state;
        self.weapon_state = state;
        changed
    }

    /// `0x6e4020`: the strong attack is armed by the delay buff or by the
    /// grace period of the previous hit.
    pub fn strong_attack_ready(
        &mut self,
        skill_level: i32,
        has_delay_buff: bool,
        now: f32,
    ) -> bool {
        if skill_level <= 0 {
            return false;
        }
        if has_delay_buff || (self.strong_attack_delay_active && self.strong_attack_delay_end > now)
        {
            return true;
        }
        self.strong_attack_delay_end = -1.0;
        self.strong_attack_delay_active = false;
        false
    }

    /// Slot 729 (`0x6e7570`) before the base: tracks whether the held entity changed.
    /// `is_player_flag` is `entity + 0x56c0`.
    pub fn set_held(&mut self, entity: Option<(u32, bool)>) {
        match entity {
            None => self.grab_target_changed = false,
            Some((handle, true)) if self.held != Some(handle) => self.grab_target_changed = true,
            Some(_) => {}
        }
        self.held = entity.map(|(handle, _)| handle);
    }

    /// Slot 730 (`0x6e40b0`): a Delia attack hit `target`.
    pub fn on_strong_attack_hit(
        &mut self,
        context: &StrongAttackContext<'_>,
        target: Option<&StrongAttackTarget>,
    ) -> StrongAttackOutcome {
        if !context.attacking
            || !self.strong_attack_ready(context.skill_level, context.has_delay_buff, context.now)
            || context.current_action != context.attack_name
            || context.holding
        {
            return StrongAttackOutcome::Ignored;
        }
        let Some(target) = target else {
            return StrongAttackOutcome::Ignored;
        };
        if target.health <= 0 || target.immune {
            return StrongAttackOutcome::Ignored;
        }
        let outcome = if target.launchable {
            StrongAttackOutcome::Launched
        } else if target.knockable {
            StrongAttackOutcome::KnockedDown
        } else if target.queueable {
            self.pending_targets.push(target.handle);
            return StrongAttackOutcome::Queued;
        } else {
            return StrongAttackOutcome::Ignored;
        };
        self.set_held(Some((target.handle, false)));
        self.strong_attack_delay_active = true;
        self.strong_attack_delay_end = context.now + STRONG_ATTACK_DELAY;
        outcome
    }

    /// Slot 628 (`0x6e77a0`) after the base.
    pub fn slot_628(&mut self, flag: bool) {
        if flag {
            self.field_af50 = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(now: f32) -> StrongAttackContext<'static> {
        StrongAttackContext {
            attacking: true,
            skill_level: 1,
            has_delay_buff: true,
            current_action: "delia_strong_attack_03a",
            attack_name: "delia_strong_attack_03a",
            holding: false,
            now,
        }
    }

    fn target() -> StrongAttackTarget {
        StrongAttackTarget {
            handle: 9,
            health: 100,
            immune: false,
            launchable: true,
            knockable: false,
            queueable: false,
        }
    }

    #[test]
    fn bastard_sword_sets_weapon_state() {
        let mut delia = DeliaServer::new(0);
        assert!(delia.on_weapon_changed("weapon_bastardsword") && delia.weapon_state == 1);
    }

    #[test]
    fn other_weapon_clears_weapon_state() {
        let mut delia = DeliaServer::new(0);
        delia.weapon_state = 1;
        assert!(delia.on_weapon_changed("weapon_longsword") && delia.weapon_state == 0);
    }

    #[test]
    fn forced_guard_is_stored_as_off() {
        let mut delia = DeliaServer::new(0);
        delia.set_guard_state(1, true);
        assert!(!delia.is_guarding());
    }

    #[test]
    fn decided_guard_flag_wins() {
        let delia = DeliaServer::new(0);
        let info = DamageFlags {
            kind: DAMAGE_FLAG_GUARD_DECIDED,
            result: DAMAGE_GUARDED,
        };
        assert!(delia.can_guard(&info, true, true));
    }

    #[test]
    fn strong_attack_hit_opens_grace_period() {
        let mut delia = DeliaServer::new(0);
        let outcome = delia.on_strong_attack_hit(&context(10.0), Some(&target()));
        assert_eq!(
            (outcome, delia.strong_attack_delay_end),
            (StrongAttackOutcome::Launched, 11.5)
        );
    }

    #[test]
    fn grace_period_arms_without_buff() {
        let mut delia = DeliaServer::new(0);
        delia.strong_attack_delay_active = true;
        delia.strong_attack_delay_end = 11.5;
        assert!(delia.strong_attack_ready(1, false, 11.0));
    }

    #[test]
    fn expired_grace_period_resets() {
        let mut delia = DeliaServer::new(0);
        delia.strong_attack_delay_active = true;
        delia.strong_attack_delay_end = 11.5;
        delia.strong_attack_ready(1, false, 12.0);
        assert_eq!(
            (
                delia.strong_attack_delay_active,
                delia.strong_attack_delay_end
            ),
            (false, -1.0)
        );
    }

    #[test]
    fn different_action_is_ignored() {
        let mut delia = DeliaServer::new(0);
        let mut other = context(10.0);
        other.attack_name = "delia_strong_attack_02a2";
        assert_eq!(
            delia.on_strong_attack_hit(&other, Some(&target())),
            StrongAttackOutcome::Ignored
        );
    }

    #[test]
    fn non_launchable_target_is_queued() {
        let mut delia = DeliaServer::new(0);
        let queued = StrongAttackTarget {
            launchable: false,
            queueable: true,
            ..target()
        };
        delia.on_strong_attack_hit(&context(10.0), Some(&queued));
        assert_eq!(delia.pending_targets, vec![9]);
    }

    #[test]
    fn armor_purge_needs_equipped_armor() {
        let mut delia = DeliaServer::new(0);
        delia.armor_purge_available = true;
        let input = ConditionInput {
            can_update_armor: true,
            armor_equipped: false,
            purge_blocked: false,
            sp_skill3: Some(true),
        };
        assert_eq!(
            delia.update_conditions(&input),
            [
                (na::CAN_ARMOR_PURGE, false),
                (na::DELIA_SP_SKILL3_ACTIVATED, true)
            ]
        );
    }

    #[test]
    fn armor_purge_is_kept_while_armor_cannot_update() {
        let mut delia = DeliaServer::new(0);
        delia.armor_purge_available = true;
        let input = ConditionInput {
            can_update_armor: false,
            armor_equipped: false,
            purge_blocked: true,
            sp_skill3: None,
        };
        assert_eq!(
            delia.update_conditions(&input)[0],
            (na::CAN_ARMOR_PURGE, true)
        );
    }
}
