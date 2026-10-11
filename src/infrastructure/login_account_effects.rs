//! Transactional source primitives and a restricted existing-account effect executor.
//! No endpoint invokes this. Plans requiring character/reward or new-user dependencies fail before SQL.
use crate::{
    domain::{
        game_user::LegacyNameKey,
        legacy_user_data::{LegacyBytes, SqlDateTime},
        login_account_data::{AchievementSnapshot, LoyaltySnapshot},
        login_effects::{EffectFlags, LoginEffect, LoginEffectPlan},
    },
    error::Error,
    infrastructure::legacy_users::SourceBytes,
};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

async fn transaction(pool: &PgPool) -> Result<sqlx::Transaction<'_, sqlx::Postgres>, Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET LOCAL lock_timeout='2s'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout='5s'")
        .execute(&mut *tx)
        .await?;
    Ok(tx)
}
#[derive(sqlx::FromRow)]
struct LoyaltyRow {
    recommender_uid: i32,
    recommender_cid: Option<i64>,
    friend_cid: Option<i64>,
    recommend_flag: i32,
    update_time_units300: i64,
}
impl LoyaltyRow {
    fn into_snapshot(self) -> Result<LoyaltySnapshot, Error> {
        Ok(LoyaltySnapshot {
            recommender_uid: self.recommender_uid,
            recommender_cid: self.recommender_cid,
            friend_cid: self.friend_cid,
            recommend_flag: self.recommend_flag,
            update_time: SqlDateTime::from_units300(self.update_time_units300)?,
        })
    }
}
pub async fn read_loyalty(pool: &PgPool, uid: i32) -> Result<Option<LoyaltySnapshot>, Error> {
    sqlx::query_as::<_, LoyaltyRow>("SELECT * FROM game_friend_loyalty WHERE recommender_uid=$1")
        .bind(uid)
        .fetch_optional(pool)
        .await?
        .map(LoyaltyRow::into_snapshot)
        .transpose()
}
pub async fn insert_loyalty_snapshot(pool: &PgPool, row: &LoyaltySnapshot) -> Result<bool, Error> {
    let mut tx = transaction(pool).await?;
    let result = sqlx::query(
        "INSERT INTO game_friend_loyalty(recommender_uid,recommender_cid,friend_cid,recommend_flag,update_time_units300) VALUES($1,$2,$3,$4,$5) ON CONFLICT DO NOTHING",
    )
    .bind(row.recommender_uid)
    .bind(row.recommender_cid)
    .bind(row.friend_cid)
    .bind(row.recommend_flag)
    .bind(row.update_time.units300())
    .execute(&mut *tx)
    .await?;
    let stored = sqlx::query_as::<_, LoyaltyRow>(
        "SELECT * FROM game_friend_loyalty WHERE recommender_uid=$1 FOR SHARE",
    )
    .bind(row.recommender_uid)
    .fetch_one(&mut *tx)
    .await?
    .into_snapshot()?;
    if stored != *row {
        return Err(Error::LoginAccountSnapshotConflict);
    }
    tx.commit().await?;
    Ok(result.rows_affected() == 1)
}
#[derive(sqlx::FromRow)]
struct AchievementRow {
    id: i32,
    uid: i32,
    name_utf16: SourceBytes,
    achievement_utf16: SourceBytes,
}
impl AchievementRow {
    fn into_snapshot(self) -> Result<AchievementSnapshot, Error> {
        Ok(AchievementSnapshot {
            id: self.id,
            uid: self.uid,
            name: LegacyBytes::from_bytes(self.name_utf16.into_bytes())?,
            achievement: LegacyBytes::from_bytes(self.achievement_utf16.into_bytes())?,
        })
    }
}
pub async fn read_achievement(
    pool: &PgPool,
    id: i32,
) -> Result<Option<AchievementSnapshot>, Error> {
    sqlx::query_as::<_, AchievementRow>(
        "SELECT id,uid,name_utf16,achievement_utf16 FROM game_user_achievements WHERE id=$1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .map(AchievementRow::into_snapshot)
    .transpose()
}
pub async fn insert_achievement_snapshot(
    pool: &PgPool,
    row: &AchievementSnapshot,
) -> Result<bool, Error> {
    let mut tx = transaction(pool).await?;
    let result = sqlx::query(
        "INSERT INTO game_user_achievements(id,uid,name_utf16,achievement_utf16) VALUES($1,$2,$3,$4)
         ON CONFLICT DO NOTHING",
    ).bind(row.id).bind(row.uid).bind(row.name.as_bytes()).bind(row.achievement.as_bytes())
        .execute(&mut *tx).await?;
    let stored = sqlx::query_as::<_, AchievementRow>(
        "SELECT id,uid,name_utf16,achievement_utf16 FROM game_user_achievements WHERE id=$1 FOR SHARE",
    ).bind(row.id).fetch_one(&mut *tx).await?.into_snapshot()?;
    if stored != *row {
        return Err(Error::LoginAccountSnapshotConflict);
    }
    tx.commit().await?;
    Ok(result.rows_affected() == 1)
}
/// Exact recovered MERGE semantics: matched UPDATE clears links but preserves UpdateTime.
/// Default GETUTCDATE applies only to INSERT; caller supplies that SQL civil value explicitly.
pub async fn set_friend_flag(
    connection: &mut PgConnection,
    uid: i32,
    flag: i32,
    sql_insert_now: SqlDateTime,
) -> Result<(), Error> {
    sqlx::query(
        "INSERT INTO game_friend_loyalty(recommender_uid,recommend_flag,update_time_units300)
         VALUES($1,$2,$3) ON CONFLICT(recommender_uid) DO UPDATE
         SET recommend_flag=EXCLUDED.recommend_flag,recommender_cid=NULL,friend_cid=NULL",
    )
    .bind(uid)
    .bind(flag)
    .bind(sql_insert_now.units300())
    .execute(connection)
    .await?;
    Ok(())
}
/// Transaction caller must serialize user creation. Only UID=-1 rows are relabeled.
/// Unknown Unicode equality in pending rows is refused rather than silently omitted.
pub async fn replace_achievement_owner(
    connection: &mut PgConnection,
    name: &str,
    uid: i32,
) -> Result<u64, Error> {
    let key = LegacyNameKey::parse(name)?;
    // Serialize pending-name imports/updates as well as this relabeling; no unproven Unicode phantom.
    sqlx::query("LOCK TABLE game_user_achievements IN SHARE ROW EXCLUSIVE MODE")
        .execute(&mut *connection)
        .await?;
    let unknown: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM game_user_achievements WHERE uid=-1 AND name_key IS NULL)",
    )
    .fetch_one(&mut *connection)
    .await?;
    if unknown {
        return Err(Error::LoginAccountEqualityUnsupported);
    }
    let result =
        sqlx::query("UPDATE game_user_achievements SET uid=$1 WHERE uid=-1 AND name_key=$2")
            .bind(uid)
            .bind(key.as_str())
            .execute(connection)
            .await?;
    Ok(result.rows_affected())
}
pub struct AccountEffectRequest<'a> {
    pub operation_id: Uuid,
    pub uid: i32,
    pub name: &'a str,
    pub expected_revision: i64,
    pub loyalty_insert_now: SqlDateTime,
    pub plan: &'a LoginEffectPlan,
}
#[derive(Debug, PartialEq, Eq)]
pub struct AccountEffectResult {
    pub flags: EffectFlags,
    pub revision: i64,
    pub replayed: bool,
}
#[derive(sqlx::FromRow)]
struct Receipt {
    uid: i32,
    name_key: String,
    request: String,
    revision_after: i64,
    is_dormant: bool,
    is_return: bool,
}
fn allowed_request(request: &AccountEffectRequest<'_>) -> Result<String, Error> {
    let mut actions = Vec::with_capacity(request.plan.actions().len());
    for action in request.plan.actions() {
        actions.push(match action {
            LoginEffect::SetFriendRecommendFlag(flag) => serde_json::json!(["friend", flag]),
            LoginEffect::ReleaseDormant => serde_json::json!(["release"]),
            LoginEffect::UpdateDormantEvent(None) => serde_json::json!(["clear_event"]),
            LoginEffect::ReadDormantEvent => serde_json::json!(["read_event"]),
            // Never mark dormant while dropping the required reward or new-user effects.
            _ => return Err(Error::LoginEffectDependencies),
        });
    }
    Ok(serde_json::json!({
        "version":1,"revision":request.expected_revision,
        "loyalty_insert_units300":request.loyalty_insert_now.units300().to_string(),
        "actions":actions,
    })
    .to_string())
}
/// Atomic existing-account subset with a durable replay receipt. Retain UUID and exact request on retry.
/// This does not check bans, authenticate or deliver rewards. Caller must observe planner inputs freshly.
pub async fn apply_existing_account_effects(
    pool: &PgPool,
    request: &AccountEffectRequest<'_>,
) -> Result<AccountEffectResult, Error> {
    let key = LegacyNameKey::parse(request.name)?;
    if request.expected_revision < 0 {
        return Err(Error::GameUserRevisionConflict);
    }
    let body = allowed_request(request)?;
    tokio::time::timeout(
        std::time::Duration::from_secs(15),
        execute(pool, request, key.as_str(), &body),
    )
    .await
    .map_err(|_| Error::LoginEffectDeadline)?
}
async fn execute(
    pool: &PgPool,
    request: &AccountEffectRequest<'_>,
    name_key: &str,
    body: &str,
) -> Result<AccountEffectResult, Error> {
    let mut tx = transaction(pool).await?;
    let (stored_key, revision, mut dormant, mut event): (String, i64, Option<i64>, Option<i64>) =
        sqlx::query_as(
            "SELECT name_key,local_revision,is_dormant_user_units300,is_dormant_user_event_units300
             FROM game_users WHERE id=$1 FOR UPDATE",
        )
        .bind(request.uid)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(Error::GameUserMissing)?;
    if stored_key != name_key {
        return Err(Error::GameUserIdConflict);
    }
    let receipt: Option<Receipt> = sqlx::query_as(
        "SELECT uid,name_key,request,revision_after,is_dormant,is_return
         FROM game_login_account_effect_runs WHERE operation_id=$1",
    )
    .bind(request.operation_id)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(receipt) = receipt {
        if receipt.uid != request.uid || receipt.name_key != name_key || receipt.request != body {
            return Err(Error::LoginEffectReplayConflict);
        }
        tx.commit().await?;
        return Ok(AccountEffectResult {
            flags: EffectFlags {
                is_dormant: receipt.is_dormant,
                is_return: receipt.is_return,
            },
            revision: receipt.revision_after,
            replayed: true,
        });
    }
    if revision != request.expected_revision {
        return Err(Error::GameUserRevisionConflict);
    }
    let mut mutated = false;
    let mut event_observation = None;
    for action in request.plan.actions() {
        match action {
            LoginEffect::SetFriendRecommendFlag(flag) => {
                set_friend_flag(&mut tx, request.uid, *flag, request.loyalty_insert_now).await?;
                mutated = true;
            }
            LoginEffect::ReleaseDormant => {
                dormant = None;
                mutated = true;
            }
            LoginEffect::UpdateDormantEvent(None) => {
                event = None;
                mutated = true;
            }
            LoginEffect::ReadDormantEvent => {
                event_observation = Some(vec![event.map(SqlDateTime::from_units300).transpose()?]);
            }
            _ => return Err(Error::LoginEffectDependencies),
        }
    }
    // One DB-managed revision covers this batch, including writes in the loyalty table.
    let after =
        if mutated {
            sqlx::query_scalar(
            "UPDATE game_users SET is_dormant_user_units300=$2,is_dormant_user_event_units300=$3
             WHERE id=$1 RETURNING local_revision",
        ).bind(request.uid).bind(dormant).bind(event).fetch_one(&mut *tx).await?
        } else {
            revision
        };
    let flags = request
        .plan
        .complete(event_observation.as_deref())
        .map_err(|_| Error::LoginEffectDependencies)?;
    let inserted = sqlx::query(
        "INSERT INTO game_login_account_effect_runs(operation_id,uid,name_key,request,revision_after,is_dormant,is_return) VALUES($1,$2,$3,$4,$5,$6,$7)
         ON CONFLICT DO NOTHING",
    )
    .bind(request.operation_id)
    .bind(request.uid)
    .bind(name_key)
    .bind(body)
    .bind(after)
    .bind(flags.is_dormant)
    .bind(flags.is_return)
    .execute(&mut *tx)
    .await?;
    if inserted.rows_affected() != 1 {
        return Err(Error::LoginEffectReplayConflict);
    }
    tx.commit().await?;
    Ok(AccountEffectResult {
        flags,
        revision: after,
        replayed: false,
    })
}
