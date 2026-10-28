//! BaseCharacter values past the ten of the 1.69 enum (docs/future/new-classes.md 8.13).
//!
//! The NA client and server name 28 classes, keep the pet at the index right after them and send
//! `m_iClass` with 5 bits; the laboratory follows that layout. ClassRestriction masks are `int`,
//! so 32 values is the ceiling. Knowing a class does not make it creatable: creation also needs
//! its loaded content (`Assets::supports_class`).

/// BaseCharacter names of the NA class table, in value order (`.hcls` holds the same list).
pub const NAMES: [&str; 28] = [
    "Lethita", "Fiona", "Evy", "Kalok", "Kay", "Vella", "Hurk", "Lynn", "Arisha", "Hagie", "Delia",
    "Miri", "Grimden", "Miul", "Bel", "Lethor", "Kael", "Tessa", "Danah", "Letty", "Latiya",
    "Czern", "Achel", "Sou", "Sanyaa", "Neamhain", "Calia", "Lumielle",
];
/// Exclusive bound of a class value: 5-bit `m_iClass` and `int` ClassRestriction masks. SQL
/// ownership checks spell the named range as `class BETWEEN 0 AND 27`.
pub const LIMIT: u8 = 32;
/// The first class after the 1.69 enum.
pub const DELIA: u8 = 10;
/// The pet's value in the laboratory DLLs: the index after the last named class, as in the NA.
pub const PET: u8 = 28;

/// Whether `class` is a named BaseCharacter (not the pet, not past the table).
pub fn is_class(class: u8) -> bool {
    usize::from(class) < NAMES.len()
}

/// The BaseCharacter name of a class value.
pub fn name(class: u8) -> Option<&'static str> {
    NAMES.get(usize::from(class)).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_follows_the_na_tables() {
        assert_eq!(name(DELIA), Some("Delia"));
        assert_eq!(usize::from(PET), NAMES.len());
        assert!(PET < LIMIT);
        assert!(is_class(9) && is_class(DELIA) && is_class(27));
        assert!(!is_class(PET) && !is_class(LIMIT));
    }
}
