//! Coupon shop messages recovered from the Framework serializer: CouponShopItemInfoQuery
//! (165), CouponShopItemInfoResult (164, ShopVersion and an IDictionary<short,
//! ShopTimeRestrictedResult>) and CouponShopItemBuy, which has no category and arrives in
//! the TypeConverter GUID envelope.
use super::{
    inventory::{Wire, object},
    wire::{PayloadError, Reader, Writer},
};
use thiserror::Error;
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

const MAX_ROWS: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    ItemInfoQuery,
    ItemInfoResult,
    ItemBuy,
}
impl Kind {
    /// Messages the client sends to the frontend.
    pub const INGRESS: [Kind; 2] = [Kind::ItemInfoQuery, Kind::ItemBuy];
    /// The registered category; 0 for the unregistered CouponShopItemBuyMessage.
    pub const fn category(self) -> i32 {
        match self {
            Self::ItemInfoQuery => 165,
            Self::ItemInfoResult => 164,
            Self::ItemBuy => 0,
        }
    }
    pub const fn type_id(self) -> Uuid {
        Uuid::from_u128(match self {
            Self::ItemInfoQuery => 0xee913526_5550_3735_8cce_2157cafa92b7,
            Self::ItemInfoResult => 0xfa215570_02da_3151_9471_c410aaebcdb9,
            Self::ItemBuy => 0xd12c42b9_06a1_3033_b291_8a5d1de23c1c,
        })
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CouponShopWireError {
    #[error(transparent)]
    Payload(#[from] PayloadError),
    #[error("message does not match its kind")]
    Kind,
    #[error("unsupported coupon shop message type")]
    Type,
}

/// ShopTimeRestrictedResult.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Restriction {
    pub buyable_count: i32,
    pub next_reset_ticks_diff: i64,
}
object!(Restriction {
    buyable_count,
    next_reset_ticks_diff
});

/// CouponShopItemInfoResultMessage: ShopVersion and the restriction of each restricted
/// Order (None for a null dictionary).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemInfoResult {
    pub shop_version: i16,
    pub restrictions: Option<Vec<(i16, Option<Restriction>)>>,
}
impl Wire for ItemInfoResult {
    fn write(&self, w: &mut Writer) -> Result<(), PayloadError> {
        self.shop_version.write(w)?;
        let Some(rows) = &self.restrictions else {
            return w.append(&[0]);
        };
        if rows.len() > MAX_ROWS {
            return Err(PayloadError::TooLarge);
        }
        let mut keys = std::collections::BTreeSet::new();
        if rows.iter().any(|(k, _)| !keys.insert(*k)) {
            return Err(PayloadError::DuplicateKey);
        }
        w.append(&super::encode_integer((rows.len() + 1) as i32))?;
        for (order, restriction) in rows {
            order.write(w)?;
            restriction.write(w)?;
        }
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PayloadError> {
        let shop_version = i16::read(r)?;
        let Some(n) = r.count()? else {
            return Ok(Self {
                shop_version,
                restrictions: None,
            });
        };
        if n > MAX_ROWS || n > r.remaining.len() {
            return Err(PayloadError::TooLarge);
        }
        let mut rows = Vec::with_capacity(n);
        let mut keys = std::collections::BTreeSet::new();
        for _ in 0..n {
            let order = i16::read(r)?;
            if !keys.insert(order) {
                return Err(PayloadError::DuplicateKey);
            }
            rows.push((order, Option::<Restriction>::read(r)?));
        }
        Ok(Self {
            shop_version,
            restrictions: Some(rows),
        })
    }
}

/// CouponShopItemBuyMessage: ShopID (the shop version), Order, Count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemBuy {
    pub shop_id: i16,
    pub order: i16,
    pub count: i32,
}
object!(ItemBuy {
    shop_id,
    order,
    count
});

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    /// CouponShopItemInfoQueryMessage.ShopVersion.
    ItemInfoQuery(i16),
    ItemInfoResult(ItemInfoResult),
    ItemBuy(ItemBuy),
}
impl Message {
    pub const fn kind(&self) -> Kind {
        match self {
            Self::ItemInfoQuery(_) => Kind::ItemInfoQuery,
            Self::ItemInfoResult(_) => Kind::ItemInfoResult,
            Self::ItemBuy(_) => Kind::ItemBuy,
        }
    }
}

pub fn encode(
    kind: Kind,
    message: Option<&Message>,
    max_body: usize,
) -> Result<Zeroizing<Vec<u8>>, CouponShopWireError> {
    let mut w = Writer {
        bytes: Vec::new(),
        limit: max_body,
    };
    let result = (|| -> Result<(), CouponShopWireError> {
        let Some(m) = message else {
            w.append(&[0])?;
            return Ok(());
        };
        if kind != m.kind() {
            return Err(CouponShopWireError::Kind);
        }
        w.append(&[1])?;
        match m {
            Message::ItemInfoQuery(v) => v.write(&mut w)?,
            Message::ItemInfoResult(v) => v.write(&mut w)?,
            Message::ItemBuy(v) => v.write(&mut w)?,
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

/// Strict decode: the whole body must be consumed (encoder round trips and fixtures).
pub fn decode(
    kind: Kind,
    body: &[u8],
    max_body: usize,
) -> Result<Option<Message>, CouponShopWireError> {
    decode_body(kind, body, max_body, true)
}

fn decode_body(
    kind: Kind,
    body: &[u8],
    max_body: usize,
    strict: bool,
) -> Result<Option<Message>, CouponShopWireError> {
    if body.len() > max_body {
        return Err(PayloadError::TooLarge.into());
    }
    let mut r = Reader { remaining: body };
    let result = if r.byte()? == 0 {
        None
    } else {
        let r = &mut r;
        Some(match kind {
            Kind::ItemInfoQuery => Message::ItemInfoQuery(Wire::read(r)?),
            Kind::ItemInfoResult => Message::ItemInfoResult(Wire::read(r)?),
            Kind::ItemBuy => Message::ItemBuy(Wire::read(r)?),
        })
    };
    if strict && !r.remaining.is_empty() {
        return Err(PayloadError::TrailingBytes.into());
    }
    Ok(result)
}

/// The kind of an ingress body: the registered category, or category 0 with the type's
/// GUID envelope (the only form of CouponShopItemBuyMessage).
pub fn ingress_kind(category: i32, body: &[u8]) -> Option<Kind> {
    Kind::INGRESS.into_iter().find(|k| {
        (k.category() != 0 && category == k.category())
            || (category == 0 && body.get(..16) == Some(k.type_id().as_bytes().as_slice()))
    })
}

/// Registered category, or the unregistered GUID form; trailing bytes are ignored as
/// SerializeReader.FromBinary does.
pub fn decode_ingress(
    kind: Kind,
    category: i32,
    body: &[u8],
    max_body: usize,
) -> Result<Option<Message>, CouponShopWireError> {
    if kind.category() != 0 && category == kind.category() {
        return decode_body(kind, body, max_body, false);
    }
    if category == 0 && body.len() <= max_body && body.get(..16) == Some(kind.type_id().as_bytes())
    {
        return decode_body(kind, &body[16..], max_body.saturating_sub(16), false);
    }
    Err(CouponShopWireError::Type)
}
