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
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    QueryInventory,
    InventoryInfo,
}
impl Kind {
    pub const fn category(self) -> i32 {
        match self {
            Self::QueryInventory => 529,
            Self::InventoryInfo => 468,
        }
    }
    pub const fn type_id(self) -> Uuid {
        Uuid::from_u128(match self {
            Self::QueryInventory => 0xefe95078_ef7e_34eb_82e1_2daf3280a576,
            Self::InventoryInfo => 0xbdc965b5_aca1_3b62_b222_279e96683d93,
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
#[derive(Clone, PartialEq, Eq)]
pub enum Message {
    QueryInventory,
    InventoryInfo(Inventory),
}
impl Message {
    pub const fn kind(&self) -> Kind {
        match self {
            Self::QueryInventory => Kind::QueryInventory,
            Self::InventoryInfo(_) => Kind::InventoryInfo,
        }
    }
}
trait Wire: Sized {
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
        if let Message::InventoryInfo(v) = m {
            v.write(&mut w)?
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
