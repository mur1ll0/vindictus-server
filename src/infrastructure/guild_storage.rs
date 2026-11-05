//! Guild storage (B5 G6b): GuildStorageManager's state read from the guild's ItemOwner
//! (GuildStorageStatus.TotalStorageCount slots of one bag, GuildStorageItems) and
//! GuildStorageSetting; HandleGuildStorageSession (the member's listening flag, then
//! GuildInventoryInfoMessage and both GuildStorageLogsMessages) and
//! UpdateGuildStorageSettings (the master only, broadcast to the listening members through
//! the party hub); PurchaseGuildStorage (B5 G6e: slots by gold, by the guildstorage_ticket
//! cash product or free, then AddGuildStorageSlots and the broadcast); AddGuildStorageItem,
//! PickGuildStorageItem (G6c: TransferToSystem and TransferFromSystem between the member and
//! the guild in one transaction, gold included) and ArrangeGuildStorageItem (G6d). A guild
//! item is an ownerless game_items row linked by game_guild_storage_items (bag collection 0,
//! gold at the AccountLocation). The storage logs and ledger are G6f: the logs stay empty.
use super::character_lobby::UserKey;
use super::guild::GuildHub;
use super::minimum_assets::Assets;
use super::minimum_character::MinimumError;
use super::minimum_inventory::{
    self as inventory, ACCOUNT_COLLECTION, GUILD_COLLECTION, GuildItem,
};
use super::minimum_mail;
use super::party_hub::PartyHub;
use crate::domain::guild::user_type;
use crate::domain::map_partition::OutgoingPacket;
use crate::protocol::DEFAULT_MAX_BODY;
use crate::protocol::guild::rank;
use crate::protocol::guild_storage::{self as wire, InventoryInfo, ItemMove, Logs, Message};
use crate::protocol::inventory::Slot;
use sqlx::{PgPool, Postgres, Transaction};

fn fail() -> MinimumError {
    MinimumError::Conflict
}

/// InitGuildStorageSetting's AccessLimitTag.
pub const INITIAL_ACCESS_LIMIT_TAG: i64 = 898_779;

/// GuildStorageManager after QuildGuildStorageStatus and GetGuildStorageSetting.
#[derive(Clone)]
pub struct Storage {
    /// StorageCount (GuildStorage.SlotCount).
    pub slot_count: i32,
    /// GoldPickLimit.
    pub gold_limit: i32,
    pub access_limit_tag: i64,
    /// SlotInfo (ApplyStorage).
    pub slots: Vec<Option<Slot>>,
    /// GoldHolding: the gold item's Num.
    pub gold_holding: i32,
}

fn packet(message: &Message) -> Result<OutgoingPacket, MinimumError> {
    let (category, body) = wire::packet(message, DEFAULT_MAX_BODY).map_err(|_| fail())?;
    Ok(OutgoingPacket { category, body })
}

