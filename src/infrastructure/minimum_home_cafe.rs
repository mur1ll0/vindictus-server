//! HomeCafeBonus (VIP and VVIP) of the selected first Fiona with the Korean rules
//! (HomeCafeBonus_Rule_zhCN off): the bonus rows of `game_home_cafe_bonus` (procedures
//! AddHomeCafeBonus, CheckExpiredHomeCafeBonus, GetHomeCafeBonusInfo and
//! GetHomeCafeExpiredDate), CafeManager.OnKoreanLogIn/OnUseItem/GetRemainTime and
//! HomeCafeBonus.OnLogIn/OnCafeStatusEffectChanged/OnEndItem. VIP (type 2) belongs to the
//! character, VVIP (type 3) to the account (CID 0). Not migrated: the CafeBonusIncreaseTime
//! buff schedule (the region value is 0), LastConnectInfo VIP login/logout times, the
//! quest NotifyLevel, the daily items and Gear.UnequipByConstraint at the VVIP end.
use super::{character_lobby::UserKey, minimum_assets::Assets, minimum_character::MinimumError};
use crate::domain::{feature_policy::FeaturePolicy, map_partition::OutgoingPacket};
use sqlx::{Postgres, Transaction};

/// CafeType values.
pub const VIP: i32 = 2;
pub const VVIP: i32 = 3;
/// SqlDateTime units per second and per day.
const UNITS_PER_SECOND: i64 = 300;
const UNITS_PER_DAY: i64 = 25_920_000;

fn fail() -> MinimumError {
    MinimumError::Conflict
}

/// The CID of a bonus row: the account (0) for VVIP.
fn row_cid(cid: i64, cafe_type: i32) -> i64 {
    if cafe_type == VVIP { 0 } else { cid }
}

/// HomeCafeBonus.GetStatusEffect: HomeCafeBonus_StatusEffect "vip,vvip" (zhCN off).
pub(super) fn effect_name(policy: &FeaturePolicy, cafe_type: i32) -> String {
    let names: Vec<&str> = policy
        .get_string("HomeCafeBonus_StatusEffect")
        .split(',')
        .collect();
    let (vip, vvip) = if names.len() >= 2 {
        (names[0], names[1])
    } else {
        ("", "")
    };
    match cafe_type {
        VIP if !vip.is_empty() => vip.to_owned(),
        VIP => "homecafe_bonus".to_owned(),
        _ if !vvip.is_empty() => vvip.to_owned(),
        _ => "account_homecafe_bonus".to_owned(),
    }
}

fn policy(assets: &Assets) -> Result<&FeaturePolicy, MinimumError> {
    Ok(&assets
        .cash_shop
        .as_ref()
        .ok_or_else(fail)?
        .customize()
        .policy)
}

fn effects(assets: &Assets) -> Result<&crate::domain::status_effect::Contents, MinimumError> {
    Ok(&assets
        .cash_shop
        .as_ref()
        .ok_or_else(fail)?
        .customize()
        .status_effects)
}

/// GetHomeCafeExpiredDate (Korean): GetHomeCafeBonusInfo(UID, CID) with no bonus type,
/// the ExpiredTime of the row of that CID (0 for VVIP).
async fn expire_units(
    tx: &mut Transaction<'_, Postgres>,
    uid: i32,
    cid: i64,
    cafe_type: i32,
) -> Result<Option<i64>, MinimumError> {
    Ok(sqlx::query_scalar(
        "SELECT expired_time_units300 FROM game_home_cafe_bonus WHERE uid=$1 AND cid=$2 ORDER BY bonus_type LIMIT 1",
    )
    .bind(uid)
    .bind(row_cid(cid, cafe_type))
    .fetch_optional(&mut **tx)
    .await?)
}

