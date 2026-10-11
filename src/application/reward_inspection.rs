//! Read-only diagnostics: public output contains counts and digests, never item classes or names.
use crate::{
    domain::{
        dormant_rewards::{RewardCharacter, plan_rewards},
        legacy_user_data::SqlDateTime,
    },
    error::Error,
    infrastructure::reward_policy::RewardPolicyPackage,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
#[derive(Serialize)]
pub struct RewardInspection {
    pub source_database_sha256: String,
    pub region: String,
    pub game_code: i32,
    pub server_code: i32,
    pub catalog_rows: usize,
    pub characters: usize,
    pub queued_calls: usize,
    pub plan_sha256: String,
    pub clock_is_synthetic: bool,
    pub database_mutations: bool,
    pub game_authentication_available: bool,
}
/// Clock zero is diagnostic-only. No SQL calls, queue INSERTs or account writes are performed.
pub fn inspect(
    package: &RewardPolicyPackage,
    characters: &[RewardCharacter<'_>],
) -> Result<RewardInspection, Error> {
    let clock = SqlDateTime::from_units300(0)?;
    let plan = plan_rewards(characters, &package.rewards, &package.policy, || Ok(clock))?;
    let mut digest = Sha256::new();
    digest.update(b"DormantRewardPlan\0v1");
    for request in &plan {
        let canonical = request.replay_key();
        digest.update((canonical.len() as u32).to_le_bytes());
        digest.update(canonical.as_slice());
    }
    Ok(RewardInspection {
        source_database_sha256: package.manifest.database_sha256.clone(),
        region: package.manifest.region.clone(),
        game_code: package.manifest.game_code,
        server_code: package.manifest.server_code,
        catalog_rows: package.rewards.len(),
        characters: characters.len(),
        queued_calls: plan.len(),
        plan_sha256: format!("{:x}", digest.finalize()),
        clock_is_synthetic: true,
        database_mutations: false,
        game_authentication_available: false,
    })
}
