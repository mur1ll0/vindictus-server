//! DyeAmpleSession of the selected character: the empty ampoule branch of
//! UseInventoryItemProcessor (opens the session, DyeAmpleUsedMessage) and
//! RequestDyeAmpleProcessor (MakeColorAmple, Save, DyeAmpleCompleteMessage, then the
//! inventory Sync). The session's SimpleLog records (cursor candidates, seed, coordinate,
//! Selected, SaveSuccess) are not written; the DestroyItem/GiveItem ledgers are.
use super::{
    character_lobby::UserKey, item_catalog, minimum_assets::Assets, minimum_character::MinimumError,
};
use crate::{
    domain::{
        dye_ample::{self, Cursor, DistortTable, Session},
        item_colors::ColorRandom,
        item_logs::Data,
        map_partition::OutgoingPacket,
    },
    protocol::{
        DEFAULT_MAX_BODY,
        dye::{self as dye_wire, Kind as DyeKind, Message as DyeMessage},
        inventory::{self as wire, Kind, Message},
    },
};
use rand_core::{OsRng, RngCore};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::io::Read;

/// ItemLogType.DestroyItem/GiveItem and ActionType.DyeCash of MakeColorAmple's ledgers.
const LEDGER_DESTROY_ITEM: i16 = 3;
const LEDGER_GIVE_ITEM: i16 = 2;
const ACTION_DYE_CASH: i16 = 21;

fn fail() -> MinimumError {
    MinimumError::Conflict
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDisplace {
    raw_hex: String,
    sha256: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    format_version: u32,
    source: String,
    database_sha256: String,
    config_sha256: String,
    region: String,
    cursor: Option<String>,
    displace: Option<RawDisplace>,
}

/// The dye sidecar (`scripts/content/export_dye_ample.py`) before its catalog binding.
pub struct Profile {
    database_sha256: String,
    config_sha256: String,
    region: String,
    contents: Contents,
}
/// FeatureMatrix DyeCashCursor (None when the region has no value) and DistortTable.
pub struct Contents {
    cursor: Option<Cursor>,
    distort: DistortTable,
}

fn decode_hex(text: &str) -> Option<Vec<u8>> {
    if text.len() > 8192 || !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
        .collect()
}

impl Profile {
    pub fn read(reader: impl Read) -> Result<Self, MinimumError> {
        let mut bytes = Vec::new();
        reader
            .take(64 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| fail())?;
        if bytes.len() > 64 * 1024 {
            return Err(fail());
        }
        let raw: Raw = serde_json::from_slice(&bytes).map_err(|_| fail())?;
        let hash = |s: &str| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit());
        if raw.format_version != 1
            || raw.source != "heroesContents.dye-ample"
            || !hash(&raw.database_sha256)
            || !hash(&raw.config_sha256)
            || raw.cursor.as_ref().is_some_and(|c| c.len() > 64)
        {
            return Err(fail());
        }
        let distort = match raw.displace {
            // DistortTable's static constructor caught a failed load: an empty table.
            None => DistortTable::empty(),
            Some(d) => {
                let data = decode_hex(&d.raw_hex).ok_or_else(fail)?;
                if !hash(&d.sha256)
                    || !format!("{:x}", Sha256::digest(&data)).eq_ignore_ascii_case(&d.sha256)
                {
                    return Err(fail());
                }
                DistortTable::from_bytes(&data)
            }
        };
        Ok(Self {
            database_sha256: raw.database_sha256,
            config_sha256: raw.config_sha256,
            region: raw.region,
            contents: Contents {
                cursor: raw.cursor.as_deref().map(Cursor::parse),
                distort,
            },
        })
    }
    pub fn bind(self, catalog: &item_catalog::Package) -> Result<Contents, MinimumError> {
        let m = &catalog.manifest;
        if !self
            .database_sha256
            .eq_ignore_ascii_case(&m.database_sha256)
            || !self.config_sha256.eq_ignore_ascii_case(&m.config_sha256)
            || self.region != m.region
        {
            return Err(fail());
        }
        Ok(self.contents)
    }
}

