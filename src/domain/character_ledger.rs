//! Original DATEDIFF(day,2011-01-01,GETDATE()) parity, using the measured civil SQL clock.
use super::legacy_user_data::SqlDateTime;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LedgerPartition {
    One,
    Two,
}
pub fn partition(civil: SqlDateTime) -> LedgerPartition {
    let days = civil.units300().div_euclid(25_920_000) - 40_542;
    if days % 2 == 0 {
        LedgerPartition::One
    } else {
        LedgerPartition::Two
    }
}
