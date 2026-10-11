//! Frontend passport gates recovered from PassportCheck.checkOp_OnComplete.
use thiserror::Error;

/// Recovered failure codes, independent of any new operational HTTP response.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum LoginRejection {
    PassportFail = 1,
    DuplicateLogin = 2,
    NoUserInfo = 3,
    Banned = 4,
    WebShutdown = 12,
    PermissionFailed = 13,
}
/// Only the permission bits used by this stage; unknown bits are retained.
#[derive(Clone, Copy)]
pub struct LoginPermissions(i32);
impl LoginPermissions {
    pub const LOGIN: i32 = 1;
    pub const MAINTENANCE: i32 = 2;
    pub const CHEAT: i32 = 0x0200_0000;
    pub const fn from_recovered_bits(bits: i32) -> Self {
        Self(bits)
    }
    pub const fn can_login(self) -> bool {
        self.0 & Self::LOGIN != 0
    }
    pub const fn bypass_maintenance(self) -> bool {
        self.0 & Self::MAINTENANCE != 0
    }
    pub const fn has_cheat(self) -> bool {
        self.0 & Self::CHEAT != 0
    }
}
#[derive(Debug, Error, PartialEq, Eq)]
#[error("passport rejected with recovered reason {0:?}")]
pub struct PassportRejected(pub LoginRejection);

/// A missing Login bit fails before maintenance. Private IP alone grants no permission.
/// The caller supplies the CheckPassport failure reason, just as MoveToInitialState does.
pub fn frontend_passport_gate(
    permissions: LoginPermissions,
    is_online: bool,
    failure_reason: LoginRejection,
) -> Result<(), PassportRejected> {
    if !permissions.can_login() {
        return Err(PassportRejected(failure_reason));
    }
    if !is_online && !permissions.bypass_maintenance() {
        return Err(PassportRejected(LoginRejection::WebShutdown));
    }
    Ok(())
}
/// GameClient.IsOTP depends on a trusted CheckPassport result, never a client flag.
pub const fn is_secured_login(secure_code: u8) -> bool {
    secure_code > 0
}
