//! TalkService whisper (FeatureMatrix UseTalkService on in EN-EU): GameClient turns
//! RequestWhisperMessage into the Whisper operation of the character's TalkClient;
//! TalkService.WhisperToAll resolves the receiver (GetCIDByName) and sends WhisperMessage to
//! its frontend (WhisperToGameClient). The sender gets nothing on success and
//! WhisperFailMessage with a UI string otherwise. Without a TalkService connection (before
//! InGame) GameClient answers Whisper_Error. The WCF relay of other instances is not
//! configured (TalkWcfService empty), so Wait never happens and an offline receiver fails at
//! once. The whisper filter is not applied on delivery (the original does not either): the
//! PlayerService WhisperFilter only keeps the WhisperBlock rows and the list the client gets
//! on InGame entry and after each change.
use super::minimum_character::MinimumError;
use super::party_hub::PartyHub;
use crate::domain::map_partition::OutgoingPacket;
use crate::protocol::{
    DEFAULT_MAX_BODY,
    mission::StrDict,
    whisper::{self as wire, Fail, Message, Request, UpdateFilter, Whisper},
};
use sqlx::PgPool;

/// Whisper.WhisperResult.OffLine.
pub const NOT_ONLINE: &str = "GameUI_Heroes_Chat_Whisper_Not_Online";
/// Whisper.WhisperResult.NoCharacter.
pub const INVALID_NAME: &str = "GameUI_Heroes_Chat_Whisper_Invalid_Character_Name";
/// Whisper.WhisperResult.LogicalFail (also an exception in the processor, such as a null
/// receiver name in the NameCache lookup).
pub const LOGICAL_ERROR: &str = "GameUI_Heroes_Chat_Whisper_Logical_Error";
/// No TalkService connection and no WhisperHandler.
pub const ERROR: &str = "GameUI_Heroes_Chat_Whisper_Error";

pub fn packet(message: &Message) -> Result<OutgoingPacket, MinimumError> {
    let kind = message.kind();
    Ok(OutgoingPacket {
        category: kind.category(),
        body: wire::encode(kind, Some(message), DEFAULT_MAX_BODY)
            .map_err(|_| MinimumError::Conflict)?
            .to_vec(),
    })
}

/// WhisperFailMessage(receiver, reason).
pub fn fail(receiver: Option<&str>, reason: &str) -> Result<OutgoingPacket, MinimumError> {
    packet(&Message::Fail(Fail {
        to_name: receiver.map(str::to_owned),
        reason: Some(reason.to_owned()),
    }))
}

/// WhisperToAll after GetCIDByName: `receiver` is the resolved CID (None for NoCharacter).
/// Returns the sender's packets; the receiver's WhisperMessage goes through the hub.
pub fn route(
    hub: &PartyHub,
    sender: &str,
    request: &Request,
    receiver: Option<i64>,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let to = request.receiver.as_deref();
    let Some(cid) = receiver else {
        return Ok(vec![fail(to, INVALID_NAME)?]);
    };
    let message = packet(&Message::Whisper(Whisper {
        sender: Some(sender.to_owned()),
        contents: request.contents.clone(),
    }))?;
    if hub.deliver(cid, vec![message]) {
        Ok(Vec::new())
    } else {
        Ok(vec![fail(to, NOT_ONLINE)?])
    }
}

/// UpdateWhisperFilter.UpdateType (the message's OperationType cast to the enum).
pub mod update_type {
    pub const ADD: i32 = 0;
    /// No processor branch: the operation fails without a message.
    pub const UPDATE: i32 = 1;
    pub const REMOVE: i32 = 2;
}
/// The level InGame always requests (UpdateWhisperFilter.Level = 1).
pub const BLOCK_LEVEL: i32 = 1;
pub const CANNOT_FILTER_YOURSELF: &str = "GameUI_Heroes_WhisperFilter_Cannot_Filter_Yourself";
pub const SUCCESS_ADD: &str = "GameUI_Heroes_WhisperFilter_Success_Add";
pub const SUCCESS_REMOVE: &str = "GameUI_Heroes_WhisperFilter_Success_Remove";
pub const CANNOT_ADD: &str = "GameUI_Heroes_WhisperFilter_Cannot_Add";
pub const CANNOT_REMOVE: &str = "GameUI_Heroes_WhisperFilter_Cannot_Remove";

