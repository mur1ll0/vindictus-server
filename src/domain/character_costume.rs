//! Recovered SetCostume alias updates and explicit Framework body-shape arithmetic.
use std::collections::BTreeMap;
use thiserror::Error;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Costume {
    pub is_base: bool,
    pub kind: u8,
    pub key: u8,
    pub serial: Option<i32>,
    pub colors: [Option<i32>; 3],
    pub gloss: Option<u8>,
    pub effect: Option<i32>,
}
impl Costume {
    pub fn identity(&self) -> (bool, u8, u8) {
        (self.is_base, self.kind, self.key)
    }
    pub fn extra(key: i32, value: i32) -> Self {
        Self {
            is_base: true,
            kind: 2,
            key: key as u8,
            serial: Some(value),
            colors: [None; 3],
            gloss: None,
            effect: None,
        }
    }
    pub fn body(key: i32, value: f32, arithmetic: BodyArithmetic) -> Self {
        let product = match arithmetic {
            BodyArithmetic::FrameworkX86 => f64::from(value) * 1000.0,
            BodyArithmetic::FrameworkX64 => f64::from(value * 1000.0_f32),
        }
        .trunc();
        // CLR conv.i4 returns Int32.MinValue for non-finite/out-of-range operands.
        let serial = if product.is_finite()
            && (f64::from(i32::MIN)..=f64::from(i32::MAX)).contains(&product)
        {
            product as i32
        } else {
            i32::MIN
        };
        Self {
            kind: 3,
            ..Self::extra(key, serial)
        }
    }
}
/// No default: the caller must choose the measured source process profile.
#[derive(Clone, Copy, Debug)]
pub enum BodyArithmetic {
    FrameworkX86,
    FrameworkX64,
}
#[derive(Debug, Error)]
#[error("costume set is ambiguous or exceeds its bounded profile")]
pub struct CostumeProfile;
#[derive(Clone, Default)]
pub struct CostumeSet {
    rows: Vec<Costume>,
    positions: BTreeMap<(bool, u8, u8), usize>,
}
impl CostumeSet {
    pub const MAX_ROWS: usize = 1024;
    pub fn load(rows: impl IntoIterator<Item = Costume>) -> Result<Self, CostumeProfile> {
        let mut set = Self::default();
        for row in rows {
            if set.rows.len() == Self::MAX_ROWS || set.positions.contains_key(&row.identity()) {
                return Err(CostumeProfile);
            }
            set.positions.insert(row.identity(), set.rows.len());
            set.rows.push(row);
        }
        Ok(set)
    }
    pub fn set(&mut self, costume: Costume) -> Result<(), CostumeProfile> {
        let key = costume.identity();
        if let Some(index) = self.positions.get(&key) {
            let current = self.rows.get_mut(*index).ok_or(CostumeProfile)?;
            // SetCostume preserves the original object, its insertion position and Effect.
            current.serial = costume.serial;
            current.colors = costume.colors;
            current.gloss = costume.gloss;
        } else {
            if self.rows.len() == Self::MAX_ROWS {
                return Err(CostumeProfile);
            }
            self.positions.insert(key, self.rows.len());
            self.rows.push(costume);
        }
        Ok(())
    }
    pub fn rows(&self) -> impl Iterator<Item = &Costume> {
        self.rows.iter()
    }
    /// Character.Costume is an EntitySet: removal shifts its enumeration, and a
    /// later Set appends. The separate lookup dictionary does not dictate row order.
    pub fn remove(&mut self, identity: (bool, u8, u8)) -> Option<Costume> {
        let index = self.positions.remove(&identity)?;
        let removed = self.rows.remove(index);
        for position in self.positions.values_mut() {
            if *position > index {
                *position -= 1
            }
        }
        Some(removed)
    }
}
