//! Newbie guild recommendation and direct guild item purchase (B5 G7, G8), recovered from the
//! Framework serializer (ServiceCore.EndPointNetwork.GuildService, all unregistered: category 0,
//! GUID envelope only): FindNewbieRecommendGuild and its result (InGameGuildInfo),
//! RequestNewbieRecommendGuild, NoticeNewbieRecommend, DirectPurchaseGuildItem and its result.
use super::{
    guild::GuildInfo,
    inventory::{Wire, object},
    wire::{PayloadError, Reader, Writer},
};
use thiserror::Error;
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

macro_rules! kinds {
    ($($name:ident = $guid:literal;)*) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Kind { $($name),* }
        impl Kind {
            /// Every message of this family is unregistered (category 0).
            pub const fn category(self) -> i32 { 0 }
            pub const fn type_id(self) -> Uuid { Uuid::from_u128(match self { $(Self::$name => $guid),* }) }
        }
    };
}
kinds! {
    FindNewbieRecommendGuild = 0x1a16885d_f10c_3778_b694_bb3ec0448294;
    FindNewbieRecommendGuildResult = 0x99f557b5_d1f3_3c4e_b5a2_9063c3dc1a9b;
    RequestNewbieRecommendGuild = 0xd84b903e_f448_3d7e_87b1_1d506f1dedcc;
    NoticeNewbieRecommend = 0x35251c7c_a752_3f8f_b8d5_b8c875311683;
    DirectPurchaseGuildItem = 0xdbc653b2_4d96_3906_bfff_4849fa819fe1;
    DirectPurchaseGuildItemResult = 0x4e5db23c_be88_3d8d_b9c6_07d2bdc040f9;
}
impl Kind {
    /// Messages the client sends (GuildServiceProxy OutGuild and InGuild).
    pub const INGRESS: [Kind; 3] = [
        Kind::FindNewbieRecommendGuild,
        Kind::RequestNewbieRecommendGuild,
        Kind::DirectPurchaseGuildItem,
    ];
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GuildNewbieWireError {
    #[error(transparent)]
    Payload(#[from] PayloadError),
    #[error("message does not match its kind")]
    Kind,
    #[error("unsupported guild message category/type")]
    Type,
}

/// A message without fields.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Empty {}
impl Wire for Empty {
    fn write(&self, _: &mut Writer) -> Result<(), PayloadError> {
        Ok(())
    }
    fn read(_: &mut Reader<'_>) -> Result<Self, PayloadError> {
        Ok(Self {})
    }
}

/// FindNewbieRecommendGuildResultMessage: info.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FindResult {
    pub info: Option<GuildInfo>,
}
object!(FindResult { info });

/// RequestNewbieRecommendGuildMessage: GuildSN.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub guild_sn: i32,
}
object!(Request { guild_sn });

/// NoticeNewbieRecommendMessage: RequestUserName, ShipID.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    pub request_user_name: Option<String>,
    pub ship_id: i64,
}
object!(Notice {
    request_user_name,
    ship_id
});

/// DirectPurchaseGuildItemMessage: ProductNo, IsCredit, ItemClass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Purchase {
    pub product_no: i32,
    pub is_credit: bool,
    pub item_class: Option<String>,
}
object!(Purchase {
    product_no,
    is_credit,
    item_class
});

/// DirectPurchaseGuildItemResultCode.
pub mod purchase_result {
    pub const SUCCESS: i32 = 1;
    pub const UNKNOWN: i32 = 2;
    pub const GUILD_NOT_INITIALIZE: i32 = 3;
    pub const CANNOT_PARSE_ITEM: i32 = 4;
    pub const OVER_MAX_MEMBER_LIMIT: i32 = 5;
    pub const DATABASE_FAIL: i32 = 6;
    pub const BEGIN_DIRECT_PURCHASE_ITEM: i32 = 8;
    pub const IS_NOT_MASTER: i32 = 10;
}

/// DirectPurchaseGuildItemResultMessage: ItemClass, Result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PurchaseResult {
    pub item_class: Option<String>,
    pub result: i32,
}
object!(PurchaseResult { item_class, result });

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    FindNewbieRecommendGuild(Empty),
    FindNewbieRecommendGuildResult(FindResult),
    RequestNewbieRecommendGuild(Request),
    NoticeNewbieRecommend(Notice),
    DirectPurchaseGuildItem(Purchase),
    DirectPurchaseGuildItemResult(PurchaseResult),
}
impl Message {
    pub const fn kind(&self) -> Kind {
        match self {
            Self::FindNewbieRecommendGuild(_) => Kind::FindNewbieRecommendGuild,
            Self::FindNewbieRecommendGuildResult(_) => Kind::FindNewbieRecommendGuildResult,
            Self::RequestNewbieRecommendGuild(_) => Kind::RequestNewbieRecommendGuild,
            Self::NoticeNewbieRecommend(_) => Kind::NoticeNewbieRecommend,
            Self::DirectPurchaseGuildItem(_) => Kind::DirectPurchaseGuildItem,
            Self::DirectPurchaseGuildItemResult(_) => Kind::DirectPurchaseGuildItemResult,
        }
    }
}

