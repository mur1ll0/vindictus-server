//! Bounded inventory snapshot codec recovered from the Framework serializer.
//! No storage reads or item mutations.
use super::{
    encode_integer,
    wire::{PayloadError, Reader, Writer},
};
use thiserror::Error;
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};
const MAX_ROWS: usize = 4096;
const MOVE_TYPE_ID: u128 = 0x7c957f24_e8d7_338f_a93c_97bc6903f320;
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    QueryInventory,
    InventoryInfo,
    UpdateInventoryInfo,
    ItemFail,
    MoveInventoryItem,
    EquipBundle,
    QuickSlotInfo,
    QueryAp,
    Ap,
    UseInventoryItem,
    UseInventoryItemOk,
    QuerySkillList,
    SkillList,
}
impl Kind {
    pub const fn category(self) -> i32 {
        match self {
            Self::QueryInventory => 529,
            Self::InventoryInfo => 468,
            Self::UpdateInventoryInfo => 617,
            Self::ItemFail => 358,
            Self::MoveInventoryItem => 502,
            Self::EquipBundle => 456,
            Self::QuickSlotInfo => 551,
            Self::QueryAp => 526,
            Self::Ap => 229,
            Self::UseInventoryItem => 629,
            Self::UseInventoryItemOk => 627,
            Self::QuerySkillList => 540,
            Self::SkillList => 583,
        }
    }
    pub const fn type_id(self) -> Uuid {
        Uuid::from_u128(match self {
            Self::QueryInventory => 0xefe95078_ef7e_34eb_82e1_2daf3280a576,
            Self::InventoryInfo => 0xbdc965b5_aca1_3b62_b222_279e96683d93,
            Self::UpdateInventoryInfo => 0xabacd3bc_1a84_3570_a62c_f1b0007675fb,
            Self::ItemFail => 0x2f756b90_8fba_38b8_bcb2_a51f0f132502,
            Self::MoveInventoryItem => MOVE_TYPE_ID,
            Self::EquipBundle => 0x6652c42f_bf01_3da0_9415_f390b085e456,
            Self::QuickSlotInfo => 0xedc4290e_196c_3f17_903f_d0f3317e47d9,
            Self::QueryAp => 0xf061ae98_f5c8_333d_82ca_3b217e268120,
            Self::Ap => 0x3d59ac68_6b3c_3f92_968e_b569b973f065,
            Self::UseInventoryItem => 0xc182cb18_7469_3390_a0e7_8eba31d6e45d,
            Self::UseInventoryItemOk => 0x451d459f_bb17_325c_bb10_d608f03f4c3f,
            Self::QuerySkillList => 0x860e6484_d1b5_34a9_9412_70807bbbf79c,
            Self::SkillList => 0xdd3b94a9_64da_381a_b2e7_0fad58835746,
        })
    }
}
#[derive(Debug, Error, PartialEq, Eq)]
pub enum InventoryWireError {
    #[error("inventory message kind mismatch")]
    Kind,
    #[error("unsupported inventory category/type")]
    Type,
    #[error(transparent)]
    Payload(#[from] PayloadError),
}
#[derive(Clone, PartialEq, Eq)]
pub struct Dict<T>(pub Option<Vec<(i32, T)>>);
#[derive(Clone, PartialEq, Eq)]
pub struct Storage {
    pub id: u8,
    pub available: u8,
    pub last_seconds: i32,
    pub name: Option<String>,
    pub tag: i32,
}
#[derive(Clone, PartialEq, Eq)]
pub struct Attribute {
    pub name: Option<String>,
    pub value: Option<String>,
    pub arg: i32,
    pub arg2: i32,
}
#[derive(Clone, PartialEq, Eq)]
pub struct Slot {
    pub id: i64,
    pub class: Option<String>,
    pub count: i32,
    pub tab: i32,
    pub slot: i32,
    pub expireable: bool,
    pub expire_diff: i64,
    pub expire_ticks: i64,
    pub colors: Option<Vec<i32>>,
    pub durability: i32,
    pub maximum: i32,
    pub bonus: i32,
    pub tradable: bool,
    pub expander: Option<String>,
    pub attributes: Option<Vec<Option<Attribute>>>,
    pub prefix: Dict<bool>,
    pub suffix: Dict<bool>,
    pub min_price: i32,
    pub max_price: i32,
    pub avg_price: i32,
    pub min_up_diff: i64,
}
#[derive(Clone, PartialEq, Eq)]
pub struct QuickSlots {
    pub classes: Dict<Option<String>>,
}
#[derive(Clone, PartialEq, Eq)]
pub struct Inventory {
    pub storages: Option<Vec<Option<Storage>>>,
    pub slots: Option<Vec<Option<Slot>>>,
    pub equipment: Dict<i64>,
    pub quick_slots: Option<QuickSlots>,
    pub unequippable: Option<Vec<i32>>,
}
/// ServiceCore HeroesString: Format and Parameters (ICollection<string>).
#[derive(Clone, PartialEq, Eq)]
pub struct HeroesString {
    pub format: Option<String>,
    pub parameters: Option<Vec<Option<String>>>,
}
#[derive(Clone, PartialEq, Eq)]
pub enum Message {
    QueryInventory,
    InventoryInfo(Inventory),
    /// UpdateInventoryInfoMessage.slotInfos.
    UpdateInventoryInfo(Option<Vec<Option<Slot>>>),
    /// ItemFailMessage.Message.
    ItemFail(Option<HeroesString>),
    /// MoveInventoryItemMessage: targetItemID, targetStorage, targetSlot.
    MoveInventoryItem(Move),
    /// EquipBundleMessage: ItemClass ("" clears), QuickSlotID.
    EquipBundle(Bundle),
    /// QuickSlotInfoMessage.info.
    QuickSlotInfo(Option<QuickSlots>),
    /// QueryAPMessage (fieldless).
    QueryAp,
    /// APMessage: field declaration order nextBonusTimeTicks, ap, maxap, apBonusInterval.
    Ap(Ap),
    UseInventoryItem(UseItem),
    /// UseInventoryItemOkMessage.UseItemClass.
    UseInventoryItemOk(Option<String>),
    /// QuerySkillListMessage (fieldless).
    QuerySkillList,
    SkillList(SkillList),
}
/// BriefSkillEnhance in declaration order.
#[derive(Clone, PartialEq, Eq)]
pub struct SkillEnhance {
    pub group_key: i32,
    pub index_key: i32,
    pub kind: u8,
    pub reduce_durability: i32,
    pub max_durability_bonus: i32,
}
/// BriefSkillInfo in declaration order.
#[derive(Clone, PartialEq, Eq)]
pub struct BriefSkill {
    pub id: Option<String>,
    pub base_rank: i32,
    pub final_rank: i32,
    pub required_ap: i32,
    pub is_locked: u8,
    pub can_start_training: u8,
    pub current_ap: i32,
    pub reset_ap: i32,
    pub untrain_ap: i32,
    pub enhances: Option<Vec<Option<SkillEnhance>>>,
}
/// SkillListMessage: LearningSkillID, CurrentAp, SkillList, IsResetVocation.
#[derive(Clone, PartialEq, Eq)]
pub struct SkillList {
    pub learning: Option<String>,
    pub current_ap: i32,
    pub skills: Option<Vec<Option<BriefSkill>>>,
    pub reset_vocation: bool,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Ap {
    pub next_bonus_ms: i64,
    pub ap: i32,
    pub max_ap: i32,
    pub bonus_interval: i32,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct UseItem {
    pub item_id: i64,
    pub target_item_id: i64,
}
#[derive(Clone, PartialEq, Eq)]
pub struct Bundle {
    pub class: Option<String>,
    pub quick_slot: i32,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Move {
    pub item_id: i64,
    pub storage: u8,
    pub slot: i32,
}
impl Message {
    pub const fn kind(&self) -> Kind {
        match self {
            Self::QueryInventory => Kind::QueryInventory,
            Self::InventoryInfo(_) => Kind::InventoryInfo,
            Self::UpdateInventoryInfo(_) => Kind::UpdateInventoryInfo,
            Self::ItemFail(_) => Kind::ItemFail,
            Self::MoveInventoryItem(_) => Kind::MoveInventoryItem,
            Self::EquipBundle(_) => Kind::EquipBundle,
            Self::QuickSlotInfo(_) => Kind::QuickSlotInfo,
            Self::QueryAp => Kind::QueryAp,
            Self::Ap(_) => Kind::Ap,
            Self::UseInventoryItem(_) => Kind::UseInventoryItem,
            Self::UseInventoryItemOk(_) => Kind::UseInventoryItemOk,
            Self::QuerySkillList => Kind::QuerySkillList,
            Self::SkillList(_) => Kind::SkillList,
        }
    }
}
pub(super) trait Wire: Sized {
    fn write(&self, w: &mut Writer) -> Result<(), PayloadError>;
    fn read(r: &mut Reader<'_>) -> Result<Self, PayloadError>;
}
macro_rules! number {
    ($t:ty,$n:expr) => {
        impl Wire for $t {
            fn write(&self, w: &mut Writer) -> Result<(), PayloadError> {
                w.append(&self.to_be_bytes())
            }
            fn read(r: &mut Reader<'_>) -> Result<Self, PayloadError> {
                Ok(<$t>::from_be_bytes(r.fixed::<$n>()?))
            }
        }
    };
}
number!(i32, 4);
number!(i64, 8);
impl Wire for u8 {
    fn write(&self, w: &mut Writer) -> Result<(), PayloadError> {
        w.append(&[*self])
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PayloadError> {
        r.byte()
    }
}
impl Wire for bool {
    fn write(&self, w: &mut Writer) -> Result<(), PayloadError> {
        u8::from(*self).write(w)
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PayloadError> {
        Ok(r.byte()? != 0)
    }
}
impl Wire for Option<String> {
    fn write(&self, w: &mut Writer) -> Result<(), PayloadError> {
        w.nullable(self.as_ref().map(|v| v.as_bytes()))
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PayloadError> {
        Ok(r.string()?.map(str::to_owned))
    }
}
impl<T: Wire> Wire for Option<Vec<T>> {
    fn write(&self, w: &mut Writer) -> Result<(), PayloadError> {
        let Some(v) = self else { return w.append(&[0]) };
        if v.len() > MAX_ROWS {
            return Err(PayloadError::TooLarge);
        }
        w.append(&encode_integer((v.len() + 1) as i32))?;
        for x in v {
            x.write(w)?
        }
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PayloadError> {
        let Some(n) = r.count()? else { return Ok(None) };
        if n > MAX_ROWS || n > r.remaining.len() {
            return Err(PayloadError::TooLarge);
        }
        let mut v = Vec::with_capacity(n);
        for _ in 0..n {
            v.push(T::read(r)?)
        }
        Ok(Some(v))
    }
}
impl<T: Wire> Wire for (i32, T) {
    fn write(&self, w: &mut Writer) -> Result<(), PayloadError> {
        self.0.write(w)?;
        self.1.write(w)
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PayloadError> {
        Ok((i32::read(r)?, T::read(r)?))
    }
}
impl<T: Wire> Wire for Dict<T> {
    fn write(&self, w: &mut Writer) -> Result<(), PayloadError> {
        check_keys(&self.0)?;
        self.0.write(w)
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PayloadError> {
        let v = Option::<Vec<(i32, T)>>::read(r)?;
        check_keys(&v)?;
        Ok(Self(v))
    }
}
fn check_keys<T>(v: &Option<Vec<(i32, T)>>) -> Result<(), PayloadError> {
    let mut keys = std::collections::BTreeSet::new();
    if let Some(v) = v {
        for (k, _) in v {
            if !keys.insert(*k) {
                return Err(PayloadError::DuplicateKey);
            }
        }
    }
    Ok(())
}
macro_rules! object {($t:ident {$($f:ident),*$(,)?})=>{
 impl Wire for $t{fn write(&self,w:&mut Writer)->Result<(),PayloadError>{$ (self.$f.write(w)?;)*Ok(())}fn read(r:&mut Reader<'_>)->Result<Self,PayloadError>{Ok(Self{$($f:Wire::read(r)?),*})}}
 impl Wire for Option<$t>{fn write(&self,w:&mut Writer)->Result<(),PayloadError>{match self{None=>w.append(&[0]),Some(v)=>{w.append(&[1])?;v.write(w)}}}fn read(r:&mut Reader<'_>)->Result<Self,PayloadError>{if r.byte()?==0{Ok(None)}else{Ok(Some($t::read(r)?))}}}
};}
object!(Storage {
    id,
    available,
    last_seconds,
    name,
    tag
});
object!(Attribute {
    name,
    value,
    arg,
    arg2
});
object!(Slot {
    id,
    class,
    count,
    tab,
    slot,
    expireable,
    expire_diff,
    expire_ticks,
    colors,
    durability,
    maximum,
    bonus,
    tradable,
    expander,
    attributes,
    prefix,
    suffix,
    min_price,
    max_price,
    avg_price,
    min_up_diff
});
object!(QuickSlots { classes });
object!(HeroesString { format, parameters });
object!(Bundle { class, quick_slot });
object!(Ap {
    next_bonus_ms,
    ap,
    max_ap,
    bonus_interval
});
object!(SkillEnhance {
    group_key,
    index_key,
    kind,
    reduce_durability,
    max_durability_bonus
});
object!(BriefSkill {
    id,
    base_rank,
    final_rank,
    required_ap,
    is_locked,
    can_start_training,
    current_ap,
    reset_ap,
    untrain_ap,
    enhances
});
object!(SkillList {
    learning,
    current_ap,
    skills,
    reset_vocation
});
object!(UseItem {
    item_id,
    target_item_id
});
object!(Move {
    item_id,
    storage,
    slot
});
object!(Inventory {
    storages,
    slots,
    equipment,
    quick_slots,
    unequippable
});
pub fn encode(
    kind: Kind,
    message: Option<&Message>,
    max_body: usize,
) -> Result<Zeroizing<Vec<u8>>, InventoryWireError> {
    let mut w = Writer {
        bytes: Vec::new(),
        limit: max_body,
    };
    let result = (|| -> Result<(), InventoryWireError> {
        let Some(m) = message else {
            w.append(&[0])?;
            return Ok(());
        };
        if kind != m.kind() {
            return Err(InventoryWireError::Kind);
        }
        w.append(&[1])?;
        match m {
            Message::QueryInventory => {}
            Message::InventoryInfo(v) => v.write(&mut w)?,
            Message::UpdateInventoryInfo(v) => v.write(&mut w)?,
            Message::ItemFail(v) => v.write(&mut w)?,
            Message::MoveInventoryItem(v) => v.write(&mut w)?,
            Message::EquipBundle(v) => v.write(&mut w)?,
            Message::QuickSlotInfo(v) => v.write(&mut w)?,
            Message::QueryAp => {}
            Message::Ap(v) => v.write(&mut w)?,
            Message::UseInventoryItem(v) => v.write(&mut w)?,
            Message::UseInventoryItemOk(v) => v.write(&mut w)?,
            Message::QuerySkillList => {}
            Message::SkillList(v) => v.write(&mut w)?,
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
pub fn decode(
    kind: Kind,
    body: &[u8],
    max_body: usize,
) -> Result<Option<Message>, InventoryWireError> {
    if body.len() > max_body {
        return Err(PayloadError::TooLarge.into());
    }
    let mut r = Reader { remaining: body };
    let result = if r.byte()? == 0 {
        None
    } else {
        Some(match kind {
            Kind::QueryInventory => Message::QueryInventory,
            Kind::InventoryInfo => Message::InventoryInfo(Inventory::read(&mut r)?),
            Kind::UpdateInventoryInfo => Message::UpdateInventoryInfo(Wire::read(&mut r)?),
            Kind::ItemFail => Message::ItemFail(Wire::read(&mut r)?),
            Kind::MoveInventoryItem => Message::MoveInventoryItem(Move::read(&mut r)?),
            Kind::EquipBundle => Message::EquipBundle(Bundle::read(&mut r)?),
            Kind::QuickSlotInfo => Message::QuickSlotInfo(Wire::read(&mut r)?),
            Kind::QueryAp => Message::QueryAp,
            Kind::Ap => Message::Ap(Ap::read(&mut r)?),
            Kind::UseInventoryItem => Message::UseInventoryItem(UseItem::read(&mut r)?),
            Kind::UseInventoryItemOk => Message::UseInventoryItemOk(Wire::read(&mut r)?),
            Kind::QuerySkillList => Message::QuerySkillList,
            Kind::SkillList => Message::SkillList(SkillList::read(&mut r)?),
        })
    };
    if !r.remaining.is_empty() {
        return Err(PayloadError::TrailingBytes.into());
    }
    Ok(result)
}
pub fn decode_ingress(
    kind: Kind,
    category: i32,
    body: &[u8],
    max_body: usize,
) -> Result<Option<Message>, InventoryWireError> {
    if category == kind.category() {
        return decode(kind, body, max_body);
    }
    if category == 0 && body.len() <= max_body && body.get(..16) == Some(kind.type_id().as_bytes())
    {
        return decode(kind, &body[16..], max_body.saturating_sub(16));
    }
    if category == 0
        && body.len() <= max_body
        && body.get(..16)
            == Some(Uuid::from_u128(0x81c5fe01_027c_3e1c_98d5_da9c9862aa21).as_bytes())
        && body.get(16..) == Some(&[0])
    {
        return Ok(None);
    }
    Err(InventoryWireError::Type)
}
