//! Mail messages, BriefMailInfo, MailItemInfo and AttachedTransferItemInfo recovered from
//! the Framework serializer (ServiceCore.EndPointNetwork). Fields are written in declaration
//! order; reference-type elements of a collection carry their null flag.
use super::{
    inventory::{Wire, object},
    wire::{PayloadError, Reader, Writer},
};
use thiserror::Error;
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    QueryMailList,
    QueryMailInfo,
    SendMail,
    GetMailItem,
    DeleteMail,
    ReturnMail,
    MailList,
    MailInfo,
    MailSent,
    MailReceived,
    GetMailItemCompleted,
}
impl Kind {
    /// Messages the client sends to the frontend (ItemServiceProxy, InGame).
    pub const INGRESS: [Kind; 6] = [
        Kind::QueryMailList,
        Kind::QueryMailInfo,
        Kind::SendMail,
        Kind::GetMailItem,
        Kind::DeleteMail,
        Kind::ReturnMail,
    ];
    pub const fn category(self) -> i32 {
        match self {
            Self::QueryMailList => 530,
            Self::QueryMailInfo => 531,
            Self::SendMail => 567,
            Self::GetMailItem => 464,
            Self::DeleteMail => 452,
            Self::ReturnMail => 561,
            Self::MailList => 495,
            Self::MailInfo => 494,
            Self::MailSent => 497,
            Self::MailReceived => 496,
            Self::GetMailItemCompleted => 426,
        }
    }
    pub const fn type_id(self) -> Uuid {
        Uuid::from_u128(match self {
            Self::QueryMailList => 0x6f7864f6_5e49_33a3_a899_6a6801411695,
            Self::QueryMailInfo => 0x55a3b131_002a_3a3a_9e65_14132b97951d,
            Self::SendMail => 0xfc081bf4_efe6_312b_9de9_b6c07f604817,
            Self::GetMailItem => 0x5c2de52a_ae2c_3c6e_9060_14ca27b9eef3,
            Self::DeleteMail => 0x316f4412_0b62_3af6_9ea6_cf98aecce9cb,
            Self::ReturnMail => 0x6eb38330_0409_37d3_9c8b_8062e9f45d48,
            Self::MailList => 0x74393467_8b2a_3100_9eb1_b5a34ec3310b,
            Self::MailInfo => 0x720d93db_8a10_349d_9f31_81ae382857a6,
            Self::MailSent => 0x06e86b5b_c00a_335e_a28f_92f6a517b156,
            Self::MailReceived => 0x67e4050b_e402_3a58_be39_fce090eb16dd,
            Self::GetMailItemCompleted => 0x203f4aaf_d073_3e30_8f1f_4d5458b4e338,
        })
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MailWireError {
    #[error("mail message kind mismatch")]
    Kind,
    #[error("unsupported mail category/type")]
    Type,
    #[error(transparent)]
    Payload(#[from] PayloadError),
}

/// MailItemInfo: ExpireDateTimeDiff is ExpireDateTime.Ticks − UtcNow.Ticks (−1 without).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MailItemInfo {
    pub mail_item_id: i64,
    pub item_id: i64,
    pub item_class_ex: Option<String>,
    pub item_count: i32,
    pub received: bool,
    pub color1: i32,
    pub color2: i32,
    pub color3: i32,
    pub max_durability_bonus: i32,
    pub reduced_durability: i32,
    pub is_character_binded: bool,
    pub is_expireable: bool,
    pub expire_date_time_diff: i64,
    pub expire_date_time_tick: i64,
}
object!(MailItemInfo {
    mail_item_id,
    item_id,
    item_class_ex,
    item_count,
    received,
    color1,
    color2,
    color3,
    max_durability_bonus,
    reduced_durability,
    is_character_binded,
    is_expireable,
    expire_date_time_diff,
    expire_date_time_tick
});
/// BriefMailInfo: ExpireDate in seconds from now (−1 without), ItemAttached is the
/// ItemAttachStatus byte.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BriefMailInfo {
    pub mail_id: i64,
    pub mail_type: u8,
    pub from_name: Option<String>,
    pub to_name: Option<String>,
    pub title: Option<String>,
    pub expire_date: i32,
    pub is_read: bool,
    pub item_attached: u8,
    pub charged_gold: i32,
    pub attached_item_info: Option<Vec<Option<MailItemInfo>>>,
}
object!(BriefMailInfo {
    mail_id,
    mail_type,
    from_name,
    to_name,
    title,
    expire_date,
    is_read,
    item_attached,
    charged_gold,
    attached_item_info
});
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AttachedTransferItemInfo {
    pub item_id: i64,
    pub item_class: Option<String>,
    pub item_count: i32,
}
object!(AttachedTransferItemInfo {
    item_id,
    item_class,
    item_count
});
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SendMail {
    pub mail_type: u8,
    pub to_name: Option<String>,
    pub title: Option<String>,
    pub content: Option<String>,
    pub item_list: Option<Vec<Option<AttachedTransferItemInfo>>>,
    pub gold: i32,
    pub express: bool,
    pub charged_gold: i32,
}
object!(SendMail {
    mail_type,
    to_name,
    title,
    content,
    item_list,
    gold,
    express,
    charged_gold
});
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MailList {
    pub received_mail_list: Option<Vec<Option<BriefMailInfo>>>,
    pub sent_mail_list: Option<Vec<Option<BriefMailInfo>>>,
    pub new_mail_count: i32,
}
object!(MailList {
    received_mail_list,
    sent_mail_list,
    new_mail_count
});
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MailInfo {
    pub mail_id: i64,
    pub items: Option<Vec<Option<MailItemInfo>>>,
    pub mail_type: u8,
    pub from_name: Option<String>,
    pub title: Option<String>,
    pub content: Option<String>,
    pub charged_gold: i32,
}
object!(MailInfo {
    mail_id,
    items,
    mail_type,
    from_name,
    title,
    content,
    charged_gold
});
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MailReceived {
    pub from_name: Option<String>,
    pub mail_type: u8,
}
object!(MailReceived {
    from_name,
    mail_type
});
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GetMailItemCompleted {
    pub mail_id: i64,
    pub result: u8,
}
object!(GetMailItemCompleted { mail_id, result });

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    QueryMailList,
    /// QueryMailInfoMessage: MailID.
    QueryMailInfo(i64),
    SendMail(SendMail),
    /// GetMailItemMessage: MailID.
    GetMailItem(i64),
    /// DeleteMailMessage: MailList.
    DeleteMail(Option<Vec<i64>>),
    /// ReturnMailMessage: MailID.
    ReturnMail(i64),
    MailList(MailList),
    MailInfo(MailInfo),
    /// MailSentMessage: ErrorCode (MailSentMessage.ErrorCodeEnum).
    MailSent(i32),
    MailReceived(MailReceived),
    GetMailItemCompleted(GetMailItemCompleted),
}
impl Message {
    pub const fn kind(&self) -> Kind {
        match self {
            Self::QueryMailList => Kind::QueryMailList,
            Self::QueryMailInfo(_) => Kind::QueryMailInfo,
            Self::SendMail(_) => Kind::SendMail,
            Self::GetMailItem(_) => Kind::GetMailItem,
            Self::DeleteMail(_) => Kind::DeleteMail,
            Self::ReturnMail(_) => Kind::ReturnMail,
            Self::MailList(_) => Kind::MailList,
            Self::MailInfo(_) => Kind::MailInfo,
            Self::MailSent(_) => Kind::MailSent,
            Self::MailReceived(_) => Kind::MailReceived,
            Self::GetMailItemCompleted(_) => Kind::GetMailItemCompleted,
        }
    }
}