/// A fresh System.Random (TickCount seed in the source; an unpredictable seed here).
fn fresh_random() -> ColorRandom {
    ColorRandom::from_seed((OsRng.next_u32() >> 1) as i32)
}

/// DyeAmpleSession.From and the DyeAmpleUsedMessage of the empty ampoule branch.
/// A region without DyeCashCursor is refused: the source's null split is not reproduced.
pub(super) async fn open(
    pool: &PgPool,
    assets: &Assets,
    flags: super::minimum_item_logs::Flags,
    item_id: i64,
    class: &str,
) -> Result<(Vec<OutgoingPacket>, Session), MinimumError> {
    let contents = assets.dye.as_ref().ok_or_else(fail)?;
    let cursor = contents.cursor.ok_or_else(fail)?;
    let color_table = dye_ample::color_table(class).ok_or_else(fail)?;
    let mut random = fresh_random();
    let mut seed = [0; 4];
    for s in &mut seed {
        *s = random.next(256).map_err(|_| fail())?;
    }
    // DyeAmpleSession constructor: ItemLog.AddItemJournal("DyeAmple").
    let journal = super::minimum_item_logs::named_journal(pool, flags, "DyeAmple").await;
    let message = DyeMessage::Used {
        color_table: Some(color_table.clone()),
        seeds: seed,
    };
    let packet = OutgoingPacket {
        category: DyeKind::Used.category(),
        body: dye_wire::encode(DyeKind::Used, Some(&message), DEFAULT_MAX_BODY)
            .map_err(|_| fail())?
            .to_vec(),
    };
    Ok((
        vec![packet],
        Session {
            item_id,
            color_table,
            seed,
            cursor,
            journal,
        },
    ))
}

/// DyeAmpleRequestMessage fields (RequestDyeAmple).
pub struct Request {
    pub x: i32,
    pub y: i32,
    pub is_avatar: bool,
    pub color_value: Option<Vec<i32>>,
}

