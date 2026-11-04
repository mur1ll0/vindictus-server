//! TalkService whisper (FeatureMatrix UseTalkService on in EN-EU): GameClient turns
//! RequestWhisperMessage into the Whisper operation of the character's TalkClient;
//! TalkService.WhisperToAll resolves the receiver (GetCIDByName) and sends WhisperMessage to
//! its frontend (WhisperToGameClient). The sender gets nothing on success and
//! WhisperFailMessage with a UI string otherwise. Without a TalkService connection (before
//! InGame) GameClient answers Whisper_Error. The WCF relay of other instances is not
//! configured (TalkWcfService empty), so Wait never happens and an offline receiver fails at
//! once. The whisper filter is not applied on delivery (the original does not either).
use super::minimum_character::MinimumError;
use super::party_hub::PartyHub;
use crate::domain::map_partition::OutgoingPacket;
use crate::protocol::{
    DEFAULT_MAX_BODY,
    whisper::{self as wire, Fail, Message, Request, Whisper},
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
