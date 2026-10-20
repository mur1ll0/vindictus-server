//! Recovered wire codecs for the minimum character selection and town channel entry path.
//! Pure serialization/deserialization only; no database or session state mutations.
use super::{
    character_list::{CharacterListError, MAX_DICTIONARY, Summary, read_summary, write_summary},
    encode_integer,
    wire::{PayloadError, Reader, Writer},
};
use thiserror::Error;
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    NoticeGameEnvironment,
    Identify,
    EnterChannel,
    ChannelServerAddress,
    IdentifyFailed,
    MovePartition,
    UpdateAction,
    Disappeared,
    NotifyLook,
    NotifyAction,
    SelectCharacter,
    LoginOk,
    CharacterCommonInfo,
    UpdateStat,
}

impl Kind {
    pub const fn category(self) -> i32 {
        match self {
            Self::NoticeGameEnvironment => 345,
            Self::Identify => 363,
            Self::EnterChannel => 364,
            Self::ChannelServerAddress => 365,
            Self::IdentifyFailed => 366,
            Self::MovePartition => 368,
            Self::UpdateAction => 369,
            Self::Disappeared => 372,
            Self::NotifyLook => 373,
            Self::NotifyAction => 374,
            Self::SelectCharacter => 481,
            Self::LoginOk => 483,
            Self::CharacterCommonInfo => 609,
            Self::UpdateStat => 619,
        }
    }

    pub const fn type_id(self) -> Uuid {
        Uuid::from_u128(match self {
            Self::NoticeGameEnvironment => 0x97636de8_6ff1_3d4c_8c53_b8f1da851df4,
            Self::Identify => 0x428dee64_190d_3cf6_8450_3e511769c608,
            Self::EnterChannel => 0x79ce9a53_8956_3645_a67b_f69681ea7a35,
            Self::ChannelServerAddress => 0x85bf1995_487c_3e7e_8603_e88d75d21b46,
            Self::IdentifyFailed => 0x42fa5d33_391e_320b_85db_0c61e1b7f6ba,
            Self::MovePartition => 0xd3cf4e65_f3a6_37f4_aae6_d8819a02e728,
            Self::UpdateAction => 0xbb98e908_edbe_32d5_89c2_eb38d6508cad,
            Self::Disappeared => 0x4f13a705_3f02_37cb_b869_ebfbf3852ce2,
            Self::NotifyLook => 0x711f48ae_c601_344c_bb5a_9320b001f809,
            Self::NotifyAction => 0x86b97474_ac74_3722_bda1_a8ba43968733,
            Self::SelectCharacter => 0x1f34c672_5581_353f_9944_d5ab72e808a5,
            Self::LoginOk => 0xe301ee14_10c2_3886_8533_fbba9ef1cbe1,
            Self::CharacterCommonInfo => 0x9756914f_6753_3228_b0b8_38b30dc1a612,
            Self::UpdateStat => 0xa1407a79_5353_3455_9a2c_859f8825c200,
        })
    }
}

/// C# value type (`struct Vector3D`); serialized without a leading nullability byte.
/// Stores raw IEEE-754 bits to preserve signed zero and NaN payloads across roundtrips.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Vector3D {
    pub x_bits: u32,
    pub y_bits: u32,
    pub z_bits: u32,
}

/// C# value type (`struct ActionSync`); serialized without a leading nullability byte.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ActionSync {
    pub position: Vector3D,
    pub velocity: Vector3D,
    pub yaw_bits: u32,
    pub sequence: i16,
    pub action_state_index: i16,
    pub start_time_bits: u32,
    pub state: i32,
}

