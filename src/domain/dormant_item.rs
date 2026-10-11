//! Parameters of recovered SendDormantItem; no inventory/mail delivery is implied.
use crate::domain::legacy_user_data::{LegacyBytes, SqlDateTime};
use zeroize::Zeroizing;

pub struct DormantItemRequest {
    pub cid: i64,
    pub item_class_ex: LegacyBytes<1024, true>,
    pub count: i32,
    pub is_binded: bool,
    pub mail_title: Option<LegacyBytes<256, true>>,
    pub mail_content: Option<LegacyBytes<1024, true>>,
    /// Quantized GETUTCDATE sample, stable across retries of the same operation.
    pub log_date: SqlDateTime,
}
impl DormantItemRequest {
    /// Versioned, length-prefixed UTF16 bytes. NULL differs from empty; no lossy UTF8.
    pub(crate) fn replay_key(&self) -> Zeroizing<Vec<u8>> {
        let mut bytes = Zeroizing::new(Vec::with_capacity(128));
        bytes.extend_from_slice(b"SendDormantItem\0v1");
        bytes.extend_from_slice(&self.cid.to_le_bytes());
        bytes.extend_from_slice(&self.count.to_le_bytes());
        bytes.push(u8::from(self.is_binded));
        bytes.extend_from_slice(&self.log_date.units300().to_le_bytes());
        for field in [
            Some(self.item_class_ex.as_bytes()),
            self.mail_title.as_ref().map(LegacyBytes::as_bytes),
            self.mail_content.as_ref().map(LegacyBytes::as_bytes),
        ] {
            match field {
                None => bytes.push(0),
                Some(field) => {
                    bytes.push(1);
                    bytes.extend_from_slice(&(field.len() as u32).to_le_bytes());
                    bytes.extend_from_slice(field);
                }
            }
        }
        bytes
    }
}
