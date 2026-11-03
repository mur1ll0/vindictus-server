//! Mail rules of the PlayerService ItemServiceCore: ItemInstance.Mail (visibility, expiry,
//! deletion), MailBox.ProcessMailList/SendMail, MailSystem.MailFactory (times, titles,
//! senders) and the MailTypeInfo content row. Times are DateTime ticks (UTC); the stored
//! values are SQL datetime, so callers round through SqlDateTime before comparing.
use serde::Deserialize;

/// ServiceCore MailType values used by the server.
pub mod mail_type {
    pub const TEXT: u8 = 1;
    pub const ITEM: u8 = 2;
    pub const ITEM_CHARGE: u8 = 3;
    /// MailType.ChargePayMail: the charged gold paid to the sender.
    pub const CHARGE_PAY: u8 = 4;
    /// MailType.ReturnedMail.
    pub const RETURNED: u8 = 5;
    /// MailType.AuctionItemMail: the marketplace's ItemGenerator.SendMail.
    pub const AUCTION_ITEM: u8 = 7;
    /// MailType.DungeonItemMail: SendGiveItemFailMail and the other system mails.
    pub const DUNGEON_ITEM: u8 = 9;
}
/// MailBox.ProcessMailList: at most 36 received and 36 sent BriefMailInfo.
pub const LISTED: usize = 36;
/// QueryMailList: TOP 300 of each branch.
pub const QUERY_TOP: i64 = 300;
/// ItemOwner: new TimeLine(Service.Thread, this, 30000).
pub const TIMELINE_INTERVAL_MS: u64 = 30_000;
/// MailFactory: Title and Content are cut to 128 and 512 UTF-16 units.
pub const TITLE_UNITS: usize = 128;
pub const CONTENT_UNITS: usize = 512;
/// MailSentMessage.ErrorCodeEnum.
pub mod sent {
    pub const RETURN_SUCCESSFUL: i32 = -2;
    pub const SUCCESSFUL: i32 = -1;
    /// TransFerError_<TransferToSystem.FailReasonEnum> are the reason values 0..=7.
    pub const TRANSFER_FEE_PAY_ERROR: i32 = 6;
    pub const CID_QUERY_FAILED: i32 = 8;
    pub const INVALID_MAIL_TYPE: i32 = 9;
    pub const SEND_FAILED: i32 = 10;
    pub const SELF_CID_FAILED: i32 = 12;
}
/// TransferToSystem.FailReasonEnum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransferToSystemFail {
    Unknown = 0,
    NoInventoryInfo = 1,
    NoItemInfo = 2,
    ImproperTradeLevel = 3,
    EquippedItem = 4,
    TrasferError = 5,
    FeePayError = 6,
    DbSubmitError = 7,
}
/// GetMailItemCompletedMessage.ResultEnum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GetMailItemResult {
    Success = 0,
    NoItem = 1,
    NoEmptySlot = 2,
    UniqueViolation = 3,
    FeePayError = 4,
    GetItemNotAllowed = 5,
    GiveItemFail = 6,
    ItemTypeMixed = 7,
    QueryMailFail = 8,
    Unknown = 9,
}
/// BriefMailInfo.ItemAttachStatus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttachStatus {
    None = 0,
    HasItem = 1,
    Received = 2,
}
impl AttachStatus {
    /// Mail.ItemAttachStatus: decided by the first MailItem only.
    pub fn of(first_received: Option<bool>) -> Self {
        match first_received {
            None => Self::None,
            Some(true) => Self::Received,
            Some(false) => Self::HasItem,
        }
    }
}

