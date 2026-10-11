use clap::Parser;
use tracing_subscriber::EnvFilter;
use vindictus_server::{
    cli::{Cli, Command, read_password},
    config::Config,
    domain::Username,
    error::Error,
    infrastructure::postgres,
};

#[tokio::main]
async fn main() -> std::process::ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .json()
        .with_writer(std::io::stderr)
        .init();
    match run(Cli::parse()).await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            // Display deliberately excludes connection strings and credentials.
            tracing::error!(error = %error, "command failed");
            std::process::ExitCode::FAILURE
        }
    }
}

async fn run(cli: Cli) -> Result<(), Error> {
    if let Command::ProbeFrontend { bind } = &cli.command {
        if !bind.ip().is_loopback() {
            return Err(vindictus_server::infrastructure::frontend::ProbeError::NonLoopback.into());
        }
        let listener = tokio::net::TcpListener::bind(bind).await?;
        tracing::info!(address = %listener.local_addr()?, game_authenticated = false, "frontend diagnostic listening; all logins refused");
        vindictus_server::infrastructure::frontend::run_probe(listener, shutdown()).await?;
        return Ok(());
    }
    let credentials = match &cli.command {
        Command::CreateAccount { username, .. } => Some((
            Username::parse(username)?,
            read_password(std::io::stdin().lock())?,
        )),
        _ => None,
    };
    let snapshots = match &cli.command {
        Command::ImportUsers { .. } => Some(
            vindictus_server::infrastructure::user_export::read_export(std::io::stdin().lock())?,
        ),
        _ => None,
    };
    let config = Config::from_env()?;
    let pool = postgres::pool(&config)?;
    match cli.command {
        Command::ProbeFrontend { .. } => {}
        Command::Migrate => {
            postgres::migrate(&pool).await?;
            println!("PostgreSQL migrations applied; migrate does not import legacy data.");
        }
        Command::CreateAccount { .. } => {
            let (username, password) = credentials.ok_or(Error::PasswordInput)?;
            let id =
                vindictus_server::application::accounts::create_account(&pool, username, password)
                    .await?;
            println!(
                "Foundation account created: {id}. Game-client authentication is not implemented."
            );
        }
        Command::ImportUsers { verify_only, .. } => {
            use vindictus_server::application::user_import::{ImportMode, import_users};
            let snapshots = snapshots.ok_or(Error::UserExport)?;
            postgres::check_ready(&pool).await?;
            let mode = if verify_only {
                ImportMode::VerifyOnly
            } else {
                ImportMode::Apply
            };
            let result = import_users(&pool, &snapshots, mode).await?;
            println!(
                "{}",
                serde_json::json!({
                    "source_rows": result.source_rows, "inserted": result.inserted,
                    "existing": result.existing, "verified": result.verified,
                    "verify_only": verify_only, "game_authentication_available": false
                })
            );
        }
        Command::Serve => {
            let listener = tokio::net::TcpListener::bind(config.bind_address).await?;
            tracing::info!(address = %listener.local_addr()?, game_client_compatible = false, "operational HTTP server listening");
            axum::serve(listener, vindictus_server::http::router(pool.clone()))
                .with_graceful_shutdown(shutdown())
                .await?;
        }
    }
    pool.close().await;
    Ok(())
}

async fn shutdown() {
    let ctrl_c = async {
        if tokio::signal::ctrl_c().await.is_err() {
            tracing::error!("could not install Ctrl-C signal handler");
        }
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(_) => tracing::error!("could not install SIGTERM handler"),
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! { () = ctrl_c => {}, () = terminate => {} }
    tracing::info!("shutdown requested");
}