/// The manager's load: the guild ItemOwner's items (ApplyStorage), the slots of
/// GuildStorageStatus (0 without a row) and the setting; a guild without a setting row gets
/// InitGuildStorageSetting while this load keeps 0/0, as the manager's fields stay unset
/// that time.
pub async fn load(pool: &PgPool, assets: &Assets, guild_sn: i32) -> Result<Storage, MinimumError> {
    let mut tx = pool.begin().await?;
    let rows = inventory::guild_items(&mut tx, guild_sn, false).await?;
    let slots = inventory::guild_slots(&mut tx, assets, &rows).await?;
    tx.commit().await?;
    let gold_holding = rows
        .iter()
        .filter(|r| assets.inventory.is_gold(&r.item_class_utf16))
        .map(|r| r.count)
        .last()
        .unwrap_or(0);
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
        slots,
        gold_holding,
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
        slot_infos: Some(storage.slots.clone()),
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
#[allow(clippy::too_many_arguments)]
pub async fn session(
    pool: &PgPool,
    assets: &Assets,
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
    let storage = load(pool, assets, guild_sn).await?;
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
    assets: &Assets,
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
    load(pool, assets, guild_sn).await?;
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
    let storage = load(pool, assets, guild_sn).await?;
    broadcast_info(
        pool,
        hub,
        party,
        guild_sn,
        cid,
        &info_packet(&storage, enabled)?,
    )
    .await
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
    let storage = load(pool, assets, guild_sn).await?;
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

/// GuildStorageManager.GuildStorageSlotsPerTab.
pub const SLOTS_PER_TAB: i32 = 72;
/// TargetTab of "any tab".
const ANY_TAB: u8 = u8::MAX;

/// The rank limit of a tab in AccessLimitTag (GuildStorageSettingFlagBitsPerTab 3, mask 7);
/// the shift is the Int64 one (count masked to 6 bits), as the source.
pub fn tab_rank_limit(access_limit_tag: i64, tab: i32) -> i32 {
    let shift = (tab.wrapping_mul(3) & 63) as u32;
    ((access_limit_tag >> shift) & 7) as i32
}

/// The FeatureMatrix gold limits: GuildStorageGoldHoldingLimit (0 is Int32.MaxValue) and
/// GuildStorageGoldDailyLimit (0 is none).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GoldLimits {
    pub holding: i32,
    pub daily: i32,
}
impl GoldLimits {
    pub fn from_assets(assets: &Assets) -> Result<Self, MinimumError> {
        let Some(policy) = assets.npc_policy() else {
            return Ok(Self::default());
        };
        Ok(Self {
            holding: policy
                .get_integer("GuildStorageGoldHoldingLimit")
                .map_err(|_| fail())?,
            daily: policy
                .get_integer("GuildStorageGoldDailyLimit")
                .map_err(|_| fail())?,
        })
    }
}

/// GuildStorageManager.IsAddLimited: the failure dialog, if any.
pub fn add_limited(
    rank: i32,
    gold: bool,
    amount: i32,
    target_tab: u8,
    storage: &Storage,
    limits: GoldLimits,
) -> Option<&'static str> {
    if rank > rank::MEMBER {
        return Some("GuildStorageFail_RankLimited");
    }
    if gold {
        let holding = if limits.holding == 0 {
            i32::MAX
        } else {
            limits.holding
        };
        return (i64::from(storage.gold_holding) > i64::from(holding) - i64::from(amount))
            .then_some("GuildStorageFail_GoldAddLimit");
    }
    let limit = tab_rank_limit(storage.access_limit_tag, i32::from(target_tab));
    (limit != 0 && rank > limit).then_some("GuildStorageFail_RankLimited")
}

/// GuildStorageManager.IsPickLimited: the failure dialog, if any. The daily gold limit
/// counts today's brief logs, which are G6f: without them only `amount` is compared.
pub fn pick_limited(
    rank: i32,
    gold: bool,
    slot: i32,
    amount: i32,
    storage: &Storage,
    limits: GoldLimits,
) -> Option<&'static str> {
    if rank > rank::MEMBER {
        return Some("GuildStorageFail_RankLimited");
    }
    if gold {
        if i64::from(storage.gold_holding) - i64::from(amount) < i64::from(storage.gold_limit) {
            return Some("GuildStorageFail_GoldPickLimitBySetting");
        }
        return (limits.daily != 0 && amount > limits.daily)
            .then_some("GuildStorageFail_GoldPickDailyLimit");
    }
    let limit = tab_rank_limit(storage.access_limit_tag, slot / SLOTS_PER_TAB);
    (limit != 0 && rank > limit).then_some("GuildStorageFail_RankLimited")
}

/// TransferFromSystem's slot range in the guild bag: any slot (TargetTab 255), the target
/// slot, or the target tab's 72 slots; within the purchased slots.
pub fn target_range(slot_count: i32, target_tab: u8, target_slot: i32) -> std::ops::Range<i32> {
    let (start, end) = if target_tab == ANY_TAB {
        (0, slot_count)
    } else if target_slot != -1 {
        (target_slot, target_slot.saturating_add(1))
    } else {
        let tab = i32::from(target_tab);
        (tab * SLOTS_PER_TAB, (tab + 1) * SLOTS_PER_TAB)
    };
    start.max(0)..end.min(slot_count)
}

/// The member acting on the storage.
pub struct Member<'a> {
    pub owner: &'a UserKey<'a>,
    pub cid: i64,
    /// GuildMember.Rank.
    pub rank: i32,
}

