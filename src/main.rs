use clap::Parser;
use tracing_subscriber::EnvFilter;
use vindictus_server::{
    cli::{Cli, Command},
    config::Config,
    domain::Username,
    error::Error,
    infrastructure::postgres,
};

/// Response size InGameCashShopDialog_V2 accepts from ingame_cashshop_shopListURL
/// (client.dll 0x38479247 and 0x38479306 pass 0x100000 to the download), and the limit
/// of a copy patched by scripts/client/patch_shop_list_limit.py.
const SHOP_LIST_CLIENT_LIMIT: usize = 0x10_0000;
const SHOP_LIST_PATCHED_LIMIT: usize = 0x100_0000;

/// Stack of the thread that drives the runtime: the command futures (asset loading and
/// the frontend session) outgrow the 1 MiB Windows main-thread stack in debug builds.
const MAIN_STACK_BYTES: usize = 32 * 1024 * 1024;

fn main() -> std::process::ExitCode {
    let driver = std::thread::Builder::new()
        .name("main-runtime".to_owned())
        .stack_size(MAIN_STACK_BYTES)
        .spawn(|| {
            match tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime.block_on(entry()),
                Err(_) => std::process::ExitCode::FAILURE,
            }
        });
    match driver {
        Ok(handle) => handle.join().unwrap_or(std::process::ExitCode::FAILURE),
        Err(_) => std::process::ExitCode::FAILURE,
    }
}

