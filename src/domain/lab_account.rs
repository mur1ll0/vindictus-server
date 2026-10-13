//! Operator-owned laboratory scope; this opaque fingerprint is not an authentication token.
//! The runtime must derive it from its explicit, verified profile and protected configuration.
//! No wire field, user-name prefix or existing database row may supply this scope.
pub struct LabAccountScope {
    fingerprint: [u8; 32],
    service_id: i32,
    channel_code: u8,
}
impl LabAccountScope {
    pub fn new(fingerprint: [u8; 32], service_id: i32, channel_code: u8) -> Self {
        Self {
            fingerprint,
            service_id,
            channel_code,
        }
    }
    pub fn fingerprint(&self) -> &[u8; 32] {
        &self.fingerprint
    }
    pub fn service_id(&self) -> i32 {
        self.service_id
    }
    pub fn channel_code(&self) -> u8 {
        self.channel_code
    }
}
