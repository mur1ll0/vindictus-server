//! Existing-account loyalty/dormancy/reward use case. Not wired to game authentication.
use crate::{
    domain::{
        game_user::LegacyNameKey,
        legacy_user_data::SqlDateTime,
        login_effects::{EffectClockSamples, EffectFlags},
    },
    error::Error,
    infrastructure::{login_rewards, reward_policy::RewardPolicyPackage},
};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;
use zeroize::Zeroizing;

/// Retain the operation ID and all explicit decision clocks across ambiguous retries.
/// Reward clocks are a dependency sampled once per eligible item on the first execution.
pub struct LoginRewardRequest<'a> {
    pub operation_id: Uuid,
    pub uid: i32,
    pub name: &'a str,
    pub expected_revision: i64,
    pub clocks: EffectClockSamples,
    pub loyalty_insert_now: SqlDateTime,
    pub content: &'a RewardPolicyPackage,
}
#[derive(Debug, PartialEq, Eq)]
pub struct LoginRewardResult {
    pub flags: EffectFlags,
    pub revision: i64,
    pub queued_row_ids: Vec<i32>,
    pub replayed: bool,
}
fn bytes(hash: &mut Sha256, value: &[u8]) {
    hash.update((value.len() as u64).to_le_bytes());
    hash.update(value);
}
fn optional(hash: &mut Sha256, value: Option<&str>) {
    hash.update([u8::from(value.is_some())]);
    if let Some(value) = value {
        bytes(hash, value.as_bytes());
    }
}
/// Bind actual resolved content, not just caller-supplied file hashes. Ordered duplicate
/// catalog rows matter. FeaturePolicy's private map is sorted and bounded at construction.
fn semantic_digest(content: &RewardPolicyPackage) -> Result<[u8; 32], Error> {
    let manifest = &content.manifest;
    if content.rewards.len() > 1000
        || manifest.catalog_rows != content.rewards.len()
        || manifest.database_sha256.len() != 64
        || manifest.config_sha256.len() != 64
        || manifest.region.len() > 64
        || manifest
            .override_region
            .as_ref()
            .is_some_and(|v| v.len() > 64)
    {
        return Err(Error::RewardPolicyExport);
    }
    let mut hash = Sha256::new();
    hash.update(b"LoginRewardContent\0v1");
    let manifest =
        Zeroizing::new(serde_json::to_vec(manifest).map_err(|_| Error::RewardPolicyExport)?);
    bytes(&mut hash, &manifest);
    for (key, value) in content.policy.entries() {
        bytes(&mut hash, key.as_bytes());
        bytes(&mut hash, value.as_bytes());
    }
    hash.update([0xff]);
    hash.update((content.rewards.len() as u64).to_le_bytes());
    for row in &content.rewards {
        if row
            .feature
            .as_ref()
            .is_some_and(|value| value.encode_utf16().count() > 50)
        {
            return Err(Error::RewardPolicyExport);
        }
        bytes(&mut hash, row.item_class.as_bytes());
        hash.update(row.num.to_le_bytes());
        hash.update(row.min_level.to_le_bytes());
        hash.update(row.max_level.to_le_bytes());
        optional(&mut hash, row.feature.as_deref());
    }
    Ok(hash.finalize().into())
}
pub async fn apply_existing_login_rewards(
    pool: &PgPool,
    request: &LoginRewardRequest<'_>,
    reward_clock: impl FnMut() -> Result<SqlDateTime, Error>,
) -> Result<LoginRewardResult, Error> {
    let name = LegacyNameKey::parse(request.name)?;
    if request.expected_revision < 0 {
        return Err(Error::GameUserRevisionConflict);
    }
    let digest = semantic_digest(request.content)?;
    let c = request.clocks;
    // Operational receipt stores only fixed clocks, revision and a semantic content digest.
    let canonical = Zeroizing::new(serde_json::to_vec(&serde_json::json!({
        "version":1,"revision":request.expected_revision,"content_sha256":digest,
        "clocks":[c.loyalty_now.ticks(),c.dormant_elapsed_now.ticks(),c.latest_elapsed_now.ticks(),c.set_dormant_now.ticks(),c.set_event_now.ticks()],
        "loyalty_insert_units300":request.loyalty_insert_now.units300(),
    })).map_err(|_|Error::RewardPolicyExport)?);
    tokio::time::timeout(
        std::time::Duration::from_secs(15),
        login_rewards::execute(pool, request, name.as_str(), &canonical, reward_clock),
    )
    .await
    .map_err(|_| Error::LoginEffectDeadline)?
}
