//! Guild storage messages recovered from the Framework serializer
//! (ServiceCore.EndPointNetwork.GuildService): the registered GuildInventoryInfoMessage (87)
//! with the storage's ItemServiceOperations.SlotInfo rows, and the unregistered (category 0,
//! GUID envelope only) AddGuildStorageItem, PickGuildStorageItem, PurchaseGuildStorage,
//! ArrangeGuildStorageItem, HandleGuildStorageSession, UpdateGuildStorageSetting and
//! GuildStorageLogs messages.
use super::{
    inventory::{Slot, Wire, object},
    wire::{PayloadError, Reader, Writer},
};
use thiserror::Error;
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

macro_rules! kinds {
    ($($name:ident = $cat:literal, $guid:literal;)*) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Kind { $($name),* }
        impl Kind {
            /// The registered category; 0 for an unregistered message.
            pub const fn category(self) -> i32 { match self { $(Self::$name => $cat),* } }
            pub const fn type_id(self) -> Uuid { Uuid::from_u128(match self { $(Self::$name => $guid),* }) }
        }
    };
}
kinds! {
    GuildInventoryInfo = 87, 0x58f0a03b_7aae_3521_bb10_7c39acc518e0;
    AddGuildStorageItem = 0, 0x3a237aa6_7470_3080_a641_274aac6d4d48;
    PickGuildStorageItem = 0, 0x4a24b568_3e57_331b_894b_4f638dbf19b0;
    PurchaseGuildStorage = 0, 0xc700797e_33e2_3192_bc91_c83c9131f339;
    ArrangeGuildStorageItem = 0, 0xa895bf47_377e_3ad7_a87c_43fea7945f06;
    HandleGuildStorageSession = 0, 0xbef7785d_6e8a_3a95_a0d9_23cf767ee4f1;
    UpdateGuildStorageSetting = 0, 0xdc4f3599_51af_344d_bca5_deea3f868484;
    GuildStorageLogs = 0, 0x9b995088_968a_3864_8045_5ae4e19e569a;
}
impl Kind {
    /// Messages the client sends (GuildServiceProxy InGuild).
    pub const INGRESS: [Kind; 6] = [
        Kind::AddGuildStorageItem,
        Kind::PickGuildStorageItem,
        Kind::PurchaseGuildStorage,
        Kind::ArrangeGuildStorageItem,
        Kind::HandleGuildStorageSession,
        Kind::UpdateGuildStorageSetting,
    ];
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GuildStorageWireError {
    #[error(transparent)]
    Payload(#[from] PayloadError),
    #[error("message does not match its kind")]
    Kind,
    #[error("unsupported guild storage category/type")]
    Type,
}

/// GuildInventoryInfoMessage: IsEnabled, StorageCount, GoldLimit, AccessLimtiTag, SlotInfos.
#[derive(Clone, PartialEq, Eq)]
pub struct InventoryInfo {
    pub is_enabled: bool,
    pub storage_count: i32,
    pub gold_limit: i32,
    pub access_limit_tag: i64,
    pub slot_infos: Option<Vec<Option<Slot>>>,
}
object!(InventoryInfo {
    is_enabled,
    storage_count,
    gold_limit,
    access_limit_tag,
    slot_infos
});
impl std::fmt::Debug for InventoryInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InventoryInfo")
            .field("storage_count", &self.storage_count)
            .field("slots", &self.slot_infos.as_ref().map(Vec::len))
            .finish_non_exhaustive()
    }
}

/// AddGuildStorageItemMessage / PickGuildStorageItemMessage: ItemID, ItemClass, Amount,
/// TargetTab, TargetSlot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemMove {
    pub item_id: i64,
    pub item_class: Option<String>,
    pub amount: i32,
    pub target_tab: u8,
    pub target_slot: i32,
}
object!(ItemMove {
    item_id,
    item_class,
    amount,
    target_tab,
    target_slot
});

/// PurchaseGuildStorageMessage: ProductNo, IsCredit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Purchase {
    pub product_no: i32,
    pub is_credit: bool,
}
object!(Purchase {
    product_no,
    is_credit
});

/// ArrangeGuildStorageItemMessage: ItemID, Slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Arrange {
    pub item_id: i64,
    pub slot: i32,
}
object!(Arrange { item_id, slot });

/// UpdateGuildStorageSettingMessage: GoldLimit, AccessLimtiTag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Setting {
    pub gold_limit: i32,
    pub access_limit_tag: i64,
}
object!(Setting {
    gold_limit,
    access_limit_tag
});

/// GuildStorageBriefLogElement: CharacterName, OperationType (GuildStorageOperationType:
/// Unknown, Gold, Item, Slot), AddCount, PickCount, Datestamp, Timestamp.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BriefLog {
    pub character_name: Option<String>,
    pub operation_type: i32,
    pub add_count: i32,
    pub pick_count: i32,
    pub datestamp: i32,
    pub timestamp: i32,
}
object!(BriefLog {
    character_name,
    operation_type,
    add_count,
    pick_count,
    datestamp,
    timestamp
});

/// GuildStorageItemLogElement: CharacterName, IsAddItem, ItemClass, Count, Datestamp,
/// Timestamp, Color1, Color2, Color3.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemLog {
    pub character_name: Option<String>,
    pub is_add_item: bool,
    pub item_class: Option<String>,
    pub count: i32,
    pub datestamp: i32,
    pub timestamp: i32,
    pub color1: i32,
    pub color2: i32,
    pub color3: i32,
}
object!(ItemLog {
    character_name,
    is_add_item,
    item_class,
    count,
    datestamp,
    timestamp,
    color1,
    color2,
    color3
});

