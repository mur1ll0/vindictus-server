//! Pet messages and PetStatusInfo recovered from the Framework serializer
//! (ServiceCore.EndPointNetwork, categories 313–332). Fields are written in declaration
//! order; DateTime is its 64-bit dateData (ticks with the kind in the two high bits).
use super::{
    inventory::{Wire, object},
    wire::{PayloadError, Reader, Writer},
};
use thiserror::Error;
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    MovePetSlot,
    RemovePet,
    SelectPet,
    QueryPetList,
    PetRevived,
    PetKilled,
    RemovePetSkill,
    PetCreateNameCheck,
    PetCreateNameCheckResult,
    PetCreate,
    UpdateCurrentPet,
    PetList,
    PetFeedList,
    RequestPetFoodShare,
    RequestPetFoodUnshare,
    PetChangeNameCheck,
    PetChangeNameCheckResult,
    PetChangeName,
    PetChangeNameResult,
    PetOperation,
}
impl Kind {
    /// Messages the client sends to the frontend (InGame) and the host sends in battle.
    pub const INGRESS: [Kind; 14] = [
        Kind::MovePetSlot,
        Kind::RemovePet,
        Kind::SelectPet,
        Kind::QueryPetList,
        Kind::PetRevived,
        Kind::PetKilled,
        Kind::RemovePetSkill,
        Kind::PetCreateNameCheck,
        Kind::PetCreate,
        Kind::RequestPetFoodShare,
        Kind::RequestPetFoodUnshare,
        Kind::PetChangeNameCheck,
        Kind::PetChangeName,
        Kind::PetOperation,
    ];
    pub const fn category(self) -> i32 {
        match self {
            Self::MovePetSlot => 313,
            Self::RemovePet => 314,
            Self::SelectPet => 315,
            Self::QueryPetList => 316,
            Self::PetRevived => 317,
            Self::PetKilled => 318,
            Self::RemovePetSkill => 319,
            Self::PetCreateNameCheck => 320,
            Self::PetCreateNameCheckResult => 321,
            Self::PetCreate => 322,
            Self::UpdateCurrentPet => 323,
            Self::PetList => 324,
            Self::PetFeedList => 325,
            Self::RequestPetFoodShare => 326,
            Self::RequestPetFoodUnshare => 327,
            Self::PetChangeNameCheck => 328,
            Self::PetChangeNameCheckResult => 329,
            Self::PetChangeName => 330,
            Self::PetChangeNameResult => 331,
            Self::PetOperation => 332,
        }
    }
    pub const fn type_id(self) -> Uuid {
        Uuid::from_u128(match self {
            Self::MovePetSlot => 0x86698206_912c_36be_9cc1_257a396935fb,
            Self::RemovePet => 0x9f8b0d6d_ae8e_388f_8841_11aa2aa37a0d,
            Self::SelectPet => 0x5736fde9_bc21_3b82_a84e_72bb0a517a14,
            Self::QueryPetList => 0xe94af65d_e66f_31fd_b8d0_53a042370b5c,
            Self::PetRevived => 0xe145a882_d89c_3494_97bf_607b1aeaaab5,
            Self::PetKilled => 0x3079bc3a_9b67_3de0_a504_87a66664647f,
            Self::RemovePetSkill => 0x8aaf9011_eb3a_3fc1_a40b_25602676547b,
            Self::PetCreateNameCheck => 0xefd95e57_04fa_3e93_9271_9bcc29422fb0,
            Self::PetCreateNameCheckResult => 0x133b280b_47e4_34cc_b16a_a8a03eb4dc97,
            Self::PetCreate => 0x942945c3_e95f_3bb4_9be2_a7f984434f41,
            Self::UpdateCurrentPet => 0x9fddbc1b_3c9e_3c79_8b50_457c8379c699,
            Self::PetList => 0x62804a1d_30eb_310c_8792_0b6aa839ffd3,
            Self::PetFeedList => 0x2fcaaf5c_a077_375d_933c_92d298581487,
            Self::RequestPetFoodShare => 0x0d488183_a8d0_3730_99ab_1088836ab2d4,
            Self::RequestPetFoodUnshare => 0x5922ce58_625f_3c91_8b08_7331f63b5a27,
            Self::PetChangeNameCheck => 0xad77125f_4472_336f_a50e_45d79ec35a0e,
            Self::PetChangeNameCheckResult => 0x8b057824_a031_3410_a24b_f52be6c03cf3,
            Self::PetChangeName => 0x041d653f_01e7_371a_b86a_1346b110d36c,
            Self::PetChangeNameResult => 0xa9c12ba2_27cd_3864_9786_42d4f295dff5,
            Self::PetOperation => 0xbf092479_3775_3d89_8f77_53bcc8c681f4,
        })
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PetWireError {
    #[error("pet message kind mismatch")]
    Kind,
    #[error("unsupported pet category/type")]
    Type,
    #[error(transparent)]
    Payload(#[from] PayloadError),
}

/// DateTime as the Framework writes the struct: its private dateData (UInt64).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DateTimeData(pub u64);
impl DateTimeData {
    const KIND_UTC: u64 = 1 << 62;
    /// DateTimeKind.Utc with `ticks` (0 ≤ ticks ≤ DateTime.MaxValue.Ticks).
    pub const fn utc(ticks: i64) -> Self {
        Self(ticks as u64 & 0x3fff_ffff_ffff_ffff | Self::KIND_UTC)
    }
    pub const fn ticks(self) -> i64 {
        (self.0 & 0x3fff_ffff_ffff_ffff) as i64
    }
}
impl Wire for DateTimeData {
    fn write(&self, w: &mut Writer) -> Result<(), PayloadError> {
        w.append(&self.0.to_be_bytes())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PayloadError> {
        Ok(Self(u64::from_be_bytes(r.fixed::<8>()?)))
    }
}

/// PetStatElement: the PetStatusBalanceInfo row of the pet's StatType and level.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PetStatElement {
    pub required_exp: i32,
    pub max_exp: i32,
    pub hp: i32,
    pub res_damage: i32,
    pub hp_recovery: i32,
    pub def_break: i32,
    pub atk_balance: i32,
    pub atk: i32,
    pub def: i32,
    pub critical: i32,
    pub res_critical: i32,
}
object!(PetStatElement {
    required_exp,
    max_exp,
    hp,
    res_damage,
    hp_recovery,
    def_break,
    atk_balance,
    atk,
    def,
    critical,
    res_critical
});
/// PetSkillElement: ExpireDateTimeDiff is ExpiredTime.Ticks − UtcNow.Ticks (−1 without).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PetSkillElement {
    pub skill_id: i32,
    pub slot_order: i32,
    pub open_level: i32,
    pub has_expire_date_time_info: bool,
    pub expire_date_time_diff: i64,
}
object!(PetSkillElement {
    skill_id,
    slot_order,
    open_level,
    has_expire_date_time_info,
    expire_date_time_diff
});
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PetAccessoryElement {
    pub item_class: Option<String>,
    pub slot_order: i32,
    pub accessory_size: i32,
    pub remaining_time: i32,
}
object!(PetAccessoryElement {
    item_class,
    slot_order,
    accessory_size,
    remaining_time
});
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PetFeedElement {
    pub feed_type: u8,
    pub count: i32,
}
object!(PetFeedElement { feed_type, count });
/// PetStatusInfo (no class field: the host creates the pet entity).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PetStatusInfo {
    pub pet_id: i64,
    pub pet_name: Option<String>,
    pub pet_type: i32,
    pub slot: i32,
    pub level: i32,
    pub exp: i32,
    pub desire: i32,
    pub stat: Option<PetStatElement>,
    pub skills: Option<Vec<Option<PetSkillElement>>>,
    pub accessories: Option<Vec<Option<PetAccessoryElement>>>,
    pub pet_status: u8,
    pub remain_active_time: i32,
    pub remain_expired_time: DateTimeData,
}
object!(PetStatusInfo {
    pet_id,
    pet_name,
    pet_type,
    slot,
    level,
    exp,
    desire,
    stat,
    skills,
    accessories,
    pet_status,
    remain_active_time,
    remain_expired_time
});
/// A nullable PetStatusInfo field of another message (CharacterSummary.Pet,
/// GameJoinMemberInfo.Pet).
pub(super) fn write_pet(w: &mut Writer, pet: Option<&PetStatusInfo>) -> Result<(), PayloadError> {
    match pet {
        None => w.append(&[0]),
        Some(p) => {
            w.append(&[1])?;
            p.write(w)
        }
    }
}
pub(super) fn read_pet(r: &mut Reader<'_>) -> Result<Option<PetStatusInfo>, PayloadError> {
    Option::<PetStatusInfo>::read(r)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PetList {
    pub pets: Option<Vec<Option<PetStatusInfo>>>,
    pub is_total: bool,
}
object!(PetList { pets, is_total });
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PetFeedList {
    pub feeds: Option<Vec<Option<PetFeedElement>>>,
    pub is_total: bool,
}
object!(PetFeedList { feeds, is_total });
/// UpdateCurrentPetMessage: UID is the member's slot in the game.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateCurrentPet {
    pub uid: i32,
    pub current_pet: Option<PetStatusInfo>,
}
object!(UpdateCurrentPet { uid, current_pet });
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MovePetSlot {
    pub pet_id: i64,
    pub dest_slot_no: i32,
}
object!(MovePetSlot {
    pet_id,
    dest_slot_no
});
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PetRevived {
    pub caster_tag: i32,
    pub reviver_tag: i32,
    pub pet_id: i64,
    pub method: Option<String>,
}
object!(PetRevived {
    caster_tag,
    reviver_tag,
    pet_id,
    method
});
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PetKilled {
    pub tag: i32,
    pub pet_id: i64,
}
object!(PetKilled { tag, pet_id });
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemovePetSkill {
    pub pet_id: i64,
    pub pet_skill_id: i32,
}
object!(RemovePetSkill {
    pet_id,
    pet_skill_id
});
/// PetCreateNameCheckMessage and PetCreateMessage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PetCreate {
    pub item_id: i64,
    pub pet_name: Option<String>,
}
object!(PetCreate { item_id, pet_name });
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PetCreateNameCheckResult {
    pub item_id: i64,
    pub pet_name: Option<String>,
    pub is_success: bool,
}
object!(PetCreateNameCheckResult {
    item_id,
    pet_name,
    is_success
});
/// PetChangeNameCheckMessage and PetChangeNameMessage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PetChangeName {
    pub item_id: i64,
    pub pet_id: i64,
    pub pet_name: Option<String>,
}
object!(PetChangeName {
    item_id,
    pet_id,
    pet_name
});
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PetChangeNameCheckResult {
    pub is_success: bool,
    pub item_id: i64,
    pub pet_id: i64,
    pub pet_name: Option<String>,
    pub result_type: Option<String>,
}
object!(PetChangeNameCheckResult {
    is_success,
    item_id,
    pet_id,
    pet_name,
    result_type
});
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PetChangeNameResult {
    pub is_success: bool,
    pub pet_name: Option<String>,
    pub result_type: Option<String>,
}
object!(PetChangeNameResult {
    is_success,
    pet_name,
    result_type
});
/// PetOperationMessage: OperationCode is PetOperationType (Int32).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PetOperation {
    pub operation_code: i32,
    pub pet_id: i64,
    pub arg: Option<String>,
    pub value1: i32,
    pub value2: i32,
    pub value3: i32,
}
object!(PetOperation {
    operation_code,
    pet_id,
    arg,
    value1,
    value2,
    value3
});

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    MovePetSlot(MovePetSlot),
    /// RemovePetMessage: PetID.
    RemovePet(i64),
    /// SelectPetMessage: PetID (0 recalls the current pet).
    SelectPet(i64),
    QueryPetList,
    PetRevived(PetRevived),
    PetKilled(PetKilled),
    RemovePetSkill(RemovePetSkill),
    PetCreateNameCheck(PetCreate),
    PetCreateNameCheckResult(PetCreateNameCheckResult),
    PetCreate(PetCreate),
    UpdateCurrentPet(UpdateCurrentPet),
    PetList(PetList),
    PetFeedList(PetFeedList),
    /// RequestPetFoodShareMessage: ItemID.
    RequestPetFoodShare(i64),
    /// RequestPetFoodUnshareMessage: ItemClass.
    RequestPetFoodUnshare(Option<String>),
    PetChangeNameCheck(PetChangeName),
    PetChangeNameCheckResult(PetChangeNameCheckResult),
    PetChangeName(PetChangeName),
    PetChangeNameResult(PetChangeNameResult),
    PetOperation(PetOperation),
}
impl Message {
    pub const fn kind(&self) -> Kind {
        match self {
            Self::MovePetSlot(_) => Kind::MovePetSlot,
            Self::RemovePet(_) => Kind::RemovePet,
            Self::SelectPet(_) => Kind::SelectPet,
            Self::QueryPetList => Kind::QueryPetList,
            Self::PetRevived(_) => Kind::PetRevived,
            Self::PetKilled(_) => Kind::PetKilled,
            Self::RemovePetSkill(_) => Kind::RemovePetSkill,
            Self::PetCreateNameCheck(_) => Kind::PetCreateNameCheck,
            Self::PetCreateNameCheckResult(_) => Kind::PetCreateNameCheckResult,
            Self::PetCreate(_) => Kind::PetCreate,
            Self::UpdateCurrentPet(_) => Kind::UpdateCurrentPet,
            Self::PetList(_) => Kind::PetList,
            Self::PetFeedList(_) => Kind::PetFeedList,
            Self::RequestPetFoodShare(_) => Kind::RequestPetFoodShare,
            Self::RequestPetFoodUnshare(_) => Kind::RequestPetFoodUnshare,
            Self::PetChangeNameCheck(_) => Kind::PetChangeNameCheck,
            Self::PetChangeNameCheckResult(_) => Kind::PetChangeNameCheckResult,
            Self::PetChangeName(_) => Kind::PetChangeName,
            Self::PetChangeNameResult(_) => Kind::PetChangeNameResult,
            Self::PetOperation(_) => Kind::PetOperation,
        }
    }
}

