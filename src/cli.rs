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
    /// Apply embedded PostgreSQL migrations. Does not import SQL Server backups.
    Migrate,
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
