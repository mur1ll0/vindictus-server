//! Ed25519 over the exact bytes of a document. The private key is 32 raw bytes kept outside the served
//! store; the public key is pinned by the launcher.

use crate::DistError;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};

/// Signing key from its 32-byte seed.
pub fn signing_key(seed: &[u8]) -> Result<SigningKey, DistError> {
    let seed: [u8; 32] = seed.try_into().map_err(|_| DistError::Key)?;
    Ok(SigningKey::from_bytes(&seed))
}

/// Public key from 64 hex digits (the pinned form).
pub fn verifying_key(hex_key: &str) -> Result<VerifyingKey, DistError> {
    let bytes: [u8; 32] = hex::decode(hex_key.trim())
        .ok()
        .and_then(|b| b.try_into().ok())
        .ok_or(DistError::Key)?;
    VerifyingKey::from_bytes(&bytes).map_err(|_| DistError::Key)
}

/// Public key as 64 hex digits.
pub fn public_hex(key: &SigningKey) -> String {
    hex::encode(key.verifying_key().to_bytes())
}

/// 64-byte signature of a document.
pub fn sign(key: &SigningKey, document: &[u8]) -> Vec<u8> {
    key.sign(document).to_bytes().to_vec()
}

/// Checks a document against its signature file.
pub fn verify(
    key: &VerifyingKey,
    document: &[u8],
    signature: &[u8],
    what: &'static str,
) -> Result<(), DistError> {
    let signature = Signature::from_slice(signature).map_err(|_| DistError::Signature(what))?;
    key.verify(document, &signature)
        .map_err(|_| DistError::Signature(what))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_signature_verifies_only_its_document() {
        let key = signing_key(&[7u8; 32]).unwrap_or_else(|_| SigningKey::from_bytes(&[0; 32]));
        let public = verifying_key(&public_hex(&key)).unwrap_or_else(|_| key.verifying_key());
        let signature = sign(&key, b"versao");
        assert!(verify(&public, b"versao", &signature, "doc").is_ok());
        assert!(verify(&public, b"versa0", &signature, "doc").is_err());
        assert!(signing_key(&[1u8; 31]).is_err());
    }
}
