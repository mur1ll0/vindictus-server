//! EnhanceItemByDataProcessor for a non-event request on a non-combined item: the
//! validations, the success ratio (EnhanceServerInfo, rune, material bonus and
//! EnhanceSuccessBonus), the consumed materials, the roll and the failure branch.
//! Randomness is injected; the caller applies the decision to storage.
use super::{
    character_stats::Arithmetic,
    enhance::{EnhanceInfo, Recipe, Rune, RuneExtra, StatRows, fail_rewards},
};

/// ItemOperationResult dialogs of SendOperationFailedDialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    NoEnhanceInfo,
    NotEnoughMaterial,
    MaxArmorBroken,
    System,
}
impl Refusal {
    /// The ItemOperationResult name after "GameUI_Heroes_".
    pub fn name(self) -> &'static str {
        match self {
            Self::NoEnhanceInfo => "EnhanceItemFail_NoEnhanceInfo",
            Self::NotEnoughMaterial => "EnhanceItemFail_NotEnoughMaterial",
            Self::MaxArmorBroken => "StatusFail_MaxArmorBroken",
            Self::System => "EnhanceItemFail_System",
        }
    }
}

/// A source path that throws (NullReference, duplicate Dictionary.Add) or a branch this
/// port does not carry (event enhance, combined items).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unsupported;

/// Contents lookups for the target item.
pub trait Data {
    /// GetEnhanceInfo(item, level).
    fn info(&self, level: i32) -> Option<&EnhanceInfo>;
    /// GetEnhanceInfo then GetEnhanceStatInfo.
    fn stat_rows(&self, level: i32) -> Option<StatRows<'_>>;
    /// GetEnhanceServerInfo($"{level}", level).SuccessRatio.
    fn server_ratio(&self, level: i32) -> Option<f32>;
    fn rune(&self, class: &str) -> Option<Rune>;
    fn rune_extra(&self, relation: i32) -> Option<RuneExtra>;
    /// GetEnhanceBonusServerInfo(material, level).Bonus.
    fn bonus(&self, material: &str, level: i32) -> Option<f64>;
    /// GetEnhanceInsteadMaterial(material, level): (ItemClass, UseCount).
    fn instead(&self, material: &str, level: i32) -> Option<(String, i32)>;
    /// GetMakeItemRecipe of the target class.
    fn recipe(&self) -> Option<&Recipe>;
    /// PriceCalc of a recipe material row.
    fn price(&self, class: &str, num: i32) -> Option<i32>;
}

/// The request fields of EnhanceItemByData.
pub struct Request<'a> {
    pub material1: &'a str,
    pub material2: &'a str,
    pub additional: &'a str,
    pub event: bool,
}

/// The item and character state the processor reads.
pub struct Target {
    pub level: i32,
    /// IsMaxArmorBroken: HasDurability and MaxDurability == 0.
    pub max_armor_broken: bool,
    /// GuildLevelUpBonus["EnhancePrice"].
    pub guild_price_bonus: i32,
}

/// FeatureMatrix values read by the processor.
pub struct Features {
    pub event_enhance_a_shot: bool,
    pub event_change_ratio: bool,
    pub success_bonus: Option<i32>,
    pub success_bonus_limit: Option<i32>,
    pub gold_class: String,
}

pub trait Random {
    fn next_double(&mut self) -> f64;
    /// Random.Next(maxValue); `None` for a negative maximum (ArgumentOutOfRange).
    fn next(&mut self, maximum: i32) -> Option<i32>;
}

/// Everything decided before the roll.
#[derive(Clone, Debug, PartialEq)]
pub struct Prepared {
    pub next_level: i32,
    pub info: EnhanceInfo,
    pub success_ratio: f64,
    pub bonus_ratio: f64,
    pub feature_bonus_ratio: f64,
    /// Dictionary<string,int> materials in insertion order (gold first).
    pub materials: Vec<(String, i32)>,
    pub rune: Option<RuneExtra>,
}

/// The applied outcome.
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    pub success: bool,
    /// Destroyed by class with ItemSelector.ItemClass (materials, and the rune when it
    /// is consumed on success).
    pub destroy: Vec<(String, i32)>,
    /// GiveItemRequests of the CraftItem on failure.
    pub give: Vec<(String, i32)>,
    /// The rune destroyed separately after a failure.
    pub destroy_rune: bool,
    pub level: i32,
    /// IsCharacterBinded set (EnhanceLevel >= 9 after success).
    pub bind: bool,
    /// MaxDurability reduction (RankDown/RankReset), applied before Durability is reset.
    pub max_durability_loss: i32,
    /// OnFail "Destroy" without a rune: the item leaves the bag.
    pub destroyed: bool,
    /// EnhanceItemResultMessage.Result; `None` sends no result.
    pub result: Option<String>,
    pub fail_reward: Vec<(String, i32)>,
}

