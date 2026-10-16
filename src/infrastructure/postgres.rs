use crate::{config::Config, domain::Username, error::Error};
use sqlx::{PgPool, migrate::Migrator, postgres::PgPoolOptions};
use std::time::Duration;
use uuid::Uuid;

pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

pub fn pool(config: &Config) -> Result<PgPool, Error> {
    PgPoolOptions::new()
        .max_connections(config.db_max_connections)
        .acquire_timeout(Duration::from_secs(3))
        .connect_lazy(&config.database_url)
        .map_err(|_| Error::Configuration("invalid PostgreSQL connection configuration".into()))
}

pub async fn migrate(pool: &PgPool) -> Result<(), Error> {
    MIGRATOR.run(pool).await?;
    Ok(())
}

/// Check connection, migration versions/checksums and columns required by this binary.
pub async fn check_ready(pool: &PgPool) -> Result<(), Error> {
    sqlx::query("SELECT 1").execute(pool).await?;
    let applied: Vec<(i64, bool, Vec<u8>)> =
        sqlx::query_as("SELECT version, success, checksum FROM _sqlx_migrations ORDER BY version")
            .fetch_all(pool)
            .await?;
    let expected: Vec<_> = MIGRATOR
        .iter()
        .filter(|m| !m.migration_type.is_down_migration())
        .collect();
    if applied.len() != expected.len()
        || expected.iter().any(|m| {
            !applied.iter().any(|(version, success, checksum)| {
                *version == m.version && *success && checksum.as_slice() == m.checksum.as_ref()
            })
        })
    {
        return Err(Error::SchemaNotReady);
    }
    sqlx::query(
        "SELECT id, username, normalized_name, password_hash, created_at FROM accounts LIMIT 0",
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "SELECT id, name, name_key, next_character_sn, service_id, channeling_code, persisted_at,
         last_login_ip_utf16, ip_lock_time_ticks, facebook_token_bytes,
         is_dormant_user_event_units300, second_password_utf16, second_password_fail_count,
         second_password_last_fail_ticks, legacy_last_update, is_dormant_user_units300,
         create_time_units300,local_revision
         FROM game_users LIMIT 0",
    )
    .execute(pool)
    .await?;
    sqlx::query("SELECT name,name_key,status,expire_time_units300,reason_utf16,source_rowversion FROM game_user_bans LIMIT 0")
        .execute(pool).await?;
    sqlx::query("SELECT local_id,nexon_id_utf16,name_key FROM game_limited_accounts LIMIT 0")
        .execute(pool)
        .await?;
    sqlx::query("SELECT recommender_uid,recommender_cid,friend_cid,recommend_flag,update_time_units300 FROM game_friend_loyalty LIMIT 0")
        .execute(pool).await?;
    sqlx::query(
        "SELECT id,uid,name_utf16,achievement_utf16,name_key FROM game_user_achievements LIMIT 0",
    )
    .execute(pool)
    .await?;
    sqlx::query("SELECT operation_id,uid,name_key,request,revision_after,is_dormant,is_return FROM game_login_account_effect_runs LIMIT 0")
        .execute(pool).await?;
    sqlx::query("SELECT operation_id,uid,name_key,request,revision_after,is_dormant,is_return,queued_row_ids FROM game_login_reward_runs LIMIT 0")
        .execute(pool).await?;
    let revision_trigger: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM pg_trigger
         WHERE tgrelid='game_users'::regclass AND tgname='game_user_revision'
         AND tgenabled='O' AND NOT tgisinternal)",
    )
    .fetch_one(pool)
    .await?;
    if !revision_trigger {
        return Err(Error::SchemaNotReady);
    }
    crate::infrastructure::characters::check_ready(pool).await?;
    crate::infrastructure::character_lobby::check_ready(pool).await?;
    crate::infrastructure::home_cafe_bonus::check_ready(pool).await?;
    crate::infrastructure::character_reservations::check_ready(pool).await?;
    crate::infrastructure::character_collections::check_ready(pool).await?;
    crate::infrastructure::character_costumes::check_ready(pool).await?;
    crate::infrastructure::character_skills::check_ready(pool).await?;
    crate::infrastructure::fiona_character_state::check_ready(pool).await?;
    crate::infrastructure::item_state::check_ready(pool).await?;
    crate::infrastructure::character_creation_ledger::check_ready(pool).await?;
    sqlx::query(
        "SELECT uid,name_key,profile_fingerprint,created_at FROM game_lab_account_origins LIMIT 0",
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn insert_account(
    pool: &PgPool,
    username: &Username,
    password_hash: &str,
) -> Result<Uuid, Error> {
    let id = Uuid::new_v4();
    let result = sqlx::query(
        "INSERT INTO accounts (id, username, normalized_name, password_hash) VALUES ($1, $2, $3, $4)",
    ).bind(id).bind(username.display()).bind(username.normalized()).bind(password_hash)
        .execute(pool).await;
    match result {
        Ok(_) => Ok(id),
        Err(sqlx::Error::Database(error)) if error.is_unique_violation() => {
            Err(Error::AccountExists)
        }
        Err(error) => Err(error.into()),
    }
}