pub fn encode(
    kind: Kind,
    message: Option<&Message>,
    max_body: usize,
) -> Result<Zeroizing<Vec<u8>>, MailWireError> {
    let mut w = Writer {
        bytes: Vec::new(),
        limit: max_body,
    };
    let result = (|| -> Result<(), MailWireError> {
        let Some(m) = message else {
            w.append(&[0])?;
            return Ok(());
        };
        if kind != m.kind() {
            return Err(MailWireError::Kind);
        }
        w.append(&[1])?;
        match m {
            Message::QueryMailList => {}
            Message::QueryMailInfo(v) | Message::GetMailItem(v) | Message::ReturnMail(v) => {
                v.write(&mut w)?
            }
            Message::SendMail(v) => v.write(&mut w)?,
            Message::DeleteMail(v) => v.write(&mut w)?,
            Message::MailList(v) => v.write(&mut w)?,
            Message::MailInfo(v) => v.write(&mut w)?,
            Message::MailSent(v) => v.write(&mut w)?,
            Message::MailReceived(v) => v.write(&mut w)?,
            Message::GetMailItemCompleted(v) => v.write(&mut w)?,
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
pub fn decode(kind: Kind, body: &[u8], max_body: usize) -> Result<Option<Message>, MailWireError> {
    decode_body(kind, body, max_body, true)
}
fn decode_body(
    kind: Kind,
    body: &[u8],
    max_body: usize,
    strict: bool,
) -> Result<Option<Message>, MailWireError> {
    if body.len() > max_body {
        return Err(PayloadError::TooLarge.into());
    }
    let mut r = Reader { remaining: body };
    let result = if r.byte()? == 0 {
        None
    } else {
        let r = &mut r;
        Some(match kind {
            Kind::QueryMailList => Message::QueryMailList,
            Kind::QueryMailInfo => Message::QueryMailInfo(Wire::read(r)?),
            Kind::SendMail => Message::SendMail(Wire::read(r)?),
            Kind::GetMailItem => Message::GetMailItem(Wire::read(r)?),
            Kind::DeleteMail => Message::DeleteMail(Wire::read(r)?),
            Kind::ReturnMail => Message::ReturnMail(Wire::read(r)?),
            Kind::MailList => Message::MailList(Wire::read(r)?),
            Kind::MailInfo => Message::MailInfo(Wire::read(r)?),
            Kind::MailSent => Message::MailSent(Wire::read(r)?),
            Kind::MailReceived => Message::MailReceived(Wire::read(r)?),
            Kind::GetMailItemCompleted => Message::GetMailItemCompleted(Wire::read(r)?),
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
) -> Result<Option<Message>, MailWireError> {
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
    Err(MailWireError::Type)
}