/// One MailTypeInfo row of the content database (bit columns as booleans).
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MailTypeInfo {
    pub mail_type: i32,
    pub description: Option<String>,
    pub sender: Option<String>,
    pub system: bool,
    pub item_attached: bool,
    pub charged: bool,
    pub replyable: bool,
    pub returnable: bool,
    pub expire_in: i32,
    pub arrive_in: i32,
    pub fee: i32,
    pub fee_for_item: i32,
    pub fee_express: i32,
    pub fee_for_item_express: i32,
}
impl MailTypeInfo {
    /// MailTypeInfo.IsCorrectUserMailType: only the three user types are accepted.
    pub fn is_correct_user_mail_type(&self, item_attached: bool, charged: bool) -> bool {
        match self.mail_type {
            1 => !item_attached && !charged,
            2 => item_attached && !charged,
            3 => item_attached && charged,
            _ => false,
        }
    }
    /// MailFactory.MakeSystemMail: FromName of a system mail (None refuses the mail).
    pub fn sender_name(&self) -> Option<String> {
        self.sender
            .as_ref()
            .map(|s| format!("Heroes_GameUI_MailSender_{s}"))
    }
    /// MailBox.SendMail: (Fee or Fee_Express, FeeForItem or FeeForItem_Express).
    pub fn fees(&self, express: bool) -> (i32, i32) {
        if express {
            (self.fee_express, self.fee_for_item_express)
        } else {
            (self.fee, self.fee_for_item)
        }
    }
}

/// MailBox.SendMail fee: `num += (int)((float)Price * ((float)rate / 100f) * (float)Count)`
/// for every transferred item (attached gold included), in single precision as the x64 JIT
/// evaluates it; an out-of-range conversion yields int.MinValue (cvttss2si) and the Int32
/// sum wraps.
pub fn send_fee(fee: i32, rate: i32, items: impl IntoIterator<Item = (i32, i32)>) -> i32 {
    let mut num = fee;
    for (price, count) in items {
        let value = price as f32 * (rate as f32 / 100f32) * count as f32;
        let part = if value.is_nan() || !(-2_147_483_648f32..2_147_483_648f32).contains(&value) {
            i32::MIN
        } else {
            value as i32
        };
        num = num.wrapping_add(part);
    }
    num
}

/// String.Substring(0, max) on UTF-16 units (a surrogate pair may be cut, as the source).
pub fn truncate_units(text: &str, max: usize) -> Vec<u16> {
    text.encode_utf16().take(max).collect()
}

const MINUTE_TICKS: i64 = 600_000_000;
const DAY_TICKS: i64 = 864_000_000_000;
const UTILITY_EPOCH_TICKS: i64 = 621_355_968_000_000_000;

/// PostedTime, ArriveTime and ExpireTime of one mail, in DateTime ticks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Times {
    pub posted: i64,
    pub arrive: i64,
    pub expire: Option<i64>,
}
impl Times {
    /// MailFactory.MakeMail: Arrive_in minutes unless Express; ExpireTime is the forced
    /// time, else UtcNow + arrive + Expire_in minutes when Expire_in > 0.
    pub fn make(now: i64, info: &MailTypeInfo, express: bool, force_expire: Option<i64>) -> Self {
        let arrive_in = if express { 0 } else { info.arrive_in };
        let arrive = now + i64::from(arrive_in) * MINUTE_TICKS;
        let expire = force_expire.or_else(|| {
            (info.expire_in > 0).then(|| arrive + i64::from(info.expire_in) * MINUTE_TICKS)
        });
        Self {
            posted: now,
            arrive,
            expire,
        }
    }
    /// Mail.IsArrived.
    pub fn is_arrived(&self, now: i64) -> bool {
        self.arrive <= now
    }
    /// Mail.IsExpired.
    pub fn is_expired(&self, now: i64) -> bool {
        self.expire.is_some_and(|e| e <= now)
    }
    /// Mail.RequiresDelete: posted more than 30 days ago with an expiry.
    pub fn requires_delete(&self, now: i64) -> bool {
        self.posted + 30 * DAY_TICKS < now && self.expire.is_some()
    }
    /// Mail.IsVisibleByReceiver.
    pub fn visible_by_receiver(&self, now: i64, deleted: bool) -> bool {
        self.is_arrived(now) && !self.is_expired(now) && !deleted && !self.requires_delete(now)
    }
    /// Mail.IsVisibleBySender.
    pub fn visible_by_sender(&self, now: i64) -> bool {
        !self.requires_delete(now)
    }
}