/// CafeManager.GetRemainTime: seconds left (VIP after the VVIP), capped like the source.
pub(super) async fn remain_seconds(
    tx: &mut Transaction<'_, Postgres>,
    policy: &FeaturePolicy,
    uid: i32,
    cid: i64,
    cafe_type: i32,
) -> Result<i32, MinimumError> {
    let now = super::minimum_inventory::now_units300()?;
    let Some(expire) = expire_units(tx, uid, cid, cafe_type).await? else {
        return Ok(0);
    };
    if expire < now {
        return Ok(0);
    }
    let units = if cafe_type == VIP {
        match expire_units(tx, uid, cid, VVIP).await? {
            Some(vvip) => expire - vvip,
            None => expire - now,
        }
    } else {
        expire - now
    };
    let seconds = units.div_euclid(UNITS_PER_SECOND);
    let cap = 946_080_000 + i64::from(policy.get_integer("CafeBonusIncreaseTime")?) * 60;
    let seconds = if seconds < cap {
        seconds.max(0)
    } else if seconds > cap {
        cap + 5
    } else {
        seconds
    };
    i32::try_from(seconds).map_err(|_| fail())
}

/// GetHomeCafeBonusInfo(UID, CID, null ExpiredTime, BonusType 0): the account VVIP, else the
/// character's type 4, else its VIP; 0 without a row. Expired rows (removed at the next
/// login check) do not count.
pub(super) async fn cafe_type(
    tx: &mut Transaction<'_, Postgres>,
    uid: i32,
    cid: i64,
) -> Result<i32, MinimumError> {
    let now = super::minimum_inventory::now_units300()?;
    let kind: Option<i16> = sqlx::query_scalar(
        "SELECT bonus_type FROM game_home_cafe_bonus WHERE uid=$1 AND expired_time_units300>=$3 AND ((cid=0 AND bonus_type=3) OR (cid=$2 AND bonus_type IN (2,4))) ORDER BY CASE bonus_type WHEN 3 THEN 0 WHEN 4 THEN 1 ELSE 2 END LIMIT 1",
    )
    .bind(uid)
    .bind(cid)
    .bind(now)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(kind.map_or(0, i32::from))
}

/// The account-level CafeType of the frontend UserInfo (GetHomeCafeBonusInfo without CID).
pub(super) async fn account_cafe_type(
    tx: &mut Transaction<'_, Postgres>,
    uid: i32,
) -> Result<i32, MinimumError> {
    let now = super::minimum_inventory::now_units300()?;
    let kind: Option<i16> = sqlx::query_scalar(
        "SELECT bonus_type FROM game_home_cafe_bonus WHERE uid=$1 AND cid=0 AND bonus_type IN (2,3,4) AND expired_time_units300>=$2 ORDER BY CASE bonus_type WHEN 3 THEN 0 WHEN 4 THEN 1 ELSE 2 END LIMIT 1",
    )
    .bind(uid)
    .bind(now)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(kind.map_or(0, i32::from))
}

