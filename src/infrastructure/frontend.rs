//! Local TCP diagnostic adapter for the recovered frontend transport.
//! It emits registrations, parses one login and refuses it. It never creates a game session.
use crate::protocol::{
    DEFAULT_MAX_BODY, Frame,
    bootstrap::{BootstrapError, encode_initial_packets},
    features::FeatureMatrix,
    login_failure::{LoginFailMessage, PERMISSION_FAILED, encode_login_failure},
    login_ingress::{LoginIngressError, decode_login_ingress},
    server_profile::{LOGIN_FAIL_MESSAGE_CATEGORY, USER_LOGIN_MESSAGE_CATEGORY},
    transport::{CipherProfile, LegacyReceiver, TransportError},
};
use std::{future::Future, time::Duration};
use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::JoinSet,
};
use zeroize::Zeroizing;

/// Bounded diagnostics: eight simultaneous sessions, each with an absolute ten-second deadline.
pub const MAX_PROBE_CONNECTIONS: usize = 8;
/// The whole handshake deadline is absolute; sending single bytes cannot extend it.
pub const PROBE_DEADLINE: Duration = Duration::from_secs(10);

/// Reasons to close a diagnostic connection. Errors never include payloads or account names.
#[derive(Debug, Error)]
pub enum ProbeError {
    /// This diagnostic command deliberately accepts only loopback bindings.
    #[error("frontend probe requires a loopback binding")]
    NonLoopback,
    /// IO display is sanitized; the original IO error remains its source.
    #[error("frontend probe IO failed")]
    Io(#[from] std::io::Error),
    /// Bounded packet construction failed.
    #[error(transparent)]
    Bootstrap(#[from] BootstrapError),
    /// Invalid encrypted stream.
    #[error(transparent)]
    Transport(#[from] TransportError),
    /// Bounded failure-response construction failed.
    #[error(transparent)]
    Payload(#[from] crate::protocol::login::PayloadError),
    /// Invalid or unsupported login envelope; never reports payload fields.
    #[error(transparent)]
    Ingress(#[from] LoginIngressError),
    /// Only category zero and the verified typed login category reach the bounded decoder.
    #[error("frontend probe does not implement category {0}")]
    UnsupportedCategory(i32),
    /// Null login is valid wire data but cannot start PassportCheck.
    #[error("frontend probe received a null login object")]
    NullLogin,
    /// A peer closed the stream inside a packet.
    #[error("frontend probe received an incomplete stream")]
    TruncatedEof,
    /// The whole connection deadline expired.
    #[error("frontend probe deadline exceeded")]
    Timeout,
}

/// Diagnostic result; neither variant represents authenticated gameplay.
#[derive(Debug, PartialEq, Eq)]
pub enum ProbeOutcome {
    /// Peer left before submitting a complete login.
    PeerClosed,
    /// Valid typed login was decoded and explicitly refused with PermissionFailed.
    LoginRefused,
}

/// Send the source-identified bootstrap, receive one known typed or category-zero login and explicitly refuse it.
/// Caller selects a fresh salt; the sender state is retained across the response.
/// Optional features are overrides, matching FeatureMatrix.GetFeatureMatrixDic().
pub async fn probe_connection(
    mut stream: TcpStream,
    salt: u32,
    features: Option<&FeatureMatrix<'_>>,
    deadline: Duration,
) -> Result<ProbeOutcome, ProbeError> {
    tokio::time::timeout(deadline, async {
        let (mut sender, packets) = encode_initial_packets(salt, features, DEFAULT_MAX_BODY, 1024)?;
        for packet in packets {
            stream.write_all(&packet).await?;
        }
        let mut receiver =
            LegacyReceiver::with_profile(CipherProfile::VindictusFrontend, DEFAULT_MAX_BODY);
        let mut buffer = Zeroizing::new([0u8; 4096]);
        loop {
            let count = stream.read(&mut buffer[..]).await?;
            if count == 0 {
                return if receiver.buffered_len() == 0 {
                    Ok(ProbeOutcome::PeerClosed)
                } else {
                    Err(ProbeError::TruncatedEof)
                };
            }
            // Limit each read independently. The transport retains at most one incomplete frame.
            let frames = receiver.push(&buffer[..count])?;
            if let Some(mut frame) = frames.into_iter().next() {
                let body = Zeroizing::new(std::mem::take(&mut frame.body));
                if frame.category_id != 0 && frame.category_id != USER_LOGIN_MESSAGE_CATEGORY {
                    return Err(ProbeError::UnsupportedCategory(frame.category_id));
                }
                // No identity or permission is derived from untrusted client fields.
                // The decoded passport/hardware/version fields are never printed or retained.
                let login = decode_login_ingress(frame.category_id, &body, DEFAULT_MAX_BODY)?;
                if login.is_none() {
                    return Err(ProbeError::NullLogin);
                }
                let failure = encode_login_failure(
                    Some(&LoginFailMessage {
                        reason: PERMISSION_FAILED,
                        banned_reason: Some(""),
                    }),
                    DEFAULT_MAX_BODY,
                )?;
                let response = sender.encode(&Frame {
                    instance_id: 0,
                    category_id: LOGIN_FAIL_MESSAGE_CATEGORY,
                    body: &failure,
                })?;
                stream.write_all(&response).await?;
                stream.shutdown().await?;
                return Ok(ProbeOutcome::LoginRefused);
            }
        }
    })
    .await
    .map_err(|_| ProbeError::Timeout)?
}

/// Run the explicitly requested loopback diagnostic independently of HTTP/PostgreSQL.
/// Shutdown aborts remaining diagnostic sessions and releases their socket resources.
pub async fn run_probe(
    listener: TcpListener,
    shutdown: impl Future<Output = ()>,
) -> Result<(), ProbeError> {
    if !listener.local_addr()?.ip().is_loopback() {
        return Err(ProbeError::NonLoopback);
    }
    let mut tasks = JoinSet::new();
    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            () = &mut shutdown => break,
            Some(result) = tasks.join_next(), if !tasks.is_empty() => {
                if result.is_err() { tracing::warn!("frontend diagnostic task ended unexpectedly"); }
            }
            accepted = listener.accept(), if tasks.len() < MAX_PROBE_CONNECTIONS => {
                let (stream, _) = accepted?;
                tasks.spawn(async move {
                    let bytes = uuid::Uuid::new_v4().into_bytes();
                    let salt = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) & 0x7fff_ffff;
                    match probe_connection(stream, salt, None, PROBE_DEADLINE).await {
                        Ok(outcome) => tracing::info!(?outcome, game_authenticated = false, "frontend diagnostic ended"),
                        Err(error) => tracing::warn!(error = %error, "frontend diagnostic closed"),
                    }
                });
            }
        }
    }
    tasks.abort_all();
    while tasks.join_next().await.is_some() {}
    Ok(())
}
