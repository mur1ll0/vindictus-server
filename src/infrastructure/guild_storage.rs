//! Guild storage (B5 G6b): GuildStorageManager's state read from the guild's ItemOwner
//! (GuildStorageStatus.TotalStorageCount slots of one bag, GuildStorageItems) and
//! GuildStorageSetting; HandleGuildStorageSession (the member's listening flag, then
//! GuildInventoryInfoMessage and both GuildStorageLogsMessages) and
//! UpdateGuildStorageSettings (the master only, broadcast to the listening members through
//! the party hub). Deposits, picks, arranging, slot purchases and the storage logs and ledger
//! are G6c–G6f: until then the storage holds no item and the logs are empty.
use super::guild::GuildHub;
use super::minimum_assets::Assets;
use super::minimum_character::MinimumError;
use super::party_hub::PartyHub;
use crate::domain::guild::user_type;
use crate::domain::map_partition::OutgoingPacket;
use crate::protocol::DEFAULT_MAX_BODY;
use crate::protocol::guild::rank;
use crate::protocol::guild_storage::{self as wire, InventoryInfo, Logs, Message};
use sqlx::PgPool;

fn fail() -> MinimumError {
    MinimumError::Conflict
}

/// InitGuildStorageSetting's AccessLimitTag.
pub const INITIAL_ACCESS_LIMIT_TAG: i64 = 898_779;

/// GuildStorageManager after QuildGuildStorageStatus and GetGuildStorageSetting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Storage {
    /// StorageCount (GuildStorage.SlotCount).
    pub slot_count: i32,
    /// GoldPickLimit.
    pub gold_limit: i32,
    pub access_limit_tag: i64,
}

fn packet(message: &Message) -> Result<OutgoingPacket, MinimumError> {
    let (category, body) = wire::packet(message, DEFAULT_MAX_BODY).map_err(|_| fail())?;
    Ok(OutgoingPacket { category, body })
}

/// The manager's load: the slots of GuildStorageStatus (0 without a row) and the setting; a
/// guild without a setting row gets InitGuildStorageSetting while this load keeps 0/0, as the
/// manager's fields stay unset that time. Items in the storage need the G6c projection.
pub async fn load(pool: &PgPool, guild_sn: i32) -> Result<Storage, MinimumError> {
    let items: i64 =
        sqlx::query_scalar("SELECT count(*) FROM game_guild_storage_items WHERE guild_id=$1")
            .bind(guild_sn)
            .fetch_one(pool)
            .await?;
    if items != 0 {
        return Err(fail());
    }
    let slot_count: Option<i32> = sqlx::query_scalar(
        "SELECT total_storage_count FROM game_guild_storage_status WHERE guild_id=$1",
    )
    .bind(guild_sn)
    .fetch_optional(pool)
    .await?;
    let setting: Option<(i64, i32)> = sqlx::query_as(
        "SELECT access_limit_tag,gold_limit FROM game_guild_storage_setting WHERE guild_id=$1",
    )
    .bind(guild_sn)
    .fetch_optional(pool)
    .await?;
    let (access_limit_tag, gold_limit) = match setting {
        Some(s) => s,
        None => {
            sqlx::query(
                "INSERT INTO game_guild_storage_setting(guild_id,access_limit_tag,gold_limit) \
                 VALUES($1,$2,0) ON CONFLICT DO NOTHING",
            )
            .bind(guild_sn)
            .bind(INITIAL_ACCESS_LIMIT_TAG)
            .execute(pool)
            .await?;
            (0, 0)
        }
    };
    Ok(Storage {
        slot_count: slot_count.unwrap_or(0),
        gold_limit,
        access_limit_tag,
    })
}

/// GuildEntity.GetRegularMemberCount.
pub async fn regular_members(pool: &PgPool, guild_sn: i32) -> Result<i32, MinimumError> {
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM game_guild_user WHERE guild_sn=$1 AND group_user_type = ANY($2)",
    )
    .bind(guild_sn)
    .bind(vec![
        user_type::MASTER,
        user_type::OPERATOR,
        user_type::MEMBER,
    ])
    .fetch_one(pool)
    .await?;
    i32::try_from(count).map_err(|_| fail())
}

/// The FeatureMatrix values of the storage.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StorageConfig {
    /// GuildStorage: the manager only exists with it.
    pub feature: bool,
    /// GuildStorageRequiredMember.
    pub required_member: i32,
}
impl StorageConfig {
    pub fn from_assets(assets: &Assets) -> Result<Self, MinimumError> {
        let Some(policy) = assets.npc_policy() else {
            return Ok(Self::default());
        };
        let feature = policy
            .is_enabled(Some("GuildStorage"))
            .map_err(|_| fail())?;
        Ok(Self {
            feature,
            required_member: if feature {
                policy
                    .get_integer("GuildStorageRequiredMember")
                    .map_err(|_| fail())?
            } else {
                0
            },
        })
    }
    /// GuildStorageManager.IsEnabled: GuildStorageRequiredMember regular members or more.
    pub fn is_enabled(&self, regular: i32) -> bool {
        regular >= self.required_member
    }
}