pub fn encode(
    kind: Kind,
    message: Option<&Message>,
    max_body: usize,
) -> Result<Zeroizing<Vec<u8>>, PetWireError> {
    let mut w = Writer {
        bytes: Vec::new(),
        limit: max_body,
    };
    let result = (|| -> Result<(), PetWireError> {
        let Some(m) = message else {
            w.append(&[0])?;
            return Ok(());
        };
        if kind != m.kind() {
            return Err(PetWireError::Kind);
        }
        w.append(&[1])?;
        match m {
            Message::QueryPetList => {}
            Message::MovePetSlot(v) => v.write(&mut w)?,
            Message::RemovePet(v) | Message::SelectPet(v) | Message::RequestPetFoodShare(v) => {
                v.write(&mut w)?
            }
            Message::PetRevived(v) => v.write(&mut w)?,
            Message::PetKilled(v) => v.write(&mut w)?,
            Message::RemovePetSkill(v) => v.write(&mut w)?,
            Message::PetCreateNameCheck(v) | Message::PetCreate(v) => v.write(&mut w)?,
            Message::PetCreateNameCheckResult(v) => v.write(&mut w)?,
            Message::UpdateCurrentPet(v) => v.write(&mut w)?,
            Message::PetList(v) => v.write(&mut w)?,
            Message::PetFeedList(v) => v.write(&mut w)?,
            Message::RequestPetFoodUnshare(v) => v.write(&mut w)?,
            Message::PetChangeNameCheck(v) | Message::PetChangeName(v) => v.write(&mut w)?,
            Message::PetChangeNameCheckResult(v) => v.write(&mut w)?,
            Message::PetChangeNameResult(v) => v.write(&mut w)?,
            Message::PetOperation(v) => v.write(&mut w)?,
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
pub fn decode(kind: Kind, body: &[u8], max_body: usize) -> Result<Option<Message>, PetWireError> {
    decode_body(kind, body, max_body, true)
}
fn decode_body(
    kind: Kind,
    body: &[u8],
    max_body: usize,
    strict: bool,
) -> Result<Option<Message>, PetWireError> {
    if body.len() > max_body {
        return Err(PayloadError::TooLarge.into());
    }
    let mut r = Reader { remaining: body };
    let result = if r.byte()? == 0 {
        None
    } else {
        let r = &mut r;
        Some(match kind {
            Kind::MovePetSlot => Message::MovePetSlot(Wire::read(r)?),
            Kind::RemovePet => Message::RemovePet(Wire::read(r)?),
            Kind::SelectPet => Message::SelectPet(Wire::read(r)?),
            Kind::QueryPetList => Message::QueryPetList,
            Kind::PetRevived => Message::PetRevived(Wire::read(r)?),
            Kind::PetKilled => Message::PetKilled(Wire::read(r)?),
            Kind::RemovePetSkill => Message::RemovePetSkill(Wire::read(r)?),
            Kind::PetCreateNameCheck => Message::PetCreateNameCheck(Wire::read(r)?),
            Kind::PetCreateNameCheckResult => Message::PetCreateNameCheckResult(Wire::read(r)?),
            Kind::PetCreate => Message::PetCreate(Wire::read(r)?),
            Kind::UpdateCurrentPet => Message::UpdateCurrentPet(Wire::read(r)?),
            Kind::PetList => Message::PetList(Wire::read(r)?),
            Kind::PetFeedList => Message::PetFeedList(Wire::read(r)?),
            Kind::RequestPetFoodShare => Message::RequestPetFoodShare(Wire::read(r)?),
            Kind::RequestPetFoodUnshare => Message::RequestPetFoodUnshare(Wire::read(r)?),
            Kind::PetChangeNameCheck => Message::PetChangeNameCheck(Wire::read(r)?),
            Kind::PetChangeNameCheckResult => Message::PetChangeNameCheckResult(Wire::read(r)?),
            Kind::PetChangeName => Message::PetChangeName(Wire::read(r)?),
            Kind::PetChangeNameResult => Message::PetChangeNameResult(Wire::read(r)?),
            Kind::PetOperation => Message::PetOperation(Wire::read(r)?),
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
) -> Result<Option<Message>, PetWireError> {
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
    Err(PetWireError::Type)
}
