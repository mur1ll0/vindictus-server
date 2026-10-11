//! Insert-only complete source snapshots. Never updates or repairs a conflicting row.
use crate::{
    domain::{
        game_user::GameUser,
        legacy_user_data::{
            LegacyBytes, LegacyUserDetails, LegacyUserSnapshot, SourceRowVersion, SqlDateTime,
            SqlDateTime2,
        },
    },
    error::Error,
};
use sqlx::{PgConnection, PgPool};
use zeroize::Zeroize;

/// SQL decoding buffers for sensitive source fields are zeroized even on conversion errors.
#[derive(sqlx::Type)]
#[sqlx(transparent)]
pub(crate) struct SourceBytes(Vec<u8>);
impl Drop for SourceBytes {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}
impl SourceBytes {
    pub(crate) fn into_bytes(mut self) -> Vec<u8> {
        std::mem::take(&mut self.0)
    }
}

const COLUMNS: &str = "id, name, next_character_sn, service_id, channeling_code,
    last_login_ip_utf16, ip_lock_time_ticks, facebook_token_bytes,
    is_dormant_user_event_units300, second_password_utf16, second_password_fail_count,
    second_password_last_fail_ticks, legacy_last_update, is_dormant_user_units300,
    create_time_units300";

#[derive(sqlx::FromRow)]
struct StoredSnapshot {
    id: i32,
    name: String,
    next_character_sn: i32,
    service_id: i32,
    channeling_code: Option<i16>,
    last_login_ip_utf16: Option<SourceBytes>,
    ip_lock_time_ticks: Option<i64>,
    facebook_token_bytes: Option<SourceBytes>,
    is_dormant_user_event_units300: Option<i64>,
    second_password_utf16: Option<SourceBytes>,
    second_password_fail_count: Option<i32>,
    second_password_last_fail_ticks: Option<i64>,
    legacy_last_update: Option<Vec<u8>>,
    is_dormant_user_units300: Option<i64>,
    create_time_units300: Option<i64>,
}

impl StoredSnapshot {
    fn into_snapshot(self) -> Result<LegacyUserSnapshot, Error> {
        let snapshot = LegacyUserSnapshot {
            user: GameUser {
                id: self.id,
                name: self.name,
                next_character_sn: self.next_character_sn,
                service_id: self.service_id,
                channeling_code: self.channeling_code,
            },
            details: LegacyUserDetails {
                last_login_ip: self
                    .last_login_ip_utf16
                    .map(|bytes| LegacyBytes::from_bytes(bytes.into_bytes()))
                    .transpose()?,
                ip_lock_time: self
                    .ip_lock_time_ticks
                    .map(SqlDateTime2::from_ticks)
                    .transpose()?,
                facebook_token: self
                    .facebook_token_bytes
                    .map(|bytes| LegacyBytes::from_bytes(bytes.into_bytes()))
                    .transpose()?,
                is_dormant_user_event: self
                    .is_dormant_user_event_units300
                    .map(SqlDateTime::from_units300)
                    .transpose()?,
                second_password: self
                    .second_password_utf16
                    .map(|bytes| LegacyBytes::from_bytes(bytes.into_bytes()))
                    .transpose()?,
                second_password_fail_count: self.second_password_fail_count,
                second_password_last_fail: self
                    .second_password_last_fail_ticks
                    .map(SqlDateTime2::from_ticks)
                    .transpose()?,
                last_update: self
                    .legacy_last_update
                    .map(|bytes| SourceRowVersion::from_bytes(&bytes))
                    .transpose()?,
                is_dormant_user: self
                    .is_dormant_user_units300
                    .map(SqlDateTime::from_units300)
                    .transpose()?,
                create_time: self
                    .create_time_units300
                    .map(SqlDateTime::from_units300)
                    .transpose()?,
            },
        };
        snapshot.validate()?;
        Ok(snapshot)
    }
}