/// InventoryMessage: GuildInventoryInfoMessage(IsEnabled, StorageCount, GoldPickLimit,
/// AccessLimitTag, SlotInfo).
pub fn info_packet(storage: &Storage, enabled: bool) -> Result<OutgoingPacket, MinimumError> {
    packet(&Message::GuildInventoryInfo(InventoryInfo {
        is_enabled: enabled,
        storage_count: storage.slot_count,
        gold_limit: storage.gold_limit,
        access_limit_tag: storage.access_limit_tag,
        slot_infos: Some(Vec::new()),
    }))
}

/// LogMessage(isTodayLog): the brief and item logs (empty until G6f).
pub fn logs_packet(today: bool) -> Result<OutgoingPacket, MinimumError> {
    packet(&Message::GuildStorageLogs(Logs {
        is_today_log: today,
        brief_logs: Some(Vec::new()),
        item_logs: Some(Vec::new()),
    }))
}

/// OnlineGuildMember.SendOperationFailedDialog: Dialog "GameUI_Heroes_" + code.
fn failed_dialog(code: &str) -> Result<OutgoingPacket, MinimumError> {
    super::guild::change_master_message(code)
}

/// HandleGuildStorageSessionProcessor for `cid` of `guild_sn` (rank `member_rank`): nothing
/// without the manager (GuildStorage off) or for a member not online; else the listening
/// flag, and with IsStarting the inventory and both logs for a regular member.
pub async fn session(
    pool: &PgPool,
    config: StorageConfig,
    hub: &GuildHub,
    guild_sn: i32,
    cid: i64,
    member_rank: i32,
    starting: bool,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    if !config.feature {
        return Ok(Vec::new());
    }
    let storage = load(pool, guild_sn).await?;
    if !hub.set_storage_listening(guild_sn, cid, starting) || !starting {
        return Ok(Vec::new());
    }
    // SendGuildStorageInfoMessage / SendGuildStorageLogsMessage: Rank <= Member.
    if member_rank > rank::MEMBER {
        return Ok(Vec::new());
    }
    let enabled = config.is_enabled(regular_members(pool, guild_sn).await?);
    Ok(vec![
        info_packet(&storage, enabled)?,
        logs_packet(true)?,
        logs_packet(false)?,
    ])
}

/// UpdateGuildStorageSettingsProcessor: GuildStorageFail_Stopped below the required members,
/// GuildStorageFail_RankLimited for a member other than the master; the master's
/// SetGuildStorageSetting is broadcast (InventoryMessage) to every listening regular member.
/// Returns the requester's own packets.
#[allow(clippy::too_many_arguments)]
pub async fn update_setting(
    pool: &PgPool,
    config: StorageConfig,
    hub: &GuildHub,
    party: &PartyHub,
    guild_sn: i32,
    cid: i64,
    member_rank: i32,
    gold_limit: i32,
    access_limit_tag: i64,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    if !config.feature {
        return Ok(vec![failed_dialog("GuildStorageFail_Processing")?]);
    }
    load(pool, guild_sn).await?;
    let enabled = config.is_enabled(regular_members(pool, guild_sn).await?);
    if !enabled {
        return Ok(vec![failed_dialog("GuildStorageFail_Stopped")?]);
    }
    if !hub.is_online(cid) {
        return Ok(Vec::new());
    }
    if member_rank != rank::MASTER {
        return Ok(vec![failed_dialog("GuildStorageFail_RankLimited")?]);
    }
    sqlx::query(
        "UPDATE game_guild_storage_setting SET gold_limit=$2,access_limit_tag=$3 WHERE guild_id=$1",
    )
    .bind(guild_sn)
    .bind(gold_limit)
    .bind(access_limit_tag)
    .execute(pool)
    .await?;
    let storage = load(pool, guild_sn).await?;
    let info = info_packet(&storage, enabled)?;
    let mut own = Vec::new();
    for listener in hub.storage_listeners(guild_sn) {
        let listener_rank = super::guild::membership(pool, listener)
            .await?
            .filter(|(sn, _)| *sn == guild_sn)
            .map_or(rank::UNKNOWN, |(_, r)| r);
        // SendGuildStorageInfoMessage: Rank <= Member.
        if listener_rank > rank::MEMBER {
            continue;
        }
        if listener == cid {
            own.push(info.clone());
        } else {
            party.deliver(listener, vec![info.clone()]);
        }
    }
    Ok(own)
}
