//! Friend lists of the laboratory messenger (B5 MS3b; docs/messenger-design.md): categories and
//! friendships per account (NexonSN), migration 0069. One row per direction; a request writes
//! Requested for the sender and Waiting for the receiver, an accepted request makes both Friends,
//! a block keeps only the blocker's row. The wire objects are built by `messenger`.
use sqlx::PgPool;
use thiserror::Error;

/// Friends (any state) per account.
pub const MAX_FRIENDS: i64 = 200;
/// Categories per account (besides the default category 0).
pub const MAX_CATEGORIES: i64 = 20;
/// Category name length (CNMCategoryInfo wstr ≤ 0x20).
pub const CATEGORY_NAME: usize = 0x20;

#[derive(Debug, Error)]
pub enum FriendError {
    #[error("messenger database error")]
    Database(#[from] sqlx::Error),
    #[error("messenger request refused")]
    Refused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Requested = 0,
    Waiting = 1,
    Friend = 2,
    Blocked = 3,
}
impl State {
    fn from_db(v: i16) -> Option<Self> {
        Some(match v {
            0 => Self::Requested,
            1 => Self::Waiting,
            2 => Self::Friend,
            3 => Self::Blocked,
            _ => return None,
        })
    }
}

/// An account as the messenger shows it: login id and the character it plays as.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    pub nexon_sn: i32,
    pub login: String,
    pub character: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Category {
    pub id: i32,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Friend {
    pub profile: Profile,
    pub state: State,
    pub category_id: i32,
    pub memo: String,
}

fn utf16_name(raw: Option<Vec<u8>>) -> String {
    raw.and_then(|raw| {
        let (pairs, rest) = raw.as_chunks::<2>();
        rest.is_empty()
            .then(|| {
                String::from_utf16(
                    &pairs
                        .iter()
                        .map(|c| u16::from_le_bytes(*c))
                        .collect::<Vec<_>>(),
                )
                .ok()
            })
            .flatten()
    })
    .unwrap_or_default()
}

/// The display character: the last one logged in, else the first living one.
const PROFILE_SQL: &str = "SELECT u.id,u.name,(SELECT c.name_utf16 FROM game_character_info c \
     WHERE c.uid=u.id AND c.status<>1 ORDER BY (c.id=i.last_login_cid) DESC NULLS LAST,c.id LIMIT 1) \
     FROM game_users u LEFT JOIN game_user_info i ON i.nexon_sn=u.id WHERE u.id=ANY($1)";

/// Profiles of accounts (missing accounts are left out).
pub async fn profiles(pool: &PgPool, ids: &[i32]) -> Result<Vec<Profile>, FriendError> {
    let rows: Vec<(i32, String, Option<Vec<u8>>)> = sqlx::query_as(PROFILE_SQL)
        .bind(ids)
        .fetch_all(pool)
        .await?;
    Ok(rows
        .into_iter()
        .map(|(nexon_sn, login, name)| Profile {
            nexon_sn,
            login: login.trim_end().to_owned(),
            character: utf16_name(name),
        })
        .collect())
}

pub async fn profile(pool: &PgPool, nexon_sn: i32) -> Result<Option<Profile>, FriendError> {
    Ok(profiles(pool, &[nexon_sn]).await?.into_iter().next())
}

/// The account a friend request names: a living character's name, else a login id.
pub async fn find(pool: &PgPool, name: &str) -> Result<Option<i32>, FriendError> {
    if name.is_empty() || name.chars().count() > 0x40 {
        return Ok(None);
    }
    if let Ok(Some((_, uid))) = super::minimum_party::character_info_by_name(pool, name).await {
        return Ok(Some(uid));
    }
    Ok(
        sqlx::query_scalar("SELECT id FROM game_users WHERE name_key=lower(rtrim($1,' '))")
            .bind(name)
            .fetch_optional(pool)
            .await?,
    )
}

pub async fn categories(pool: &PgPool, nexon_sn: i32) -> Result<Vec<Category>, FriendError> {
    let rows: Vec<(i32, String)> = sqlx::query_as(
        "SELECT category_id,name FROM game_messenger_categories WHERE nexon_sn=$1 ORDER BY category_id LIMIT 100",
    )
    .bind(nexon_sn)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(id, name)| Category { id, name })
        .collect())
}

pub async fn friends(pool: &PgPool, nexon_sn: i32) -> Result<Vec<Friend>, FriendError> {
    let rows: Vec<(i32, i16, i32, Option<String>)> = sqlx::query_as(
        "SELECT friend_sn,state,category_id,memo FROM game_messenger_friends WHERE nexon_sn=$1 \
         ORDER BY created_at,friend_sn LIMIT 1000",
    )
    .bind(nexon_sn)
    .fetch_all(pool)
    .await?;
    let ids: Vec<i32> = rows.iter().map(|r| r.0).collect();
    let profiles = profiles(pool, &ids).await?;
    Ok(rows
        .into_iter()
        .filter_map(|(sn, state, category_id, memo)| {
            Some(Friend {
                profile: profiles.iter().find(|p| p.nexon_sn == sn)?.clone(),
                state: State::from_db(state)?,
                category_id,
                memo: memo.unwrap_or_default(),
            })
        })
        .collect())
}

