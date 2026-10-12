use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid or oversized private item catalog projection; values omitted")]
    ItemCatalogExport,
    #[error(transparent)]
    ItemCatalog(#[from] crate::domain::item_catalog::CatalogError),
    #[error("invalid or oversized private reward policy; values omitted")]
    RewardPolicyExport,
    #[error("feature expression is malformed or exceeds bounded evaluation")]
    FeatureExpression,
    #[error("feature version comparison lacks verified Framework evidence")]
    FeatureVersionUnsupported,
    #[error("feature integer overflows source Int32 range")]
    FeatureIntegerOverflow,
    #[error("reward planning exceeds bounded character/catalog/output limits")]
    RewardPlanLimit,
    #[error("queue identity requires the explicit supported source counter; reseeding refused")]
    QueueIdentityProfile,
    #[error("queue identity has not been initialized from the source profile")]
    QueueIdentityUninitialized,
    #[error("new queue snapshots cannot be imported after identity initialization")]
    QueueImportAfterInitialization,
    #[error("queue operation differs from its durable replay receipt")]
    QueueReplayConflict,
    #[error("queue deadline exceeded; replay the same operation to verify commit")]
    QueueDeadline,
    #[error("invalid or oversized character/login bundle; values omitted")]
    CharacterExport,
    #[error("character/login snapshot differs from preserved source data")]
    CharacterSnapshotConflict,
    #[error("character import deadline exceeded; verify original batch before retry")]
    CharacterImportDeadline,
    #[error("character query exceeds bounded result limit")]
    CharacterQueryLimit,
    #[error("login effects require unmigrated new-user/character/reward dependencies")]
    LoginEffectDependencies,
    #[error("login effect operation conflicts with its durable replay receipt")]
    LoginEffectReplayConflict,
    #[error("login effect deadline exceeded; replay the same operation to verify commit")]
    LoginEffectDeadline,
    #[error("login account snapshot conflicts with preserved source data")]
    LoginAccountSnapshotConflict,
    #[error("pending achievement name requires unsupported source equality")]
    LoginAccountEqualityUnsupported,
    #[error("LimitedAccount has multiple matching rows; policy refused")]
    LimitedAccountDuplicate,
    #[error("LimitedAccount contains data outside the verified equality contract; policy refused")]
    LimitedAccountUnsupported,
    #[error("legacy clock profile requires an explicit offset within 14 hours")]
    LegacyClockProfile,
    #[error("legacy local clock arithmetic exceeds DateTime range; mutation refused")]
    LegacyClockOverflow,
    #[error("invalid legacy ban data; values are intentionally omitted")]
    LegacyBanData,
    #[error("legacy ban snapshot conflicts with stored data")]
    LegacyBanConflict,
    #[error("game user was not found")]
    GameUserMissing,
    #[error("game user local revision changed; mutation refused")]
    GameUserRevisionConflict,
    #[error("invalid or oversized legacy user export; values are intentionally omitted")]
    UserExport,
    #[error("user import deadline exceeded; verify the batch before retrying")]
    UserImportDeadline,
    #[error(transparent)]
    LegacyUserData(#[from] crate::domain::legacy_user_data::LegacyUserDataError),
    #[error("legacy user snapshot conflicts with persisted source data")]
    LegacyUserSnapshotConflict,
    #[error(transparent)]
    GameUserName(#[from] crate::domain::game_user::GameUserNameError),
    #[error("game user name is already bound to another legacy ID")]
    GameUserNameConflict,
    #[error("legacy ID is already bound to another game user name")]
    GameUserIdConflict,
    #[error("game user identity changed concurrently; provisioning refused")]
    GameUserConcurrentChange,
    #[error(transparent)]
    FrontendProbe(#[from] crate::infrastructure::frontend::ProbeError),
    #[error("configuration error: {0}")]
    Configuration(String),
    #[error("invalid username: use 3-32 ASCII letters, digits or underscores")]
    InvalidUsername,
    #[error("password must contain 12-128 bytes, without CR, LF or NUL")]
    InvalidPassword,
    #[error("could not read password from standard input")]
    PasswordInput,
    #[error("could not hash password")]
    PasswordHash,
    #[error("account name already exists")]
    AccountExists,
    #[error("database operation failed; check database availability and schema")]
    Database(#[source] sqlx::Error),
    #[error("database migration failed")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("database schema is not ready; run the migrate command")]
    SchemaNotReady,
    #[error("background password task failed")]
    PasswordTask(#[from] tokio::task::JoinError),
    #[error("server IO operation failed")]
    Io(#[from] std::io::Error),
}

impl From<sqlx::Error> for Error {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error)
    }
}
