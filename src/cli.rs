use crate::error::Error;
use clap::{Parser, Subcommand};
use std::{io::Read, path::PathBuf};
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

/// Folders of the client builder service (docs/future/game-launcher.md 4.10).
#[derive(Debug, Clone, clap::Args)]
pub struct ClientBuilderArgs {
    /// The update store (read only).
    #[arg(long)]
    pub store: PathBuf,
    /// The server's client data (`${CLIENT_SERVER_DIR}`): work/, staging/, build-state/, inputs/workspace.
    #[arg(long)]
    pub server_dir: PathBuf,
    /// The recipe (scripts/client and scripts/content) inside the image.
    #[arg(long, default_value = "/app/recipe")]
    pub recipe: PathBuf,
    /// The published panel content the game writes at start (content-state/content-edits.json).
    #[arg(long)]
    pub edits: PathBuf,
    #[arg(long, default_value = "python3")]
    pub python: PathBuf,
}

/// Subcommands of `client-builder`. Both open the database (the drafts and jobs).
#[derive(Debug, Clone, Subcommand)]
pub enum ClientBuilderCommand {
    /// The service: applies the published panel content to the server's client when the panel asks,
    /// and puts what changed in the draft of the next version.
    Worker {
        #[command(flatten)]
        paths: ClientBuilderArgs,
    },
    /// One-time build of the server's client tree from the imported 1.69 original (after
    /// scripts/dev/import-client-base.ps1); what differs from the published version goes to the draft.
    Seed {
        #[command(flatten)]
        paths: ClientBuilderArgs,
        /// Layer of the recipe (3 = base + Delia + Miri).
        #[arg(long, default_value_t = 3)]
        layer: u8,
        /// The Messenger DLL (tools/nmcogame), imported with the original.
        #[arg(long)]
        nmcogame: PathBuf,
    },
}

/// Subcommands of `client-dist`. None opens the database.
#[derive(Debug, Clone, Subcommand)]
pub enum ClientDistCommand {
    /// Create the Ed25519 key pair (refuses to replace one); prints the public key to pin in the launcher.
    Keygen {
        /// Folder of the private key, outside the served store.
        #[arg(long)]
        keys: PathBuf,
    },
    /// Publish a client tree (scripts/client/build_client_version.py output) as the next version.
    Publish {
        #[arg(long)]
        store: PathBuf,
        #[arg(long)]
        keys: PathBuf,
        #[arg(long)]
        tree: PathBuf,
        /// The 4096-byte pack keystream (published as an object of the manifest).
        #[arg(long)]
        keystream: PathBuf,
        #[arg(long)]
        title: String,
        #[arg(long, default_value = "")]
        notes: String,
        /// Oldest version still accepted (default: the previous one's, or 1).
        #[arg(long)]
        minimum: Option<u32>,
        /// Compression threads (1-4; the operator machine runs hot: default 2).
        #[arg(long, default_value_t = 2)]
        threads: usize,
    },
    /// Check signatures and objects of a store (`--deep` decompresses every object).
    Verify {
        #[arg(long)]
        store: PathBuf,
        /// Public key (64 hex digits); default: the store's updates.ed25519.pub.
        #[arg(long)]
        public_key: Option<String>,
        #[arg(long)]
        deep: bool,
    },
    /// Oldest client version allowed to play (re-signs versao.json).
    Minimum {
        #[arg(long)]
        store: PathBuf,
        #[arg(long)]
        keys: PathBuf,
        #[arg(long)]
        minimum: u32,
    },
    /// Withdraw the current version: publishes N+1 with the files of N-1 (nothing is deleted).
    Withdraw {
        #[arg(long)]
        store: PathBuf,
        #[arg(long)]
        keys: PathBuf,
        #[arg(long, default_value = "Versão retirada")]
        title: String,
    },
    /// Publish a launcher executable for self-update (vindictus-launcher.exe of tools/launcher/build.ps1).
    Launcher {
        #[arg(long)]
        store: PathBuf,
        #[arg(long)]
        keys: PathBuf,
        #[arg(long)]
        exe: PathBuf,
        /// Its version (the `version` of tools/launcher/Cargo.toml).
        #[arg(long)]
        version: String,
    },
    /// Read-only HTTP of the store on its own listener (port 8090 in compose).
    Serve {
        #[arg(long)]
        store: PathBuf,
        #[arg(long, default_value = "127.0.0.1:8090")]
        bind: std::net::SocketAddr,
        /// Service reported by status.json, `name=host:port` (repeatable).
        #[arg(long = "probe")]
        probes: Vec<crate::http::updates::Probe>,
    },
}

