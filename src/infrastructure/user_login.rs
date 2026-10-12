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
        let revision = own.2;
        tx.commit().await?;
        return Ok(PreparedLoginUser {
            created: false,
            revision,
        });
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
