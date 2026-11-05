//! Guild storage (B5 G6b): GuildStorageManager's state read from the guild's ItemOwner
//! (GuildStorageStatus.TotalStorageCount slots of one bag, GuildStorageItems) and
//! GuildStorageSetting; HandleGuildStorageSession (the member's listening flag, then
//! GuildInventoryInfoMessage and both GuildStorageLogsMessages) and
//! UpdateGuildStorageSettings (the master only, broadcast to the listening members through
//! the party hub); PurchaseGuildStorage (B5 G6e: slots by gold, by the guildstorage_ticket
//! cash product or free, then AddGuildStorageSlots and the broadcast). Deposits, picks,
//! arranging and the storage logs and ledger are G6c, G6d and G6f: until then the storage
//! holds no item and the logs are empty.
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

/// GuildStorageManager slot constants.
pub const SLOTS_FIRST_PURCHASE: i32 = 24;
pub const SLOTS_PER_PURCHASE: i32 = 24;
pub const SLOTS_MAX: i32 = 504;
/// DirectPickUpByProductNo's resulting item a cash slot purchase needs.
pub const SLOT_TICKET: &str = "guildstorage_ticket";

/// The FeatureMatrix values of PurchaseGuildStorageProcessor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PurchaseConfig {
    /// GuildStoargeSlotAllCash (sic): the first purchase is by cash too.
    pub all_cash: bool,
    /// GuildStoargeSlotFree: the cash purchases skip DirectPickUpByProductNo.
    pub free: bool,
    /// GuildStorageSlotGoldPrice: the first purchase's gold (none when 0).
    pub gold_price: i32,
}
impl PurchaseConfig {
    pub fn from_assets(assets: &Assets) -> Result<Self, MinimumError> {
        let Some(policy) = assets.npc_policy() else {
            return Ok(Self::default());
        };
        Ok(Self {
            all_cash: policy
                .is_enabled(Some("GuildStoargeSlotAllCash"))
                .map_err(|_| fail())?,
            free: policy
                .is_enabled(Some("GuildStoargeSlotFree"))
                .map_err(|_| fail())?,
            gold_price: policy
                .get_integer("GuildStorageSlotGoldPrice")
                .map_err(|_| fail())?,
        })
    }
}

/// How a slot purchase pays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Payment {
    /// DestroyItem("gold", price) from the buyer (none when the price is 0).
    Gold(i32),
    /// DirectPickUpByProductNo of ProductNo (its guildstorage_ticket's SalePrice).
    Cash(i32),
    /// GuildStoargeSlotFree.
    Free,
}

/// PurchaseGuildStorageProcessor's choice for a storage of `slot_count` slots: the payment
/// and the slots it adds, or the failure dialog.
pub fn purchase_plan(
    config: PurchaseConfig,
    slot_count: i32,
    product_no: i32,
) -> Result<(Payment, i32), &'static str> {
    if slot_count >= SLOTS_MAX {
        return Err("GuildStorageFail_Full");
    }
    if slot_count == 0 && !config.all_cash {
        return Ok((Payment::Gold(config.gold_price), SLOTS_FIRST_PURCHASE));
    }
    if config.free {
        return Ok((Payment::Free, SLOTS_PER_PURCHASE));
    }
    if product_no == -1 {
        return Err("GuildStorageFail_PurchaseFailed");
    }
    Ok((Payment::Cash(product_no), SLOTS_PER_PURCHASE))
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
    broadcast_info(pool, hub, party, guild_sn, cid, &info_packet(&storage, enabled)?).await
}

/// BroadCastInventoryInfo through the party hub: every listening regular member gets `info`;
/// returns the requester's own copy.
async fn broadcast_info(
    pool: &PgPool,
    hub: &GuildHub,
    party: &PartyHub,
    guild_sn: i32,
    cid: i64,
    info: &OutgoingPacket,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
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

/// The buyer of a slot purchase: the account key, the character and its item log context.
pub struct Buyer<'a> {
    pub owner: &'a super::character_lobby::UserKey<'a>,
    pub cid: i64,
    pub log: super::minimum_item_logs::Context,
}

/// PurchaseGuildStorageProcessor (no rank check, as the source): GuildStorageFail_Processing
/// without the manager or during another purchase, GuildStorageFail_Stopped below the
/// required members, GuildStorageFail_Full at 504 slots, GuildStorageFail_PurchaseFailed when
/// the payment fails; else AddGuildStorageSlots and the new InventoryMessage broadcast
/// (UpdateGuildInventory). The storage logs (ReportExtendSlot) are G6f. Returns the buyer's
/// own packets (inventory or cash balance, then the inventory message).
#[allow(clippy::too_many_arguments)]
pub async fn purchase(
    pool: &PgPool,
    assets: &Assets,
    config: StorageConfig,
    hub: &GuildHub,
    party: &PartyHub,
    guild_sn: i32,
    buyer: Buyer<'_>,
    product_no: i32,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    if !config.feature {
        return Ok(vec![failed_dialog("GuildStorageFail_Processing")?]);
    }
    if !hub.is_online(buyer.cid) {
        return Ok(Vec::new());
    }
    if !hub.begin_storage_processing(guild_sn) {
        return Ok(vec![failed_dialog("GuildStorageFail_Processing")?]);
    }
    let result = purchase_locked(
        pool, assets, config, hub, party, guild_sn, &buyer, product_no,
    )
    .await;
    hub.end_storage_processing(guild_sn);
    result
}

