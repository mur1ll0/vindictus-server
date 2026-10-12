//! Bounded frontend UserLogin dispatch recovered from the Core Framework reader/writer.
//! Category zero carries a CLR type GUID before the sealed payload. No remote type loading
//! or peer registration is performed; only the pinned UserLogin type is supported.
use super::{
    login::{UserLoginMessage, decode_user_login},
    server_profile::{USER_LOGIN_MESSAGE_CATEGORY, USER_LOGIN_MESSAGE_TYPE_ID},
    wire::PayloadError,
};
use thiserror::Error;
use uuid::Uuid;

// Framework BCL System.Object GUID verified in both oracle architectures. The fallback
// ToBinary((object)null) emits this GUID followed by the null presence byte.
const FRAMEWORK_OBJECT_TYPE_ID: Uuid = Uuid::from_u128(0x81c5fe01027c3e1c98d5da9c9862aa21);

/// Failure to dispatch a bounded known login, with no sensitive fields in Display/Debug.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum LoginIngressError {
    /// No decoder is registered for this category.
    #[error("unsupported login category")]
    UnsupportedCategory,
    /// The GUID identifies an unsupported type, including peer TypeConverter messages.
    #[error("unsupported login envelope type")]
    UnsupportedType,
    /// Only a null System.Object has login fallback meaning.
    #[error("non-null object cannot represent login")]
    NonLoginObject,
    /// The bounded payload is malformed.
    #[error(transparent)]
    Payload(#[from] PayloadError),
}

/// Decode a complete typed478 or polymorphic0 login without copying or allocating fields.
/// The limit includes the 16-byte GUID. Null objects remain null, never authenticated.
/// Generic/array/unknown types and peer negotiation are explicitly unsupported.
pub fn decode_login_ingress(
    category: i32,
    body: &[u8],
    max_body: usize,
) -> Result<Option<UserLoginMessage<'_>>, LoginIngressError> {
    if body.len() > max_body {
        return Err(PayloadError::TooLarge.into());
    }
    if category == USER_LOGIN_MESSAGE_CATEGORY {
        return Ok(decode_user_login(body, max_body)?);
    }
    if category != 0 {
        return Err(LoginIngressError::UnsupportedCategory);
    }
    let guid = body.get(..16).ok_or(PayloadError::Truncated)?;
    let fields = &body[16..];
    if guid == USER_LOGIN_MESSAGE_TYPE_ID.as_bytes() {
        return Ok(decode_user_login(fields, max_body - 16)?);
    }
    if guid == FRAMEWORK_OBJECT_TYPE_ID.as_bytes() {
        let presence = fields.first().ok_or(PayloadError::Truncated)?;
        if *presence != 0 {
            return Err(LoginIngressError::NonLoginObject);
        }
        if fields.len() != 1 {
            return Err(PayloadError::TrailingBytes.into());
        }
        return Ok(None);
    }
    Err(LoginIngressError::UnsupportedType)
}