/// The state `owner` keeps for `other`, if any.
pub async fn state(pool: &PgPool, owner: i32, other: i32) -> Result<Option<State>, FriendError> {
    let v: Option<i16> = sqlx::query_scalar(
        "SELECT state FROM game_messenger_friends WHERE nexon_sn=$1 AND friend_sn=$2",
    )
    .bind(owner)
    .bind(other)
    .fetch_optional(pool)
    .await?;
    Ok(v.and_then(State::from_db))
}

/// What a request did.
#[derive(Debug, PartialEq, Eq)]
pub enum Requested {
    /// The receiver now has a request waiting.
    Sent,
    /// The receiver had already asked: both are friends now.
    Accepted,
}

/// RequestNewFriend: refused for oneself, a full list, an existing friendship or request, or a
/// receiver who blocked the sender (the sender is not told which).
pub async fn request(pool: &PgPool, from: i32, to: i32) -> Result<Requested, FriendError> {
    if from == to {
        return Err(FriendError::Refused);
    }
    let mut tx = pool.begin().await?;
    // Both accounts' rows in a fixed order, so crossing requests do not deadlock.
    sqlx::query("SELECT id FROM game_users WHERE id=ANY($1) ORDER BY id FOR UPDATE")
        .bind([from.min(to), from.max(to)])
        .fetch_all(&mut *tx)
        .await?;
    let rows: Vec<(i32, i32, i16)> = sqlx::query_as(
        "SELECT nexon_sn,friend_sn,state FROM game_messenger_friends \
         WHERE (nexon_sn=$1 AND friend_sn=$2) OR (nexon_sn=$2 AND friend_sn=$1)",
    )
    .bind(from)
    .bind(to)
    .fetch_all(&mut *tx)
    .await?;
    let mine = rows
        .iter()
        .find(|r| r.0 == from)
        .and_then(|r| State::from_db(r.2));
    let theirs = rows
        .iter()
        .find(|r| r.0 == to)
        .and_then(|r| State::from_db(r.2));
    let outcome = match (mine, theirs) {
        (_, Some(State::Blocked)) | (Some(State::Friend | State::Requested), _) => {
            return Err(FriendError::Refused);
        }
        (Some(State::Waiting), _) => {
            sqlx::query(
                "UPDATE game_messenger_friends SET state=2 \
                 WHERE (nexon_sn=$1 AND friend_sn=$2) OR (nexon_sn=$2 AND friend_sn=$1)",
            )
            .bind(from)
            .bind(to)
            .execute(&mut *tx)
            .await?;
            Requested::Accepted
        }
        (None | Some(State::Blocked), _) => {
            let count: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM game_messenger_friends WHERE nexon_sn=ANY($1) GROUP BY nexon_sn \
                 ORDER BY 1 DESC LIMIT 1",
            )
            .bind([from, to])
            .fetch_optional(&mut *tx)
            .await?
            .unwrap_or(0);
            if count >= MAX_FRIENDS {
                return Err(FriendError::Refused);
            }
            sqlx::query(
                "INSERT INTO game_messenger_friends(nexon_sn,friend_sn,state) VALUES($1,$2,0),($2,$1,1) \
                 ON CONFLICT(nexon_sn,friend_sn) DO UPDATE SET state=EXCLUDED.state,category_id=0",
            )
            .bind(from)
            .bind(to)
            .execute(&mut *tx)
            .await?;
            Requested::Sent
        }
    };
    tx.commit().await?;
    Ok(outcome)
}