pub fn encode(
    kind: Kind,
    message: Option<&Message>,
    max_body: usize,
) -> Result<Zeroizing<Vec<u8>>, GuildNewbieWireError> {
    let mut w = Writer {
        bytes: Vec::new(),
        limit: max_body,
    };
    let result = (|| -> Result<(), GuildNewbieWireError> {
        let Some(m) = message else {
            w.append(&[0])?;
            return Ok(());
        };
        if kind != m.kind() {
            return Err(GuildNewbieWireError::Kind);
        }
        w.append(&[1])?;
        match m {
            Message::FindNewbieRecommendGuild(v) => v.write(&mut w)?,
            Message::FindNewbieRecommendGuildResult(v) => v.write(&mut w)?,
            Message::RequestNewbieRecommendGuild(v) => v.write(&mut w)?,
            Message::NoticeNewbieRecommend(v) => v.write(&mut w)?,
            Message::DirectPurchaseGuildItem(v) => v.write(&mut w)?,
            Message::DirectPurchaseGuildItemResult(v) => v.write(&mut w)?,
        }
        Ok(())
    })();
    match result {
        Ok(()) => Ok(Zeroizing::new(w.bytes)),
        Err(e) => {
            w.bytes.zeroize();
            Err(e)
        }
    }
}

/// The packet: category 0 with the type's GUID before the body.
pub fn packet(message: &Message, max_body: usize) -> Result<(i32, Vec<u8>), GuildNewbieWireError> {
    let kind = message.kind();
    let body = encode(kind, Some(message), max_body)?;
    let mut out = Vec::with_capacity(16 + body.len());
    out.extend_from_slice(kind.type_id().as_bytes());
    out.extend_from_slice(&body);
    if out.len() > max_body {
        return Err(PayloadError::TooLarge.into());
    }
    Ok((0, out))
}

/// Strict decode: the whole body must be consumed (encoder round trips and fixtures).
pub fn decode(
    kind: Kind,
    body: &[u8],
    max_body: usize,
) -> Result<Option<Message>, GuildNewbieWireError> {
    decode_body(kind, body, max_body, true)
}

fn decode_body(
    kind: Kind,
    body: &[u8],
    max_body: usize,
    strict: bool,
) -> Result<Option<Message>, GuildNewbieWireError> {
    if body.len() > max_body {
        return Err(PayloadError::TooLarge.into());
    }
    let mut r = Reader { remaining: body };
    let result = if r.byte()? == 0 {
        None
    } else {
        let r = &mut r;
        Some(match kind {
            Kind::FindNewbieRecommendGuild => Message::FindNewbieRecommendGuild(Wire::read(r)?),
            Kind::FindNewbieRecommendGuildResult => {
                Message::FindNewbieRecommendGuildResult(Wire::read(r)?)
            }
            Kind::RequestNewbieRecommendGuild => {
                Message::RequestNewbieRecommendGuild(Wire::read(r)?)
            }
            Kind::NoticeNewbieRecommend => Message::NoticeNewbieRecommend(Wire::read(r)?),
            Kind::DirectPurchaseGuildItem => Message::DirectPurchaseGuildItem(Wire::read(r)?),
            Kind::DirectPurchaseGuildItemResult => {
                Message::DirectPurchaseGuildItemResult(Wire::read(r)?)
            }
        })
    };
    if strict && !r.remaining.is_empty() {
        return Err(PayloadError::TrailingBytes.into());
    }
    Ok(result)
}

/// The kind of an ingress body (the GUID envelope).
pub fn ingress_kind(category: i32, body: &[u8]) -> Option<Kind> {
    Kind::INGRESS
        .into_iter()
        .find(|k| category == 0 && body.get(..16) == Some(k.type_id().as_bytes().as_slice()))
}

/// The unregistered GUID form; trailing bytes are ignored as SerializeReader.FromBinary does.
pub fn decode_ingress(
    kind: Kind,
    category: i32,
    body: &[u8],
    max_body: usize,
) -> Result<Option<Message>, GuildNewbieWireError> {
    if category == 0 && body.len() <= max_body && body.get(..16) == Some(kind.type_id().as_bytes())
    {
        return decode_body(kind, &body[16..], max_body.saturating_sub(16), false);
    }
    Err(GuildNewbieWireError::Type)
}