fn utf16(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

fn units(class: &[u8]) -> Vec<u16> {
    class
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_le_bytes(*c))
        .collect()
}

fn dialog(code: &str) -> Result<Vec<OutgoingPacket>, MinimumError> {
    Ok(vec![failed_dialog(code)?])
}

/// The common start of the item processors: no manager, a member not online (nothing) or
/// Processing; else Processing is set and must be cleared by the caller.
fn begin(
    config: StorageConfig,
    hub: &GuildHub,
    guild_sn: i32,
    cid: i64,
) -> Result<Option<Vec<OutgoingPacket>>, MinimumError> {
    if !config.feature {
        return dialog("GuildStorageFail_Processing").map(Some);
    }
    if !hub.is_online(cid) {
        return Ok(Some(Vec::new()));
    }
    if !hub.begin_storage_processing(guild_sn) {
        return dialog("GuildStorageFail_Processing").map(Some);
    }
    Ok(None)
}

async fn has_attributes(tx: &mut Transaction<'_, Postgres>, id: i64) -> Result<bool, MinimumError> {
    Ok(
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM game_item_attributes WHERE item_id=$1)")
            .bind(id)
            .fetch_one(&mut **tx)
            .await?,
    )
}

/// Sets a guild item's count (the gold checksum follows the count).
async fn set_guild_count(
    tx: &mut Transaction<'_, Postgres>,
    assets: &Assets,
    row: &GuildItem,
    count: i32,
) -> Result<(), MinimumError> {
    let checksum = if assets.inventory.is_gold(&row.item_class_utf16) {
        Some(crate::domain::initial_inventory::gold_checksum(
            row.id,
            None,
            &units(&row.item_class_utf16),
            count,
        ))
    } else {
        row.checksum
    };
    sqlx::query("UPDATE game_items SET count=$2,checksum=$3 WHERE id=$1 AND owner_id IS NULL")
        .bind(row.id)
        .bind(count)
        .bind(checksum)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Locates a system item in the guild bag and links it to the guild.
async fn link(
    tx: &mut Transaction<'_, Postgres>,
    guild_sn: i32,
    id: i64,
    collection: i16,
    slot: i32,
) -> Result<(), MinimumError> {
    sqlx::query("UPDATE game_items SET collection=$2,slot=$3 WHERE id=$1 AND owner_id IS NULL")
        .bind(id)
        .bind(collection)
        .bind(slot)
        .execute(&mut **tx)
        .await?;
    sqlx::query("INSERT INTO game_guild_storage_items(id,guild_id) VALUES($1,$2)")
        .bind(id)
        .bind(guild_sn)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// TransferFromSystem into the guild ItemOwner of one system item: gold joins the guild's
/// gold; a stackable item without attributes joins a stack of the range with room; else the
/// first empty slot of the range. False when there is no empty slot.
#[allow(clippy::too_many_arguments)]
async fn place_in_guild(
    tx: &mut Transaction<'_, Postgres>,
    assets: &Assets,
    guild_sn: i32,
    rows: &[GuildItem],
    slot_count: i32,
    item_id: i64,
    target_tab: u8,
    target_slot: i32,
) -> Result<bool, MinimumError> {
    let system: Option<GuildItem> = sqlx::query_as(
        "SELECT id,item_class_utf16,count,collection,slot,expire_ticks,tradable,checksum FROM game_items WHERE id=$1 AND owner_id IS NULL AND collection=255 AND slot=-1 FOR UPDATE",
    )
    .bind(item_id)
    .fetch_optional(&mut **tx)
    .await?;
    let system = system.ok_or_else(fail)?;
    let class = &system.item_class_utf16;
    if assets.inventory.is_gold(class) {
        if let Some(gold) = rows
            .iter()
            .find(|r| assets.inventory.is_gold(&r.item_class_utf16))
        {
            let count = gold.count.checked_add(system.count).ok_or_else(fail)?;
            set_guild_count(tx, assets, gold, count).await?;
            minimum_mail::destroy_system_items(tx, &[system.id]).await?;
        } else {
            link(tx, guild_sn, system.id, ACCOUNT_COLLECTION, -1).await?;
        }
        return Ok(true);
    }
    let range = target_range(slot_count, target_tab, target_slot);
    let info = super::minimum_give::class_info(assets, class)?.ok_or_else(fail)?;
    if !info.equippable && info.max_stack > 1 && !has_attributes(tx, system.id).await? {
        for r in rows.iter().filter(|r| {
            r.collection == GUILD_COLLECTION
                && range.contains(&r.slot)
                && r.item_class_utf16 == *class
                && r.tradable == system.tradable
                && r.expire_ticks == system.expire_ticks
                && r.count.saturating_add(system.count) <= info.max_stack
        }) {
            if has_attributes(tx, r.id).await? {
                continue;
            }
            set_guild_count(tx, assets, r, r.count + system.count).await?;
            minimum_mail::destroy_system_items(tx, &[system.id]).await?;
            return Ok(true);
        }
    }
    let used: std::collections::BTreeSet<i32> = rows
        .iter()
        .filter(|r| r.collection == GUILD_COLLECTION)
        .map(|r| r.slot)
        .collect();
    let Some(free) = range.clone().find(|s| !used.contains(s)) else {
        return Ok(false);
    };
    link(tx, guild_sn, system.id, GUILD_COLLECTION, free).await?;
    Ok(true)
}

/// UpdateGuildInventory after a change: the new InventoryMessage to the listening members;
/// returns the requester's own copy.
async fn after_change(
    pool: &PgPool,
    assets: &Assets,
    config: StorageConfig,
    hub: &GuildHub,
    party: &PartyHub,
    guild_sn: i32,
    cid: i64,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let storage = load(pool, assets, guild_sn).await?;
    let enabled = config.is_enabled(regular_members(pool, guild_sn).await?);
    broadcast_info(
        pool,
        hub,
        party,
        guild_sn,
        cid,
        &info_packet(&storage, enabled)?,
    )
    .await
}

/// AddGuildStorageItemProcessor: GuildStorageFail_Processing, _Stopped, _NoSlot; for an item
/// (ItemID != 0) of the member: _InternalError when missing, _NoCashItem (cash tab),
/// _NeedsRepair, _InvalidItem (not tradable); IsAddLimited; TransferToSystem from the member
/// (_TransferFailed) and TransferFromSystem into the guild (_NoEmptySlot, the transaction
/// rolls back: the source's recovery). Returns the member's inventory update and the new
/// inventory message (also broadcast).
#[allow(clippy::too_many_arguments)]
pub async fn add_item(
    pool: &PgPool,
    assets: &Assets,
    config: StorageConfig,
    hub: &GuildHub,
    party: &PartyHub,
    guild_sn: i32,
    member: Member<'_>,
    request: &ItemMove,
    ids: Vec<i64>,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    if let Some(early) = begin(config, hub, guild_sn, member.cid)? {
        return Ok(early);
    }
    let result = add_locked(pool, assets, config, guild_sn, &member, request, ids).await;
    hub.end_storage_processing(guild_sn);
    let mut own = result?;
    if own
        .last()
        .is_some_and(|p| p.category == inventory_category())
    {
        own.extend(after_change(pool, assets, config, hub, party, guild_sn, member.cid).await?);
    }
    Ok(own)
}

fn inventory_category() -> i32 {
    crate::protocol::inventory::Kind::UpdateInventoryInfo.category()
}

async fn add_locked(
    pool: &PgPool,
    assets: &Assets,
    config: StorageConfig,
    guild_sn: i32,
    member: &Member<'_>,
    request: &ItemMove,
    ids: Vec<i64>,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let storage = load(pool, assets, guild_sn).await?;
    if !config.is_enabled(regular_members(pool, guild_sn).await?) {
        return dialog("GuildStorageFail_Stopped");
    }
    if storage.slot_count == 0 {
        return dialog("GuildStorageFail_NoSlot");
    }
    let limits = GoldLimits::from_assets(assets)?;
    let mut tx =
        super::fiona_character_state::owned_first_fiona(pool, member.owner, member.cid).await?;
    let mut class = utf16(request.item_class.as_deref().unwrap_or_default());
    if request.item_id != 0 {
        // QueryItemInfoByItemID: Auctionable, the cash tab and the durability.
        let row: Option<(Vec<u8>, i16, bool)> = sqlx::query_as(
            "SELECT item_class_utf16,collection,tradable FROM game_items WHERE id=$1 AND owner_id=$2",
        )
        .bind(request.item_id)
        .bind(member.cid)
        .fetch_optional(&mut *tx)
        .await?;
        let Some((row_class, collection, tradable)) = row else {
            return dialog("GuildStorageFail_InternalError");
        };
        let repaired = minimum_mail::equip_row(&mut tx, request.item_id)
            .await?
            .is_none_or(|e| e.reduced_durability == 0);
        if collection == 100 {
            return dialog("GuildStorageFail_NoCashItem");
        }
        if !repaired {
            return dialog("GuildStorageFail_NeedsRepair");
        }
        if !tradable {
            return dialog("GuildStorageFail_InvalidItem");
        }
        class = row_class;
    }
    let gold = assets.inventory.is_gold(&class);
    if let Some(code) = add_limited(
        member.rank,
        gold,
        request.amount,
        request.target_tab,
        &storage,
        limits,
    ) {
        return dialog(code);
    }
    let now = minimum_mail::now_ticks()?;
    let mut changed = minimum_mail::Changed::default();
    let mut ids = ids.into_iter();
    let transferred = match minimum_mail::transfer_to_system(
        &mut tx,
        assets,
        member.cid,
        None,
        &[(request.item_id, class, request.amount)],
        &mut ids,
        now,
        &mut changed,
    )
    .await?
    {
        Ok(t) => t,
        Err(_) => return dialog("GuildStorageFail_TransferFailed"),
    };
    let rows = inventory::guild_items(&mut tx, guild_sn, true).await?;
    for t in &transferred {
        if !place_in_guild(
            &mut tx,
            assets,
            guild_sn,
            &rows,
            storage.slot_count,
            t.item_id,
            request.target_tab,
            request.target_slot,
        )
        .await?
        {
            return dialog("GuildStorageFail_NoEmptySlot");
        }
    }
    let slots = changed
        .slots(&mut tx, member.owner, assets, member.cid)
        .await?;
    tx.commit().await?;
    Ok(vec![minimum_mail::inventory_update(slots)?])
}

/// PickGuildStorageItemProcessor: GuildStorageFail_Processing, _NoSlot; the guild item
/// (_InternalError when missing) or, for ItemID 0, the guild's gold; IsPickLimited;
/// TransferToSystem from the guild (_TransferFailed beyond the stack) and TransferFromSystem
/// to the member as a user trade (_NoEmptySlot or _TransferFailed roll back). The target
/// tab and slot of the member's inventory are left to the give placement.
#[allow(clippy::too_many_arguments)]
pub async fn pick_item(
    pool: &PgPool,
    assets: &Assets,
    config: StorageConfig,
    hub: &GuildHub,
    party: &PartyHub,
    guild_sn: i32,
    member: Member<'_>,
    request: &ItemMove,
    ids: Vec<i64>,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    if let Some(early) = begin(config, hub, guild_sn, member.cid)? {
        return Ok(early);
    }
    let result = pick_locked(pool, assets, guild_sn, &member, request, ids).await;
    hub.end_storage_processing(guild_sn);
    let mut own = result?;
    if own
        .last()
        .is_some_and(|p| p.category == inventory_category())
    {
        own.extend(after_change(pool, assets, config, hub, party, guild_sn, member.cid).await?);
    }
    Ok(own)
}

async fn pick_locked(
    pool: &PgPool,
    assets: &Assets,
    guild_sn: i32,
    member: &Member<'_>,
    request: &ItemMove,
    ids: Vec<i64>,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let storage = load(pool, assets, guild_sn).await?;
    if storage.slot_count == 0 {
        return dialog("GuildStorageFail_NoSlot");
    }
    let limits = GoldLimits::from_assets(assets)?;
    let mut tx =
        super::fiona_character_state::owned_first_fiona(pool, member.owner, member.cid).await?;
    let rows = inventory::guild_items(&mut tx, guild_sn, true).await?;
    let row = if request.item_id != 0 {
        let Some(row) = rows.iter().find(|r| r.id == request.item_id) else {
            return dialog("GuildStorageFail_InternalError");
        };
        let gold = assets.inventory.is_gold(&row.item_class_utf16);
        if let Some(code) = pick_limited(
            member.rank,
            gold,
            row.slot,
            request.amount,
            &storage,
            limits,
        ) {
            return dialog(code);
        }
        row
    } else {
        let gold = assets
            .inventory
            .is_gold(&utf16(request.item_class.as_deref().unwrap_or_default()));
        if let Some(code) = pick_limited(member.rank, gold, 0, request.amount, &storage, limits) {
            return dialog(code);
        }
        let Some(row) = rows
            .iter()
            .find(|r| gold && assets.inventory.is_gold(&r.item_class_utf16))
        else {
            return dialog("GuildStorageFail_TransferFailed");
        };
        row
    };
    let info = super::minimum_give::class_info(assets, &row.item_class_utf16)?.ok_or_else(fail)?;
    if request.amount <= 0 || request.amount > row.count || (info.equippable && request.amount != 1)
    {
        return dialog("GuildStorageFail_TransferFailed");
    }
    let attributes = minimum_mail::attributes_of(&mut tx, row.id).await?;
    let equipment = minimum_mail::equip_row(&mut tx, row.id).await?;
    let bound = !row.tradable
        || minimum_mail::user_trade_binds(assets, &row.item_class_utf16, &attributes, false)?;
    let give = super::minimum_give::Request {
        class: &row.item_class_utf16,
        count: request.amount,
        bound,
        equipment: equipment.as_ref().map(|e| super::minimum_give::Equipment {
            colors: [e.color1, e.color2, e.color3],
            reduced: e.reduced_durability,
            bonus: e.max_durability_bonus,
        }),
        attributes: &attributes,
    };
    let mut ids = ids.into_iter();
    let applied = super::minimum_give::apply_expiring(
        &mut tx,
        member.owner,
        assets,
        member.cid,
        &[give],
        &[row.expire_ticks],
        false,
        &mut || ids.next().ok_or_else(fail),
    )
    .await?;
    let applied = match applied {
        Ok(a) => a,
        Err(crate::domain::give_item::GiveError::NoEmptySlot) => {
            return dialog("GuildStorageFail_NoEmptySlot");
        }
        Err(_) => return dialog("GuildStorageFail_TransferFailed"),
    };
    if request.amount == row.count {
        sqlx::query("DELETE FROM game_guild_storage_items WHERE id=$1")
            .bind(row.id)
            .execute(&mut *tx)
            .await?;
        minimum_mail::destroy_system_items(&mut tx, &[row.id]).await?;
    } else {
        set_guild_count(&mut tx, assets, row, row.count - request.amount).await?;
    }
    tx.commit().await?;
    Ok(vec![minimum_mail::inventory_update(applied.slots)?])
}

/// ArrangeGuildStorageItemProcessor: GuildStorageFail_Processing, _Stopped; the guild item
/// (_InternalError when missing, _InvalidItem for gold); IsPickLimited at both slots;
/// MoveInventoryItem(ItemID, 0, Slot) in the guild bag (an occupied target swaps;
/// _TransferFailed outside the purchased slots). Returns the new inventory message.
#[allow(clippy::too_many_arguments)]
pub async fn arrange_item(
    pool: &PgPool,
    assets: &Assets,
    config: StorageConfig,
    hub: &GuildHub,
    party: &PartyHub,
    guild_sn: i32,
    cid: i64,
    member_rank: i32,
    item_id: i64,
    slot: i32,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    if let Some(early) = begin(config, hub, guild_sn, cid)? {
        return Ok(early);
    }
    let result = arrange_locked(pool, assets, config, guild_sn, member_rank, item_id, slot).await;
    hub.end_storage_processing(guild_sn);
    match result? {
        Some(own) => Ok(own),
        None => after_change(pool, assets, config, hub, party, guild_sn, cid).await,
    }
}

async fn arrange_locked(
    pool: &PgPool,
    assets: &Assets,
    config: StorageConfig,
    guild_sn: i32,
    member_rank: i32,
    item_id: i64,
    slot: i32,
) -> Result<Option<Vec<OutgoingPacket>>, MinimumError> {
    let storage = load(pool, assets, guild_sn).await?;
    if !config.is_enabled(regular_members(pool, guild_sn).await?) {
        return dialog("GuildStorageFail_Stopped").map(Some);
    }
    let limits = GoldLimits::from_assets(assets)?;
    let mut tx = pool.begin().await?;
    let rows = inventory::guild_items(&mut tx, guild_sn, true).await?;
    let Some(row) = rows.iter().find(|r| r.id == item_id) else {
        return dialog("GuildStorageFail_InternalError").map(Some);
    };
    if assets.inventory.is_gold(&row.item_class_utf16) {
        return dialog("GuildStorageFail_InvalidItem").map(Some);
    }
    if let Some(code) = pick_limited(member_rank, false, row.slot, 1, &storage, limits)
        .or_else(|| pick_limited(member_rank, false, slot, 1, &storage, limits))
    {
        return dialog(code).map(Some);
    }
    if !(0..storage.slot_count).contains(&slot) {
        return dialog("GuildStorageFail_TransferFailed").map(Some);
    }
    if let Some(other) = rows
        .iter()
        .find(|r| r.collection == GUILD_COLLECTION && r.slot == slot && r.id != row.id)
    {
        sqlx::query("UPDATE game_items SET slot=$2 WHERE id=$1 AND owner_id IS NULL")
            .bind(other.id)
            .bind(row.slot)
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query("UPDATE game_items SET slot=$2 WHERE id=$1 AND owner_id IS NULL")
        .bind(row.id)
        .bind(slot)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(None)
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

    fn storage(gold: i32, gold_limit: i32, access_limit_tag: i64) -> Storage {
        Storage {
            slot_count: 144,
            gold_limit,
            access_limit_tag,
            slots: Vec::new(),
            gold_holding: gold,
        }
    }

    #[test]
    fn limits_follow_the_manager() {
        let limits = GoldLimits {
            holding: 10_000_000,
            daily: 2_500_000,
        };
        // InitGuildStorageSetting 898779: tab 0 limit 3 (member), tab 1 limit 3.
        assert_eq!(tab_rank_limit(INITIAL_ACCESS_LIMIT_TAG, 0), 3);
        assert_eq!(tab_rank_limit(INITIAL_ACCESS_LIMIT_TAG, 1), 3);
        let s = storage(9_000_000, 0, 0b010_001);
        assert_eq!(
            add_limited(4, false, 1, 0, &s, limits),
            Some("GuildStorageFail_RankLimited")
        );
        assert_eq!(add_limited(3, true, 1_000_000, 255, &s, limits), None);
        assert_eq!(
            add_limited(3, true, 1_000_001, 255, &s, limits),
            Some("GuildStorageFail_GoldAddLimit")
        );
        assert_eq!(
            add_limited(2, false, 1, 0, &s, limits),
            Some("GuildStorageFail_RankLimited")
        );
        assert_eq!(add_limited(1, false, 1, 0, &s, limits), None);
        assert_eq!(add_limited(2, false, 1, 1, &s, limits), None);
        assert_eq!(
            pick_limited(3, false, 72, 1, &s, limits),
            Some("GuildStorageFail_RankLimited")
        );
        let s = storage(5_000_000, 1_000_000, 0);
        assert_eq!(
            pick_limited(3, true, 0, 4_000_001, &s, limits),
            Some("GuildStorageFail_GoldPickLimitBySetting")
        );
        assert_eq!(
            pick_limited(3, true, 0, 2_500_001, &s, limits),
            Some("GuildStorageFail_GoldPickDailyLimit")
        );
        assert_eq!(pick_limited(3, true, 0, 2_500_000, &s, limits), None);
    }

    #[test]
    fn transfer_ranges_stay_in_the_purchased_slots() {
        assert_eq!(target_range(48, 255, 0), 0..48);
        assert_eq!(target_range(48, 0, 5), 5..6);
        assert_eq!(target_range(48, 0, -1), 0..48);
        assert_eq!(target_range(144, 1, -1), 72..144);
        assert!(target_range(48, 1, -1).is_empty());
        assert!(target_range(48, 0, 60).is_empty());
    }
}