/// Utility.Extention.ToInt32: signed seconds since 1970 truncated toward zero, then the
/// unchecked Int32 conversion.
pub fn to_int32(ticks: i64) -> i32 {
    ((ticks - UTILITY_EPOCH_TICKS) / 10_000_000) as i32
}
/// BriefMailInfo.ExpireDate: ExpireDate.ToInt32() − UtcNow.ToInt32(), or −1 without one.
pub fn expire_date(expire: Option<i64>, now: i64) -> i32 {
    match expire {
        None => -1,
        Some(e) => to_int32(e).wrapping_sub(to_int32(now)),
    }
}

/// What ProcessMailList reads of one queried mail.
#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub from_cid: i64,
    pub to_cid: i64,
    pub times: Times,
    pub is_read: bool,
    pub is_deleted: bool,
    /// MailTypeInfo.Returnable of its type (None: unknown type).
    pub returnable_type: Option<bool>,
    /// Received of the first MailItem (None without items).
    pub first_received: Option<bool>,
}
impl Entry {
    /// Mail.IsVisibleBy.
    pub fn visible_by(&self, cid: i64, now: i64) -> bool {
        if self.from_cid == cid {
            self.times.visible_by_sender(now)
        } else if self.to_cid == cid {
            self.times.visible_by_receiver(now, self.is_deleted)
        } else {
            false
        }
    }
    /// Mail.isReturnable: the first attachment is not received.
    pub fn attachments_pending(&self) -> bool {
        self.first_received != Some(true)
    }
    /// Mail.IsReturnableWithReloadDB: a Returnable type whose reloaded first item is not
    /// received; ReloadMailItem fails for a mail without items. An unknown type throws in
    /// the source (GetMailTypeInfo is null).
    pub fn returnable(&self) -> Result<bool, MailRuleError> {
        let returnable = self.returnable_type.ok_or(MailRuleError::UnknownType)?;
        Ok(returnable && self.first_received == Some(false))
    }
}
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum MailRuleError {
    #[error("mail of a type without MailTypeInfo")]
    UnknownType,
}
/// MailBox.ProcessMailList over the QueryMailList rows (ArriveTime descending).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Listing {
    pub delete: Vec<usize>,
    pub returned: Vec<usize>,
    pub sent: Vec<usize>,
    pub received: Vec<usize>,
    pub new_mail_count: i32,
}
pub fn process_list(owner: i64, entries: &[Entry], now: i64) -> Result<Listing, MailRuleError> {
    let mut out = Listing::default();
    for (i, e) in entries.iter().enumerate() {
        if e.times.requires_delete(now) {
            out.delete.push(i);
            continue;
        }
        if e.times.is_expired(now) && e.returnable()? {
            out.returned.push(i);
            continue;
        }
        if e.from_cid == owner && e.times.visible_by_sender(now) && out.sent.len() < LISTED {
            out.sent.push(i);
        }
        if e.to_cid == owner && e.times.visible_by_receiver(now, e.is_deleted) {
            if !e.is_read && e.first_received != Some(true) {
                out.new_mail_count += 1;
            }
            if out.received.len() < LISTED {
                out.received.push(i);
            }
        }
    }
    Ok(out)
}
/// MailBox.ScheduleMail: the times at which PushMailList runs for this owner (a past time
/// runs at once). `just_sent` is NoticeMail's scheduling of a new mail.
pub fn schedule(owner: i64, e: &Entry, just_sent: bool, now: i64) -> Vec<i64> {
    let mut at = Vec::new();
    if e.to_cid == owner && (just_sent || !e.times.is_arrived(now)) {
        at.push(e.times.arrive);
    }
    if (e.to_cid == owner || e.from_cid == owner) && !e.times.is_expired(now) {
        at.extend(e.times.expire);
    }
    at
}

