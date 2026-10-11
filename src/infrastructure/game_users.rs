//! Insert-only provisioning for game users. No authentication or session effects.
use crate::{
    domain::game_user::{GameUser, NewGameUser, ProvisionedGameUser},
    error::Error,
};
use sqlx::PgPool;

#[derive(sqlx::FromRow)]
struct StoredUser {
    id: i32,
    name: String,
    name_key: String,
    next_character_sn: i32,
    service_id: i32,
    channeling_code: Option<i16>,
}

impl StoredUser {
    fn into_user(self) -> GameUser {
        GameUser {
            id: self.id,
            name: self.name,
            next_character_sn: self.next_character_sn,
            service_id: self.service_id,
            channeling_code: self.channeling_code,
        }
    }
}

/// Create a user, or return an unchanged existing row only when BOTH identity keys match.
/// Name/ID collisions fail closed. This is not evidence that two passports are authentic.
pub async fn insert_or_get(
    pool: &PgPool,
    candidate: &NewGameUser<'_>,
) -> Result<ProvisionedGameUser, Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL lock_timeout = '3s'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout = '5s'")
        .execute(&mut *tx)
        .await?;
    let inserted = sqlx::query_as::<_, StoredUser>(
        "INSERT INTO game_users (id, name, next_character_sn, service_id, channeling_code)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT DO NOTHING
         RETURNING id, name, name_key, next_character_sn, service_id, channeling_code",
    )
    .bind(candidate.id())
    .bind(candidate.name())
    .bind(candidate.next_character_sn())
    .bind(candidate.service_id())
    .bind(candidate.channeling_code().map(i16::from))
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(user) = inserted {
        tx.commit().await?;
        return Ok(ProvisionedGameUser {
            user: user.into_user(),
            created: true,
        });
    }
    // A separate Read Committed statement sees a concurrent INSERT after it commits.
    // A single INSERT/SELECT CTE can miss that row because its snapshot predates the wait.
    let existing = sqlx::query_as::<_, StoredUser>(
        "SELECT id, name, name_key, next_character_sn, service_id, channeling_code
         FROM game_users WHERE id = $1 OR name_key = $2 ORDER BY id FOR SHARE",
    )
    .bind(candidate.id())
    .bind(candidate.name_key())
    .fetch_all(&mut *tx)
    .await?;
    let result = if let Some(user) = existing.iter().find(|user| user.id == candidate.id()) {
        if user.name_key != candidate.name_key() {
            Err(Error::GameUserIdConflict)
        } else {
            Ok(())
        }
    } else if existing
        .iter()
        .any(|user| user.name_key == candidate.name_key())
    {
        Err(Error::GameUserNameConflict)
    } else {
        // Concurrent delete/rename: no silent retry or ambiguous rebinding.
        Err(Error::GameUserConcurrentChange)
    };
    if let Err(error) = result {
        tx.rollback().await?;
        return Err(error);
    }
    let user = existing
        .into_iter()
        .find(|user| user.id == candidate.id() && user.name_key == candidate.name_key())
        .ok_or(Error::GameUserConcurrentChange)?;
    tx.commit().await?;
    Ok(ProvisionedGameUser {
        user: user.into_user(),
        created: false,
    })
}