/// AddHomeCafeBonus / AddHomeCafeBonusByMinute: a VVIP also extends the account's VIP
/// rows; the row's ExpiredTime grows from the later of now and its current value.
async fn add_bonus(
    tx: &mut Transaction<'_, Postgres>,
    uid: i32,
    cid: i64,
    cafe_type: i32,
    amount: i32,
    minutes: bool,
) -> Result<(), MinimumError> {
    let units = if minutes {
        i64::from(amount) * 60 * UNITS_PER_SECOND
    } else {
        i64::from(amount) * UNITS_PER_DAY
    };
    let now = super::minimum_inventory::now_units300()?;
    if cafe_type == VVIP || cafe_type == 4 {
        sqlx::query("UPDATE game_home_cafe_bonus SET expired_time_units300=LEAST(expired_time_units300+$2,76683386879999) WHERE uid=$1 AND bonus_type=2")
            .bind(uid)
            .bind(units)
            .execute(&mut **tx)
            .await?;
    }
    sqlx::query("INSERT INTO game_home_cafe_bonus(uid,cid,bonus_type,expired_time_units300) VALUES($1,$2,$3,LEAST($4+$5,76683386879999)) ON CONFLICT (uid,cid,bonus_type) DO UPDATE SET expired_time_units300=LEAST(GREATEST(game_home_cafe_bonus.expired_time_units300,$4)+$5,76683386879999)")
        .bind(uid)
        .bind(row_cid(cid, cafe_type))
        .bind(i16::try_from(cafe_type).map_err(|_| fail())?)
        .bind(now)
        .bind(units)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// CheckExpiredHomeCafeBonus: deletes the expired row of the type; true when one was.
async fn check_expired(
    tx: &mut Transaction<'_, Postgres>,
    uid: i32,
    cid: i64,
    cafe_type: i32,
) -> Result<bool, MinimumError> {
    let now = super::minimum_inventory::now_units300()?;
    let deleted = sqlx::query("DELETE FROM game_home_cafe_bonus WHERE uid=$1 AND cid=$2 AND bonus_type=$3 AND expired_time_units300<$4")
        .bind(uid)
        .bind(row_cid(cid, cafe_type))
        .bind(i16::try_from(cafe_type).map_err(|_| fail())?)
        .bind(now)
        .execute(&mut **tx)
        .await?
        .rows_affected();
    Ok(deleted > 0)
}

/// HomeCafeBonus.OnCafeStatusEffectChanged: the effect at level (Level − 1) / 10 + 1 (no
/// increase-time bonus) for the remaining time; returns the remaining seconds.
async fn refresh_effect(
    tx: &mut Transaction<'_, Postgres>,
    assets: &Assets,
    uid: i32,
    cid: i64,
    level: i32,
    cafe_type: i32,
) -> Result<i32, MinimumError> {
    let policy = policy(assets)?;
    let bonus = if policy.get_integer("CafeBonusIncreaseTime")? > 0 {
        // The increase-time schedule is not migrated (the region value is 0).
        return Err(fail());
    } else {
        0
    };
    let remain = remain_seconds(tx, policy, uid, cid, cafe_type).await?;
    if remain > 0 {
        super::minimum_status_effect::add(
            tx,
            effects(assets)?,
            cid,
            &effect_name(policy, cafe_type),
            bonus + (level - 1) / 10 + 1,
            Some(remain),
        )
        .await?;
    }
    Ok(remain)
}

/// Frontend messages of a home cafe change, in order.
pub(super) struct Changed {
    pub packets: Vec<OutgoingPacket>,
}

/// CafeManager.OnKoreanLogIn for the character's CafeType: an expired VIP/VVIP ends with
/// its dialog (OnEndItem; a VIP that remains relays to it), otherwise HomeCafeBonus.OnLogIn
/// sends "GameUI_Heroes_DailyItemProcess_<effect>_First" and refreshes the effect.
async fn korean_login(
    tx: &mut Transaction<'_, Postgres>,
    assets: &Assets,
    uid: i32,
    cid: i64,
    level: i32,
    mut cafe_type: i32,
    packets: &mut Vec<OutgoingPacket>,
) -> Result<i32, MinimumError> {
    let policy = policy(assets)?;
    if (cafe_type == VIP || cafe_type == VVIP) && check_expired(tx, uid, cid, cafe_type).await? {
        let dialog = if cafe_type == VVIP {
            "GameUI_Heroes_End_Account_HomeCafe_Message"
        } else {
            "GameUI_Heroes_End_HomeCafe_Message"
        };
        packets.push(super::minimum_shop::system_message(
            super::minimum_shop::DIALOG,
            dialog,
            &[],
        )?);
        // OnEndItem: a VVIP that ends relays to a remaining VIP (CafeManager.OnLogIn
        // without the bonus dialog); otherwise the character has no CafeType.
        let relay = cafe_type == VVIP && expire_units(tx, uid, cid, VIP).await?.is_some();
        if !relay {
            return Ok(0);
        }
        cafe_type = VIP;
        return Box::pin(korean_login(
            tx, assets, uid, cid, level, cafe_type, packets,
        ))
        .await;
    }
    if cafe_type == VIP || cafe_type == VVIP {
        packets.push(super::minimum_shop::system_message(
            super::minimum_shop::SYSTEM,
            &format!(
                "GameUI_Heroes_DailyItemProcess_{}_First",
                effect_name(policy, cafe_type)
            ),
            &[],
        )?);
        refresh_effect(tx, assets, uid, cid, level, cafe_type).await?;
    }
    Ok(cafe_type)
}

/// SelectCharacter: StartHomeCafeBonus (CafeType from the bonus rows when HomeCafeBonus is
/// enabled) and CafeManager.OnLogIn. The packets follow the entry messages.
pub(super) async fn login(
    tx: &mut Transaction<'_, Postgres>,
    assets: &Assets,
    owner: &UserKey<'_>,
    cid: i64,
    level: i32,
) -> Result<Changed, MinimumError> {
    let mut packets = Vec::new();
    let Some(shop) = assets.cash_shop.as_ref() else {
        return Ok(Changed { packets });
    };
    let policy = &shop.customize().policy;
    if !policy.is_enabled(Some("HomeCafeBonus"))?
        || policy.is_enabled(Some("HomeCafeBonus_Rule_zhCN"))?
    {
        return Ok(Changed { packets });
    }
    // The bonus type of the rows, expired ones included (CheckExpired removes those).
    let stored: Option<i16> = sqlx::query_scalar(
        "SELECT bonus_type FROM game_home_cafe_bonus WHERE uid=$1 AND ((cid=0 AND bonus_type=3) OR (cid=$2 AND bonus_type IN (2,4))) ORDER BY CASE bonus_type WHEN 3 THEN 0 WHEN 4 THEN 1 ELSE 2 END LIMIT 1",
    )
    .bind(owner.uid)
    .bind(cid)
    .fetch_optional(&mut **tx)
    .await?;
    korean_login(
        tx,
        assets,
        owner.uid,
        cid,
        level,
        stored.map_or(0, i32::from),
        &mut packets,
    )
    .await?;
    if !packets.is_empty() {
        packets
            .push(super::minimum_status_effect::updated_packet(tx, effects(assets)?, cid).await?);
    }
    Ok(Changed { packets })
}

/// UseInventoryItem's homecafe_bonus_ / account_homecafe_bonus_ checks before the item is
/// consumed: the failure dialog name, or None to go on. `class` is the item's ItemClass.
pub(super) async fn use_check(
    tx: &mut Transaction<'_, Postgres>,
    owner: &UserKey<'_>,
    cid: i64,
    class: &str,
    days: i32,
) -> Result<Option<&'static str>, MinimumError> {
    let current = cafe_type(tx, owner.uid, cid).await?;
    // GetHomeCafeType: HomeCafeDict (the zhCN classes are not sold in this region).
    let kind = home_cafe_kind(class);
    if class.starts_with("homecafe_bonus_") {
        if current == VVIP || current == 4 {
            return Ok(Some("UseItemFail_CannotUseWithAccoutnHomeCafe"));
        }
        if current == 1 {
            return Ok(Some("UseItemFail_CannotUseAtCafe"));
        }
        if kind.is_none() || days == 0 {
            return Ok(Some("UseItemFail_NotUseableItem"));
        }
    } else {
        if current == 1 {
            return Ok(Some("UseItemFail_CannotUseAtCafe"));
        }
        if days == 0 || kind.is_none() {
            return Ok(Some("UseItemFail_NotUseableItem"));
        }
    }
    Ok(None)
}

