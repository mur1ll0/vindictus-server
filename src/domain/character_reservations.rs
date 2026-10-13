//! Restricted printable ASCII account equality, measured against source SQL CI_AS.
use thiserror::Error;
use zeroize::Zeroizing;
#[derive(Debug, Error, PartialEq, Eq)]
#[error("reservation account name is outside the recovered ASCII collation subset")]
pub struct ReservationNameError;
/// Preserve source nvarchar(50) storage separately. This key never changes returned names.
pub fn account_key(bytes: &[u8]) -> Result<Zeroizing<Vec<u8>>, ReservationNameError> {
    if bytes.len() > 100 || !bytes.len().is_multiple_of(2) {
        return Err(ReservationNameError);
    }
    let mut key = Zeroizing::new(Vec::with_capacity(bytes.len() / 2));
    for b in bytes.as_chunks::<2>().0 {
        let u = u16::from_le_bytes(*b);
        if !(32..=126).contains(&u) {
            return Err(ReservationNameError);
        }
        key.push((u as u8).to_ascii_lowercase());
    }
    while key.last() == Some(&b' ') {
        key.pop();
    }
    Ok(key)
}