/// MailFactory titles and contents (HeroesString keys with comma arguments).
pub fn give_fail_title(class: &str, count: i32) -> String {
    format!("#GameUI_Heroes_GiveFailMail_Title,{class},{count}")
}
pub fn give_fail_content(class: &str, count: i32) -> String {
    format!("#GameUI_Heroes_GiveFailMail_Content,{class},{count}")
}
pub fn return_title(title: &str) -> String {
    format!("GameUI_Heroes_Server_MailFactory_MakeReturnMail_Title,{title}")
}
pub fn return_content(content: &str) -> String {
    format!("GameUI_Heroes_Server_MailFactory_MakeReturnMail_Content,{content}")
}
pub fn charge_pay_title(title: &str) -> String {
    format!("GameUI_Heroes_Server_MailFactory_MakeChargePayMail_Title,{title}")
}
pub fn charge_pay_content(content: &str) -> String {
    format!("GameUI_Heroes_Server_MailFactory_MakeChargePayMail_Content,{content}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(mail_type: i32, arrive_in: i32, expire_in: i32, returnable: bool) -> MailTypeInfo {
        MailTypeInfo {
            mail_type,
            description: None,
            sender: (mail_type == 9).then(|| "SYSTEM".to_owned()),
            system: mail_type > 3,
            item_attached: mail_type != 1,
            charged: mail_type == 3,
            replyable: mail_type <= 3,
            returnable,
            expire_in,
            arrive_in,
            fee: 100,
            fee_for_item: 2,
            fee_express: 200,
            fee_for_item_express: 5,
        }
    }
    const NOW: i64 = 639_000_000_000_000_000;

    #[test]
    fn user_mail_types_follow_attachment_and_charge() {
        assert!(info(1, 60, 10080, false).is_correct_user_mail_type(false, false));
        assert!(!info(1, 60, 10080, false).is_correct_user_mail_type(true, false));
        assert!(info(2, 60, 10080, true).is_correct_user_mail_type(true, false));
        assert!(!info(2, 60, 10080, true).is_correct_user_mail_type(true, true));
        assert!(info(3, 60, 180, true).is_correct_user_mail_type(true, true));
        assert!(!info(9, 0, 10080, false).is_correct_user_mail_type(true, false));
        assert_eq!(
            info(9, 0, 10080, false).sender_name().as_deref(),
            Some("Heroes_GameUI_MailSender_SYSTEM")
        );
        assert_eq!(info(2, 60, 10080, true).sender_name(), None);
    }
    #[test]
    fn fee_is_single_precision_and_truncated() {
        // 2% of 1234 gold (Price 1): 24.68 → 24; a 333-price item ×3 at 5%: 49.95 → 49.
        assert_eq!(send_fee(100, 2, [(1, 1234)]), 124);
        assert_eq!(send_fee(200, 5, [(333, 3), (1, 10)]), 200 + 49);
        assert_eq!(send_fee(100, 0, [(5000, 7)]), 100);
        assert_eq!(send_fee(0, 100, [(i32::MAX, 2)]), i32::MIN);
    }
    #[test]
    fn times_follow_mail_factory() {
        let t = Times::make(NOW, &info(2, 60, 10080, true), false, None);
        assert_eq!(t.arrive, NOW + 60 * MINUTE_TICKS);
        assert_eq!(t.expire, Some(NOW + (60 + 10080) * MINUTE_TICKS));
        let express = Times::make(NOW, &info(2, 60, 10080, true), true, None);
        assert_eq!(express.arrive, NOW);
        assert_eq!(
            Times::make(NOW, &info(11, 0, -1, false), true, None).expire,
            None
        );
        assert_eq!(
            Times::make(NOW, &info(9, 0, 10080, false), true, Some(5)).expire,
            Some(5)
        );
        assert!(!t.is_arrived(NOW));
        assert!(t.is_arrived(t.arrive));
        assert!(!t.visible_by_receiver(NOW, false));
        assert!(t.visible_by_sender(NOW));
        assert!(t.is_expired(t.expire.unwrap_or_default()));
        assert!(!t.requires_delete(NOW + 30 * DAY_TICKS));
        assert!(t.requires_delete(NOW + 30 * DAY_TICKS + 1));
        let gm = Times::make(NOW, &info(11, 0, -1, false), true, None);
        assert!(!gm.requires_delete(NOW + 400 * DAY_TICKS));
    }
    #[test]
    fn titles_are_cut_on_utf16_units() {
        assert_eq!(truncate_units(&"a".repeat(200), TITLE_UNITS).len(), 128);
        assert_eq!(truncate_units("ペット", 2).len(), 2);
        assert_eq!(truncate_units("\u{1F600}x", 1), vec![0xd83d]);
    }
    #[test]
    fn expire_date_is_whole_second_difference() {
        assert_eq!(expire_date(None, NOW), -1);
        assert_eq!(expire_date(Some(NOW + 10_000_000 * 90 + 5), NOW), 90);
        assert_eq!(to_int32(UTILITY_EPOCH_TICKS - 5_000_000), 0);
    }
    fn entry(from: i64, to: i64, times: Times, returnable: bool, first: Option<bool>) -> Entry {
        Entry {
            from_cid: from,
            to_cid: to,
            times,
            is_read: false,
            is_deleted: false,
            returnable_type: Some(returnable),
            first_received: first,
        }
    }
    #[test]
    fn process_list_deletes_returns_and_counts() {
        let item = info(2, 60, 10080, true);
        let fresh = Times::make(NOW - 120 * MINUTE_TICKS, &item, false, None);
        let expired = Times {
            posted: NOW - 8 * DAY_TICKS,
            arrive: NOW - 8 * DAY_TICKS,
            expire: Some(NOW - DAY_TICKS),
        };
        let old = Times {
            posted: NOW - 31 * DAY_TICKS,
            arrive: NOW - 31 * DAY_TICKS,
            expire: Some(NOW - 20 * DAY_TICKS),
        };
        let pending = Times::make(NOW, &item, false, None);
        let entries = [
            entry(2, 1, fresh, true, Some(false)),
            entry(2, 1, expired, true, Some(false)),
            entry(2, 1, expired, false, Some(false)),
            entry(1, 3, old, true, Some(false)),
            entry(1, 3, fresh, true, Some(true)),
            entry(2, 1, pending, true, None),
            entry(2, 1, fresh, false, Some(true)),
        ];
        let l = process_list(1, &entries, NOW).unwrap();
        assert_eq!(l.delete, vec![3]);
        assert_eq!(l.returned, vec![1]);
        assert_eq!(l.sent, vec![4]);
        assert_eq!(l.received, vec![0, 6]);
        // The received attachment does not count as new mail.
        assert_eq!(l.new_mail_count, 1);
        let mut unknown = entries[1];
        unknown.returnable_type = None;
        assert_eq!(
            process_list(1, &[unknown], NOW),
            Err(MailRuleError::UnknownType)
        );
    }
    #[test]
    fn schedule_follows_mail_box() {
        let item = info(2, 60, 10080, true);
        let t = Times::make(NOW, &item, false, None);
        let e = entry(2, 1, t, true, Some(false));
        assert_eq!(
            schedule(1, &e, false, NOW),
            vec![t.arrive, t.expire.unwrap_or_default()]
        );
        assert_eq!(
            schedule(2, &e, false, NOW),
            vec![t.expire.unwrap_or_default()]
        );
        assert_eq!(
            schedule(1, &e, false, t.arrive),
            vec![t.expire.unwrap_or_default()]
        );
        assert_eq!(schedule(1, &e, true, t.arrive).len(), 2);
        assert!(schedule(3, &e, true, NOW).is_empty());
    }
}
