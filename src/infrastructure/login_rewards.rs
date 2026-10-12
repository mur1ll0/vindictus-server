//! Atomic existing-account batch. Source operations originally committed separately;
//! this local adapter deliberately adds rollback, revision checking and durable replay.
use crate::{
    application::login_rewards::{LoginRewardRequest, LoginRewardResult},
    domain::{
        dormant_rewards::{RewardCharacter, plan_rewards},
        legacy_user_data::SqlDateTime,
        login_effects::{
            EffectFeatures, EffectFlags, EffectInputs, LoginEffect, plan_login_effects,
        },
    },
    error::Error,
    infrastructure::{characters, dormant_items, login_account_effects::set_friend_flag},
};
use sqlx::PgPool;

#[derive(sqlx::FromRow)]
struct Receipt {
    uid: i32,
    name_key: String,
    request: Vec<u8>,
    revision_after: Option<i64>,
    is_dormant: Option<bool>,
    is_return: Option<bool>,
    queued_row_ids: Option<Vec<i32>>,
}
pub(crate) async fn execute(
    pool: &PgPool,
    request: &LoginRewardRequest<'_>,
    name_key: &str,
    canonical: &[u8],
    mut reward_clock: impl FnMut() -> Result<SqlDateTime, Error>,
) -> Result<LoginRewardResult, Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET LOCAL lock_timeout='2s'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout='5s'")
        .execute(&mut *tx)
        .await?;
    // Global UUID claim first: competing accounts using the same UUID cannot allocate items.
    let claimed = sqlx::query("INSERT INTO game_login_reward_runs(operation_id,uid,name_key,request) VALUES($1,$2,$3,$4) ON CONFLICT DO NOTHING")
        .bind(request.operation_id).bind(request.uid).bind(name_key).bind(canonical).execute(&mut *tx).await?.rows_affected()==1;
    if !claimed {
        let stored:Receipt=sqlx::query_as("SELECT uid,name_key,request,revision_after,is_dormant,is_return,queued_row_ids FROM game_login_reward_runs WHERE operation_id=$1")
            .bind(request.operation_id).fetch_one(&mut *tx).await?;
        if stored.uid != request.uid || stored.name_key != name_key || stored.request != canonical {
            return Err(Error::LoginEffectReplayConflict);
        }
        let (Some(revision), Some(is_dormant), Some(is_return), Some(queued_row_ids)) = (
            stored.revision_after,
            stored.is_dormant,
            stored.is_return,
            stored.queued_row_ids,
        ) else {
            return Err(Error::LoginEffectReplayConflict);
        };
        tx.commit().await?;
        return Ok(LoginRewardResult {
            flags: EffectFlags {
                is_dormant,
                is_return,
            },
            revision,
            queued_row_ids,
            replayed: true,
        });
    }
    let (stored_key,revision,dormant): (String,i64,Option<i64>) = sqlx::query_as(
        "SELECT name_key,local_revision,is_dormant_user_units300 FROM game_users WHERE id=$1 FOR UPDATE")
        .bind(request.uid).fetch_optional(&mut *tx).await?.ok_or(Error::GameUserMissing)?;
    if stored_key != name_key {
        return Err(Error::GameUserIdConflict);
    }
    if revision != request.expected_revision {
        return Err(Error::GameUserRevisionConflict);
    }
    let policy = &request.content.policy;
    let friend = policy.is_enabled(Some("friend_loyalty_system"))?;
    let flag: Option<i32> = if friend {
        sqlx::query_scalar(
            "SELECT recommend_flag FROM game_friend_loyalty WHERE recommender_uid=$1 FOR UPDATE",
        )
        .bind(request.uid)
        .fetch_optional(&mut *tx)
        .await?
    } else {
        None
    };
    let latest_friend = if friend && flag.is_none() {
        characters::get_latest_connected_date_on_connection(&mut tx, request.uid)
            .await?
            .map(SqlDateTime::framework_date_time)
            .transpose()?
    } else {
        None
    };
    let friend_days = if friend && flag.is_none() {
        policy.get_integer("friend_loyalty_return_interval")?
    } else {
        0
    };
    let dormant_enabled = policy.is_enabled(Some("DormantAccount"))?;
    let since = if dormant_enabled {
        dormant
            .map(SqlDateTime::from_units300)
            .transpose()?
            .map(SqlDateTime::framework_date_time)
            .transpose()?
    } else {
        None
    };
    let new_days = if since.is_some() {
        policy.get_integer("DormantNewAccountInterval")?
    } else {
        0
    };
    // Separate READ COMMITTED observations retain the source CASE/MAX/NULL behavior.
    let latest_dormant = if dormant_enabled {
        characters::get_latest_connected_date_on_connection(&mut tx, request.uid)
            .await?
            .map(SqlDateTime::framework_date_time)
            .transpose()?
    } else {
        None
    };
    let dormant_days = if dormant_enabled {
        policy.get_integer("DormantAccountInterval")?
    } else {
        0
    };
    let v2 = dormant_enabled && policy.is_enabled(Some("DormantAccount_ver2"))?;
    let plan = plan_login_effects(
        &EffectInputs {
            new_user_required: false,
            friend_recommend_flag: flag,
            latest_for_loyalty: latest_friend,
            dormant_since: since,
            latest_for_dormancy: latest_dormant,
        },
        EffectFeatures {
            friend_loyalty: friend,
            friend_return_days: friend_days,
            dormant_account: dormant_enabled,
            dormant_new_account_days: new_days,
            dormant_account_days: dormant_days,
            dormant_account_v2: v2,
        },
        request.clocks,
    );
    let mut mutated = false;
    let mut user_mutated = false;
    let mut event_observation = None;
    let mut ids = Vec::new();
    for action in plan.actions() {
        match action {
            LoginEffect::SetFriendRecommendFlag(value) => {
                set_friend_flag(&mut tx, request.uid, *value, request.loyalty_insert_now).await?;
                mutated = true;
            }
            LoginEffect::ReleaseDormant => {
                user_mutated = true;
                sqlx::query("UPDATE game_users SET is_dormant_user_units300=NULL WHERE id=$1")
                    .bind(request.uid)
                    .execute(&mut *tx)
                    .await?;
                mutated = true;
            }
            LoginEffect::SetDormantUser(now) => {
                user_mutated = true;
                sqlx::query("UPDATE game_users SET is_dormant_user_units300=$2 WHERE id=$1")
                    .bind(request.uid)
                    .bind(now.sql_parameter()?.units300())
                    .execute(&mut *tx)
                    .await?;
                mutated = true;
            }
            LoginEffect::SendDormantRewards => {
                let characters =
                    characters::get_characters_on_connection(&mut tx, request.uid, true).await?;
                let views: Vec<_> = characters.iter().map(RewardCharacter::from).collect();
                // Clock dependency is synchronous/bounded and called once per eligible item;
                // sampling precedes the SQL inserts, unlike the original separate roundtrips.
                let items =
                    plan_rewards(&views, &request.content.rewards, policy, &mut reward_clock)?;
                for item in &items {
                    ids.push(dormant_items::send_dormant_item(&mut tx, item).await?);
                }
            }
            LoginEffect::UpdateDormantEvent(now) => {
                user_mutated = true;
                let value = now
                    .map(|date| date.sql_parameter().map(SqlDateTime::units300))
                    .transpose()?;
                sqlx::query("UPDATE game_users SET is_dormant_user_event_units300=$2 WHERE id=$1")
                    .bind(request.uid)
                    .bind(value)
                    .execute(&mut *tx)
                    .await?;
                mutated = true;
            }
            LoginEffect::ReadDormantEvent => {
                let value: Option<i64> = sqlx::query_scalar(
                    "SELECT is_dormant_user_event_units300 FROM game_users WHERE id=$1",
                )
                .bind(request.uid)
                .fetch_one(&mut *tx)
                .await?;
                event_observation = Some(vec![value.map(SqlDateTime::from_units300).transpose()?]);
            }
            LoginEffect::InsertNewUser | LoginEffect::ReplaceAchievementOwner => {
                return Err(Error::LoginEffectDependencies);
            }
        }
    }
    // Source operations retain order and each UPDATE advances the existing DB revision
    // trigger. A loyalty-only mutation also participates in optimistic concurrency.
    let after: i64 = if mutated && !user_mutated {
        sqlx::query_scalar("UPDATE game_users SET local_revision=local_revision WHERE id=$1 RETURNING local_revision")
            .bind(request.uid).fetch_one(&mut *tx).await?
    } else {
        sqlx::query_scalar("SELECT local_revision FROM game_users WHERE id=$1")
            .bind(request.uid)
            .fetch_one(&mut *tx)
            .await?
    };
    let flags = plan
        .complete(event_observation.as_deref())
        .map_err(|_| Error::LoginEffectDependencies)?;
    sqlx::query("UPDATE game_login_reward_runs SET revision_after=$2,is_dormant=$3,is_return=$4,queued_row_ids=$5 WHERE operation_id=$1")
        .bind(request.operation_id).bind(after).bind(flags.is_dormant).bind(flags.is_return).bind(&ids).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(LoginRewardResult {
        flags,
        revision: after,
        queued_row_ids: ids,
        replayed: false,
    })
}