/// Validations and ratios up to the materials, in the source order.
pub fn prepare(
    request: &Request<'_>,
    target: &Target,
    data: &dyn Data,
    features: &Features,
    useable_count: &dyn Fn(&str) -> i64,
) -> Result<Result<Prepared, Refusal>, Unsupported> {
    let rune = data.rune(request.additional);
    if !request.additional.is_empty() {
        if useable_count(request.additional) == 0 {
            return Ok(Err(Refusal::NotEnoughMaterial));
        }
        let Some(rune) = rune else {
            return Ok(Err(Refusal::NotEnoughMaterial));
        };
        let next = target.level.wrapping_add(1);
        if next < rune.min_level || next > rune.max_level {
            return Ok(Err(Refusal::NotEnoughMaterial));
        }
    }
    if target.max_armor_broken {
        return Ok(Err(Refusal::MaxArmorBroken));
    }
    if request.event {
        return if features.event_enhance_a_shot {
            Err(Unsupported)
        } else {
            Ok(Err(Refusal::System))
        };
    }
    let next_level = target.level.wrapping_add(1);
    // The missing-server-info log reads nextEnhanceInfo.EnhanceType, and the ratio is
    // then assigned to it: a null EnhanceInfo throws either way.
    let info = data.info(next_level).ok_or(Unsupported)?.clone();
    let Some(server) = data.server_ratio(next_level) else {
        return Ok(Err(Refusal::NoEnhanceInfo));
    };
    let mut success_ratio = f64::from(server);
    let mut extra = None;
    if let Some(rune) = rune {
        let Some(found) = data.rune_extra(rune.relation) else {
            return Ok(Err(Refusal::NoEnhanceInfo));
        };
        if !found.use_item_success_ratio {
            success_ratio = found.success_ratio.min(0.4000000059604645);
        }
        extra = Some(found);
    } else if features.event_change_ratio && success_ratio < 1.0 {
        return Err(Unsupported);
    }
    let mut bonus_ratio = 0.0;
    for (slot, own) in [
        (request.material1, &info.material1),
        (request.material2, &info.material2),
    ] {
        // IsNullOrEmpty(slot) short-circuits before materialclass.ToString().
        if !slot.is_empty() && own.as_deref().ok_or(Unsupported)? != slot {
            let Some(bonus) = data.bonus(slot, target.level.wrapping_add(1)) else {
                return Ok(Err(Refusal::NoEnhanceInfo));
            };
            if bonus >= 10.0 {
                bonus_ratio = 0.0;
            } else {
                bonus_ratio += bonus * 0.009999999776482582;
            }
        }
    }
    let feature_bonus_ratio = match features.success_bonus {
        None => 0.0,
        Some(b) if b >= 10 => 0.0,
        Some(b) => match features.success_bonus_limit {
            Some(limit) if !(limit > 0 && limit >= target.level.wrapping_add(1)) => 0.0,
            _ => f64::from(b as f32 * 0.01f32),
        },
    };
    let gold = info
        .gold
        .wrapping_mul(100_i32.wrapping_sub(target.guild_price_bonus))
        / 100;
    let mut materials = vec![(features.gold_class.clone(), gold)];
    let mut add = |key: String, n: i32| -> Result<(), Unsupported> {
        // Dictionary.Add of an existing key throws.
        if materials.iter().any(|(k, _)| *k == key) {
            return Err(Unsupported);
        }
        materials.push((key, n));
        Ok(())
    };
    for (i, (own, num, slot)) in [
        (&info.material1, info.num1, request.material1),
        (&info.material2, info.num2, request.material2),
    ]
    .into_iter()
    .enumerate()
    {
        let own = own.as_deref().ok_or(Unsupported)?;
        if own == slot && !slot.is_empty() {
            add(own.to_owned(), num)?;
            continue;
        }
        match data.instead(slot, target.level.wrapping_add(1)) {
            Some((class, count)) => add(class, count)?,
            None if i == 0 && !features.event_enhance_a_shot => {
                return Ok(Err(Refusal::NoEnhanceInfo));
            }
            None => {}
        }
    }
    Ok(Ok(Prepared {
        next_level,
        info,
        success_ratio,
        bonus_ratio,
        feature_bonus_ratio,
        materials,
        rune: extra,
    }))
}

