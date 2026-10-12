//! Recovered item location fields; IsNullLocation alone does not prove Item.IsValid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemLocation {
    pub collection: u8,
    pub slot: i32,
}
impl ItemLocation {
    pub const NULL: Self = Self {
        collection: 255,
        slot: -1,
    };
    pub fn is_null(self) -> bool {
        self == Self::NULL
    }
}
