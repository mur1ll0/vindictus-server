//! Atomic local new-user preparation; SQL Server remains a read-only reference.
use crate::{
    domain::{game_user::NewGameUser, legacy_user_data::SqlDateTime},
    error::Error,
    infrastructure::{
        legacy_users::SourceBytes,
        login_account_effects::{replace_achievement_owner, set_friend_flag},
    },
};
use sqlx::PgPool;

pub struct PreparedLoginUser {
    pub created: bool,
    pub revision: i64,
}
/// Short bounded creation lock serializes BOTH identity keys and ownership replacement.
/// Existing users are never overwritten. The new-user barrier deliberately rolls back
/// its loyalty/user/achievement mutations together, unlike legacy separate commits.
pub async fn prepare_user(
    pool: &PgPool,
    user: &NewGameUser<'_>,
    friend: bool,
    now: SqlDateTime,
) -> Result<PreparedLoginUser, Error> {
    prepare_user_inner(pool, user, friend, now, None).await
}
/// Runtime path: provenance is checked inside the creation lock, before all effects.
/// Existing unmarked users are refused, never adopted or retroactively marked.
pub async fn prepare_scoped_lab_user(
    pool: &PgPool,
    user: &NewGameUser<'_>,
    friend: bool,
    now: SqlDateTime,
    scope: &crate::domain::lab_account::LabAccountScope,
) -> Result<PreparedLoginUser, Error> {
    if user.name_key().is_empty()
        || user.service_id() != scope.service_id()
        || user.channeling_code() != (scope.channel_code() != 0).then_some(scope.channel_code())
    {
        return Err(Error::LabAccountProvenance);
    }
    prepare_user_inner(pool, user, friend, now, Some(scope)).await
}
async fn prepare_user_inner(
    pool: &PgPool,
    user: &NewGameUser<'_>,
    friend: bool,
    now: SqlDateTime,
    scope: Option<&crate::domain::lab_account::LabAccountScope>,
) -> Result<PreparedLoginUser, Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET LOCAL lock_timeout='2s'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout='5s'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("LOCK TABLE game_users IN SHARE ROW EXCLUSIVE MODE")
        .execute(&mut *tx)
        .await?;
    let rows:Vec<(i32,String,i64)>=sqlx::query_as("SELECT id,name_key,local_revision FROM game_users WHERE id=$1 OR name_key=$2 ORDER BY id FOR UPDATE")
        .bind(user.id()).bind(user.name_key()).fetch_all(&mut *tx).await?;
    if !rows.is_empty() {
        let own = rows
            .iter()
            .find(|r| r.0 == user.id())
            .ok_or(Error::GameUserNameConflict)?;
        if own.1 != user.name_key() || rows.len() != 1 {
            return Err(Error::GameUserIdConflict);
        }
        if let Some(scope) = scope {
            let valid: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM game_lab_account_origins o JOIN game_users u ON u.id=o.uid
                 WHERE o.uid=$1 AND o.name_key=$2 AND o.profile_fingerprint=$3
                 AND u.service_id=$4 AND u.channeling_code IS NOT DISTINCT FROM $5)")
                .bind(user.id()).bind(user.name_key()).bind(scope.fingerprint().as_slice())
                .bind(scope.service_id()).bind(user.channeling_code().map(i16::from))
                .fetch_one(&mut *tx).await?;
            if !valid {
                return Err(Error::LabAccountProvenance);
            }
        }
        let revision = own.2;
        tx.commit().await?;
        return Ok(PreparedLoginUser {
            created: false,
            revision,
        });
    }
    if scope.is_some() {
        // Imports do not necessarily have a User row. Serialize these checks with writes,
        // so a fresh laboratory ID/name cannot take ownership of dangling source state.
        sqlx::query(
            "LOCK TABLE game_character_info,game_friend_loyalty,game_user_achievements,
            game_user_info,game_user_products,game_user_care_info,game_home_cafe_bonus,
            game_login_account_effect_runs,game_login_reward_runs IN SHARE ROW EXCLUSIVE MODE",
        )
        .execute(&mut *tx)
        .await?;
        let occupied: bool = sqlx::query_scalar(
            "SELECT
            EXISTS(SELECT 1 FROM game_character_info WHERE uid=$1)
            OR EXISTS(SELECT 1 FROM game_friend_loyalty WHERE recommender_uid=$1)
            OR EXISTS(SELECT 1 FROM game_user_achievements WHERE uid=$1 OR name_key=$2)
            OR EXISTS(SELECT 1 FROM game_user_info WHERE nexon_sn=$1)
            OR EXISTS(SELECT 1 FROM game_user_products WHERE nexon_sn=$1)
            OR EXISTS(SELECT 1 FROM game_user_care_info WHERE uid=$1)
            OR EXISTS(SELECT 1 FROM game_home_cafe_bonus WHERE uid=$1)
            OR EXISTS(SELECT 1 FROM game_login_account_effect_runs WHERE uid=$1 OR name_key=$2)
            OR EXISTS(SELECT 1 FROM game_login_reward_runs WHERE uid=$1 OR name_key=$2)",
        )
        .bind(user.id())
        .bind(user.name_key())
        .fetch_one(&mut *tx)
        .await?;
        if occupied {
            return Err(Error::LabAccountProvenance);
        }
    }
    if friend {
        let flag: Option<i32> = sqlx::query_scalar(
            "SELECT recommend_flag FROM game_friend_loyalty WHERE recommender_uid=$1 FOR UPDATE",
        )
        .bind(user.id())
        .fetch_optional(&mut *tx)
        .await?;
        if flag.is_none() {
            set_friend_flag(&mut tx, user.id(), 1, now).await?;
        }
    }
    let revision=sqlx::query_scalar("INSERT INTO game_users(id,name,next_character_sn,service_id,channeling_code) VALUES($1,$2,$3,$4,$5) RETURNING local_revision")
        .bind(user.id()).bind(user.name()).bind(user.next_character_sn()).bind(user.service_id()).bind(user.channeling_code().map(i16::from))
        .fetch_one(&mut *tx).await?;
    if let Some(scope) = scope {
        sqlx::query("INSERT INTO game_lab_account_origins(uid,name_key,profile_fingerprint) VALUES($1,$2,$3)")
            .bind(user.id()).bind(user.name_key()).bind(scope.fingerprint().as_slice())
            .execute(&mut *tx).await?;
    }
    replace_achievement_owner(&mut tx, user.name(), user.id()).await?;
    tx.commit().await?;
    Ok(PreparedLoginUser {
        created: true,
        revision,
    })
}
/// Only token/revision are read; no private full snapshot is materialized for feedback.
pub async fn read_token(
    pool: &PgPool,
    uid: i32,
    expected_revision: i64,
) -> Result<Option<crate::domain::legacy_user_data::LegacyBytes<255, false>>, Error> {
    let (revision, token): (i64, Option<SourceBytes>) =
        sqlx::query_as("SELECT local_revision,facebook_token_bytes FROM game_users WHERE id=$1")
            .bind(uid)
            .fetch_optional(pool)
            .await?
            .ok_or(Error::GameUserMissing)?;
    if revision != expected_revision {
        return Err(Error::GameUserRevisionConflict);
    }
    token
        .map(|b| {
            crate::domain::legacy_user_data::LegacyBytes::from_bytes(b.into_bytes())
                .map_err(Error::from)
        })
        .transpose()
}