/// The roll and its branch. `restorable` is RestoreItem && !DidRestored &&
/// CalcBuybackAP > 0 (the item then moves to the restore storage on Destroy).
pub fn resolve(
    prepared: &Prepared,
    request: &Request<'_>,
    target: &Target,
    data: &dyn Data,
    random: &mut dyn Random,
    arithmetic: Arithmetic,
) -> Result<Outcome, Unsupported> {
    let roll = random.next_double();
    let success =
        roll <= prepared.success_ratio + prepared.bonus_ratio + prepared.feature_bonus_ratio;
    let mut outcome = Outcome {
        success,
        destroy: prepared.materials.clone(),
        give: Vec::new(),
        destroy_rune: false,
        level: target.level,
        bind: false,
        max_durability_loss: 0,
        destroyed: false,
        result: None,
        fail_reward: Vec::new(),
    };
    let next = prepared.next_level;
    if success {
        if prepared.rune.is_some_and(|r| r.destroy_on_success) {
            if outcome.destroy.iter().any(|(k, _)| k == request.additional) {
                return Err(Unsupported);
            }
            outcome.destroy.push((request.additional.to_owned(), 1));
        }
        outcome.level = next;
        outcome.bind = next >= 9;
        outcome.result = Some("Success".to_owned());
        return Ok(outcome);
    }
    let info = &prepared.info;
    let rewards = fail_rewards(info, data.recipe(), |c, n| data.price(c, n), arithmetic)
        .ok_or(Unsupported)?;
    let on_fail = info.on_fail.as_deref();
    if let Some(rune) = prepared.rune {
        // A rune voids the fail rewards.
        if rune.reset_rank_on_fail {
            outcome.destroy_rune = true;
            outcome.level = 0;
            outcome.result = Some("ProtectRune_Fail".to_owned());
        } else if rune.force_rank_down_on_fail > 0 {
            outcome.destroy_rune = true;
            outcome.level = if rune.force_rank_down_on_fail >= target.level {
                0
            } else {
                target.level - rune.force_rank_down_on_fail
            };
            outcome.result = Some("ProtectRune_Fail".to_owned());
        } else if let Some(kind @ ("RankDown" | "RankReset" | "Destroy")) = on_fail {
            outcome.destroy_rune = true;
            outcome.result = Some(format!("NoPenalty_{kind}"));
        }
        return Ok(outcome);
    }
    outcome.give = rewards.clone();
    outcome.fail_reward = rewards;
    match on_fail {
        Some("RankDown") => outcome.level = target.level.wrapping_sub(1),
        Some("RankReset") => outcome.level = 0,
        Some("Destroy") => outcome.destroyed = true,
        _ => {}
    }
    // A null OnFail formats as "" in the message; switch(null) takes the default.
    outcome.result = Some(on_fail.unwrap_or("").to_owned());
    if matches!(on_fail, Some("RankDown" | "RankReset")) {
        let durability_row = |level: i32| -> Result<Option<i32>, Unsupported> {
            let Some(rows) = data.stat_rows(level) else {
                return Ok(None);
            };
            // GetEnhanceStatInfo is enumerated: null rows throw.
            let rows = rows.ok_or(Unsupported)?;
            let durability: Vec<u16> = "MaxDurability".encode_utf16().collect();
            Ok(rows
                .iter()
                .filter(|r| r.base.as_deref() == Some(&durability[..]))
                .map(|r| super::enhance::multiply(1, r.value, Arithmetic::FrameworkX64))
                .next_back())
        };
        let num4 = durability_row(next)?
            .unwrap_or(0)
            .wrapping_sub(durability_row(next.wrapping_sub(1))?.unwrap_or(0));
        let threshold = 30_i32.wrapping_sub(2_i32.wrapping_mul(next.wrapping_sub(1)));
        if random.next(100).ok_or(Unsupported)? >= threshold {
            let maximum = num4.wrapping_mul(2).wrapping_add(99) / 100;
            let draw = random.next(maximum).ok_or(Unsupported)?;
            outcome.max_durability_loss = 1_i32.wrapping_add(draw).wrapping_mul(100);
        }
    }
    Ok(outcome)
}
