use crate::{domain::validate_password, error::Error};
use clap::{Parser, Subcommand};
use std::io::Read;
use zeroize::Zeroizing;

#[derive(Debug, Parser)]
#[command(
    version,
    about = "Vindictus preservation foundation (game migration incomplete)"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Serve HTTP operational endpoints. Does not start a game protocol.
    Serve,
    /// Diagnostic frontend on loopback: handshake and login refusal only, no game authentication.
    ProbeFrontend {
        /// Loopback IP:port; this probe runs independently of the operational HTTP/database.
        #[arg(long, default_value = "127.0.0.1:27015")]
        bind: std::net::SocketAddr,
    },
    /// Restricted loopback candidate frontend: synthetic login/lobby; character services pending.
    LabFrontend {
        #[arg(long, required = true)]
        input_stdin: bool,
        #[arg(long)]
        policy_file: std::path::PathBuf,
        /// Private finite name comparison proof, prepared from read-only recovered evidence.
        #[arg(long)]
        name_proof_file: Option<std::path::PathBuf>,
    },
    /// Apply embedded PostgreSQL migrations. Does not import SQL Server backups.
    Migrate,
    /// Read-only reward diagnostic. Reads private policy from stdin and prints counts/digest only.
    InspectRewardPolicy {
        #[arg(long, required = true)]
        input_stdin: bool,
        /// Bounded private/synthetic character projection; never interpreted as a game login.
        #[arg(long)]
        characters_file: std::path::PathBuf,
    },
    /// Validate protected laboratory configuration without opening TCP or the database.
    InspectLabFrontendProfile {
        #[arg(long, required = true)]
        input_stdin: bool,
        #[arg(long)]
        policy_file: std::path::PathBuf,
    },
    /// Read-only catalog projection diagnostic; stdin input, counts/digest output only.
    /// Validate private equipment stats against the pinned effective source catalog.
    InspectItemStats {
        #[arg(long, required = true)]
        input_stdin: bool,
        #[arg(long)]
        catalog_file: std::path::PathBuf,
    },
    /// Read-only catalog projection diagnostic; stdin input, counts/digest output only.
    InspectItemCatalog {
        #[arg(long, required = true)]
        input_stdin: bool,
    },
    /// Initialize the queue's recovered identity counter after snapshot import; no reseeding.
    InitializeQueueIdentity {
        /// Last source sys.identity_columns.last_value; omit only for a never-used empty queue.
        #[arg(long)]
        source_last_value: Option<i32>,
    },
    /// Preserve all character/login/queue snapshots; does not create playable characters.
    ImportCharacters {
        /// Required: private bundle is read only from standard input.
        #[arg(long, required = true)]
        input_stdin: bool,
        #[arg(long)]
        verify_only: bool,
    },
    /// Preserve full Item/Attribute/Equippable/Storage state from private stdin.
    ImportItemState {
        #[arg(long, required = true)]
        input_stdin: bool,
        #[arg(long)]
        verify_only: bool,
    },
    /// Preserve recovered Costume rows and identity from private stdin; no playable creation.
    ImportCostumes {
        #[arg(long, required = true)]
        input_stdin: bool,
        #[arg(long)]
        verify_only: bool,
    },
    /// Preserve creation collection owners and MySettings; does not create a character.
    ImportCharacterCollections {
        #[arg(long, required = true)]
        input_stdin: bool,
        #[arg(long)]
        verify_only: bool,
    },
    /// Preserve HomeCafeBonus source rows only; no client authentication.
    ImportHomeCafeBonus {
        #[arg(long, required = true)]
        input_stdin: bool,
        #[arg(long)]
        verify_only: bool,
    },
    /// Preserve UserInfo/UserProducts/UserCareInfo only; no playable character creation.
    ImportCharacterLobby {
        #[arg(long, required = true)]
        input_stdin: bool,
        #[arg(long)]
        verify_only: bool,
    },
    /// Preserve complete legacy user snapshots; never activates game authentication.
    ImportUsers {
        /// Required: private JSON is read only from standard input.
        #[arg(long, required = true)]
        input_stdin: bool,
        /// Compare all fifteen fields without inserting any rows.
        #[arg(long)]
        verify_only: bool,
    },
    /// Provision a foundation account, not a legacy game account.
    CreateAccount {
        #[arg(long)]
        username: String,
        /// Required: password is read from stdin, never command-line arguments.
        #[arg(long, required = true)]
        password_stdin: bool,
    },
}

/// Read at most 131 bytes. A final LF/CRLF supports redirected files and pipes.
pub fn read_password(reader: impl Read) -> Result<Zeroizing<Vec<u8>>, Error> {
    let mut password = Zeroizing::new(Vec::new());
    reader
        .take(131)
        .read_to_end(&mut password)
        .map_err(|_| Error::PasswordInput)?;
    if password.last() == Some(&b'\n') {
        password.pop();
        if password.last() == Some(&b'\r') {
            password.pop();
        }
    }
    validate_password(&password)?;
    Ok(password)
}
