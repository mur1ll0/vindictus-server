//! Initial server packets recovered from FrontendService.OnClientAccept.
//! No listener, account validation or game session is implemented here.
use super::Frame;
use super::converter::{ConverterError, encode_converter};
use super::features::{FeatureMatrix, encode_feature_matrix};
use super::server_profile::{REGISTERED_CLASSES, SYNC_FEATURE_MATRIX_MESSAGE_CATEGORY};
use super::transport::{CipherProfile, LegacySender, TransportError};
use super::wire::PayloadError;
use thiserror::Error;

/// Failure building a bounded initial packet sequence.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum BootstrapError {
    /// Registration payload error.
    #[error(transparent)]
    Converter(#[from] ConverterError),
    /// Feature dictionary error.
    #[error(transparent)]
    Payload(#[from] PayloadError),
    /// Transport error.
    #[error(transparent)]
    Transport(#[from] TransportError),
}
/// Build the exact source-identified registration packet and optional nonempty feature sync.
/// Outgoing ISAAC state is continuous across the two packets. A fresh salt is required per connection.
/// This returns a bootstrap sequence only; a live session must retain sender state afterwards.
pub fn encode_initial_packets(
    salt: u32,
    features: Option<&FeatureMatrix<'_>>,
    max_body: usize,
    max_entries: usize,
) -> Result<(LegacySender, Vec<Vec<u8>>), BootstrapError> {
    let converter = encode_converter(Some(REGISTERED_CLASSES), max_body)?;
    let feature_body = match features {
        Some(matrix)
            if matrix
                .features
                .as_ref()
                .is_some_and(|entries| !entries.is_empty()) =>
        {
            Some(encode_feature_matrix(Some(matrix), max_body, max_entries)?)
        }
        _ => None,
    };
    let mut sender = LegacySender::with_profile(CipherProfile::VindictusFrontend, salt, max_body)?;
    let mut packets = vec![sender.encode(&Frame {
        instance_id: 0,
        category_id: 0,
        body: &converter,
    })?];
    if let Some(body) = feature_body {
        packets.push(sender.encode(&Frame {
            instance_id: 0,
            category_id: SYNC_FEATURE_MATRIX_MESSAGE_CATEGORY,
            body: &body,
        })?);
    }
    Ok((sender, packets))
}
