//! Marketplace (TradeSystem) messages recovered from the Framework serializer
//! (ServiceCore.EndPointNetwork): the searches, registration, purchase, cancellation and
//! the trade session envelope. Fields are written in declaration order; reference-type
//! elements of a collection carry their null flag. SortOrder travels as Int32.
use super::{
    inventory::{Wire, object},
    wire::{PayloadError, Reader, Writer},
};
use thiserror::Error;
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    TradeCategorySearch,
    TradeItemClassListSearch,
    TradeCancelMyItem,
    TradeCancelMyItemResult,
    TradeRequestMyItem,
    TradeMyItemInfos,
    TradePurchaseItem,
    TradePurchaseItemResult,
    TradeSearchResult,
    InsertTradeOrder,
    InsertTradeOrderResult,
    InsertTradeTirOrder,
    StartTradeSession,
    EndTradeSession,
}
impl Kind {
    /// Messages the client sends to the frontend (ItemServiceProxy, InGame).
    pub const INGRESS: [Kind; 9] = [
        Kind::TradeCategorySearch,
        Kind::TradeItemClassListSearch,
        Kind::TradeCancelMyItem,
        Kind::TradeRequestMyItem,
        Kind::TradePurchaseItem,
        Kind::InsertTradeOrder,
        Kind::InsertTradeTirOrder,
        Kind::StartTradeSession,
        Kind::EndTradeSession,
    ];
    pub const fn category(self) -> i32 {
        match self {
            Self::TradeCancelMyItem => 302,
            Self::TradeCancelMyItemResult => 303,
            Self::TradeCategorySearch => 305,
            Self::TradeItemClassListSearch => 306,
            Self::TradeMyItemInfos => 308,
            Self::TradePurchaseItem => 309,
            Self::TradePurchaseItemResult => 310,
            Self::TradeRequestMyItem => 311,
            Self::TradeSearchResult => 312,
            Self::InsertTradeOrder => 427,
            Self::InsertTradeOrderResult => 428,
            Self::InsertTradeTirOrder => 429,
            Self::StartTradeSession => 592,
            Self::EndTradeSession => 594,
        }
    }
    pub const fn type_id(self) -> Uuid {
        Uuid::from_u128(match self {
            Self::TradeCancelMyItem => 0x63d59401_6b18_305a_af2f_98a77d2d977b,
            Self::TradeCancelMyItemResult => 0xbbae86b3_29cf_3b81_afd5_5736213f0c2d,
            Self::TradeCategorySearch => 0x77610fcf_fc87_3633_8f8e_de283eff2e6f,
            Self::TradeItemClassListSearch => 0x92300b56_a6fc_35ec_aa0c_c344e2615e6f,
            Self::TradeMyItemInfos => 0x1d431b50_2a5d_3e6e_829f_8c1a5057b5b3,
            Self::TradePurchaseItem => 0x6d284225_0676_3001_9369_ba59fe31cb4f,
            Self::TradePurchaseItemResult => 0x813c7eaf_a144_334e_81cd_9d066f11000e,
            Self::TradeRequestMyItem => 0xe0188571_a1a3_3b93_8772_9c44ad8d1114,
            Self::TradeSearchResult => 0xf6cbae04_367a_3926_b010_aaed1b28790a,
            Self::InsertTradeOrder => 0x5dd5d7fb_1389_353f_8bcf_7d2fbd9edb82,
            Self::InsertTradeOrderResult => 0x7ab66731_d827_3eee_bcec_1c50b5a066fc,
            Self::InsertTradeTirOrder => 0x97f6218c_8733_3b4b_bdd7_af26cf089413,
            Self::StartTradeSession => 0x99350e05_2420_364a_a3bc_4be82c6f1cf7,
            Self::EndTradeSession => 0x84675e2c_42fe_3895_9f1f_b913dd2d1918,
        })
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MarketplaceWireError {
    #[error("marketplace message kind mismatch")]
    Kind,
    #[error("unsupported marketplace category/type")]
    Type,
    #[error(transparent)]
    Payload(#[from] PayloadError),
}

/// DetailOption: Key, Value, SearchType (the procedures' OperatorType).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DetailOption {
    pub key: Option<String>,
    pub value: i32,
    pub search_type: u8,
}
object!(DetailOption {
    key,
    value,
    search_type
});
/// TradeItemInfo: CloseDate is the remaining seconds (TradeItem.GetRemainTimeFromNow).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TradeItemInfo {
    pub tid: i64,
    pub cid: i64,
    pub character_name: Option<String>,
    pub item_class: Option<String>,
    pub item_count: i64,
    pub item_price: i64,
    pub close_date: i32,
    pub attribute_ex: Option<String>,
    pub max_armor_condition: i32,
    pub color1: i32,
    pub color2: i32,
    pub color3: i32,
    pub min_price: i32,
    pub max_price: i32,
    pub avg_price: i32,
    pub trade_type: u8,
}
object!(TradeItemInfo {
    tid,
    cid,
    character_name,
    item_class,
    item_count,
    item_price,
    close_date,
    attribute_ex,
    max_armor_condition,
    color1,
    color2,
    color3,
    min_price,
    max_price,
    avg_price,
    trade_type
});
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TradeCategorySearch {
    pub trade_category: Option<String>,
    pub trade_category_sub: Option<String>,
    pub min_level: i32,
    pub max_level: i32,
    pub unique_number: i32,
    pub chunk_page_number: i32,
    pub order: i32,
    pub is_descending: bool,
    pub detail_options: Option<Vec<Option<DetailOption>>>,
}
object!(TradeCategorySearch {
    trade_category,
    trade_category_sub,
    min_level,
    max_level,
    unique_number,
    chunk_page_number,
    order,
    is_descending,
    detail_options
});
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TradeItemClassListSearch {
    pub item_class_list: Option<Vec<Option<String>>>,
    pub unique_number: i32,
    pub chunk_page_number: i32,
    pub order: i32,
    pub is_descending: bool,
    pub detail_options: Option<Vec<Option<DetailOption>>>,
}
object!(TradeItemClassListSearch {
    item_class_list,
    unique_number,
    chunk_page_number,
    order,
    is_descending,
    detail_options
});
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TradeCancelMyItem {
    pub tid: i64,
    pub unique_number: i32,
}
object!(TradeCancelMyItem { tid, unique_number });
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TradeCancelMyItemResult {
    pub unique_number: i32,
    pub result: i32,
}
object!(TradeCancelMyItemResult {
    unique_number,
    result
});
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TradeMyItemInfos {
    pub trade_item_list: Option<Vec<Option<TradeItemInfo>>>,
    pub result: i32,
}
object!(TradeMyItemInfos {
    trade_item_list,
    result
});
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TradePurchaseItem {
    pub tid: i64,
    pub purchase_count: i32,
    pub unique_number: i32,
}
object!(TradePurchaseItem {
    tid,
    purchase_count,
    unique_number
});
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TradePurchaseItemResult {
    pub unique_number: i32,
    pub result: i32,
    pub left_number: i32,
}
object!(TradePurchaseItemResult {
    unique_number,
    result,
    left_number
});
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TradeSearchResult {
    pub trade_item_list: Option<Vec<Option<TradeItemInfo>>>,
    pub unique_number: i32,
    pub is_more_result: bool,
    pub result: i32,
}
object!(TradeSearchResult {
    trade_item_list,
    unique_number,
    is_more_result,
    result
});
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InsertTradeOrder {
    pub item_id: i64,
    pub num: i32,
    pub duration_min: i32,
    pub unit_price: i32,
    pub trade_type: u8,
}
object!(InsertTradeOrder {
    item_id,
    num,
    duration_min,
    unit_price,
    trade_type
});
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InsertTradeTirOrder {
    pub num: i32,
    pub duration_min: i32,
    pub unit_price: i32,
}
object!(InsertTradeTirOrder {
    num,
    duration_min,
    unit_price
});

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    TradeCategorySearch(TradeCategorySearch),
    TradeItemClassListSearch(TradeItemClassListSearch),
    TradeCancelMyItem(TradeCancelMyItem),
    TradeCancelMyItemResult(TradeCancelMyItemResult),
    TradeRequestMyItem,
    TradeMyItemInfos(TradeMyItemInfos),
    TradePurchaseItem(TradePurchaseItem),
    TradePurchaseItemResult(TradePurchaseItemResult),
    TradeSearchResult(TradeSearchResult),
    InsertTradeOrder(InsertTradeOrder),
    /// InsertTradeOrderResultMessage: Result (InsertTradeOrderResultMessage.ErrorCode).
    InsertTradeOrderResult(i32),
    InsertTradeTirOrder(InsertTradeTirOrder),
    StartTradeSession,
    EndTradeSession,
}
impl Message {
    pub const fn kind(&self) -> Kind {
        match self {
            Self::TradeCategorySearch(_) => Kind::TradeCategorySearch,
            Self::TradeItemClassListSearch(_) => Kind::TradeItemClassListSearch,
            Self::TradeCancelMyItem(_) => Kind::TradeCancelMyItem,
            Self::TradeCancelMyItemResult(_) => Kind::TradeCancelMyItemResult,
            Self::TradeRequestMyItem => Kind::TradeRequestMyItem,
            Self::TradeMyItemInfos(_) => Kind::TradeMyItemInfos,
            Self::TradePurchaseItem(_) => Kind::TradePurchaseItem,
            Self::TradePurchaseItemResult(_) => Kind::TradePurchaseItemResult,
            Self::TradeSearchResult(_) => Kind::TradeSearchResult,
            Self::InsertTradeOrder(_) => Kind::InsertTradeOrder,
            Self::InsertTradeOrderResult(_) => Kind::InsertTradeOrderResult,
            Self::InsertTradeTirOrder(_) => Kind::InsertTradeTirOrder,
            Self::StartTradeSession => Kind::StartTradeSession,
            Self::EndTradeSession => Kind::EndTradeSession,
        }
    }
}

