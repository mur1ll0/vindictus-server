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
    if let Command::InspectRewardPolicy {
        characters_file, ..
    } = &cli.command
    {
        let package =
            vindictus_server::infrastructure::reward_policy::read_policy(std::io::stdin().lock())?;
        let characters = vindictus_server::infrastructure::reward_policy::read_characters(
            std::fs::File::open(characters_file)?,
        )?;
        let views = characters
            .iter()
            .map(|row| row.as_character())
            .collect::<Vec<_>>();
        let result = vindictus_server::application::reward_inspection::inspect(&package, &views)?;
        println!(
            "{}",
            serde_json::to_string(&result).map_err(|_| Error::RewardPolicyExport)?
        );
        return Ok(());
    }
    if let Command::InspectLabFrontendProfile { policy_file, .. } = &cli.command {
        let content = vindictus_server::infrastructure::reward_policy::read_policy(
            std::fs::File::open(policy_file)?,
        )?;
        let profile = vindictus_server::infrastructure::lab_runtime_config::LabRuntimeConfig::read(
            std::io::stdin().lock(),
            &content,
        )?;
        println!(
            "{}",
            serde_json::json!({"profile_validated":true,"allowlisted_count":profile.allowed_count(),"loopback":true,"original_manifest_matches":true,"database_opened":false,"listener_opened":false,"game_authentication_available":false})
        );
        return Ok(());
    }
    if let Command::InspectItemCatalog { .. } = &cli.command {
        let package =
            vindictus_server::infrastructure::item_catalog::read_package(std::io::stdin().lock())?;
        let result = vindictus_server::application::item_catalog_inspection::inspect(&package)?;
        println!(
            "{}",
            serde_json::to_string(&result).map_err(|_| Error::ItemCatalogExport)?
        );
        return Ok(());
    }
    if let Command::LabFrontend {
        policy_file,
        name_proof_file,
        ..
    } = &cli.command
    {
        use vindictus_server::infrastructure::{
            lab_frontend, lab_runtime_config::LabRuntimeConfig, reward_policy,
        };
        let content = reward_policy::read_policy(std::fs::File::open(policy_file)?)?;
        let profile = LabRuntimeConfig::read(std::io::stdin().lock(), &content)?;
        let name_proof = name_proof_file
            .as_ref()
            .map(|path| -> Result<_, Error> {
                Ok(
                    vindictus_server::infrastructure::character_name_proof::NameProof::read(
                        std::fs::File::open(path)?,
                        &content,
                    )?,
                )
            })
            .transpose()?;
        let runtime = std::sync::Arc::new(
            lab_frontend::Runtime::new(profile, content)?.with_name_check(name_proof),
        );
        let config = Config::from_env()?;
        let pool = postgres::pool(&config)?;
        postgres::check_ready(&pool).await?;
        let listener = tokio::net::TcpListener::bind(runtime.bind()).await?;
        tracing::info!(address=%listener.local_addr()?,client_acceptance_verified=false,character_creation_available=false,"restricted laboratory frontend listening");
        let result = lab_frontend::run(listener, runtime, pool.clone(), shutdown()).await;
        pool.close().await;
        result?;
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
    let character_bundle = match &cli.command {
        Command::ImportCharacters { .. } => Some(
            vindictus_server::infrastructure::character_export::read_export(
                std::io::stdin().lock(),
            )?,
        ),
        _ => None,
    };
    let lobby = match &cli.command {
        Command::ImportCharacterLobby { .. } => Some(
            vindictus_server::infrastructure::lobby_export::read_snapshot(std::io::stdin().lock())?,
        ),
        _ => None,
    };
    let home_cafe = match &cli.command {
        Command::ImportHomeCafeBonus { .. } => Some(
            vindictus_server::infrastructure::home_cafe_bonus::read_snapshot(
                std::io::stdin().lock(),
            )?,
        ),
        _ => None,
    };
    let item_state = match &cli.command {
        Command::ImportItemState { .. } => Some(
            vindictus_server::infrastructure::item_state::read_snapshot(std::io::stdin().lock())?,
        ),
        _ => None,
    };
    let costumes = match &cli.command {
        Command::ImportCostumes { .. } => Some(
            vindictus_server::infrastructure::character_costumes::read_snapshot(
                std::io::stdin().lock(),
            )?,
        ),
        _ => None,
    };
    let collections = match &cli.command {
        Command::ImportCharacterCollections { .. } => Some(
            vindictus_server::infrastructure::character_collections::read_snapshot(
                std::io::stdin().lock(),
            )?,
        ),
        _ => None,
    };
    let config = Config::from_env()?;
    let pool = postgres::pool(&config)?;
    match cli.command {
        Command::ProbeFrontend { .. }
        | Command::InspectRewardPolicy { .. }
        | Command::InspectItemCatalog { .. }
        | Command::InspectLabFrontendProfile { .. }
        | Command::LabFrontend { .. } => {}
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
        Command::ImportCharacters { verify_only, .. } => {
            let bundle = character_bundle.ok_or(Error::CharacterExport)?;
            postgres::check_ready(&pool).await?;
            let result = vindictus_server::infrastructure::characters::import_bundle(
                &pool,
                &bundle,
                verify_only,
            )
            .await?;
            println!(
                "{}",
                serde_json::json!({"summary":result,"verify_only":verify_only,"game_authentication_available":false})
            );
        }
        Command::ImportItemState { verify_only, .. } => {
            use vindictus_server::infrastructure::item_state::{ItemStateError, preserve};
            let snapshot = item_state.ok_or(ItemStateError::Export)?;
            let summary = preserve(&pool, &snapshot, verify_only).await?;
            println!(
                "{}",
                serde_json::to_string(&summary).map_err(|_| ItemStateError::Export)?
            );
        }
        Command::ImportCostumes { verify_only, .. } => {
            let snapshot = costumes.ok_or(
                vindictus_server::infrastructure::character_costumes::CostumeError::Export,
            )?;
            let summary = vindictus_server::infrastructure::character_costumes::preserve(
                &pool,
                &snapshot,
                verify_only,
            )
            .await?;
            println!(
                "{}",
                serde_json::to_string(&summary).map_err(|_| {
                    vindictus_server::infrastructure::character_costumes::CostumeError::Export
                })?
            );
        }
        Command::ImportCharacterCollections { verify_only, .. } => {
            let snapshot = collections.ok_or(
                vindictus_server::infrastructure::character_collections::CollectionError::Export,
            )?;
            let summary = vindictus_server::infrastructure::character_collections::preserve(
                &pool,
                &snapshot,
                verify_only,
            )
            .await?;
            println!(
                "{}",
                serde_json::to_string(&summary).map_err(|_| {
                    vindictus_server::infrastructure::character_collections::CollectionError::Export
                })?
            );
        }
        Command::ImportHomeCafeBonus { verify_only, .. } => {
            let rows = home_cafe
                .ok_or(vindictus_server::infrastructure::home_cafe_bonus::HomeCafeError::Export)?;
            let summary = vindictus_server::infrastructure::home_cafe_bonus::preserve(
                &pool,
                &rows,
                verify_only,
            )
            .await?;
            println!(
                "{}",
                serde_json::to_string(&summary).map_err(|_| {
                    vindictus_server::infrastructure::home_cafe_bonus::HomeCafeError::Export
                })?
            );
        }
        Command::ImportCharacterLobby { verify_only, .. } => {
            let snapshot = lobby.ok_or(Error::CharacterExport)?;
            let summary = vindictus_server::infrastructure::character_lobby::preserve(
                &pool,
                &snapshot,
                verify_only,
            )
            .await?;
            println!(
                "{}",
                serde_json::to_string(&summary).map_err(|_| Error::CharacterExport)?
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
        Command::InitializeQueueIdentity { source_last_value } => {
            postgres::check_ready(&pool).await?;
            let initialized = vindictus_server::infrastructure::dormant_items::initialize_identity(
                &pool,
                source_last_value,
            )
            .await?;
            println!(
                "{}",
                serde_json::json!({"initialized":initialized,"game_authentication_available":false})
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