/// ConfirmNewFriend: `owner` answers the request `other` sent.
pub async fn confirm(
    pool: &PgPool,
    owner: i32,
    other: i32,
    accept: bool,
) -> Result<(), FriendError> {
    let mut tx = pool.begin().await?;
    let waiting: Option<i16> = sqlx::query_scalar(
        "SELECT state FROM game_messenger_friends WHERE nexon_sn=$1 AND friend_sn=$2 FOR UPDATE",
    )
    .bind(owner)
    .bind(other)
    .fetch_optional(&mut *tx)
    .await?;
    if waiting != Some(State::Waiting as i16) {
        return Err(FriendError::Refused);
    }
    let sql = if accept {
        "UPDATE game_messenger_friends SET state=2 \
         WHERE ((nexon_sn=$1 AND friend_sn=$2) OR (nexon_sn=$2 AND friend_sn=$1)) AND state<>3"
    } else {
        "DELETE FROM game_messenger_friends \
         WHERE ((nexon_sn=$1 AND friend_sn=$2) OR (nexon_sn=$2 AND friend_sn=$1)) AND state<>3"
    };
    sqlx::query(sql)
        .bind(owner)
        .bind(other)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

/// Removes a friendship or request in both directions (a block of the other side stays).
pub async fn remove(pool: &PgPool, owner: i32, other: i32) -> Result<bool, FriendError> {
    let mut tx = pool.begin().await?;
    let mine = sqlx::query("DELETE FROM game_messenger_friends WHERE nexon_sn=$1 AND friend_sn=$2")
        .bind(owner)
        .bind(other)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    sqlx::query(
        "DELETE FROM game_messenger_friends WHERE nexon_sn=$2 AND friend_sn=$1 AND state<>3",
    )
    .bind(owner)
    .bind(other)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(mine > 0)
}

/// BlockFriend: `block` keeps only the blocker's Blocked row; unblocking removes it.
pub async fn block(pool: &PgPool, owner: i32, other: i32, block: bool) -> Result<(), FriendError> {
    if owner == other {
        return Err(FriendError::Refused);
    }
    let mut tx = pool.begin().await?;
    if block {
        sqlx::query(
            "INSERT INTO game_messenger_friends(nexon_sn,friend_sn,state) VALUES($1,$2,3) \
             ON CONFLICT(nexon_sn,friend_sn) DO UPDATE SET state=3",
        )
        .bind(owner)
        .bind(other)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "DELETE FROM game_messenger_friends WHERE nexon_sn=$2 AND friend_sn=$1 AND state<>3",
        )
        .bind(owner)
        .bind(other)
        .execute(&mut *tx)
        .await?;
    } else {
        sqlx::query(
            "DELETE FROM game_messenger_friends WHERE nexon_sn=$1 AND friend_sn=$2 AND state=3",
        )
        .bind(owner)
        .bind(other)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

fn category_name(name: &str) -> Result<String, FriendError> {
    let name: String = name.trim().chars().take(CATEGORY_NAME).collect();
    if name.is_empty() {
        return Err(FriendError::Refused);
    }
    Ok(name)
}

/// AddCategory: the new category's id.
pub async fn add_category(pool: &PgPool, owner: i32, name: &str) -> Result<i32, FriendError> {
    let name = category_name(name)?;
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT id FROM game_users WHERE id=$1 FOR UPDATE")
        .bind(owner)
        .execute(&mut *tx)
        .await?;
    let (count, next): (i64, i32) = sqlx::query_as(
        "SELECT count(*),COALESCE(max(category_id),0)+1 FROM game_messenger_categories WHERE nexon_sn=$1",
    )
    .bind(owner)
    .fetch_one(&mut *tx)
    .await?;
    if count >= MAX_CATEGORIES || next > 1000 {
        return Err(FriendError::Refused);
    }
    sqlx::query(
        "INSERT INTO game_messenger_categories(nexon_sn,category_id,name) VALUES($1,$2,$3)",
    )
    .bind(owner)
    .bind(next)
    .bind(&name)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(next)
}

/// DeleteCategory: its friends go back to the default category.
pub async fn delete_category(pool: &PgPool, owner: i32, category: i32) -> Result<(), FriendError> {
    let mut tx = pool.begin().await?;
    let gone =
        sqlx::query("DELETE FROM game_messenger_categories WHERE nexon_sn=$1 AND category_id=$2")
            .bind(owner)
            .bind(category)
            .execute(&mut *tx)
            .await?
            .rows_affected();
    if gone == 0 {
        return Err(FriendError::Refused);
    }
    sqlx::query(
        "UPDATE game_messenger_friends SET category_id=0 WHERE nexon_sn=$1 AND category_id=$2",
    )
    .bind(owner)
    .bind(category)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn rename_category(
    pool: &PgPool,
    owner: i32,
    category: i32,
    name: &str,
) -> Result<(), FriendError> {
    let name = category_name(name)?;
    let changed = sqlx::query(
        "UPDATE game_messenger_categories SET name=$3 WHERE nexon_sn=$1 AND category_id=$2",
    )
    .bind(owner)
    .bind(category)
    .bind(&name)
    .execute(pool)
    .await?
    .rows_affected();
    if changed == 0 {
        return Err(FriendError::Refused);
    }
    Ok(())
}

/// Add/Move/DeleteFriendFromCategory: a friend belongs to one category (0 = default).
pub async fn move_friend(
    pool: &PgPool,
    owner: i32,
    friend: i32,
    category: i32,
) -> Result<(), FriendError> {
    let changed = sqlx::query(
        "UPDATE game_messenger_friends f SET category_id=$3 WHERE f.nexon_sn=$1 AND f.friend_sn=$2 \
         AND ($3=0 OR EXISTS(SELECT 1 FROM game_messenger_categories c WHERE c.nexon_sn=$1 AND c.category_id=$3))",
    )
    .bind(owner)
    .bind(friend)
    .bind(category)
    .execute(pool)
    .await?
    .rows_affected();
    if changed == 0 {
        return Err(FriendError::Refused);
    }
    Ok(())
}
