//! PeriodicEventScheduler and StoryEntity.DailyProcess for the entered first Fiona: the
//! BoostEvent reward of each BoostEventPeriod of the session (ItemOwnerEvent: gold, AP,
//! EXP or an item, its GetItemMessage and the Telepathy announce) and the storylines a
//! StoryLinePeriodInfo period returns to their target phase. The item journal and ledger
//! of the reward are not migrated.
use super::{character_lobby::UserKey, minimum_assets::Assets, minimum_character::MinimumError};
use crate::domain::{
    map_partition::OutgoingPacket,
    mission::MissionRandom,
    periodic_event::{PeriodicReward, RewardType, StoryProgress},
};
use sqlx::PgPool;

/// SystemMessageCategory.System / Telepathy.
const SYSTEM: u8 = 0;
const TELEPATHY: u8 = 2;

fn fail() -> MinimumError {
    MinimumError::Conflict
}
fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
fn text(bytes: &[u8]) -> Result<String, MinimumError> {
    if !bytes.len().is_multiple_of(2) {
        return Err(fail());
    }
    String::from_utf16(
        &bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_le_bytes(*c))
            .collect::<Vec<_>>(),
    )
    .map_err(|_| fail())
}

/// NotifyClient of a reward: every frontend client (town only with RandomItemBroadcastMode,
/// from RandomItem_AnnounceLevel).
pub struct Announce {
    pub packet: OutgoingPacket,
    pub town_only: bool,
    pub level: i32,
}

#[derive(Default)]
pub struct Boost {
    pub packets: Vec<OutgoingPacket>,
    pub announce: Option<Announce>,
}

/// PeriodicEventWaiting: the BoostEventPeriod of a logged-on character.
pub fn period(assets: &Assets) -> Option<std::time::Duration> {
    let npc = assets.npc.as_ref()?;
    crate::domain::periodic_event::Events::period(&npc.policy).map(std::time::Duration::from_secs)
}

/// ItemOwnerEvent: the BoostEvent reward drawn for this period, if any.
pub async fn boost(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    ctx: super::minimum_growth::GrowthContext,
    ids: Vec<i64>,
) -> Result<Boost, MinimumError> {
    let npc = assets.npc.as_ref().ok_or_else(fail)?;
    let mut random = super::minimum_mission::OsMissionRandom;
    let reward = npc
        .events
        .pick(&npc.policy, &mut |n| random.next_range(0, n))
        .map_err(|_| fail())?;
    match reward {
        Some(reward) => give(pool, owner, assets, cid, reward, ctx, ids).await,
        None => Ok(Boost::default()),
    }
}

fn get_item(name: &str, class: &str, count: i32) -> Result<OutgoingPacket, MinimumError> {
    // GetItemMessage(CharacterName, class, Amount, 0, Lucky: false, 0, Success).
    super::minimum_mission::get_item_packet(name, class, count, 0)
}

/// One PeriodicEventReward for the character, in an ItemTransactionScope.
pub async fn give(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    reward: &PeriodicReward,
    ctx: super::minimum_growth::GrowthContext,
    ids: Vec<i64>,
) -> Result<Boost, MinimumError> {
    let npc = assets.npc.as_ref().ok_or_else(fail)?;
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
    let name: Option<Vec<u8>> =
        sqlx::query_scalar("SELECT name_utf16 FROM game_character_info WHERE id=$1")
            .bind(cid)
            .fetch_one(&mut *tx)
            .await?;
    let name = name.as_deref().map(text).transpose()?.unwrap_or_default();
    let mut ids = ids.into_iter();
    let mut next_id = || ids.next().ok_or_else(fail);
    let mut packets = Vec::new();
    let amount = reward.amount;
    let given = match reward.kind {
        RewardType::Gold => {
            let attributes = crate::domain::item_attribute::Attributes::default();
            let request = [super::minimum_give::Request {
                class: assets.inventory.gold_class(),
                count: amount,
                bound: false,
                equipment: None,
                attributes: &attributes,
            }];
            if let Ok(applied) =
                super::minimum_give::apply(&mut tx, owner, assets, cid, &[], &request, &mut next_id)
                    .await?
                && !applied.slots.is_empty()
            {
                packets.push(super::minimum_mission::inventory_packet(applied.slots)?);
            }
            packets.push(get_item(&name, "give_gold", amount)?);
            "give_gold".to_owned()
        }
        RewardType::Ap => {
            // Character.GiveAP(Amount, allowExceed: true).
            let ap = super::minimum_ap::give_exceeding(
                &mut tx,
                owner,
                assets,
                cid,
                ctx.arithmetic,
                amount,
            )
            .await?;
            packets.push(super::minimum_ap::packet(ap)?);
            packets.push(get_item(&name, "give_ap", amount)?);
            "give_ap".to_owned()
        }
        RewardType::Exp => {
            // Character.GiveReward(Amount, 0, [], {}).
            let mut rest = std::iter::from_fn(|| next_id().ok());
            packets.extend(
                super::minimum_growth::give_reward(
                    &mut tx,
                    owner,
                    assets,
                    cid,
                    i64::from(amount),
                    0,
                    ctx,
                    &mut rest,
                )
                .await?,
            );
            packets.push(get_item(&name, "give_exp", amount)?);
            "give_exp".to_owned()
        }
        RewardType::Item => {
            let arg = reward.arg.clone().unwrap_or_default();
            match crate::domain::item_attribute::parse_class_ex(&arg) {
                Ok((class, attributes)) => {
                    let key = utf16(&class);
                    let request = [super::minimum_give::Request {
                        class: &key,
                        count: amount,
                        bound: false,
                        equipment: None,
                        attributes: &attributes,
                    }];
                    match super::minimum_give::apply_force_mail(
                        &mut tx,
                        owner,
                        assets,
                        cid,
                        &[],
                        &request,
                        true,
                        &mut next_id,
                    )
                    .await?
                    {
                        Ok(given) => {
                            if !given.applied.slots.is_empty() {
                                packets.push(super::minimum_mission::inventory_packet(
                                    given.applied.slots,
                                )?);
                            }
                            match given.result {
                                0 => packets.push(get_item(&name, &arg, amount)?),
                                // SendGiveItemFailMail, then SendOperationFailedMessage.
                                result => {
                                    packets.extend(given.packets);
                                    packets.push(super::minimum_shop::system_message(
                                        SYSTEM,
                                        if result == 1 {
                                            "GameUI_Heroes_GiveItemFail_UniqueConstraint"
                                        } else {
                                            "GameUI_Heroes_GiveItemFail_NoEmptySlot"
                                        },
                                        &[],
                                    )?);
                                }
                            }
                        }
                        // Another result: the system items are zeroed ("Logical Error").
                        Err(_) => {
                            tracing::warn!("periodic event item reward not given; values omitted")
                        }
                    }
                }
                Err(_) => tracing::warn!("periodic event ItemClassEx refused; values omitted"),
            }
            arg
        }
        RewardType::None => String::new(),
    };
    tx.commit().await?;
    let announce = match reward.announce.as_deref() {
        Some(format) if !given.is_empty() && !format.is_empty() && !name.is_empty() => {
            Some(Announce {
                packet: super::minimum_shop::system_message(
                    TELEPATHY,
                    format,
                    &[name.clone(), amount.to_string(), given],
                )?,
                town_only: npc
                    .policy
                    .is_enabled(Some("RandomItemBroadcastMode"))
                    .map_err(|_| fail())?,
                level: npc
                    .policy
                    .get_integer("RandomItem_AnnounceLevel")
                    .unwrap_or(0),
            })
        }
        _ => None,
    };
    Ok(Boost { packets, announce })
}