pub async fn read_snapshot(pool: &PgPool, id: i32) -> Result<Option<LegacyUserSnapshot>, Error> {
    let row = sqlx::query_as::<_, StoredSnapshot>(&format!(
        "SELECT {COLUMNS} FROM game_users WHERE id=$1"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?;
    row.map(StoredSnapshot::into_snapshot).transpose()
}

/// Returns true for an INSERT, false only if all fifteen source values already match.
/// Credentials remain opaque source bytes. This does not activate them or authenticate.
pub async fn insert_snapshot(pool: &PgPool, snapshot: &LegacyUserSnapshot) -> Result<bool, Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL lock_timeout='3s'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout='5s'")
        .execute(&mut *tx)
        .await?;
    let result = insert_in_transaction(&mut tx, snapshot).await;
    match result {
        Ok(inserted) => {
            tx.commit().await?;
            Ok(inserted)
        }
        Err(error) => {
            tx.rollback().await?;
            Err(error)
        }
    }
}

async fn insert_in_transaction(
    connection: &mut PgConnection,
    snapshot: &LegacyUserSnapshot,
) -> Result<bool, Error> {
    let name_key = snapshot.validate()?;
    let details = &snapshot.details;
    let inserted: Option<i32> = sqlx::query_scalar(&format!(
        "INSERT INTO game_users ({COLUMNS}) VALUES
         ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)
         ON CONFLICT DO NOTHING RETURNING id"
    ))
    .bind(snapshot.user.id)
    .bind(&snapshot.user.name)
    .bind(snapshot.user.next_character_sn)
    .bind(snapshot.user.service_id)
    .bind(snapshot.user.channeling_code)
    .bind(details.last_login_ip.as_ref().map(LegacyBytes::as_bytes))
    .bind(details.ip_lock_time.map(SqlDateTime2::ticks))
    .bind(details.facebook_token.as_ref().map(LegacyBytes::as_bytes))
    .bind(details.is_dormant_user_event.map(SqlDateTime::units300))
    .bind(details.second_password.as_ref().map(LegacyBytes::as_bytes))
    .bind(details.second_password_fail_count)
    .bind(details.second_password_last_fail.map(SqlDateTime2::ticks))
    .bind(details.last_update.as_ref().map(SourceRowVersion::as_bytes))
    .bind(details.is_dormant_user.map(SqlDateTime::units300))
    .bind(details.create_time.map(SqlDateTime::units300))
    .fetch_optional(&mut *connection)
    .await?;
    if inserted.is_some() {
        return Ok(true);
    }
    let existing = sqlx::query_as::<_, StoredSnapshot>(&format!(
        "SELECT {COLUMNS} FROM game_users WHERE id=$1 OR name_key=$2 ORDER BY id FOR SHARE"
    ))
    .bind(snapshot.user.id)
    .bind(name_key.as_str())
    .fetch_all(&mut *connection)
    .await?;
    if existing.len() != 1 {
        return Err(Error::LegacyUserSnapshotConflict);
    }
    for row in existing {
        if row.into_snapshot()? != *snapshot {
            return Err(Error::LegacyUserSnapshotConflict);
        }
    }
    Ok(false)
}

/// All rows are locked and compared in full before committing. No UPDATE/delete.
pub(crate) async fn execute_batch(
    pool: &PgPool,
    snapshots: &[&LegacyUserSnapshot],
    verify_only: bool,
) -> Result<usize, Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL lock_timeout='3s'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout='5s'")
        .execute(&mut *tx)
        .await?;
    let result = async {
        let mut inserted = 0;
        if !verify_only {
            for snapshot in snapshots {
                inserted += usize::from(insert_in_transaction(&mut tx, snapshot).await?);
            }
        }
        for snapshot in snapshots {
            let row = sqlx::query_as::<_, StoredSnapshot>(&format!(
                "SELECT {COLUMNS} FROM game_users WHERE id=$1 FOR SHARE"
            ))
            .bind(snapshot.user.id)
            .fetch_optional(&mut *tx)
            .await?;
            if !row
                .map(StoredSnapshot::into_snapshot)
                .transpose()?
                .is_some_and(|persisted| persisted == **snapshot)
            {
                return Err(Error::LegacyUserSnapshotConflict);
            }
        }
        Ok(inserted)
    }
    .await;
    match result {
        Ok(summary) => {
            tx.commit().await?;
            Ok(summary)
        }
        Err(error) => {
            tx.rollback().await?;
            Err(error)
        }
    }
}