fn utf16(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

/// RequestDyeAmpleProcessor. Every refusal is the source's FailMessage: nothing is
/// written and no frontend message is sent. The session stays open, as in the source.
pub async fn request(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    session: Option<&Session>,
    request: Request,
    log: super::minimum_item_logs::Context,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    // Constructor: ItemLog.GetJournalID (SetItemOpLog RequestDyeAmple).
    let _journal = super::minimum_item_logs::operation_journal(pool, log.flags).await;
    let session = session.ok_or_else(fail)?;
    let contents = assets.dye.as_ref().ok_or_else(fail)?;
    let color_value = request.color_value.as_deref().ok_or_else(fail)?;
    let (packets, ledgers) = tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
        sqlx::query(
            "LOCK TABLE game_items,game_equippables,game_item_attributes IN SHARE ROW EXCLUSIVE MODE",
        )
        .execute(&mut *tx)
        .await?;
        let before =
            super::minimum_inventory::snapshot_with_equipment(&mut tx, owner, assets, cid, None)
                .await?;
        let item = before
            .slots
            .as_ref()
            .ok_or_else(fail)?
            .iter()
            .flatten()
            .find(|s| s.id == session.item_id)
            .cloned()
            .ok_or_else(fail)?;
        let class = item.class.clone().unwrap_or_default();
        // "Already used DyeAmple" / "Already used AvatarDyeAmple": exact class matches.
        if class == dye_ample::COLOR_CLASS || class == dye_ample::AVATAR_COLOR_CLASS {
            return Err(fail());
        }
        // Only the VALUE that MakeColorAmple itself writes is modeled on the ampoule.
        let attributes = item.attributes.as_deref().unwrap_or_default();
        if attributes
            .iter()
            .any(|a| a.as_ref().and_then(|a| a.name.as_deref()) != Some("VALUE"))
        {
            return Err(fail());
        }
        let made = session
            .make(
                assets.colors_materials(),
                &contents.distort,
                &class,
                item.tradable,
                request.x,
                request.y,
                request.is_avatar,
                color_value,
                |width, height| {
                    let mut random = fresh_random();
                    Ok((
                        random.next(width).map_err(dye_ample::DyeError::Color)?,
                        random.next(height).map_err(dye_ample::DyeError::Color)?,
                    ))
                },
            )
            .map_err(|_| fail())?;
        let key = utf16(&made.class);
        let indexed = assets.stats_index.class(Some(&key)).ok_or_else(fail)?;
        // SetColor needs an equippable class with EquipItemInfo.
        if !indexed.equippable {
            return Err(fail());
        }
        sqlx::query(
            "UPDATE game_items SET item_class_utf16=$3,tradable=$4 WHERE id=$1 AND owner_id=$2",
        )
        .bind(item.id)
        .bind(cid)
        .bind(&key)
        .bind(made.tradable)
        .execute(&mut *tx)
        .await?;
        // Ample.Equippable = new Equippable(), then SetColor(color, color, color).
        sqlx::query("DELETE FROM game_equippables WHERE id=$1")
            .bind(item.id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO game_equippables(id,part,color1,color2,color3,reduced_durability,max_durability_bonus) VALUES($1,NULL,$2,$2,$2,0,0)")
            .bind(item.id)
            .bind(made.color)
            .execute(&mut *tx)
            .await?;
        // SetGeneralValue(color.ToString(), 0): SetItemAttribute("VALUE", value, 0, 0).
        let value = made.color.to_string();
        let updated = sqlx::query("UPDATE game_item_attributes SET value_bytes=$2,arg=0,arg2=0 WHERE item_id=$1 AND attribute_bytes=$3")
            .bind(item.id)
            .bind(value.as_bytes())
            .bind(b"VALUE".as_slice())
            .execute(&mut *tx)
            .await?
            .rows_affected();
        if updated == 0 {
            sqlx::query("INSERT INTO game_item_attributes(id,item_id,attribute_bytes,value_bytes,arg,arg2) VALUES(nextval('game_item_attribute_id'),$1,$2,$3,0,0)")
                .bind(item.id)
                .bind(b"VALUE".as_slice())
                .bind(value.as_bytes())
                .execute(&mut *tx)
                .await?;
        }
        let after =
            super::minimum_inventory::snapshot_with_equipment(&mut tx, owner, assets, cid, None)
                .await?;
        let slot = after
            .slots
            .as_ref()
            .and_then(|s| s.iter().flatten().find(|s| s.id == item.id))
            .cloned()
            .ok_or_else(fail)?;
        tx.commit().await?;
        let complete = DyeMessage::Complete { color: made.color };
        let update = Message::UpdateInventoryInfo(Some(vec![Some(slot)]));
        let packets = vec![
            OutgoingPacket {
                category: DyeKind::Complete.category(),
                body: dye_wire::encode(DyeKind::Complete, Some(&complete), DEFAULT_MAX_BODY)
                    .map_err(|_| fail())?
                    .to_vec(),
            },
            OutgoingPacket {
                category: Kind::UpdateInventoryInfo.category(),
                body: wire::encode(Kind::UpdateInventoryInfo, Some(&update), DEFAULT_MAX_BODY)
                    .map_err(|_| fail())?
                    .to_vec(),
            },
        ];
        // LogData.From(owner, Ample) before and after the change.
        let destroyed = Data::new(item.id, utf16(&class), 1, [-1; 3], 0, 0, false);
        let given = Data::new(item.id, key, 1, [made.color; 3], 0, 0, false);
        Ok((packets, [destroyed, given]))
    })
    .await
    .map_err(|_| fail())??;
    for (event, data) in [LEDGER_DESTROY_ITEM, LEDGER_GIVE_ITEM]
        .into_iter()
        .zip(ledgers)
    {
        let Ok(data) = data else {
            tracing::warn!("minimum dye ledger skipped; values omitted");
            continue;
        };
        super::minimum_item_logs::write_item_ledger(
            pool,
            cid,
            session.journal,
            event,
            ACTION_DYE_CASH,
            data,
            log.flags,
        )
        .await;
    }
    Ok(packets)
}
