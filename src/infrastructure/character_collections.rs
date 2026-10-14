//! Insert-only creation owner preservation. No character creation or playable list.
use super::{
    legacy_users::SourceBytes,
    user_export::{PrivateText, hex, integer},
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::{collections::HashSet, io::Read, time::Duration};
use thiserror::Error;
use zeroize::Zeroizing;

const MAX_ROWS: usize = 1000;
const MAX_BYTES: u64 = 2 * 1024 * 1024;
#[derive(Debug, Error)]
pub enum CollectionError {
    #[error("creation collection database operation failed")]
    Database(#[from] sqlx::Error),
    #[error("creation collection export IO failed")]
    Io(#[from] std::io::Error),
    #[error("invalid bounded creation collection snapshot")]
    Export,
    #[error("creation collection preservation conflict")]
    Conflict,
    #[error("creation collection deadline exceeded")]
    Timeout,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Export {
    format_version: u32,
    source: String,
    item_collections: Vec<OwnerRow>,
    quest_collections: Vec<OwnerRow>,
    mailboxes: Vec<OwnerRow>,
    story_entries: Vec<StoryRow>,
    settings: Vec<SettingRow>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnerRow {
    owner_id: PrivateText,
    permission: i32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StoryRow {
    cid: PrivateText,
    permission: i32,
    hint_category: u8,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SettingRow {
    code_hex: PrivateText,
    value_hex: PrivateText,
}
#[derive(PartialEq, Eq)]
struct Owner {
    id: i64,
    permission: i32,
}
#[derive(PartialEq, Eq)]
struct Story {
    id: i64,
    permission: i32,
    hint_category: u8,
}
type Setting = (Zeroizing<Vec<u8>>, Zeroizing<Vec<u8>>);
pub struct Snapshot {
    owners: [Vec<Owner>; 3],
    stories: Vec<Story>,
    settings: Vec<Setting>,
}
fn unique(ids: impl Iterator<Item = i64>) -> bool {
    let mut seen = HashSet::new();
    ids.into_iter().all(|id| seen.insert(id))
}
pub fn read_snapshot(input: impl Read) -> Result<Snapshot, CollectionError> {
    let mut bytes = Zeroizing::new(Vec::new());
    input.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(CollectionError::Export);
    }
    let e: Export = serde_json::from_slice(&bytes).map_err(|_| CollectionError::Export)?;
    if e.format_version != 1
        || e.source != "study_heroes.creation-collections"
        || [
            e.item_collections.len(),
            e.quest_collections.len(),
            e.mailboxes.len(),
            e.story_entries.len(),
            e.settings.len(),
        ]
        .into_iter()
        .any(|n| n > MAX_ROWS)
    {
        return Err(CollectionError::Export);
    }
    let mut owners: [Vec<Owner>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    for (target, rows) in
        owners
            .iter_mut()
            .zip([e.item_collections, e.quest_collections, e.mailboxes])
    {
        for row in rows {
            target.push(Owner {
                id: integer(row.owner_id).map_err(|_| CollectionError::Export)?,
                permission: row.permission,
            });
        }
        if !unique(target.iter().map(|r| r.id)) {
            return Err(CollectionError::Export);
        }
        target.sort_by_key(|r| r.id);
    }
    let mut stories = Vec::new();
    for row in e.story_entries {
        stories.push(Story {
            id: integer(row.cid).map_err(|_| CollectionError::Export)?,
            permission: row.permission,
            hint_category: row.hint_category,
        });
    }
    if !unique(stories.iter().map(|r| r.id)) {
        return Err(CollectionError::Export);
    }
    stories.sort_by_key(|r| r.id);
    let mut settings = Vec::new();
    for row in e.settings {
        settings.push((
            Zeroizing::new(hex(row.code_hex, 255).map_err(|_| CollectionError::Export)?),
            Zeroizing::new(hex(row.value_hex, 255).map_err(|_| CollectionError::Export)?),
        ));
    }
    // Sort by raw bytes, retaining duplicates. Do not impose invented source uniqueness.
    settings.sort_by(|a, b| {
        a.0.as_slice()
            .cmp(b.0.as_slice())
            .then_with(|| a.1.as_slice().cmp(b.1.as_slice()))
    });
    Ok(Snapshot {
        owners,
        stories,
        settings,
    })
}
#[derive(Serialize)]
pub struct ImportSummary {
    pub source_rows: [usize; 5],
    pub inserted: [usize; 5],
    pub existing: [usize; 5],
    pub verified: [usize; 5],
    pub character_creation_available: bool,
}
pub async fn preserve(
    pool: &PgPool,
    s: &Snapshot,
    verify: bool,
) -> Result<ImportSummary, CollectionError> {
    tokio::time::timeout(Duration::from_secs(10), async {
        let mut tx = pool.begin().await?;
        sqlx::query("SET LOCAL lock_timeout='2s'").execute(&mut *tx).await?;
        sqlx::query("SET LOCAL statement_timeout='5s'").execute(&mut *tx).await?;
        // No key in the source settings table: serialize whole-multiset comparison/import.
        sqlx::query("LOCK TABLE game_my_settings IN SHARE ROW EXCLUSIVE MODE").execute(&mut *tx).await?;
        let source_rows = [s.owners[0].len(), s.owners[1].len(), s.owners[2].len(), s.stories.len(), s.settings.len()];
        let mut inserted = [0usize; 5];
        let statements = [
            ("INSERT INTO game_item_collections(owner_id,permission) VALUES($1,$2) ON CONFLICT DO NOTHING", "SELECT permission FROM game_item_collections WHERE owner_id=$1 FOR SHARE"),
            ("INSERT INTO game_quest_collections(owner_id,permission) VALUES($1,$2) ON CONFLICT DO NOTHING", "SELECT permission FROM game_quest_collections WHERE owner_id=$1 FOR SHARE"),
            ("INSERT INTO game_mailboxes(owner_id,permission) VALUES($1,$2) ON CONFLICT DO NOTHING", "SELECT permission FROM game_mailboxes WHERE owner_id=$1 FOR SHARE"),
        ];
        for (index, (insert, select)) in statements.into_iter().enumerate() {
            for row in &s.owners[index] {
                if !verify { inserted[index] += sqlx::query(insert).bind(row.id).bind(row.permission).execute(&mut *tx).await?.rows_affected() as usize; }
                let value: Option<i32> = sqlx::query_scalar(select).bind(row.id).fetch_optional(&mut *tx).await?;
                if value != Some(row.permission) { return Err(CollectionError::Conflict); }
            }
        }
        for row in &s.stories {
            if !verify { inserted[3] += sqlx::query("INSERT INTO game_story_entries(cid,permission,hint_category) VALUES($1,$2,$3) ON CONFLICT DO NOTHING").bind(row.id).bind(row.permission).bind(i16::from(row.hint_category)).execute(&mut *tx).await?.rows_affected() as usize; }
            let value: Option<(i32,i16)> = sqlx::query_as("SELECT permission,hint_category FROM game_story_entries WHERE cid=$1 FOR SHARE").bind(row.id).fetch_optional(&mut *tx).await?;
            if value != Some((row.permission,i16::from(row.hint_category))) { return Err(CollectionError::Conflict); }
        }
        let stored: Vec<(SourceBytes,SourceBytes)> = sqlx::query_as("SELECT code_bytes,value_bytes FROM game_my_settings LIMIT 1001").fetch_all(&mut *tx).await?;
        if stored.len() > MAX_ROWS { return Err(CollectionError::Export); }
        let mut current: Vec<Setting> = stored.into_iter().map(|(c,v)| (Zeroizing::new(c.into_bytes()),Zeroizing::new(v.into_bytes()))).collect();
        current.sort_by(|a,b| a.0.as_slice().cmp(b.0.as_slice()).then_with(|| a.1.as_slice().cmp(b.1.as_slice())));
        if current.is_empty() && !verify {
            for (code,value) in &s.settings {
                inserted[4] += sqlx::query("INSERT INTO game_my_settings(code_bytes,value_bytes) VALUES($1,$2)").bind(code.as_slice()).bind(value.as_slice()).execute(&mut *tx).await?.rows_affected() as usize;
            }
        } else if current != s.settings { return Err(CollectionError::Conflict); }
        tx.commit().await?;
        Ok(ImportSummary { source_rows, inserted, existing: std::array::from_fn(|i| source_rows[i]-inserted[i]), verified: source_rows, character_creation_available: false })
    }).await.map_err(|_| CollectionError::Timeout)?
}

/// Check every imported column without reading private rows.
pub async fn check_ready(pool: &PgPool) -> Result<(), CollectionError> {
    for statement in [
        "SELECT owner_id,permission FROM game_item_collections LIMIT 0",
        "SELECT owner_id,permission FROM game_quest_collections LIMIT 0",
        "SELECT owner_id,permission FROM game_mailboxes LIMIT 0",
        "SELECT cid,permission,hint_category FROM game_story_entries LIMIT 0",
        "SELECT code_bytes,value_bytes FROM game_my_settings LIMIT 0",
    ] {
        sqlx::query(statement).execute(pool).await?;
    }
    Ok(())
}