/// WhisperFilter.GetWhisperfilter: WhisperBlock rows of `cid` joined with the blocked
/// character's name (any status, as the LINQ join), the first row of a repeated name kept.
pub async fn filter(pool: &PgPool, cid: i64) -> Result<StrDict<i32>, MinimumError> {
    let rows: Vec<(Option<Vec<u8>>, i32)> = sqlx::query_as(
        "SELECT c.name_utf16,b.block_level FROM game_whisper_block b \
         JOIN game_character_info c ON c.id=b.to_cid WHERE b.from_cid=$1 ORDER BY b.row_id LIMIT 4096",
    )
    .bind(cid)
    .fetch_all(pool)
    .await?;
    let mut out: Vec<(String, i32)> = Vec::with_capacity(rows.len());
    for (raw, level) in rows {
        let Some(raw) = raw else { continue };
        let name = super::minimum_party::decode(&raw)?;
        if !out.iter().any(|(n, _)| *n == name) {
            out.push((name, level));
        }
    }
    Ok(StrDict(Some(out)))
}

/// QueryWhisperFilter on InGame entry → WhisperFilterListMessage.
pub async fn filter_list_message(pool: &PgPool, cid: i64) -> Result<OutgoingPacket, MinimumError> {
    packet(&Message::FilterList(filter(pool, cid).await?))
}

/// WhisperFilter.AddToWhisperFilter: a new row only when the pair is absent.
async fn add(pool: &PgPool, from: i64, to: i64, level: i32) -> Result<bool, MinimumError> {
    let done = sqlx::query(
        "INSERT INTO game_whisper_block(from_cid,to_cid,block_level) VALUES($1,$2,$3) \
         ON CONFLICT (from_cid,to_cid) DO NOTHING",
    )
    .bind(from)
    .bind(to)
    .bind(level)
    .execute(pool)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// WhisperFilter.RemoveFromWhisperFilter: every row of the pair, false when there is none.
async fn remove(pool: &PgPool, from: i64, to: i64) -> Result<bool, MinimumError> {
    let done = sqlx::query("DELETE FROM game_whisper_block WHERE from_cid=$1 AND to_cid=$2")
        .bind(from)
        .bind(to)
        .execute(pool)
        .await?;
    Ok(done.rows_affected() > 0)
}

fn system(text: &str, parameters: &[String]) -> Result<OutgoingPacket, MinimumError> {
    super::minimum_shop::system_message(0, text, parameters)
}

/// InGame UpdateWhisperFilterMessage of `cid` (account `own_uid`): QueryCIDByName of the
/// target, refusal of any character of the own account, then UpdateWhisperFilter with
/// level 1 and its SystemMessage + WhisperFilterListMessage, or the failure message.
pub async fn update_filter(
    pool: &PgPool,
    cid: i64,
    own_uid: i32,
    request: &UpdateFilter,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let target = match request.target_id.as_deref() {
        Some(name) => super::minimum_party::character_info_by_name(pool, name)
            .await
            .unwrap_or(None),
        None => None,
    };
    let Some((target_cid, target_uid)) = target else {
        return Ok(vec![system(INVALID_NAME, &[])?]);
    };
    if target_uid == own_uid {
        return Ok(vec![system(CANNOT_FILTER_YOURSELF, &[])?]);
    }
    let op = request.operation_type;
    let done = match op {
        update_type::ADD => add(pool, cid, target_cid, BLOCK_LEVEL).await,
        update_type::REMOVE => remove(pool, cid, target_cid).await,
        _ => Ok(false),
    };
    let done = match done {
        Ok(done) => done,
        Err(_) => {
            tracing::warn!("whisper filter update refused; values omitted");
            false
        }
    };
    let target_name: Vec<String> = request.target_id.iter().cloned().collect();
    if done {
        let list = filter(pool, cid).await?;
        let text = if op == update_type::ADD {
            SUCCESS_ADD
        } else {
            SUCCESS_REMOVE
        };
        return Ok(vec![
            system(text, &target_name)?,
            packet(&Message::FilterList(list))?,
        ]);
    }
    Ok(match op {
        update_type::ADD => vec![system(CANNOT_ADD, &[])?],
        update_type::REMOVE => vec![system(CANNOT_REMOVE, &[])?],
        _ => Vec::new(),
    })
}

/// RequestWhisperMessage of the entered character `cid` (InGame, TalkConn connected).
pub async fn whisper(
    pool: &PgPool,
    hub: &PartyHub,
    cid: i64,
    request: &Request,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let to = request.receiver.as_deref();
    let (Some(sender), Some(name)) = (hub.name(cid), to) else {
        return Ok(vec![fail(to, LOGICAL_ERROR)?]);
    };
    match super::minimum_party::character_by_name(pool, name).await {
        Ok(receiver) => route(hub, &sender, request, receiver),
        Err(_) => Ok(vec![fail(to, LOGICAL_ERROR)?]),
    }
}