/// UseInventoryItemProcessor.HomeCafeDict.
pub fn home_cafe_kind(class: &str) -> Option<i32> {
    match class {
        "homecafe_bonus_day" | "homecafe_bonus_min" => Some(VIP),
        "account_homecafe_bonus_day" | "account_homecafe_bonus_min" => Some(VVIP),
        _ => None,
    }
}

/// HomeCafeBonus.OnUseItem after the item is consumed: AddHomeCafeBonus(ByMinute), the
/// CafeType, the VIP effect removed under a VVIP, then CafeManager.OnLogIn.
pub(super) async fn use_bonus(
    tx: &mut Transaction<'_, Postgres>,
    assets: &Assets,
    owner: &UserKey<'_>,
    cid: i64,
    level: i32,
    class: &str,
    amount: i32,
) -> Result<Changed, MinimumError> {
    let kind = home_cafe_kind(class).ok_or_else(fail)?;
    let minutes = !class.contains("_bonus_day");
    add_bonus(tx, owner.uid, cid, kind, amount, minutes).await?;
    let policy = policy(assets)?;
    if kind == VVIP {
        super::minimum_status_effect::remove(tx, effects(assets)?, cid, &effect_name(policy, VIP))
            .await?;
    }
    let mut packets = Vec::new();
    korean_login(tx, assets, owner.uid, cid, level, kind, &mut packets).await?;
    packets.push(super::minimum_status_effect::updated_packet(tx, effects(assets)?, cid).await?);
    Ok(Changed { packets })
}