#[derive(PartialEq, Eq)]
pub enum Message<'a> {
    NoticeGameEnvironment {
        cafe_type: i32,
        is_otp: bool,
    },
    Identify {
        id: i64,
        key: i32,
    },
    EnterChannel {
        channel_id: i64,
        partition_id: i64,
        action: ActionSync,
    },
    ChannelServerAddress {
        channel_id: i64,
        address: Option<&'a str>,
        port: i32,
        key: i32,
    },
    IdentifyFailed(Option<&'a str>),
    MovePartition {
        target_partition_id: i64,
    },
    UpdateAction {
        data: ActionSync,
    },
    Disappeared {
        id: i64,
    },
    NotifyLook {
        id: i64,
        look: Option<Summary<'a>>,
    },
    NotifyAction {
        id: i64,
        action: ActionSync,
    },
    SelectCharacter {
        index: i32,
    },
    LoginOk {
        region_code: i32,
        time: i32,
        limited: i32,
        facebook_token: Option<&'a str>,
        user_care_type: i32,
        user_care_next_state: i32,
        map_state_info: u8,
    },
    CharacterCommonInfo {
        query_id: i64,
        info: Option<Summary<'a>>,
    },
    UpdateStat {
        stat: Option<Vec<(&'a str, i32)>>,
    },
}

impl Message<'_> {
    pub const fn kind(&self) -> Kind {
        match self {
            Self::NoticeGameEnvironment { .. } => Kind::NoticeGameEnvironment,
            Self::Identify { .. } => Kind::Identify,
            Self::EnterChannel { .. } => Kind::EnterChannel,
            Self::ChannelServerAddress { .. } => Kind::ChannelServerAddress,
            Self::IdentifyFailed(_) => Kind::IdentifyFailed,
            Self::MovePartition { .. } => Kind::MovePartition,
            Self::UpdateAction { .. } => Kind::UpdateAction,
            Self::Disappeared { .. } => Kind::Disappeared,
            Self::NotifyLook { .. } => Kind::NotifyLook,
            Self::NotifyAction { .. } => Kind::NotifyAction,
            Self::SelectCharacter { .. } => Kind::SelectCharacter,
            Self::LoginOk { .. } => Kind::LoginOk,
            Self::CharacterCommonInfo { .. } => Kind::CharacterCommonInfo,
            Self::UpdateStat { .. } => Kind::UpdateStat,
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MinimumMapWireError {
    #[error("minimum-map message kind mismatch")]
    Kind,
    #[error("unsupported minimum-map category/type")]
    Type,
    #[error("duplicate stat dictionary key")]
    DuplicateKey,
    #[error(transparent)]
    Summary(#[from] CharacterListError),
    #[error(transparent)]
    Payload(#[from] PayloadError),
}

fn write_vector(w: &mut Writer, v: &Vector3D) -> Result<(), PayloadError> {
    w.append(&v.x_bits.to_be_bytes())?;
    w.append(&v.y_bits.to_be_bytes())?;
    w.append(&v.z_bits.to_be_bytes())
}

fn read_vector(r: &mut Reader<'_>) -> Result<Vector3D, PayloadError> {
    Ok(Vector3D {
        x_bits: u32::from_be_bytes(r.fixed()?),
        y_bits: u32::from_be_bytes(r.fixed()?),
        z_bits: u32::from_be_bytes(r.fixed()?),
    })
}

fn write_action(w: &mut Writer, a: &ActionSync) -> Result<(), PayloadError> {
    write_action_fields(w, a)
}

fn write_action_fields(w: &mut Writer, a: &ActionSync) -> Result<(), PayloadError> {
    write_vector(w, &a.position)?;
    write_vector(w, &a.velocity)?;
    w.append(&a.yaw_bits.to_be_bytes())?;
    w.append(&a.sequence.to_be_bytes())?;
    w.append(&a.action_state_index.to_be_bytes())?;
    w.append(&a.start_time_bits.to_be_bytes())?;
    w.append(&a.state.to_be_bytes())
}

fn read_action(r: &mut Reader<'_>) -> Result<ActionSync, PayloadError> {
    Ok(ActionSync {
        position: read_vector(r)?,
        velocity: read_vector(r)?,
        yaw_bits: u32::from_be_bytes(r.fixed()?),
        sequence: i16::from_be_bytes(r.fixed()?),
        action_state_index: i16::from_be_bytes(r.fixed()?),
        start_time_bits: u32::from_be_bytes(r.fixed()?),
        state: i32::from_be_bytes(r.fixed()?),
    })
}

pub fn encode(
    kind: Kind,
    message: Option<&Message<'_>>,
    max_body: usize,
) -> Result<Zeroizing<Vec<u8>>, MinimumMapWireError> {
    let mut w = Writer {
        bytes: Vec::new(),
        limit: max_body,
    };
    let result = (|| -> Result<(), MinimumMapWireError> {
        let Some(m) = message else {
            w.append(&[0])?;
            return Ok(());
        };
        if m.kind() != kind {
            return Err(MinimumMapWireError::Kind);
        }
        w.append(&[1])?;
        match m {
            Message::NoticeGameEnvironment { cafe_type, is_otp } => {
                w.append(&cafe_type.to_be_bytes())?;
                w.append(&[u8::from(*is_otp)])?;
            }
            Message::Identify { id, key } => {
                w.append(&id.to_be_bytes())?;
                w.append(&key.to_be_bytes())?;
            }
            Message::EnterChannel {
                channel_id,
                partition_id,
                action,
            } => {
                w.append(&channel_id.to_be_bytes())?;
                w.append(&partition_id.to_be_bytes())?;
                write_action(&mut w, action)?;
            }
            Message::ChannelServerAddress {
                channel_id,
                address,
                port,
                key,
            } => {
                w.append(&channel_id.to_be_bytes())?;
                w.nullable(address.map(str::as_bytes))?;
                w.append(&port.to_be_bytes())?;
                w.append(&key.to_be_bytes())?;
            }
            Message::IdentifyFailed(error) => {
                w.nullable(error.map(str::as_bytes))?;
            }
            Message::MovePartition {
                target_partition_id,
            } => {
                w.append(&target_partition_id.to_be_bytes())?;
            }
            Message::UpdateAction { data } => {
                write_action(&mut w, data)?;
            }
            Message::Disappeared { id } => {
                w.append(&id.to_be_bytes())?;
            }
            Message::NotifyLook { id, look } => {
                w.append(&id.to_be_bytes())?;
                write_summary(&mut w, look.as_ref())?;
            }
            Message::NotifyAction { id, action } => {
                w.append(&id.to_be_bytes())?;
                write_action(&mut w, action)?;
            }
            Message::SelectCharacter { index } => {
                w.append(&index.to_be_bytes())?;
            }
            Message::LoginOk {
                region_code,
                time,
                limited,
                facebook_token,
                user_care_type,
                user_care_next_state,
                map_state_info,
            } => {
                w.append(&region_code.to_be_bytes())?;
                w.append(&time.to_be_bytes())?;
                w.append(&limited.to_be_bytes())?;
                w.nullable(facebook_token.map(str::as_bytes))?;
                w.append(&user_care_type.to_be_bytes())?;
                w.append(&user_care_next_state.to_be_bytes())?;
                w.append(&[*map_state_info])?;
            }
            Message::CharacterCommonInfo { query_id, info } => {
                w.append(&query_id.to_be_bytes())?;
                write_summary(&mut w, info.as_ref())?;
            }
            Message::UpdateStat { stat } => match stat {
                None => w.append(&[0])?,
                Some(entries) => {
                    if entries.len() > MAX_DICTIONARY {
                        return Err(PayloadError::TooLarge.into());
                    }
                    w.append(&encode_integer((entries.len() + 1) as i32))?;
                    for (idx, (key, val)) in entries.iter().enumerate() {
                        if entries[..idx].iter().any(|(other, _)| other == key) {
                            return Err(MinimumMapWireError::DuplicateKey);
                        }
                        w.nullable(Some(key.as_bytes()))?;
                        w.append(&val.to_be_bytes())?;
                    }
                }
            },
        }
        Ok(())
    })();
    if result.is_err() {
        w.bytes.zeroize();
    }
    result?;
    Ok(Zeroizing::new(w.bytes))
}

pub fn decode<'a>(
    kind: Kind,
    body: &'a [u8],
    max_body: usize,
) -> Result<Option<Message<'a>>, MinimumMapWireError> {
    if body.len() > max_body {
        return Err(PayloadError::TooLarge.into());
    }
    let mut r = Reader { remaining: body };
    let out = if r.byte()? == 0 {
        None
    } else {
        Some(match kind {
            Kind::NoticeGameEnvironment => Message::NoticeGameEnvironment {
                cafe_type: i32::from_be_bytes(r.fixed()?),
                is_otp: r.byte()? != 0,
            },
            Kind::Identify => Message::Identify {
                id: i64::from_be_bytes(r.fixed()?),
                key: i32::from_be_bytes(r.fixed()?),
            },
            Kind::EnterChannel => Message::EnterChannel {
                channel_id: i64::from_be_bytes(r.fixed()?),
                partition_id: i64::from_be_bytes(r.fixed()?),
                action: read_action(&mut r)?,
            },
            Kind::ChannelServerAddress => Message::ChannelServerAddress {
                channel_id: i64::from_be_bytes(r.fixed()?),
                address: r.string()?,
                port: i32::from_be_bytes(r.fixed()?),
                key: i32::from_be_bytes(r.fixed()?),
            },
            Kind::IdentifyFailed => Message::IdentifyFailed(r.string()?),
            Kind::MovePartition => Message::MovePartition {
                target_partition_id: i64::from_be_bytes(r.fixed()?),
            },
            Kind::UpdateAction => Message::UpdateAction {
                data: read_action(&mut r)?,
            },
            Kind::Disappeared => Message::Disappeared {
                id: i64::from_be_bytes(r.fixed()?),
            },
            Kind::NotifyLook => Message::NotifyLook {
                id: i64::from_be_bytes(r.fixed()?),
                look: read_summary(&mut r)?,
            },
            Kind::NotifyAction => Message::NotifyAction {
                id: i64::from_be_bytes(r.fixed()?),
                action: read_action(&mut r)?,
            },
            Kind::SelectCharacter => Message::SelectCharacter {
                index: i32::from_be_bytes(r.fixed()?),
            },
            Kind::LoginOk => Message::LoginOk {
                region_code: i32::from_be_bytes(r.fixed()?),
                time: i32::from_be_bytes(r.fixed()?),
                limited: i32::from_be_bytes(r.fixed()?),
                facebook_token: r.string()?,
                user_care_type: i32::from_be_bytes(r.fixed()?),
                user_care_next_state: i32::from_be_bytes(r.fixed()?),
                map_state_info: r.byte()?,
            },
            Kind::CharacterCommonInfo => Message::CharacterCommonInfo {
                query_id: i64::from_be_bytes(r.fixed()?),
                info: read_summary(&mut r)?,
            },
            Kind::UpdateStat => {
                let n = r.count()?;
                if n.is_some_and(|n| n > MAX_DICTIONARY) {
                    return Err(PayloadError::TooLarge.into());
                }
                let stat = if let Some(n) = n {
                    let mut rows = Vec::with_capacity(n);
                    for _ in 0..n {
                        let key = r.string()?.ok_or(PayloadError::NullKey)?;
                        let val = i32::from_be_bytes(r.fixed()?);
                        if rows.iter().any(|(other, _)| *other == key) {
                            return Err(MinimumMapWireError::DuplicateKey);
                        }
                        rows.push((key, val));
                    }
                    Some(rows)
                } else {
                    None
                };
                Message::UpdateStat { stat }
            }
        })
    };
    if !r.remaining.is_empty() {
        return Err(PayloadError::TrailingBytes.into());
    }
    Ok(out)
}

pub fn decode_ingress<'a>(
    kind: Kind,
    category: i32,
    body: &'a [u8],
    max_body: usize,
) -> Result<Option<Message<'a>>, MinimumMapWireError> {
    if body.len() > max_body {
        return Err(PayloadError::TooLarge.into());
    }
    if category == kind.category() {
        return decode(kind, body, max_body);
    }
    if category != 0 {
        return Err(MinimumMapWireError::Type);
    }
    let guid = body.get(..16).ok_or(PayloadError::Truncated)?;
    let fields = &body[16..];
    if guid == kind.type_id().as_bytes() {
        return decode(kind, fields, max_body - 16);
    }
    if guid == Uuid::from_u128(0x81c5fe01_027c_3e1c_98d5_da9c9862aa21).as_bytes() && fields == [0] {
        return Ok(None);
    }
    Err(MinimumMapWireError::Type)
}