async fn entry() -> std::process::ExitCode {
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
    if let Command::InspectMinimumCreationAssets { assets_file } = &cli.command {
        vindictus_server::infrastructure::minimum_assets::Assets::read(assets_file)
            .map_err(|_| Error::Configuration("minimum creation assets invalid".into()))?;
        println!(
            "{}",
            serde_json::json!({"assets_validated":true,"database_opened":false,"listener_opened":false,"client_acceptance_verified":false})
        );
        return Ok(());
    }
    if let Command::LabExportShopList {
        assets_file,
        output,
        per_category,
    } = &cli.command
    {
        let assets = vindictus_server::infrastructure::minimum_assets::Assets::read(assets_file)
            .map_err(|_| Error::Configuration("minimum creation assets invalid".into()))?;
        let list = assets
            .cash_shop_list(*per_category)
            .ok_or_else(|| Error::Configuration("cash shop assets not exported".into()))?;
        let partial = output.with_extension("partial");
        std::fs::write(&partial, list.as_bytes())
            .and_then(|()| std::fs::rename(&partial, output))
            .map_err(|_| Error::Configuration("shop list not written".into()))?;
        println!(
            "{}",
            serde_json::json!({"products":list.matches("<Table>").count(),"written_bytes":list.len(),"fits_original_client":list.len()<=SHOP_LIST_CLIENT_LIMIT,"fits_patched_client":list.len()<=SHOP_LIST_PATCHED_LIMIT})
        );
        return Ok(());
    }
    if let Command::InspectMissionContent {
        assets_file,
        difficulty,
    } = &cli.command
    {
        use vindictus_server::{
            infrastructure::{minimum_assets::Assets, minimum_mission},
            protocol::{DEFAULT_MAX_BODY, mission},
        };
        let assets = Assets::read(assets_file)
            .map_err(|_| Error::Configuration("minimum creation assets invalid".into()))?;
        let contents = assets
            .mission()
            .ok_or_else(|| Error::Configuration("profile has no mission tables".into()))?;
        let (mut built, mut unsupported, mut failed, mut sectors, mut over) = (0, 0, 0, 0, 0);
        let mut no_difficulty = 0;
        let (mut trackers_failed, mut uncompiled) = (0, 0);
        let mut largest = 0_usize;
        for quest in contents.quest_ids() {
            // CheckConstraint answers NoSuchDifficulty before any sequence is built.
            if *difficulty > 0 && !contents.has_difficulty(quest, *difficulty) {
                no_difficulty += 1;
                continue;
            }
            match minimum_mission::preview(contents, quest, -1, *difficulty, false) {
                Ok(granted) => {
                    built += 1;
                    // InitializeTracker of the quest without a swear.
                    match contents.tracker(quest, -1, &[1]) {
                        Ok(t) => uncompiled += t.uncompiled,
                        Err(e) => {
                            trackers_failed += 1;
                            eprintln!("{quest}: tracker {e}");
                        }
                    }
                    sectors += granted
                        .sectors
                        .as_ref()
                        .and_then(|s| s.sectors.as_ref())
                        .map_or(0, Vec::len);
                    let message = mission::Message::GameStartGranted(granted);
                    match mission::encode(mission::Kind::GameStartGranted, Some(&message), 16 << 20)
                    {
                        Ok(body) => {
                            largest = largest.max(body.len());
                            if body.len() > DEFAULT_MAX_BODY {
                                over += 1;
                            }
                        }
                        Err(_) => failed += 1,
                    }
                }
                Err(vindictus_server::domain::mission::MissionError::Unsupported) => {
                    unsupported += 1
                }
                Err(e) => {
                    failed += 1;
                    eprintln!("{quest}: {e}");
                }
            }
        }
        println!(
            "{}",
            serde_json::json!({"quests_built":built,"trackers_failed":trackers_failed,"uncompiled_targets":uncompiled,"unsupported":unsupported,"no_difficulty":no_difficulty,"failed":failed,"sectors":sectors,"largest_body":largest,"over_default_body":over,"database_opened":false,"listener_opened":false})
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
    if let Command::InspectFionaInventory {
        catalog_file,
        stats_file,
        colors_file,
        ..
    } = &cli.command
    {
        use vindictus_server::{
            application::{character_creation::EntityAuthority, fiona_inventory as fiona},
            domain::{
                item_colors::ColorRandom, item_time::ItemTimeError, legacy_user_data::SqlDateTime2,
            },
            infrastructure::{
                fiona_inventory_profile::Profile, item_catalog, item_colors, item_stats,
            },
        };
        let catalog = item_catalog::read_package(std::fs::File::open(catalog_file)?)?;
        let stats = item_stats::Package::read(std::fs::File::open(stats_file)?)?;
        let colors = item_colors::Package::read(std::fs::File::open(colors_file)?)?;
        let profile = Profile::read(std::io::stdin().lock())?;
        let mut authority = EntityAuthority::new(17);
        let mut random = ColorRandom::from_seed(0);
        let mut clock_calls = 0_i64;
        let mut utc = || {
            let ticks = 638448480000000000_i64 + 101 * clock_calls;
            clock_calls += 1;
            SqlDateTime2::from_ticks(ticks).map_err(|_| ItemTimeError::Clock)
        };
        let inventory = fiona::compose(
            fiona::Inputs {
                profile: &profile,
                catalog: &catalog,
                stats: &stats,
                colors: &colors,
            },
            1,
            &mut authority,
            &mut random,
            &mut utc,
        )?;
        let mut report = serde_json::to_value(fiona::inspect(&inventory))
            .map_err(|_| Error::FionaInventoryProfile)?;
        report["synthetic_diagnostic"] = serde_json::json!(true);
        report["clock_calls"] = serde_json::json!(clock_calls);
        report["default_candidates"] = serde_json::json!(profile.candidates());
        println!(
            "{}",
            serde_json::to_string(&report).map_err(|_| Error::FionaInventoryProfile)?
        );
        return Ok(());
    }
    if let Command::InspectFionaTitleGoals {
        initializer_file,
        reference_file,
        catalog_file,
        ..
    } = &cli.command
    {
        use vindictus_server::infrastructure::{
            fiona_initializer_profile::Profile, fiona_title_preset::GoalProfile, item_catalog,
        };
        let catalog = item_catalog::read_package(std::fs::File::open(catalog_file)?)?;
        let profile = Profile::read(
            std::fs::File::open(initializer_file)?,
            std::fs::File::open(reference_file)?,
        )?;
        let goals = GoalProfile::read(std::io::stdin().lock(), &profile, &catalog)?;
        println!(
            "{}",
            serde_json::json!({"titles":goals.title_count(),"goals":goals.goal_count(),"bound":true,"create_available":false})
        );
        return Ok(());
    }
    if let Command::InspectItemColors { catalog_file, .. } = &cli.command {
        let catalog = vindictus_server::infrastructure::item_catalog::read_package(
            std::fs::File::open(catalog_file)?,
        )?;
        let package =
            vindictus_server::infrastructure::item_colors::Package::read(std::io::stdin().lock())?;
        let inspection = package.inspect(&catalog)?;
        println!(
            "{}",
            serde_json::to_string(&inspection).map_err(|_| Error::ItemColorsExport)?
        );
        return Ok(());
    }
    if let Command::InspectItemStats { catalog_file, .. } = &cli.command {
        let catalog = vindictus_server::infrastructure::item_catalog::read_package(
            std::fs::File::open(catalog_file)?,
        )?;
        let package =
            vindictus_server::infrastructure::item_stats::Package::read(std::io::stdin().lock())?;
        let inspection = package.inspect(&catalog)?;
        println!(
            "{}",
            serde_json::to_string(&inspection).map_err(|_| Error::ItemStatsExport)?
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
    if let Command::ExportOpenapi { output } = &cli.command {
        std::fs::write(output, vindictus_server::http::admin::openapi_json())?;
        println!("Admin panel OpenAPI written");
        return Ok(());
    }
    if let Command::LabFrontend {
        policy_file,
        name_proof_file,
        minimum_creation_assets,
        container_listen,
        admin_listen,
        admin_ui_dir,
        admin_secure_cookie,
        ..
    } = &cli.command
    {
        use vindictus_server::infrastructure::{
            lab_frontend, lab_peer, lab_runtime_config::LabRuntimeConfig, reward_policy,
        };
        if *container_listen {
            lab_peer::enable_container_publication();
        }
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
        let config = Config::from_env()?;
        let pool = postgres::pool(&config)?;
        postgres::check_ready(&pool).await?;
        let mut runtime = lab_frontend::Runtime::new(profile, content)?.with_name_check(name_proof);
        if let Some(path) = minimum_creation_assets {
            let assets = vindictus_server::infrastructure::minimum_assets::Assets::read(path)
                .map_err(|_| {
                    Error::Configuration("invalid private minimum creation assets".into())
                })?;
            vindictus_server::infrastructure::fiona_character_state::verify_source_binding(
                &pool,
                assets.initializer.source_state(),
            )
            .await
            .map_err(|_| {
                Error::Configuration("minimum creation source binding unavailable".into())
            })?;
            runtime = runtime.with_minimum_creation(
                pool.clone(),
                assets,
                name_proof_file.as_deref().ok_or_else(|| {
                    Error::Configuration("minimum creation requires name proof".into())
                })?,
            )?;
        }
        let runtime = std::sync::Arc::new(runtime);
        let listener = tokio::net::TcpListener::bind(lab_peer::listen_addr(runtime.bind())).await?;
        let channel_listener = match runtime.channel() {
            Some((bind, channel)) => Some((
                tokio::net::TcpListener::bind(lab_peer::listen_addr(bind)).await?,
                channel,
            )),
            None => None,
        };
        tracing::info!(address=%listener.local_addr()?,client_acceptance_verified=false,character_creation_available=runtime.minimum_creation_available(),"restricted laboratory frontend listening");
        // RelayServer of mission entry: served until the frontend stops, then aborted.
        let relay_task = match runtime.relay() {
            Some(relay) => {
                let relay_listener =
                    tokio::net::TcpListener::bind(lab_peer::listen_addr(relay.endpoint())).await?;
                tracing::info!(address=%relay_listener.local_addr()?, "mission relay listening");
                Some(tokio::spawn(relay.serve(relay_listener)))
            }
            None => None,
        };
        // In-game cash shop purchases (ingame_cashshop_purchaseURL): served like the relay.
        let web_shop_task = match runtime.minimum_assets() {
            Some(assets) => {
                use vindictus_server::infrastructure::lab_web_shop;
                let web_listener =
                    tokio::net::TcpListener::bind(lab_peer::listen_addr(lab_web_shop::BIND))
                        .await?;
                tracing::info!(address=%web_listener.local_addr()?, "laboratory web shop listening");
                Some(tokio::spawn(lab_web_shop::serve(
                    web_listener,
                    runtime.web_shop(),
                    pool.clone(),
                    assets,
                )))
            }
            None => None,
        };
        // Administration panel (B4): its own loopback listener, aborted with the frontend.
        let admin_task = match admin_listen {
            Some(bind) => {
                use vindictus_server::{application::admin_session::PanelAuth, http::admin};
                if !bind.ip().is_loopback() || bind.port() == 0 {
                    return Err(Error::Configuration(
                        "admin panel must listen on a loopback port".into(),
                    ));
                }
                let admin_listener =
                    tokio::net::TcpListener::bind(lab_peer::listen_addr(*bind)).await?;
                tracing::info!(address=%admin_listener.local_addr()?, "admin panel listening");
                let app = admin::router(
                    admin::AdminState::new(PanelAuth::new(pool.clone()), *admin_secure_cookie),
                    admin_ui_dir.as_deref(),
                );
                Some(tokio::spawn(admin::serve(admin_listener, app)))
            }
            None => None,
        };
        let result = if let Some((channel_listener, channel)) = channel_listener {
            tracing::info!(address=%channel_listener.local_addr()?, "minimum channel listening without cipher");
            let (stop, signal) = tokio::sync::watch::channel(false);
            let mut front_signal = signal.clone();
            let mut channel_signal = signal;
            let front = lab_frontend::run(listener, runtime.clone(), pool.clone(), async move {
                let _ = front_signal.changed().await;
            });
            let city = vindictus_server::infrastructure::channel_server::run(
                channel_listener,
                channel,
                pool.clone(),
                async move {
                    let _ = channel_signal.changed().await;
                },
            );
            tokio::pin!(front, city);
            tokio::select! {
                () = shutdown() => { let _ = stop.send(true); let (f,c) = tokio::join!(front, city); f.and(c) }
                result = &mut front => { let _ = stop.send(true); result.and(city.await) }
                result = &mut city => { let _ = stop.send(true); result.and(front.await) }
            }
        } else {
            lab_frontend::run(listener, runtime.clone(), pool.clone(), shutdown()).await
        };
        if let Some(task) = relay_task {
            task.abort();
        }
        if let Some(task) = web_shop_task {
            task.abort();
        }
        if let Some(task) = admin_task {
            task.abort();
        }
        let minimum_cleanup = runtime.shutdown_minimum().await;
        pool.close().await;
        result?;
        minimum_cleanup?;
        return Ok(());
    }
    let credentials = match &cli.command {
        Command::CreateAccount {
            username,
            lab_short_password,
            ..
        } => Some((
            Username::parse(username)?,
            vindictus_server::cli::read_password_with(
                std::io::stdin().lock(),
                if *lab_short_password {
                    vindictus_server::domain::MIN_LAB_SHORT_PASSWORD
                } else {
                    vindictus_server::domain::MIN_PASSWORD
                },
            )?,
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
    let character_state = match &cli.command {
        Command::ImportCharacterState { .. } => Some(
            vindictus_server::infrastructure::fiona_character_state::read_snapshot(
                std::io::stdin().lock(),
            )?,
        ),
        _ => None,
    };
    let shared = match &cli.command {
        Command::ImportSharedItems { .. } => Some(
            vindictus_server::infrastructure::shared_item_state::read_snapshot(
                std::io::stdin().lock(),
            )?,
        ),
        _ => None,
    };
    let journals = match &cli.command {
        Command::ImportJournalIdentities { .. } => Some(
            vindictus_server::infrastructure::item_journals::read_snapshot(
                std::io::stdin().lock(),
            )?,
        ),
        _ => None,
    };
    let item_loader = match &cli.command {
        Command::ImportItemLoaderState { .. } => Some(
            vindictus_server::infrastructure::item_loader_state::read_snapshot(
                std::io::stdin().lock(),
            )?,
        ),
        _ => None,
    };
    let mail_reference = match &cli.command {
        Command::ImportMailReference { .. } => Some(
            vindictus_server::infrastructure::mail_reference::read_snapshot(
                std::io::stdin().lock(),
            )?,
        ),
        _ => None,
    };
    let newbie_attendance = match &cli.command {
        Command::ImportNewbieAttendance { .. } => Some(
            vindictus_server::infrastructure::newbie_attendance::read_snapshot(
                std::io::stdin().lock(),
            )?,
        ),
        _ => None,
    };
    let transfer_guard = match &cli.command {
        Command::ImportCharacterTransferGuard { .. } => Some(
            vindictus_server::infrastructure::character_transfer_guard::read_snapshot(
                std::io::stdin().lock(),
            )?,
        ),
        _ => None,
    };
    let captcha = match &cli.command {
        Command::ImportUserCaptcha { .. } => Some(
            vindictus_server::infrastructure::user_captcha::read_snapshot(std::io::stdin().lock())?,
        ),
        _ => None,
    };
    let status_effects = match &cli.command {
        Command::ImportCharacterStatusEffects { .. } => Some(
            vindictus_server::infrastructure::character_status_effects::read_snapshot(
                std::io::stdin().lock(),
            )?,
        ),
        _ => None,
    };
    let vocation = match &cli.command {
        Command::ImportCharacterVocation { .. } => Some(
            vindictus_server::infrastructure::character_vocation::read_snapshot(
                std::io::stdin().lock(),
            )?,
        ),
        _ => None,
    };
    let skills = match &cli.command {
        Command::ImportCharacterSkills { .. } => Some(
            vindictus_server::infrastructure::character_skills::read_snapshot(
                std::io::stdin().lock(),
            )?,
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
        | Command::InspectFionaInventory { .. }
        | Command::InspectItemColors { .. }
        | Command::InspectItemStats { .. }
        | Command::InspectFionaTitleGoals { .. }
        | Command::InspectItemCatalog { .. }
        | Command::InspectLabFrontendProfile { .. }
        | Command::InspectMinimumCreationAssets { .. }
        | Command::InspectMissionContent { .. }
        | Command::LabExportShopList { .. }
        | Command::ExportOpenapi { .. }
        | Command::LabFrontend { .. } => {}
        Command::AccountSetRole {
            username,
            role,
            reason,
        } => {
            use vindictus_server::{
                application::admin_session::set_role_from_cli, domain::admin::PanelRole,
            };
            postgres::check_ready(&pool).await?;
            let role = PanelRole::parse(&role);
            set_role_from_cli(&pool, &username, role, &reason)
                .await
                .map_err(|e| Error::Configuration(format!("role change refused: {e}")))?;
            println!(
                "{}",
                serde_json::json!({"username": username, "panel_role": role})
            );
        }
        Command::Migrate => {
            postgres::migrate(&pool).await?;
            println!("PostgreSQL migrations applied; migrate does not import legacy data.");
        }
        Command::LabGiveItem {
            assets_file,
            cid,
            item_class,
            count,
            service_id,
        } => {
            use vindictus_server::infrastructure::{minimum_assets::Assets, minimum_give};
            let assets = Assets::read(&assets_file)
                .map_err(|_| Error::Configuration("minimum creation assets invalid".into()))?;
            postgres::check_ready(&pool).await?;
            let outcome =
                minimum_give::lab_grant(&pool, &assets, cid, &item_class, count, service_id)
                    .await
                    .map_err(|_| {
                        Error::Configuration(
                            "grant refused: character must be an offline initialized first Fiona"
                                .into(),
                        )
                    })?;
            match outcome {
                Ok(slots) => println!(
                    "{}",
                    serde_json::json!({"granted":true,"changed_slots":slots,"client_visible_after":"character selection"})
                ),
                Err(e) => println!(
                    "{}",
                    serde_json::json!({"granted":false,"result":e.to_string()})
                ),
            }
        }
        Command::LabGiveCash { cid, amount } => {
            use vindictus_server::infrastructure::minimum_cash_shop;
            postgres::check_ready(&pool).await?;
            let balance = minimum_cash_shop::lab_credit(&pool, cid, amount)
                .await
                .map_err(|_| {
                    Error::Configuration(
                        "credit refused: one initialized first Fiona and a positive amount".into(),
                    )
                })?;
            println!(
                "{}",
                serde_json::json!({"credited":amount,"balance":balance,"client_visible_after":"opening the cash shop"})
            );
        }
        Command::CreateAccount {
            lab_short_password, ..
        } => {
            let (username, password) = credentials.ok_or(Error::PasswordInput)?;
            let minimum = if lab_short_password {
                vindictus_server::domain::MIN_LAB_SHORT_PASSWORD
            } else {
                vindictus_server::domain::MIN_PASSWORD
            };
            let id = vindictus_server::application::accounts::create_account_with(
                &pool, username, password, minimum,
            )
            .await?;
            println!(
                "Account created: {id}. Game login: ask a play code (scripts/dev/play.ps1 or the panel's /jogar page)."
            );
        }
        Command::AccountLinkGame {
            username,
            game_name,
            reason,
        } => {
            postgres::check_ready(&pool).await?;
            vindictus_server::application::admin_session::link_game_from_cli(
                &pool, &username, &game_name, &reason,
            )
            .await
            .map_err(|e| Error::Configuration(format!("game link refused: {e}")))?;
            println!(
                "{}",
                serde_json::json!({"username": username, "game_name": game_name})
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
        Command::ImportCharacterState { verify_only, .. } => {
            use vindictus_server::infrastructure::fiona_character_state::{StateError, preserve};
            let snapshot = character_state.ok_or(StateError::Export)?;
            let summary = preserve(&pool, &snapshot, verify_only).await?;
            println!(
                "{}",
                serde_json::to_string(&summary).map_err(|_| StateError::Export)?
            );
        }
        Command::ImportSharedItems {
            verify_only,
            require_empty_owner,
            ..
        } => {
            use vindictus_server::infrastructure::shared_item_state::{SharedError, preserve};
            let mut summary =
                preserve(&pool, &shared.ok_or(SharedError::Export)?, verify_only).await?;
            if let Some(name) = require_empty_owner {
                use vindictus_server::{
                    domain::game_user::LegacyNameKey, infrastructure::character_lobby::UserKey,
                };
                let key = LegacyNameKey::parse(&name).map_err(|_| SharedError::Unavailable)?;
                let user: Option<(i32, String, i64)> = sqlx::query_as(
                    "SELECT id,name,local_revision FROM game_users WHERE name_key=$1",
                )
                .bind(key.as_str())
                .fetch_optional(&pool)
                .await?;
                let (uid, name, revision) = user.ok_or(SharedError::Unavailable)?;
                vindictus_server::infrastructure::shared_item_state::require_empty(
                    &pool,
                    &UserKey {
                        uid,
                        name: &name,
                        revision,
                    },
                )
                .await?;
                summary.empty_owner_verified = true;
            }
            println!(
                "{}",
                serde_json::to_string(&summary).map_err(|_| SharedError::Export)?
            );
        }
        Command::ImportJournalIdentities { verify_only, .. } => {
            use vindictus_server::infrastructure::item_journals::{JournalError, preserve};
            let summary =
                preserve(&pool, &journals.ok_or(JournalError::Input)?, verify_only).await?;
            println!(
                "{}",
                serde_json::to_string(&summary).map_err(|_| JournalError::Input)?
            );
        }
        Command::ImportItemLoaderState { verify_only, .. } => {
            use vindictus_server::infrastructure::item_loader_state::{LoaderError, preserve};
            let summary =
                preserve(&pool, &item_loader.ok_or(LoaderError::Input)?, verify_only).await?;
            println!(
                "{}",
                serde_json::to_string(&summary).map_err(|_| LoaderError::Input)?
            );
        }
        Command::ImportMailReference { verify_only, .. } => {
            use vindictus_server::infrastructure::mail_reference::{MailError, preserve};
            let summary =
                preserve(&pool, &mail_reference.ok_or(MailError::Input)?, verify_only).await?;
            println!(
                "{}",
                serde_json::to_string(&summary).map_err(|_| MailError::Input)?
            );
        }
        Command::ImportNewbieAttendance {
            verify_only,
            require_empty_owner,
            ..
        } => {
            use vindictus_server::{
                domain::game_user::LegacyNameKey,
                infrastructure::newbie_attendance::{AttendanceError, preserve, require_absent},
            };
            let summary = preserve(
                &pool,
                &newbie_attendance.ok_or(AttendanceError::Input)?,
                verify_only,
            )
            .await?;
            let mut empty_owner_verified = false;
            if let Some(name) = require_empty_owner {
                let key = LegacyNameKey::parse(&name).map_err(|_| AttendanceError::Input)?;
                let mut tx = pool.begin().await?;
                let uid: Option<i32> =
                    sqlx::query_scalar("SELECT id FROM game_users WHERE name_key=$1 FOR SHARE")
                        .bind(key.as_str())
                        .fetch_optional(&mut *tx)
                        .await?;
                require_absent(&mut tx, uid.ok_or(AttendanceError::Input)?).await?;
                tx.commit().await?;
                empty_owner_verified = true;
            }
            println!("{}",serde_json::to_string(&serde_json::json!({"summary":summary,"empty_owner_verified":empty_owner_verified})).map_err(|_|AttendanceError::Input)?);
        }
        Command::ImportCharacterTransferGuard { verify_only, .. } => {
            use vindictus_server::infrastructure::character_transfer_guard::{
                TransferError, preserve,
            };
            let summary = preserve(
                &pool,
                &transfer_guard.ok_or(TransferError::Input)?,
                verify_only,
            )
            .await?;
            println!(
                "{}",
                serde_json::to_string(&summary).map_err(|_| TransferError::Input)?
            );
        }
        Command::ImportUserCaptcha {
            verify_only,
            require_empty_owner,
            ..
        } => {
            use vindictus_server::{
                domain::game_user::LegacyNameKey,
                infrastructure::{
                    character_lobby::UserKey,
                    user_captcha::{CaptchaError, preserve, require_absent},
                },
            };
            let mut summary =
                preserve(&pool, &captcha.ok_or(CaptchaError::Export)?, verify_only).await?;
            if let Some(name) = require_empty_owner {
                let key = LegacyNameKey::parse(&name).map_err(|_| CaptchaError::Owner)?;
                let mut tx = pool.begin().await?;
                let row: Option<(i32, String, i64)> = sqlx::query_as(
                    "SELECT id,name,local_revision FROM game_users WHERE name_key=$1",
                )
                .bind(key.as_str())
                .fetch_optional(&mut *tx)
                .await?;
                let (uid, name, revision) = row.ok_or(CaptchaError::Owner)?;
                require_absent(
                    &mut tx,
                    &UserKey {
                        uid,
                        name: &name,
                        revision,
                    },
                )
                .await?;
                tx.commit().await?;
                summary.empty_owner_verified = true;
            }
            println!(
                "{}",
                serde_json::to_string(&summary).map_err(|_| CaptchaError::Export)?
            );
        }
        Command::ImportCharacterStatusEffects { verify_only, .. } => {
            use vindictus_server::infrastructure::character_status_effects::{
                StatusError, preserve,
            };
            let summary = preserve(
                &pool,
                &status_effects.ok_or(StatusError::Export)?,
                verify_only,
            )
            .await?;
            println!(
                "{}",
                serde_json::to_string(&summary).map_err(|_| StatusError::Export)?
            );
        }
        Command::ImportCharacterVocation { verify_only, .. } => {
            use vindictus_server::infrastructure::character_vocation::{VocationError, preserve};
            let summary =
                preserve(&pool, &vocation.ok_or(VocationError::Export)?, verify_only).await?;
            println!(
                "{}",
                serde_json::to_string(&summary).map_err(|_| VocationError::Export)?
            );
        }
        Command::ImportCharacterSkills { verify_only, .. } => {
            use vindictus_server::infrastructure::character_skills::{SkillError, preserve};
            let snapshot = skills.ok_or(SkillError::Export)?;
            let summary = preserve(&pool, &snapshot, verify_only).await?;
            println!(
                "{}",
                serde_json::to_string(&summary).map_err(|_| SkillError::Export)?
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