#[allow(clippy::too_many_arguments)]
async fn purchase_locked(
    pool: &PgPool,
    assets: &Assets,
    config: StorageConfig,
    hub: &GuildHub,
    party: &PartyHub,
    guild_sn: i32,
    buyer: &Buyer<'_>,
    product_no: i32,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let storage = load(pool, guild_sn).await?;
    let enabled = config.is_enabled(regular_members(pool, guild_sn).await?);
    if !enabled {
        return Ok(vec![failed_dialog("GuildStorageFail_Stopped")?]);
    }
    let plan = purchase_plan(
        PurchaseConfig::from_assets(assets)?,
        storage.slot_count,
        product_no,
    );
    let (payment, slots) = match plan {
        Ok(plan) => plan,
        Err(code) => return Ok(vec![failed_dialog(code)?]),
    };
    let purchase_failed = || Ok(vec![failed_dialog("GuildStorageFail_PurchaseFailed")?]);
    let mut own = Vec::new();
    let mut tx = pool.begin().await?;
    match payment {
        Payment::Gold(price) if price > 0 => {
            match super::minimum_shop::destroy_class(
                pool,
                buyer.owner,
                assets,
                buyer.cid,
                "gold",
                price,
                buyer.log,
            )
            .await
            {
                Ok(packets) => own.extend(packets),
                Err(_) => return purchase_failed(),
            }
        }
        Payment::Gold(_) | Payment::Free => {}
        Payment::Cash(no) => {
            let product = assets
                .cash_shop
                .as_ref()
                .and_then(|c| c.product(no))
                .filter(|p| p.element.product_id.as_deref() == Some(SLOT_TICKET));
            let Some(product) = product else {
                return purchase_failed();
            };
            // DirectPickUpByProductNo: the NISMS purchase of the product (NotEnoughCash fails).
            let debited: Option<i32> = sqlx::query_scalar(
                "UPDATE game_cash_balances SET balance=balance-$2 WHERE uid=$1 AND balance>=$2 RETURNING balance",
            )
            .bind(buyer.owner.uid)
            .bind(product.element.sale_price)
            .fetch_optional(&mut *tx)
            .await?;
            if debited.is_none() {
                return purchase_failed();
            }
        }
    }
    // AddGuildStorageSlots: NonCharacterDBLoader.ExtendStorage.
    let slot_count: i32 = sqlx::query_scalar(
        "INSERT INTO game_guild_storage_status(guild_id,total_storage_count) VALUES($1,$2) \
         ON CONFLICT(guild_id) DO UPDATE SET total_storage_count = \
         game_guild_storage_status.total_storage_count + EXCLUDED.total_storage_count \
         RETURNING total_storage_count",
    )
    .bind(guild_sn)
    .bind(slots)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    if matches!(payment, Payment::Cash(_)) {
        own.push(super::minimum_cash_shop::balance(pool, buyer.owner.uid).await?);
    }
    let storage = Storage {
        slot_count,
        ..storage
    };
    let info = info_packet(&storage, enabled)?;
    own.extend(broadcast_info(pool, hub, party, guild_sn, buyer.cid, &info).await?);
    Ok(own)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn purchase_plan_follows_the_processor() {
        let eu = PurchaseConfig {
            all_cash: true,
            free: false,
            gold_price: 30_000_000,
        };
        assert_eq!(purchase_plan(eu, 0, 7), Ok((Payment::Cash(7), 24)));
        assert_eq!(
            purchase_plan(eu, 0, -1),
            Err("GuildStorageFail_PurchaseFailed")
        );
        let gold = PurchaseConfig {
            all_cash: false,
            ..eu
        };
        assert_eq!(
            purchase_plan(gold, 0, -1),
            Ok((Payment::Gold(30_000_000), 24))
        );
        assert_eq!(purchase_plan(gold, 24, 7), Ok((Payment::Cash(7), 24)));
        let free = PurchaseConfig { free: true, ..eu };
        assert_eq!(purchase_plan(free, 480, -1), Ok((Payment::Free, 24)));
        assert_eq!(purchase_plan(free, 504, -1), Err("GuildStorageFail_Full"));
    }
}
