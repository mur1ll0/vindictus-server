//! Committed ban reads and optimistic IP mutations. No game endpoint invokes this yet.
use crate::{
    domain::{
        legacy_user_data::*,
        login_policy::{IpState, UserBan},
    },
    error::Error,
    infrastructure::legacy_users::SourceBytes,
};
use sqlx::{PgPool, Postgres, Transaction};

pub struct VersionedIpState {
    pub state: IpState,
    pub revision: i64,
}
#[derive(sqlx::FromRow)]
struct IpRow {
    last_login_ip_utf16: Option<SourceBytes>,
    ip_lock_time_ticks: Option<i64>,
    local_revision: i64,
}
pub async fn read_ip_state(pool: &PgPool, id: i32) -> Result<VersionedIpState, Error> {
    let row = sqlx::query_as::<_, IpRow>(
        "SELECT last_login_ip_utf16,ip_lock_time_ticks,local_revision FROM game_users WHERE id=$1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(Error::GameUserMissing)?;
    Ok(VersionedIpState {
        state: IpState {
            last_login_ip: row
                .last_login_ip_utf16
                .map(|b| LegacyBytes::from_bytes(b.into_bytes()))
                .transpose()?,
            lock_until: row
                .ip_lock_time_ticks
                .map(SqlDateTime2::from_ticks)
                .transpose()?,
        },
        revision: row.local_revision,
    })
}
async fn policy_transaction(pool: &PgPool) -> Result<Transaction<'_, Postgres>, Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL lock_timeout='2s'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout='5s'")
        .execute(&mut *tx)
        .await?;
    Ok(tx)
}
/// Trigger increments revision on every actual UPDATE, including writers outside this adapter.
pub(crate) async fn commit_ip_state(
    pool: &PgPool,
    id: i32,
    expected: i64,
    state: &IpState,
    changed: bool,
) -> Result<i64, Error> {
    let mut tx = policy_transaction(pool).await?;
    let revision: Option<i64> = if changed {
        sqlx::query_scalar(
            "UPDATE game_users SET last_login_ip_utf16=$3,ip_lock_time_ticks=$4
             WHERE id=$1 AND local_revision=$2 RETURNING local_revision",
        )
        .bind(id)
        .bind(expected)
        .bind(state.last_login_ip.as_ref().map(LegacyBytes::as_bytes))
        .bind(state.lock_until.map(SqlDateTime2::ticks))
        .fetch_optional(&mut *tx)
        .await?
    } else {
        sqlx::query_scalar(
            "SELECT local_revision FROM game_users WHERE id=$1 AND local_revision=$2 FOR SHARE",
        )
        .bind(id)
        .bind(expected)
        .fetch_optional(&mut *tx)
        .await?
    };
    match revision {
        Some(revision) if changed && expected.checked_add(1) != Some(revision) => {
            tx.rollback().await?;
            Err(Error::SchemaNotReady)
        }
        Some(revision) => {
            tx.commit().await?;
            Ok(revision)
        }
        None => {
            tx.rollback().await?;
            Err(Error::GameUserRevisionConflict)
        }
    }
}
#[derive(sqlx::FromRow)]
struct BanRow {
    name: String,
    status: i32,
    expire_time_units300: Option<i64>,
    reason_utf16: Option<SourceBytes>,
    source_rowversion: Option<Vec<u8>>,
}
impl BanRow {
    fn into_ban(self) -> Result<UserBan, Error> {
        let ban = UserBan {
            name: self.name,
            status: self.status,
            expire_time: self
                .expire_time_units300
                .map(SqlDateTime::from_units300)
                .transpose()?,
            reason: self
                .reason_utf16
                .map(|b| LegacyBytes::from_bytes(b.into_bytes()))
                .transpose()?,
            source_rowversion: self
                .source_rowversion
                .map(|b| SourceRowVersion::from_bytes(&b))
                .transpose()?,
        };
        ban.name_key()?;
        Ok(ban)
    }
}
const BAN_COLUMNS: &str =
    "b.name,b.status,b.expire_time_units300,b.reason_utf16,b.source_rowversion";
/// Procedure's UID-only path: User.Name -> varchar(50) -> UserBan.ID (ASCII subset).
/// READ COMMITTED deliberately replaces source NOLOCK; database errors remain errors.
pub async fn get_user_ban_info(
    pool: &PgPool,
    id: i32,
    sql_local_now: SqlDateTime,
) -> Result<Option<UserBan>, Error> {
    let mut tx = policy_transaction(pool).await?;
    let row = sqlx::query_as::<_, BanRow>(&format!(
        "SELECT {BAN_COLUMNS} FROM game_users u JOIN game_user_bans b ON b.name_key=u.name_key
         WHERE u.id=$1 AND b.status<>0 AND
         (b.expire_time_units300>$2 OR b.expire_time_units300 IS NULL)"
    ))
    .bind(id)
    .bind(sql_local_now.units300())
    .fetch_optional(&mut *tx)
    .await?;
    let result = row.map(BanRow::into_ban).transpose()?;
    tx.commit().await?;
    Ok(result)
}
/// Complete source ban snapshots are insert-only, preserving names/reasons/tokens.
pub async fn insert_ban_snapshot(pool: &PgPool, ban: &UserBan) -> Result<bool, Error> {
    let key = ban.name_key()?;
    let mut tx = policy_transaction(pool).await?;
    let inserted: Option<bool> = sqlx::query_scalar(
        "INSERT INTO game_user_bans(name,status,expire_time_units300,reason_utf16,source_rowversion)
         VALUES($1,$2,$3,$4,$5) ON CONFLICT DO NOTHING RETURNING true"
    ).bind(&ban.name).bind(ban.status).bind(ban.expire_time.map(SqlDateTime::units300))
        .bind(ban.reason.as_ref().map(LegacyBytes::as_bytes))
        .bind(ban.source_rowversion.as_ref().map(SourceRowVersion::as_bytes))
        .fetch_optional(&mut *tx).await?;
    if inserted == Some(true) {
        tx.commit().await?;
        return Ok(true);
    }
    let row = sqlx::query_as::<_, BanRow>(&format!(
        "SELECT {BAN_COLUMNS} FROM game_user_bans b WHERE b.name_key=$1 FOR SHARE"
    ))
    .bind(key)
    .fetch_optional(&mut *tx)
    .await?;
    if !row
        .map(BanRow::into_ban)
        .transpose()?
        .is_some_and(|existing| existing == *ban)
    {
        tx.rollback().await?;
        return Err(Error::LegacyBanConflict);
    }
    tx.commit().await?;
    Ok(false)
}