pub fn encode(
    kind: Kind,
    message: Option<&Message>,
    max_body: usize,
) -> Result<Zeroizing<Vec<u8>>, MarketplaceWireError> {
    let mut w = Writer {
        bytes: Vec::new(),
        limit: max_body,
    };
    let result = (|| -> Result<(), MarketplaceWireError> {
        let Some(m) = message else {
            w.append(&[0])?;
            return Ok(());
        };
        if kind != m.kind() {
            return Err(MarketplaceWireError::Kind);
        }
        w.append(&[1])?;
        match m {
            Message::TradeRequestMyItem | Message::StartTradeSession | Message::EndTradeSession => {
            }
            Message::TradeCategorySearch(v) => v.write(&mut w)?,
            Message::TradeItemClassListSearch(v) => v.write(&mut w)?,
            Message::TradeCancelMyItem(v) => v.write(&mut w)?,
            Message::TradeCancelMyItemResult(v) => v.write(&mut w)?,
            Message::TradeMyItemInfos(v) => v.write(&mut w)?,
            Message::TradePurchaseItem(v) => v.write(&mut w)?,
            Message::TradePurchaseItemResult(v) => v.write(&mut w)?,
            Message::TradeSearchResult(v) => v.write(&mut w)?,
            Message::InsertTradeOrder(v) => v.write(&mut w)?,
            Message::InsertTradeOrderResult(v) => v.write(&mut w)?,
            Message::InsertTradeTirOrder(v) => v.write(&mut w)?,
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
) -> Result<Option<Message>, MarketplaceWireError> {
    decode_body(kind, body, max_body, true)
}
fn decode_body(
    kind: Kind,
    body: &[u8],
    max_body: usize,
    strict: bool,
) -> Result<Option<Message>, MarketplaceWireError> {
    if body.len() > max_body {
        return Err(PayloadError::TooLarge.into());
    }
    let mut r = Reader { remaining: body };
    let result = if r.byte()? == 0 {
        None
    } else {
        let r = &mut r;
        Some(match kind {
            Kind::TradeCategorySearch => Message::TradeCategorySearch(Wire::read(r)?),
            Kind::TradeItemClassListSearch => Message::TradeItemClassListSearch(Wire::read(r)?),
            Kind::TradeCancelMyItem => Message::TradeCancelMyItem(Wire::read(r)?),
            Kind::TradeCancelMyItemResult => Message::TradeCancelMyItemResult(Wire::read(r)?),
            Kind::TradeRequestMyItem => Message::TradeRequestMyItem,
            Kind::TradeMyItemInfos => Message::TradeMyItemInfos(Wire::read(r)?),
            Kind::TradePurchaseItem => Message::TradePurchaseItem(Wire::read(r)?),
            Kind::TradePurchaseItemResult => Message::TradePurchaseItemResult(Wire::read(r)?),
            Kind::TradeSearchResult => Message::TradeSearchResult(Wire::read(r)?),
            Kind::InsertTradeOrder => Message::InsertTradeOrder(Wire::read(r)?),
            Kind::InsertTradeOrderResult => Message::InsertTradeOrderResult(Wire::read(r)?),
            Kind::InsertTradeTirOrder => Message::InsertTradeTirOrder(Wire::read(r)?),
            Kind::StartTradeSession => Message::StartTradeSession,
            Kind::EndTradeSession => Message::EndTradeSession,
        })
    };
    if strict && !r.remaining.is_empty() {
        return Err(PayloadError::TrailingBytes.into());
    }
    Ok(result)
}
/// Registered category, or the unregistered GUID form; trailing bytes are ignored as
/// SerializeReader.FromBinary does.
pub fn decode_ingress(
    kind: Kind,
    category: i32,
    body: &[u8],
    max_body: usize,
) -> Result<Option<Message>, MarketplaceWireError> {
    if category == kind.category() {
        return decode_body(kind, body, max_body, false);
    }
    if category == 0 && body.len() <= max_body && body.get(..16) == Some(kind.type_id().as_bytes())
    {
        return decode_body(kind, &body[16..], max_body.saturating_sub(16), false);
    }
    if category == 0
        && body.len() <= max_body
        && body.get(..16)
            == Some(Uuid::from_u128(0x81c5fe01_027c_3e1c_98d5_da9c9862aa21).as_bytes())
        && body.get(16..) == Some(&[0])
    {
        return Ok(None);
    }
    Err(MarketplaceWireError::Type)
}