/// LastUpdate (1/300 s since 1900-01-01) as Unix seconds.
fn unix_seconds(units300: i64) -> i64 {
    units300.div_euclid(300) - 2_208_988_800
}

/// StoryEntity.DailyProcess (TriggerDailyStory at InGame.OnEnter and each period boundary):
/// GotoPhase of the storylines whose period turned, then the time until the next boundary.
/// Nothing reaches the client: its next story query sees the phase.
pub async fn daily_story(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
) -> Result<Option<std::time::Duration>, MinimumError> {
    let npc = assets.npc.as_ref().ok_or_else(fail)?;
    let scripts = &npc.scripts;
    let has_phase = |line: &str, phase: &str| {
        scripts
            .story_line(line)
            .is_some_and(|l| l.phase(phase).is_some())
    };
    story_periods(pool, owner, cid, &npc.events, &npc.policy, &has_phase).await
}

/// `daily_story` over the given contents.
pub async fn story_periods(
    pool: &PgPool,
    owner: &UserKey<'_>,
    cid: i64,
    events: &crate::domain::periodic_event::Events,
    policy: &crate::domain::feature_policy::FeaturePolicy,
    has_phase: &(dyn Fn(&str, &str) -> bool + Sync),
) -> Result<Option<std::time::Duration>, MinimumError> {
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
    let rows: Vec<(Vec<u8>, Option<Vec<u8>>, i64)> = sqlx::query_as(
        "SELECT story_line_utf16,phase_utf16,last_update_units300 FROM game_story_progress WHERE cid=$1 ORDER BY row_id LIMIT 4096",
    )
    .bind(cid)
    .fetch_all(&mut *tx)
    .await?;
    let mut progress = rows
        .iter()
        .map(|(line, phase, last)| {
            Ok(StoryProgress {
                story_line: text(line)?,
                phase: phase.as_deref().map(text).transpose()?,
                last_update: unix_seconds(*last),
            })
        })
        .collect::<Result<Vec<_>, MinimumError>>()?;
    let now_units = super::minimum_quest::now_units()?;
    let now = unix_seconds(now_units);
    let daily = events
        .daily_story(policy, &mut progress, now, has_phase)
        .map_err(|_| fail())?;
    for (line, phase) in &daily.goto {
        // GotoPhase: PhaseID and LastUpdate, then Save (story drops have no saved tries).
        sqlx::query("UPDATE game_story_progress SET phase_utf16=$3,last_update_units300=$4 WHERE cid=$1 AND story_line_utf16=$2")
            .bind(cid)
            .bind(utf16(line))
            .bind(utf16(phase))
            .bind(now_units)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    if !daily.goto.is_empty() {
        tracing::info!(
            storylines = daily.goto.len(),
            "story periods moved storylines to their target phase; values omitted"
        );
    }
    Ok(daily
        .next
        .map(|n| std::time::Duration::from_secs(u64::try_from(n - now).unwrap_or(0))))
}