// Parsed once at start; the lab-frontend flags make it the largest variant.
#[allow(clippy::large_enum_variant)]
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
        /// Explicit native minimum;optional game services remain deferred.
        #[arg(long)]
        minimum_creation_assets: Option<std::path::PathBuf>,
        /// Laboratory container: bind all interfaces on the configured loopback ports, which
        /// Docker publishes only on the host loopback; advertised addresses stay loopback.
        #[arg(long)]
        container_listen: bool,
        /// Loopback IP:port of the administration panel (for example 127.0.0.1:27090);
        /// omitted, no panel listener is opened.
        #[arg(long)]
        admin_listen: Option<std::net::SocketAddr>,
        /// Built panel single-page app (web/admin/dist) served by the panel listener.
        #[arg(long, requires = "admin_listen")]
        admin_ui_dir: Option<std::path::PathBuf>,
        /// Mark the panel cookie Secure (only behind HTTPS).
        #[arg(long, requires = "admin_listen")]
        admin_secure_cookie: bool,
        /// English item/skill/status names for the panel (scripts/content/export_item_names.py).
        #[arg(long, requires = "admin_listen")]
        admin_item_names: Option<std::path::PathBuf>,
        /// Panel catalog (scripts/content/export_panel_catalog.py: shops' NPCs, item icons).
        #[arg(long, requires = "admin_item_names")]
        admin_panel_catalog: Option<std::path::PathBuf>,
        /// Exported icons directory (icons/item/*.png).
        #[arg(long, requires = "admin_listen")]
        admin_icons_dir: Option<std::path::PathBuf>,
        /// Read-only client copy (its hfs/ packs), for the panel's model viewer and kits.
        #[arg(long, requires = "admin_client_keystream")]
        admin_client_dir: Option<std::path::PathBuf>,
        /// The 4096-byte HFS pack keystream (scripts/client/hfs.py keystream), never versioned.
        #[arg(long, requires = "admin_client_dir")]
        admin_client_keystream: Option<std::path::PathBuf>,
        /// Item editor: private assets folder (.cache/private-assets) where uploaded model kits
        /// are kept; never versioned.
        #[arg(long, requires = "admin_listen")]
        admin_private_assets: Option<std::path::PathBuf>,
        /// "Versões do cliente" (L5): the update store (CLIENT_DIST_DIR), read and written by the panel.
        #[arg(long, requires_all = ["admin_listen", "admin_client_dist_keys"])]
        admin_client_dist: Option<std::path::PathBuf>,
        /// The folder of the store's private key (client-dist keygen), mounted only into the panel.
        #[arg(long, requires = "admin_client_dist")]
        admin_client_dist_keys: Option<std::path::PathBuf>,
        /// Client version drafts (docs/future/game-launcher.md 4.10): the staging folder of the server's
        /// client data (`${CLIENT_SERVER_DIR}/staging`), where uploaded files wait for publishing.
        #[arg(long, requires = "admin_client_dist")]
        admin_client_staging: Option<std::path::PathBuf>,
        /// Item editor: the host's item export stamp (.cache/private-content/item-content.json,
        /// scripts/content/refresh_item_content.py). A published item version newer than it
        /// is not recorded as applied at start.
        #[arg(long)]
        item_content_stamp: Option<std::path::PathBuf>,
        /// Write the cash shop ShopList.aspx (published prices included) at start.
        #[arg(long)]
        shop_list_output: Option<std::path::PathBuf>,
        /// IPv4 other computers reach this host at (LAN or VPN, compose PUBLIC_HOST): sent to clients
        /// for the channel, the relay and the cash shop instead of the loopback.
        #[arg(long)]
        advertise_host: Option<std::net::Ipv4Addr>,
        /// Write the bootstrap EndPoint.txt (server list) with the advertised host at start.
        #[arg(long)]
        endpoint_output: Option<std::path::PathBuf>,
        /// Write the published content overlay (content-edits.json) at start, for the client
        /// launcher that updates the client copy.
        #[arg(long)]
        content_edits_output: Option<std::path::PathBuf>,
    },
    /// Give, change or remove (`none`) the panel role of an existing account; audited.
    AccountSetRole {
        #[arg(long)]
        username: String,
        #[arg(long, value_parser = ["admin", "gm", "leitura", "none"])]
        role: String,
    },
    /// Write the cumulative overlay of the published content edits (B4.4) for the content
    /// exporters; prints the version.
    ContentEditsExport {
        #[arg(long)]
        output: std::path::PathBuf,
    },
    /// Record that the host applied the content edits up to this version.
    ContentEditsApplied {
        #[arg(long)]
        version: i32,
    },
    /// Write the panel OpenAPI document (web/admin/openapi.json); no database or listener.
    ExportOpenapi {
        #[arg(long)]
        output: std::path::PathBuf,
    },
    /// Laboratory GiveItem for an offline first Fiona (gold or any item class); the
    /// client sees the result after selecting the character. Not a game command.
    LabGiveItem {
        #[arg(long)]
        assets_file: std::path::PathBuf,
        /// Omit to use the only initialized first Fiona.
        #[arg(long)]
        cid: Option<i64>,
        /// Source ItemClass or ItemClassEx, ASCII (for example `gold` or
        /// `enchant_scroll[PREFIX:bitter]`).
        #[arg(long)]
        item_class: String,
        #[arg(long)]
        count: i32,
        /// Entity Service.ID for new item IDs; must differ from the running frontend's.
        #[arg(long, default_value_t = 65534)]
        service_id: i32,
        /// GiveItem with FailMethodEnum.ForceMail: what does not fit goes to the mailbox
        /// (DungeonItemMail), as the original rewards do.
        #[arg(long)]
        force_mail: bool,
    },
    /// Laboratory cash credit for the account of an initialized first Fiona (the NISMS
    /// charge the cash shop reads). Not a game command.
    LabGiveCash {
        /// Omit to use the only initialized first Fiona.
        #[arg(long)]
        cid: Option<i64>,
        #[arg(long)]
        amount: i32,
    },
    /// Write the laboratory ShopList.aspx (the in-game cash shop product list the client
    /// downloads from ingame_cashshop_shopListURL); no database or listener.
    LabExportShopList {
        #[arg(long)]
        assets_file: std::path::PathBuf,
        #[arg(long)]
        output: std::path::PathBuf,
        /// Products listed per category (the client stalls on the whole catalog).
        #[arg(long, default_value_t = 10)]
        per_category: usize,
    },
    /// Apply embedded PostgreSQL migrations. Does not import SQL Server backups.
    Migrate,
    /// Client distribution of the launcher (docs/future/game-launcher.md 4.2-4.7): keys, publish, verify, serve.
    ClientDist {
        #[command(subcommand)]
        command: ClientDistCommand,
    },
    /// The server's client tree and the drafts of the next version (docs/future/game-launcher.md 4.10).
    ClientBuilder {
        #[command(subcommand)]
        command: ClientBuilderCommand,
    },
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
    /// Compose the private normal-Fiona profile without opening a database or listener.
    InspectMinimumCreationAssets {
        #[arg(long)]
        assets_file: std::path::PathBuf,
    },
    /// Generate a solo sector sequence for every quest of the private profile and report
    /// aggregate counts and GameStartGranted sizes; no database or listener.
    InspectMissionContent {
        #[arg(long)]
        assets_file: std::path::PathBuf,
        #[arg(long, default_value_t = 1)]
        difficulty: i32,
    },
    /// Compose the private normal-Fiona profile without opening a database or listener.
    InspectFionaInventory {
        #[arg(long, required = true)]
        input_stdin: bool,
        #[arg(long)]
        catalog_file: std::path::PathBuf,
        #[arg(long)]
        stats_file: std::path::PathBuf,
        #[arg(long)]
        colors_file: std::path::PathBuf,
    },
    /// Validate private material colors against the pinned effective source catalog.
    InspectItemColors {
        #[arg(long, required = true)]
        input_stdin: bool,
        #[arg(long)]
        catalog_file: std::path::PathBuf,
    },
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
    /// Preserve title, MicroPlay and mission initialization reference state from private stdin.
    ImportCharacterState {
        #[arg(long, required = true)]
        input_stdin: bool,
        #[arg(long)]
        verify_only: bool,
    },
    /// Preserve the shared inventory references/status and identity from private stdin.
    ImportSharedItems {
        #[arg(long)]
        require_empty_owner: Option<String>,
        #[arg(long, required = true)]
        input_stdin: bool,
        #[arg(long)]
        verify_only: bool,
    },
    /// Preserve the two independent legacy journal identities from private stdin.
    ImportJournalIdentities {
        #[arg(long, required = true)]
        input_stdin: bool,
        #[arg(long)]
        verify_only: bool,
    },
    /// Inspect the bound private title-goal constructor profile;counts only.
    InspectFionaTitleGoals {
        #[arg(long)]
        initializer_file: PathBuf,
        #[arg(long)]
        reference_file: PathBuf,
        #[arg(long)]
        catalog_file: PathBuf,
        #[arg(long, required = true)]
        input_stdin: bool,
    },
    /// Preserve all initial item-loader association rows and five identities.
    ImportItemLoaderState {
        #[arg(long, required = true)]
        input_stdin: bool,
        #[arg(long)]
        verify_only: bool,
    },
    /// Preserve complete Mail records;protected stdin,counts-only output.
    ImportMailReference {
        #[arg(long, required = true)]
        input_stdin: bool,
        #[arg(long)]
        verify_only: bool,
    },
    /// Preserve all four AttendanceNewbieInfo fields from protected stdin.
    ImportNewbieAttendance {
        #[arg(long, required = true)]
        input_stdin: bool,
        #[arg(long)]
        verify_only: bool,
        #[arg(long)]
        require_empty_owner: Option<String>,
    },
    /// Preserve the proven empty CharacterInfo_Trans reference for native Join.
    ImportCharacterTransferGuard {
        #[arg(long, required = true)]
        input_stdin: bool,
        #[arg(long)]
        verify_only: bool,
    },
    /// Preserve all six UserCaptcha fields from private stdin.
    ImportUserCaptcha {
        #[arg(long, required = true)]
        input_stdin: bool,
        #[arg(long)]
        verify_only: bool,
        #[arg(long)]
        require_empty_owner: Option<String>,
    },
    /// Preserve all StatusEffect fields and original identity from private stdin.
    ImportCharacterStatusEffects {
        #[arg(long, required = true)]
        input_stdin: bool,
        #[arg(long)]
        verify_only: bool,
    },
    /// Preserve Vocation/VocationSkill and original identity from private stdin.
    ImportCharacterVocation {
        #[arg(long, required = true)]
        input_stdin: bool,
        #[arg(long)]
        verify_only: bool,
    },
    /// Preserve Skill/SPSkill rows and the original SPSkill identity from private stdin.
    ImportCharacterSkills {
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
        /// Laboratory only (operator decision 2026-10-25): accept a password of 5 or more
        /// bytes instead of 12. Insecure outside a loopback laboratory.
        #[arg(long)]
        lab_short_password: bool,
    },
    /// Make an account enter an existing game user (NexonID), keeping its characters; the
    /// game name alone (allowlist passport) then no longer logs in. Audited.
    AccountLinkGame {
        #[arg(long)]
        username: String,
        #[arg(long)]
        game_name: String,
    },
}

/// Read at most 131 bytes. A final LF/CRLF supports redirected files and pipes.
pub fn read_password(reader: impl Read) -> Result<Zeroizing<Vec<u8>>, Error> {
    read_password_with(reader, crate::domain::MIN_PASSWORD)
}

pub fn read_password_with(reader: impl Read, minimum: usize) -> Result<Zeroizing<Vec<u8>>, Error> {
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
    crate::domain::validate_password_with(&password, minimum)?;
    Ok(password)
}