/// GuildStorageLogsMessage: IsTodayLog, BriefLogs, ItemLogs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Logs {
    pub is_today_log: bool,
    pub brief_logs: Option<Vec<Option<BriefLog>>>,
    pub item_logs: Option<Vec<Option<ItemLog>>>,
}
object!(Logs {
    is_today_log,
    brief_logs,
    item_logs
});

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    GuildInventoryInfo(InventoryInfo),
    AddGuildStorageItem(ItemMove),
    PickGuildStorageItem(ItemMove),
    PurchaseGuildStorage(Purchase),
    ArrangeGuildStorageItem(Arrange),
    /// HandleGuildStorageSessionMessage.IsStarted.
    HandleGuildStorageSession(bool),
    UpdateGuildStorageSetting(Setting),
    GuildStorageLogs(Logs),
}
impl Message {
    pub const fn kind(&self) -> Kind {
        match self {
            Self::GuildInventoryInfo(_) => Kind::GuildInventoryInfo,
            Self::AddGuildStorageItem(_) => Kind::AddGuildStorageItem,
            Self::PickGuildStorageItem(_) => Kind::PickGuildStorageItem,
            Self::PurchaseGuildStorage(_) => Kind::PurchaseGuildStorage,
            Self::ArrangeGuildStorageItem(_) => Kind::ArrangeGuildStorageItem,
            Self::HandleGuildStorageSession(_) => Kind::HandleGuildStorageSession,
            Self::UpdateGuildStorageSetting(_) => Kind::UpdateGuildStorageSetting,
            Self::GuildStorageLogs(_) => Kind::GuildStorageLogs,
        }
    }
}

/// The message body after the presence byte (no envelope).
pub fn encode(
    kind: Kind,
    message: Option<&Message>,
    max_body: usize,
) -> Result<Zeroizing<Vec<u8>>, GuildStorageWireError> {
    let mut w = Writer {
        bytes: Vec::new(),
        limit: max_body,
    };
    let result = (|| -> Result<(), GuildStorageWireError> {
        let Some(m) = message else {
            w.append(&[0])?;
            return Ok(());
        };
        if kind != m.kind() {
            return Err(GuildStorageWireError::Kind);
        }
        w.append(&[1])?;
        match m {
            Message::GuildInventoryInfo(v) => v.write(&mut w)?,
            Message::AddGuildStorageItem(v) | Message::PickGuildStorageItem(v) => {
                v.write(&mut w)?
            }
            Message::PurchaseGuildStorage(v) => v.write(&mut w)?,
            Message::ArrangeGuildStorageItem(v) => v.write(&mut w)?,
            Message::HandleGuildStorageSession(v) => v.write(&mut w)?,
            Message::UpdateGuildStorageSetting(v) => v.write(&mut w)?,
            Message::GuildStorageLogs(v) => v.write(&mut w)?,
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

/// The packet (category, body) as sent: the registered category, or category 0 with the
/// type's GUID before the body (the TypeConverter envelope of an unregistered message).
pub fn packet(message: &Message, max_body: usize) -> Result<(i32, Vec<u8>), GuildStorageWireError> {
    let kind = message.kind();
    let body = encode(kind, Some(message), max_body)?;
    if kind.category() != 0 {
        return Ok((kind.category(), body.to_vec()));
    }
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
) -> Result<Option<Message>, GuildStorageWireError> {
    decode_body(kind, body, max_body, true)
}

fn decode_body(
    kind: Kind,
    body: &[u8],
    max_body: usize,
    strict: bool,
) -> Result<Option<Message>, GuildStorageWireError> {
    if body.len() > max_body {
        return Err(PayloadError::TooLarge.into());
    }
    let mut r = Reader { remaining: body };
    let result = if r.byte()? == 0 {
        None
    } else {
        let r = &mut r;
        Some(match kind {
            Kind::GuildInventoryInfo => Message::GuildInventoryInfo(Wire::read(r)?),
            Kind::AddGuildStorageItem => Message::AddGuildStorageItem(Wire::read(r)?),
            Kind::PickGuildStorageItem => Message::PickGuildStorageItem(Wire::read(r)?),
            Kind::PurchaseGuildStorage => Message::PurchaseGuildStorage(Wire::read(r)?),
            Kind::ArrangeGuildStorageItem => Message::ArrangeGuildStorageItem(Wire::read(r)?),
            Kind::HandleGuildStorageSession => Message::HandleGuildStorageSession(Wire::read(r)?),
            Kind::UpdateGuildStorageSetting => Message::UpdateGuildStorageSetting(Wire::read(r)?),
            Kind::GuildStorageLogs => Message::GuildStorageLogs(Wire::read(r)?),
        })
    };
    if strict && !r.remaining.is_empty() {
        return Err(PayloadError::TrailingBytes.into());
    }
    Ok(result)
}

/// The kind of an ingress body: category 0 with the type's GUID envelope (every guild
/// storage ingress message is unregistered).
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
) -> Result<Option<Message>, GuildStorageWireError> {
    if kind.category() != 0 && category == kind.category() {
        return decode_body(kind, body, max_body, false);
    }
    if category == 0 && body.len() <= max_body && body.get(..16) == Some(kind.type_id().as_bytes())
    {
        return decode_body(kind, &body[16..], max_body.saturating_sub(16), false);
    }
    Err(GuildStorageWireError::Type)
}
