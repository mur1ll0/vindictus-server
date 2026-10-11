//! Game-user storage use case, intentionally disconnected from frontend authentication.
use crate::{
    domain::game_user::{NewGameUser, ProvisionedGameUser},
    error::Error,
    infrastructure::game_users,
};
use sqlx::PgPool;

/// Provision only the initial recovered user subset. Caller-provided ID is a legacy signed
/// NexonSN; this function neither derives it nor grants credentials, permissions or a session.
pub async fn provision_game_user(
    pool: &PgPool,
    id: i32,
    name: &str,
    service_id: i32,
    channel_code: u8,
) -> Result<ProvisionedGameUser, Error> {
    let candidate = NewGameUser::new(id, name, service_id, channel_code)?;
    game_users::insert_or_get(pool, &candidate).await
}
