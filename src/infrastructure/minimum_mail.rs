//! Mail of the PlayerService ItemServiceCore (MailBox, MailFactory, the mail processors and
//! ItemOwner.TransferToSystem/TransferFromSystem) over game_mail and game_mail_items. An
//! attachment is a system item of game_items (owner NULL, null location) referenced by
//! MailItem.ItemID; receiving gives a copy and destroys it. Every operation runs in one
//! transaction of the owned character (the source's ItemTransactionScope, Save and Undo).
//! MailID comes from the mail table itself (the source's MakeEntityID ticks): the next
//! value above both the stored maximum and the current UTC ticks.
use super::{
    character_lobby::UserKey,
    minimum_assets::Assets,
    minimum_character::MinimumError,
    minimum_give::{self, Equipment, Request},
};
use crate::{
    domain::{
        give_item::GiveError,
        initial_inventory::gold_checksum,
        item_attribute::{self, Attributes},
        legacy_user_data::{SqlDateTime, SqlDateTime2},
        mail::{
            self, Entry, GetMailItemResult, MailTypeInfo, Times, TransferToSystemFail, mail_type,
        },
        map_partition::OutgoingPacket,
    },
    protocol::{
        DEFAULT_MAX_BODY,
        inventory::{self as inv},
        mail::{self as wire, BriefMailInfo, Kind, MailItemInfo, Message},
    },
};
use sqlx::{PgPool, Postgres, Transaction};
use std::{
    collections::BTreeSet,
    sync::{Mutex, PoisonError},
    time::Duration,
};

fn fail() -> MinimumError {
    MinimumError::Conflict
}
const TIMEOUT: Duration = Duration::from_secs(15);
/// RestoreStorage collection (ItemStorage.ItemClassDict leaves it out).
const RESTORE: i16 = 102;
/// Attachments of one mail handled by the laboratory (the client attaches one item and
/// gold); more is refused instead of allocating unbounded entity IDs.
pub const MAX_ATTACHMENTS: usize = 8;
/// Entity IDs a GetMailItem may draw: one per received copy and the charged gold.
pub const GET_ENTITY_IDS: usize = MAX_ATTACHMENTS + 2;
/// Entity IDs a SendMail may draw: one system item per attachment and the gold.
pub const SEND_ENTITY_IDS: usize = MAX_ATTACHMENTS + 1;

/// What one mail request answers and whom it wakes.
#[derive(Default)]
pub struct Outcome {
    pub packets: Vec<OutgoingPacket>,
    /// Comm.NoticeMail(ToCID, MailID) of the mails this request created for other owners.
    pub notices: Vec<(i64, i64)>,
    /// MailBox.ScheduleMail times (DateTime ticks) of this owner's new mails.
    pub schedule: Vec<i64>,
}

// --- NoticeMail of this process's own owner (ForceMail inside other operations). ---

static OWN_NOTICES: Mutex<BTreeSet<i64>> = Mutex::new(BTreeSet::new());
/// Comm.NoticeMail(own CID) from a give inside another transaction: the connection
/// pushes the mail list after its request (an express system mail arrives at once).
pub(super) fn notice(cid: i64) {
    OWN_NOTICES
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(cid);
}
/// Whether a NoticeMail of `cid` is pending (and clears it).
pub fn take_notice(cid: i64) -> bool {
    OWN_NOTICES
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(&cid)
}

/// MailSystem.TimeLine: jobs keyed by time, polled every 30 s; a past time runs at once.
#[derive(Debug, Default)]
pub struct TimeLine {
    events: BTreeSet<i64>,
}
impl TimeLine {
    /// AddTimeLine: true when the job runs at once (the time is already past).
    pub fn add(&mut self, at: i64, now: i64) -> bool {
        if at < now {
            return true;
        }
        let mut at = at;
        while !self.events.insert(at) {
            at += 1;
        }
        false
    }
    /// Polling: the number of jobs due before `now` (removed).
    pub fn poll(&mut self, now: i64) -> usize {
        let due: Vec<i64> = self.events.range(..now).copied().collect();
        for at in &due {
            self.events.remove(at);
        }
        due.len()
    }
    pub fn clear(&mut self) {
        self.events.clear();
    }
}

// --- Rows. ---

#[derive(sqlx::FromRow, Clone)]
struct MailRow {
    mail_id: i64,
    from_cid: i64,
    from_name_utf16: Vec<u8>,
    to_cid: i64,
    to_name_utf16: Vec<u8>,
    mail_type: i16,
    posted_units300: i64,
    title_utf16: Vec<u8>,
    content_utf16: Vec<u8>,
    arrive_units300: i64,
    expire_units300: Option<i64>,
    is_read: Option<bool>,
    charged_gold: i32,
    is_deleted: bool,
}
#[derive(sqlx::FromRow, Clone)]
struct ItemRow {
    mail_item_id: i64,
    mail_id: i64,
    item_id: i64,
    item_class_ex_utf16: Vec<u8>,
    item_count: i32,
    received: bool,
    color1: i32,
    color2: i32,
    color3: i32,
    max_durability_bonus: i32,
    reduced_durability: i32,
    is_character_binded: bool,
    expire_ticks: Option<i64>,
}
struct Mail {
    row: MailRow,
    times: Times,
    items: Vec<ItemRow>,
}
impl Mail {
    fn mail_type(&self) -> u8 {
        u8::try_from(self.row.mail_type).unwrap_or(u8::MAX)
    }
    fn entry(&self, assets: &Assets) -> Entry {
        Entry {
            from_cid: self.row.from_cid,
            to_cid: self.row.to_cid,
            times: self.times,
            // The CLR column is bool: a NULL reference row reads as unread.
            is_read: self.row.is_read.unwrap_or(false),
            is_deleted: self.row.is_deleted,
            returnable_type: mail_type_info(assets, self.mail_type()).map(|i| i.returnable),
            first_received: self.items.first().map(|i| i.received),
        }
    }
}
const MAIL_COLUMNS: &str = "mail_id,from_cid,from_name_utf16,to_cid,to_name_utf16,mail_type,posted_units300,title_utf16,content_utf16,arrive_units300,expire_units300,is_read,charged_gold,is_deleted";
const ITEM_COLUMNS: &str = "mail_item_id,mail_id,item_id,item_class_ex_utf16,item_count,received,color1,color2,color3,max_durability_bonus,reduced_durability,is_character_binded,expire_ticks";

fn mail_type_info(assets: &Assets, mail_type: u8) -> Option<&MailTypeInfo> {
    assets.economy.as_ref()?.mail_type(mail_type)
}
fn to_ticks(units300: i64) -> Result<i64, MinimumError> {
    Ok(SqlDateTime::from_units300(units300)
        .and_then(SqlDateTime::framework_date_time)
        .map_err(|_| fail())?
        .ticks())
}
fn to_units300(ticks: i64) -> Result<i64, MinimumError> {
    Ok(SqlDateTime2::from_ticks(ticks)
        .and_then(SqlDateTime2::sql_parameter)
        .map_err(|_| fail())?
        .units300())
}
fn utc_now() -> Result<i64, MinimumError> {
    Ok(super::lab_clock::utc_now().map_err(|_| fail())?.ticks())
}
fn text(raw: &[u8]) -> String {
    let units: Vec<u16> = raw
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_le_bytes(*c))
        .collect();
    String::from_utf16_lossy(&units)
}
fn bytes(units: impl IntoIterator<Item = u16>) -> Vec<u8> {
    units.into_iter().flat_map(u16::to_le_bytes).collect()
}
fn utf16(text: &str) -> Vec<u8> {
    bytes(text.encode_utf16())
}

async fn timeouts(tx: &mut Transaction<'_, Postgres>) -> Result<(), MinimumError> {
    sqlx::query("SET LOCAL lock_timeout='5s'")
        .execute(&mut **tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout='10s'")
        .execute(&mut **tx)
        .await?;
    Ok(())
}
/// A mail request writes items and mail: the item tables are locked first, as every give
/// does before a ForceMail inserts its mail, so the lock order never inverts.
async fn bounds(tx: &mut Transaction<'_, Postgres>) -> Result<(), MinimumError> {
    timeouts(tx).await?;
    sqlx::query("LOCK TABLE game_items,game_equippables IN SHARE ROW EXCLUSIVE MODE")
        .execute(&mut **tx)
        .await?;
    sqlx::query("LOCK TABLE game_mail IN SHARE ROW EXCLUSIVE MODE")
        .execute(&mut **tx)
        .await?;
    Ok(())
}
async fn attach(
    tx: &mut Transaction<'_, Postgres>,
    rows: Vec<MailRow>,
) -> Result<Vec<Mail>, MinimumError> {
    let ids: Vec<i64> = rows.iter().map(|r| r.mail_id).collect();
    let items: Vec<ItemRow> = sqlx::query_as(&format!(
        "SELECT {ITEM_COLUMNS} FROM game_mail_items WHERE mail_id=ANY($1) ORDER BY mail_item_id LIMIT 65537"
    ))
    .bind(&ids)
    .fetch_all(&mut **tx)
    .await?;
    rows.into_iter()
        .map(|row| {
            let times = Times {
                posted: to_ticks(row.posted_units300)?,
                arrive: to_ticks(row.arrive_units300)?,
                expire: row.expire_units300.map(to_ticks).transpose()?,
            };
            let mine = items
                .iter()
                .filter(|i| i.mail_id == row.mail_id)
                .cloned()
                .collect();
            Ok(Mail {
                row,
                times,
                items: mine,
            })
        })
        .collect()
}
/// DBLoader.QueryMail (row locked for the request).
async fn query_mail(
    tx: &mut Transaction<'_, Postgres>,
    mail_id: i64,
) -> Result<Option<Mail>, MinimumError> {
    let row: Option<MailRow> = sqlx::query_as(&format!(
        "SELECT {MAIL_COLUMNS} FROM game_mail WHERE mail_id=$1 FOR UPDATE"
    ))
    .bind(mail_id)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(attach(tx, row.into_iter().collect()).await?.pop())
}
/// The QueryMailList procedure: TOP 300 received from others and not deleted, UNION ALL
/// TOP 300 sent, ORDER BY ArriveTime DESC.
async fn query_list(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
) -> Result<Vec<Mail>, MinimumError> {
    let rows: Vec<MailRow> = sqlx::query_as(&format!(
        "SELECT {MAIL_COLUMNS} FROM ((SELECT * FROM game_mail WHERE to_cid=$1 AND from_cid<>$1 AND NOT is_deleted ORDER BY arrive_units300 DESC,mail_id DESC LIMIT $2) UNION ALL (SELECT * FROM game_mail WHERE from_cid=$1 ORDER BY arrive_units300 DESC,mail_id DESC LIMIT $2)) m ORDER BY arrive_units300 DESC,mail_id DESC"
    ))
    .bind(cid)
    .bind(mail::QUERY_TOP)
    .fetch_all(&mut **tx)
    .await?;
    attach(tx, rows).await
}

/// MailAttachment fields of TransferredItemInfo.
pub(super) struct Transferred {
    pub(super) item_id: i64,
    pub(super) class_ex: String,
    pub(super) count: i32,
    pub(super) colors: [i32; 3],
    pub(super) reduced: i32,
    pub(super) bonus: i32,
    pub(super) bound: bool,
    pub(super) expire_ticks: Option<i64>,
}
/// A mail as MailFactory.MakeMail builds it.
struct NewMail<'a> {
    from_cid: i64,
    from_name: &'a str,
    to_cid: i64,
    to_name: &'a str,
    title: &'a str,
    content: &'a str,
    mail_type: u8,
    times: Times,
    charged_gold: i32,
    items: &'a [Transferred],
}
/// Loader.SendMail: the mail and its MailItem rows (Received false).
async fn insert_mail(
    tx: &mut Transaction<'_, Postgres>,
    m: &NewMail<'_>,
) -> Result<i64, MinimumError> {
    // Mail requests hold this lock from their start; a ForceMail takes it after its items.
    sqlx::query("LOCK TABLE game_mail IN SHARE ROW EXCLUSIVE MODE")
        .execute(&mut **tx)
        .await?;
    let top: Option<i64> = sqlx::query_scalar("SELECT max(mail_id) FROM game_mail")
        .fetch_one(&mut **tx)
        .await?;
    let mail_id = top
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(fail)?
        .max(m.times.posted);
    sqlx::query(&format!(
        "INSERT INTO game_mail({MAIL_COLUMNS}) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,false,$12,false)"
    ))
    .bind(mail_id)
    .bind(m.from_cid)
    .bind(utf16(m.from_name))
    .bind(m.to_cid)
    .bind(utf16(m.to_name))
    .bind(i16::from(m.mail_type))
    .bind(to_units300(m.times.posted)?)
    .bind(bytes(mail::truncate_units(m.title, mail::TITLE_UNITS)))
    .bind(bytes(mail::truncate_units(m.content, mail::CONTENT_UNITS)))
    .bind(to_units300(m.times.arrive)?)
    .bind(m.times.expire.map(to_units300).transpose()?)
    .bind(m.charged_gold)
    .execute(&mut **tx)
    .await?;
    for i in m.items {
        sqlx::query("INSERT INTO game_mail_items(mail_id,item_id,item_class_ex_utf16,item_count,received,color1,color2,color3,max_durability_bonus,reduced_durability,is_character_binded,expire_ticks) VALUES($1,$2,$3,$4,false,$5,$6,$7,$8,$9,$10,$11)")
            .bind(mail_id)
            .bind(i.item_id)
            .bind(utf16(&i.class_ex))
            .bind(i.count)
            .bind(i.colors[0])
            .bind(i.colors[1])
            .bind(i.colors[2])
            .bind(i.bonus)
            .bind(i.reduced)
            .bind(i.bound)
            .bind(i.expire_ticks)
            .execute(&mut **tx)
            .await?;
    }
    Ok(mail_id)
}
/// MailBox.AddSendMailLog/AddMailLog: AddPostalJournal then the ledger keys.
async fn journal(
    tx: &mut Transaction<'_, Postgres>,
    mail_id: i64,
    from: i64,
    to: i64,
    task: &str,
    charged: i32,
    items: &[(String, i64, i32)],
) -> Result<(), MinimumError> {
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO game_postal_journals(mail_id,from_cid,to_cid) VALUES($1,$2,$3) RETURNING journal_id",
    )
    .bind(mail_id)
    .bind(from)
    .bind(to)
    .fetch_one(&mut **tx)
    .await?;
    let mut rows: Vec<(&str, String, Option<i64>, Option<i32>)> = vec![
        ("Task", task.to_owned(), None, None),
        ("FromCID", from.to_string(), None, None),
        ("ToCID", to.to_string(), None, None),
        ("ChargedGold", charged.to_string(), None, None),
    ];
    for (value, item, count) in items {
        rows.push(("Item", value.clone(), Some(*item), Some(*count)));
    }
    for (key, value, item, count) in rows {
        sqlx::query(
            "INSERT INTO game_postal_ledgers(journal_id,key,value,item_id,count) VALUES($1,$2,$3,$4,$5)",
        )
        .bind(id)
        .bind(key)
        .bind(value)
        .bind(item)
        .bind(count)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}
fn ledger_items(items: &[ItemRow]) -> Vec<(String, i64, i32)> {
    items
        .iter()
        .map(|i| {
            (
                format!(
                    "{}/{}/{}/{}",
                    text(&i.item_class_ex_utf16),
                    i.color1,
                    i.color2,
                    i.color3
                ),
                i.item_id,
                i.item_count,
            )
        })
        .collect()
}
async fn mail_log(
    tx: &mut Transaction<'_, Postgres>,
    m: &Mail,
    task: &str,
) -> Result<(), MinimumError> {
    journal(
        tx,
        m.row.mail_id,
        m.row.from_cid,
        m.row.to_cid,
        task,
        m.row.charged_gold,
        &ledger_items(&m.items),
    )
    .await
}
/// A failure log written after the request's rollback (the log database is separate).
async fn failure_log(pool: &PgPool, mail_id: i64, from: i64, to: i64, task: &str) {
    let write = async {
        let mut tx = pool.begin().await?;
        journal(&mut tx, mail_id, from, to, task, 0, &[]).await?;
        tx.commit().await?;
        Ok::<(), MinimumError>(())
    };
    if write.await.is_err() {
        tracing::warn!("postal failure log not written; values omitted");
    }
}

/// MailFactory.MakeReturnMail and Loader.ReturnMail: the same attachments (Received false)
/// back to the sender as ReturnedMail; the returned mail is deleted.
async fn return_mail(
    tx: &mut Transaction<'_, Postgres>,
    assets: &Assets,
    m: &Mail,
    now: i64,
) -> Result<(i64, i64), MinimumError> {
    let info = mail_type_info(assets, mail_type::RETURNED).ok_or_else(fail)?;
    let items: Vec<Transferred> = m
        .items
        .iter()
        .map(|i| Transferred {
            item_id: i.item_id,
            class_ex: text(&i.item_class_ex_utf16),
            count: i.item_count,
            colors: [i.color1, i.color2, i.color3],
            reduced: i.reduced_durability,
            bonus: i.max_durability_bonus,
            bound: i.is_character_binded,
            expire_ticks: i.expire_ticks,
        })
        .collect();
    let (title, content) = (text(&m.row.title_utf16), text(&m.row.content_utf16));
    let (from_name, to_name) = (text(&m.row.to_name_utf16), text(&m.row.from_name_utf16));
    let new = NewMail {
        from_cid: m.row.to_cid,
        from_name: &from_name,
        to_cid: m.row.from_cid,
        to_name: &to_name,
        title: &mail::return_title(&title),
        content: &mail::return_content(&content),
        mail_type: mail_type::RETURNED,
        times: Times::make(now, info, true, None),
        charged_gold: 0,
        items: &items,
    };
    let id = insert_mail(tx, &new).await?;
    sqlx::query("DELETE FROM game_mail_items WHERE mail_id=$1")
        .bind(m.row.mail_id)
        .execute(&mut **tx)
        .await?;
    sqlx::query("DELETE FROM game_mail WHERE mail_id=$1")
        .bind(m.row.mail_id)
        .execute(&mut **tx)
        .await?;
    mail_log(tx, m, "ReturnedMail").await?;
    Ok((m.row.from_cid, id))
}
/// Loader.DeleteMail (RequiresDelete cleansing): the rows go with their unreceived
/// system items, which nothing can reach any more.
async fn purge(tx: &mut Transaction<'_, Postgres>, m: &Mail) -> Result<(), MinimumError> {
    mail_log(tx, m, "Delete").await?;
    let pending: Vec<i64> = m
        .items
        .iter()
        .filter(|i| !i.received && i.item_id > 0)
        .map(|i| i.item_id)
        .collect();
    destroy_system_items(tx, &pending).await?;
    sqlx::query("DELETE FROM game_mail_items WHERE mail_id=$1")
        .bind(m.row.mail_id)
        .execute(&mut **tx)
        .await?;
    sqlx::query("DELETE FROM game_mail WHERE mail_id=$1")
        .bind(m.row.mail_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}
/// Num = 0 of system items (owner NULL): the rows, attributes and equippables go.
pub(super) async fn destroy_system_items(
    tx: &mut Transaction<'_, Postgres>,
    ids: &[i64],
) -> Result<(), MinimumError> {
    if ids.is_empty() {
        return Ok(());
    }
    for q in [
        "DELETE FROM game_item_attributes WHERE item_id=ANY($1) AND item_id IN (SELECT id FROM game_items WHERE owner_id IS NULL)",
        "DELETE FROM game_equippables WHERE id=ANY($1) AND id IN (SELECT id FROM game_items WHERE owner_id IS NULL)",
        "DELETE FROM game_items WHERE id=ANY($1) AND owner_id IS NULL",
    ] {
        sqlx::query(q).bind(ids).execute(&mut **tx).await?;
    }
    Ok(())
}

fn item_info(i: &ItemRow, now: i64) -> MailItemInfo {
    MailItemInfo {
        mail_item_id: i.mail_item_id,
        item_id: i.item_id,
        item_class_ex: Some(text(&i.item_class_ex_utf16)),
        item_count: i.item_count,
        received: i.received,
        color1: i.color1,
        color2: i.color2,
        color3: i.color3,
        max_durability_bonus: i.max_durability_bonus,
        reduced_durability: i.reduced_durability,
        is_character_binded: i.is_character_binded,
        is_expireable: i.expire_ticks.is_some(),
        expire_date_time_diff: i.expire_ticks.map_or(-1, |e| e - now),
        expire_date_time_tick: i.expire_ticks.unwrap_or(-1),
    }
}
/// Mail.ToBriefMailInfo: the attachments other than gold (ItemClassEx ≠ Gold).
fn brief(m: &Mail, now: i64, gold: &[u8]) -> BriefMailInfo {
    let items: Vec<Option<MailItemInfo>> = m
        .items
        .iter()
        .filter(|i| i.item_class_ex_utf16 != gold)
        .map(|i| Some(item_info(i, now)))
        .collect();
    BriefMailInfo {
        mail_id: m.row.mail_id,
        mail_type: m.mail_type(),
        from_name: Some(text(&m.row.from_name_utf16)),
        to_name: Some(text(&m.row.to_name_utf16)),
        title: Some(text(&m.row.title_utf16)),
        expire_date: mail::expire_date(m.times.expire, now),
        is_read: m.row.is_read.unwrap_or(false),
        item_attached: mail::AttachStatus::of(m.items.first().map(|i| i.received)) as u8,
        charged_gold: m.row.charged_gold,
        attached_item_info: Some(items),
    }
}
fn packet(kind: Kind, message: Message) -> Result<OutgoingPacket, MinimumError> {
    Ok(OutgoingPacket {
        category: kind.category(),
        body: wire::encode(kind, Some(&message), DEFAULT_MAX_BODY)
            .map_err(|_| fail())?
            .to_vec(),
    })
}
fn sent(code: i32) -> Result<OutgoingPacket, MinimumError> {
    packet(Kind::MailSent, Message::MailSent(code))
}
pub(super) fn inventory_update(
    slots: Vec<Option<inv::Slot>>,
) -> Result<OutgoingPacket, MinimumError> {
    Ok(OutgoingPacket {
        category: inv::Kind::UpdateInventoryInfo.category(),
        body: inv::encode(
            inv::Kind::UpdateInventoryInfo,
            Some(&inv::Message::UpdateInventoryInfo(Some(slots))),
            DEFAULT_MAX_BODY,
        )
        .map_err(|_| fail())?
        .to_vec(),
    })
}

/// MailBox.ProcessMailList inside the caller's transaction: RequiresDelete rows are
/// cleansed, expired returnable mails go back to their senders, and the lists are built.
async fn process(
    tx: &mut Transaction<'_, Postgres>,
    assets: &Assets,
    cid: i64,
    now: i64,
) -> Result<(wire::MailList, Vec<(i64, i64)>), MinimumError> {
    let mails = query_list(tx, cid).await?;
    let entries: Vec<Entry> = mails.iter().map(|m| m.entry(assets)).collect();
    let listing = mail::process_list(cid, &entries, now).map_err(|_| fail())?;
    for &i in &listing.delete {
        purge(tx, &mails[i]).await?;
    }
    let mut notices = Vec::new();
    for &i in &listing.returned {
        notices.push(return_mail(tx, assets, &mails[i], now).await?);
    }
    let pick = |idx: &[usize]| -> Vec<Option<BriefMailInfo>> {
        idx.iter()
            .map(|&i| Some(brief(&mails[i], now, assets.inventory.gold_class())))
            .collect()
    };
    Ok((
        wire::MailList {
            received_mail_list: Some(pick(&listing.received)),
            sent_mail_list: Some(pick(&listing.sent)),
            new_mail_count: listing.new_mail_count,
        },
        notices,
    ))
}

/// QueryMailList (ItemServiceProxy.UpdateMailList and MailBox.PushMailList): MailListMessage.
pub async fn mail_list(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
) -> Result<Outcome, MinimumError> {
    tokio::time::timeout(TIMEOUT, async {
        let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
        bounds(&mut tx).await?;
        let now = utc_now()?;
        let (list, notices) = process(&mut tx, assets, cid, now).await?;
        tx.commit().await?;
        Ok(Outcome {
            packets: vec![packet(Kind::MailList, Message::MailList(list))?],
            notices,
            schedule: Vec::new(),
        })
    })
    .await
    .map_err(|_| fail())?
}

/// QueryMailInfoProcessor: MailInfo of a visible mail; the receiver reads it (SetRead).
pub async fn mail_info(
    pool: &PgPool,
    owner: &UserKey<'_>,
    cid: i64,
    mail_id: i64,
) -> Result<Outcome, MinimumError> {
    tokio::time::timeout(TIMEOUT, async {
        let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
        bounds(&mut tx).await?;
        let now = utc_now()?;
        let Some(m) = query_mail(&mut tx, mail_id).await? else {
            return Ok(Outcome::default());
        };
        let e = Entry {
            from_cid: m.row.from_cid,
            to_cid: m.row.to_cid,
            times: m.times,
            is_read: m.row.is_read.unwrap_or(false),
            is_deleted: m.row.is_deleted,
            returnable_type: None,
            first_received: None,
        };
        if !e.visible_by(cid, now) {
            return Ok(Outcome::default());
        }
        let info = wire::MailInfo {
            mail_id: m.row.mail_id,
            items: Some(m.items.iter().map(|i| Some(item_info(i, now))).collect()),
            mail_type: m.mail_type(),
            from_name: Some(text(&m.row.from_name_utf16)),
            title: Some(text(&m.row.title_utf16)),
            content: Some(text(&m.row.content_utf16)),
            charged_gold: m.row.charged_gold,
        };
        if m.row.to_cid == cid && m.row.is_read != Some(true) {
            sqlx::query("UPDATE game_mail SET is_read=true WHERE mail_id=$1")
                .bind(mail_id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(Outcome {
            packets: vec![packet(Kind::MailInfo, Message::MailInfo(info))?],
            ..Outcome::default()
        })
    })
    .await
    .map_err(|_| fail())?
}

// --- Items. ---

#[derive(sqlx::FromRow)]
struct OwnedItem {
    id: i64,
    item_class_utf16: Vec<u8>,
    count: i32,
    collection: i16,
    slot: i32,
    expire_ticks: Option<i64>,
    tradable: bool,
    checksum: Option<i32>,
}
#[derive(sqlx::FromRow)]
pub(super) struct EquipRow {
    pub(super) part: Option<i16>,
    pub(super) color1: i32,
    pub(super) color2: i32,
    pub(super) color3: i32,
    pub(super) reduced_durability: i32,
    pub(super) max_durability_bonus: i32,
}
pub(super) async fn equip_row(
    tx: &mut Transaction<'_, Postgres>,
    id: i64,
) -> Result<Option<EquipRow>, MinimumError> {
    Ok(sqlx::query_as(
        "SELECT part,color1,color2,color3,reduced_durability,max_durability_bonus FROM game_equippables WHERE id=$1",
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?)
}
pub(super) async fn attributes_of(
    tx: &mut Transaction<'_, Postgres>,
    id: i64,
) -> Result<Attributes, MinimumError> {
    Ok(super::minimum_inventory::load_attributes(tx, &[id])
        .await?
        .remove(&id)
        .unwrap_or_default())
}
/// The item's account characters (Character.AccountCharacters): the same UID.
async fn same_account(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
    other: i64,
) -> Result<bool, MinimumError> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM game_character_info a JOIN game_character_info b ON a.uid=b.uid WHERE a.id=$1 AND b.id=$2)",
    )
    .bind(cid)
    .bind(other)
    .fetch_one(&mut **tx)
    .await?)
}
/// Item.IsMailableTo(toCID).
#[allow(clippy::too_many_arguments)]
async fn mailable(
    tx: &mut Transaction<'_, Postgres>,
    assets: &Assets,
    cid: i64,
    to: Option<i64>,
    class: &[u8],
    tradable: bool,
    expire_ticks: Option<i64>,
    attributes: &Attributes,
    now: i64,
) -> Result<bool, MinimumError> {
    if expire_ticks.is_some_and(|e| e < now) || !tradable {
        return Ok(false);
    }
    if attributes
        .get("RESTORED")
        .is_some_and(|a| a.value.trim().parse::<i32>().is_ok_and(|v| v != 0))
    {
        return Ok(false);
    }
    if attributes
        .get("VALUE")
        .is_some_and(|a| a.value.to_uppercase() == "SKILLENHANCE_PROTECTED" && a.arg > 0)
    {
        return Ok(false);
    }
    let indexed = assets.stats_index.class(Some(class)).ok_or_else(fail)?;
    let economy = assets
        .economy
        .as_ref()
        .and_then(|e| e.get(indexed.source_index))
        .ok_or_else(fail)?;
    if let Some(restriction) = &economy.trade_restriction {
        if restriction != "ACCOUNTONLY" {
            return Ok(false);
        }
        return Ok(match to {
            Some(to) if to != cid => same_account(tx, cid, to).await?,
            _ => false,
        });
    }
    Ok(indexed.storage != 2)
}
/// One system item (MakeSystemItem): owner NULL, null location.
#[allow(clippy::too_many_arguments)]
async fn insert_system_item(
    tx: &mut Transaction<'_, Postgres>,
    assets: &Assets,
    id: i64,
    class: &[u8],
    count: i32,
    tradable: bool,
    expire_ticks: Option<i64>,
    attributes: &Attributes,
    equipment: Option<&EquipRow>,
) -> Result<(), MinimumError> {
    let units: Vec<u16> = class
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_le_bytes(*c))
        .collect();
    let checksum = assets
        .inventory
        .is_gold(class)
        .then(|| gold_checksum(id, None, &units, count));
    sqlx::query("INSERT INTO game_items(id,item_class_utf16,owner_id,count,collection,slot,expire_ticks,tradable,checksum) VALUES($1,$2,NULL,$3,255,-1,$4,$5,$6)")
        .bind(id)
        .bind(class)
        .bind(count)
        .bind(expire_ticks)
        .bind(tradable)
        .bind(checksum)
        .execute(&mut **tx)
        .await?;
    minimum_give::insert_attributes(tx, id, attributes).await?;
    if let Some(e) = equipment {
        sqlx::query("INSERT INTO game_equippables(id,part,color1,color2,color3,reduced_durability,max_durability_bonus) VALUES($1,NULL,$2,$3,$4,$5,$6)")
            .bind(id)
            .bind(e.color1)
            .bind(e.color2)
            .bind(e.color3)
            .bind(e.reduced_durability)
            .bind(e.max_durability_bonus)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}
/// Item.Num of an owned item: 0 deletes it with its attributes and equippable row.
async fn set_count(
    tx: &mut Transaction<'_, Postgres>,
    assets: &Assets,
    cid: i64,
    item: &OwnedItem,
    count: i32,
) -> Result<(), MinimumError> {
    if count == 0 {
        minimum_give::delete_attributes(tx, item.id).await?;
        sqlx::query("DELETE FROM game_equippables WHERE id=$1")
            .bind(item.id)
            .execute(&mut **tx)
            .await?;
        sqlx::query("DELETE FROM game_items WHERE id=$1 AND owner_id=$2")
            .bind(item.id)
            .bind(cid)
            .execute(&mut **tx)
            .await?;
        return Ok(());
    }
    let units: Vec<u16> = item
        .item_class_utf16
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_le_bytes(*c))
        .collect();
    let checksum = assets
        .inventory
        .is_gold(&item.item_class_utf16)
        .then(|| gold_checksum(item.id, Some(cid), &units, count));
    sqlx::query("UPDATE game_items SET count=$3,checksum=$4 WHERE id=$1 AND owner_id=$2")
        .bind(item.id)
        .bind(cid)
        .bind(count)
        .bind(checksum)
        .execute(&mut **tx)
        .await?;
    Ok(())
}
/// The changed owned items of a transfer, as the UpdateInventoryInfo diff.
#[derive(Default)]
pub(super) struct Changed {
    modified: Vec<i64>,
    deleted: Vec<i64>,
}
impl Changed {
    pub(super) fn record(&mut self, id: i64, count: i32) {
        self.modified.retain(|m| *m != id);
        self.deleted.retain(|m| *m != id);
        if count == 0 {
            self.deleted.push(id);
        } else {
            self.modified.push(id);
        }
    }
    pub(super) async fn slots(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        owner: &UserKey<'_>,
        assets: &Assets,
        cid: i64,
    ) -> Result<Vec<Option<inv::Slot>>, MinimumError> {
        let rows: Vec<inv::Slot> =
            super::minimum_inventory::snapshot_with_equipment(tx, owner, assets, cid, None)
                .await?
                .slots
                .ok_or_else(fail)?
                .into_iter()
                .flatten()
                .collect();
        let mut out = Vec::new();
        for id in &self.modified {
            out.push(Some(
                rows.iter()
                    .find(|s| s.id == *id)
                    .cloned()
                    .ok_or_else(fail)?,
            ));
        }
        for id in &self.deleted {
            out.push(Some(super::minimum_move::deleted_slot(*id)));
        }
        Ok(out)
    }
}
/// DestroyItem(selector, num) with DestroyComparer order (count ascending, slot
/// descending; the selector leaves bound items out). False when short.
#[allow(clippy::too_many_arguments)]
async fn destroy_class(
    tx: &mut Transaction<'_, Postgres>,
    assets: &Assets,
    cid: i64,
    class: &[u8],
    num: i32,
    mail_to: Option<Option<i64>>,
    now: i64,
    changed: &mut Changed,
) -> Result<bool, MinimumError> {
    if num < 0 {
        return Ok(false);
    }
    let rows: Vec<OwnedItem> = sqlx::query_as(
        "SELECT id,item_class_utf16,count,collection,slot,expire_ticks,tradable,checksum FROM game_items WHERE owner_id=$1 AND item_class_utf16=$2 AND collection<>$3 ORDER BY id LIMIT 4097 FOR UPDATE",
    )
    .bind(cid)
    .bind(class)
    .bind(RESTORE)
    .fetch_all(&mut **tx)
    .await?;
    let mut candidates = Vec::new();
    for r in rows {
        let selected = match mail_to {
            None => true,
            Some(to) => {
                let attributes = attributes_of(tx, r.id).await?;
                mailable(
                    tx,
                    assets,
                    cid,
                    to,
                    class,
                    r.tradable,
                    r.expire_ticks,
                    &attributes,
                    now,
                )
                .await?
            }
        };
        if selected {
            candidates.push(r);
        }
    }
    candidates.sort_by(|x, y| {
        x.tradable
            .cmp(&y.tradable)
            .then(x.count.cmp(&y.count))
            .then(y.slot.cmp(&x.slot))
    });
    let mut left = num;
    let mut plan = Vec::new();
    for c in &candidates {
        if left == 0 {
            break;
        }
        let take = c.count.min(left);
        left -= take;
        plan.push((c, c.count - take));
    }
    if left != 0 {
        return Ok(false);
    }
    for (item, count) in plan {
        set_count(tx, assets, cid, item, count).await?;
        changed.record(item.id, count);
    }
    Ok(true)
}
/// DestroyItem(ItemSelector.ItemClass(class), num) of the owned inventory (the
/// marketplace's fee and purchase gold). False when short.
pub(super) async fn destroy_owned_class(
    tx: &mut Transaction<'_, Postgres>,
    assets: &Assets,
    cid: i64,
    class: &[u8],
    num: i32,
    changed: &mut Changed,
) -> Result<bool, MinimumError> {
    destroy_class(tx, assets, cid, class, num, None, 0, changed).await
}
/// DestroyItem(ItemSelector.ItemID(item), num) of an owned, unequipped item. False when
/// the item is missing, equipped or short.
pub(super) async fn destroy_owned_item(
    tx: &mut Transaction<'_, Postgres>,
    assets: &Assets,
    cid: i64,
    item_id: i64,
    num: i32,
    changed: &mut Changed,
) -> Result<bool, MinimumError> {
    let item: Option<OwnedItem> = sqlx::query_as(
        "SELECT id,item_class_utf16,count,collection,slot,expire_ticks,tradable,checksum FROM game_items WHERE id=$1 AND owner_id=$2 FOR UPDATE",
    )
    .bind(item_id)
    .bind(cid)
    .fetch_optional(&mut **tx)
    .await?;
    let Some(item) = item else {
        return Ok(false);
    };
    if equip_row(tx, item.id)
        .await?
        .is_some_and(|e| e.part.is_some())
        || num <= 0
        || num > item.count
    {
        return Ok(false);
    }
    set_count(tx, assets, cid, &item, item.count - num).await?;
    changed.record(item.id, item.count - num);
    Ok(true)
}
/// ItemGenerator.SendMail(CID, MailType.AuctionItemMail, MakeSystemItem(request), title,
/// content): a system item and the AuctionItemMail to the owner, in the caller's
/// transaction (express system mail: it has arrived).
#[allow(clippy::too_many_arguments)]
pub(super) async fn send_auction_item(
    tx: &mut Transaction<'_, Postgres>,
    assets: &Assets,
    cid: i64,
    item_id: i64,
    class: &[u8],
    count: i32,
    bound: bool,
    attributes: &Attributes,
    equipment: Option<Equipment>,
    title: &str,
    content: &str,
) -> Result<i64, MinimumError> {
    let info = mail_type_info(assets, mail_type::AUCTION_ITEM)
        .cloned()
        .ok_or_else(fail)?;
    let sender = info.sender_name().ok_or_else(fail)?;
    let equip = equipment.map(|e| EquipRow {
        part: None,
        color1: e.colors[0],
        color2: e.colors[1],
        color3: e.colors[2],
        reduced_durability: e.reduced,
        max_durability_bonus: e.bonus,
    });
    insert_system_item(
        tx,
        assets,
        item_id,
        class,
        count,
        !bound,
        None,
        attributes,
        equip.as_ref(),
    )
    .await?;
    let name = text(class);
    let transferred = Transferred {
        item_id,
        class_ex: item_attribute::build_class_ex(&name, attributes).map_err(|_| fail())?,
        count,
        colors: equipment.map_or([-1; 3], |e| e.colors),
        reduced: equipment.map_or(0, |e| e.reduced),
        bonus: equipment.map_or(0, |e| e.bonus),
        bound,
        expire_ticks: None,
    };
    let to_name = name_in(tx, cid).await?;
    let new = NewMail {
        from_cid: -1,
        from_name: &sender,
        to_cid: cid,
        to_name: &to_name,
        title,
        content,
        mail_type: mail_type::AUCTION_ITEM,
        times: Times::make(utc_now()?, &info, true, None),
        charged_gold: 0,
        items: std::slice::from_ref(&transferred),
    };
    let id = insert_mail(tx, &new).await?;
    journal(
        tx,
        id,
        -1,
        cid,
        "AuctionItemMail",
        0,
        &[(
            format!(
                "{}/{}/{}/{}",
                transferred.class_ex,
                transferred.colors[0],
                transferred.colors[1],
                transferred.colors[2]
            ),
            item_id,
            count,
        )],
    )
    .await?;
    notice(cid);
    Ok(id)
}
/// ItemOwner.TransferToSystem(items, out transferredItems, IsMailableTo(toCID)).
#[allow(clippy::too_many_arguments)]
pub(super) async fn transfer_to_system(
    tx: &mut Transaction<'_, Postgres>,
    assets: &Assets,
    cid: i64,
    to: Option<i64>,
    requests: &[(i64, Vec<u8>, i32)],
    ids: &mut impl Iterator<Item = i64>,
    now: i64,
    changed: &mut Changed,
) -> Result<Result<Vec<Transferred>, TransferToSystemFail>, MinimumError> {
    let mut out = Vec::new();
    for (item_id, class, count) in requests {
        let Some(info) = minimum_give::class_info(assets, class)? else {
            return Ok(Err(TransferToSystemFail::NoItemInfo));
        };
        if info.equippable || *item_id > 0 {
            let item: Option<OwnedItem> = sqlx::query_as(
                "SELECT id,item_class_utf16,count,collection,slot,expire_ticks,tradable,checksum FROM game_items WHERE id=$1 AND owner_id=$2 FOR UPDATE",
            )
            .bind(item_id)
            .bind(cid)
            .fetch_optional(&mut **tx)
            .await?;
            let Some(item) = item else {
                return Ok(Err(TransferToSystemFail::NoItemInfo));
            };
            let equipment = equip_row(tx, item.id).await?;
            if equipment.as_ref().is_some_and(|e| e.part.is_some()) {
                return Ok(Err(TransferToSystemFail::EquippedItem));
            }
            let attributes = attributes_of(tx, item.id).await?;
            if !mailable(
                tx,
                assets,
                cid,
                to,
                &item.item_class_utf16,
                item.tradable,
                item.expire_ticks,
                &attributes,
                now,
            )
            .await?
            {
                return Ok(Err(TransferToSystemFail::ImproperTradeLevel));
            }
            if *count > item.count || *count <= 0 {
                return Ok(Err(TransferToSystemFail::TrasferError));
            }
            let id = ids.next().ok_or_else(fail)?;
            insert_system_item(
                tx,
                assets,
                id,
                &item.item_class_utf16,
                *count,
                item.tradable,
                item.expire_ticks,
                &attributes,
                equipment.as_ref(),
            )
            .await?;
            set_count(tx, assets, cid, &item, item.count - count).await?;
            changed.record(item.id, item.count - count);
            let class = text(&item.item_class_utf16);
            out.push(Transferred {
                item_id: id,
                class_ex: item_attribute::build_class_ex(&class, &attributes)
                    .map_err(|_| fail())?,
                count: *count,
                colors: equipment
                    .as_ref()
                    .map_or([-1; 3], |e| [e.color1, e.color2, e.color3]),
                reduced: equipment.as_ref().map_or(0, |e| e.reduced_durability),
                bonus: equipment.as_ref().map_or(0, |e| e.max_durability_bonus),
                bound: !item.tradable,
                expire_ticks: item.expire_ticks,
            });
        } else {
            if !destroy_class(tx, assets, cid, class, *count, Some(to), now, changed).await? {
                return Ok(Err(TransferToSystemFail::TrasferError));
            }
            // MakeSystemItem(new GiveItemRequest(class, count)) requires IsSingle.
            if *count <= 0 || *count > info.max_stack {
                return Ok(Err(TransferToSystemFail::TrasferError));
            }
            let id = ids.next().ok_or_else(fail)?;
            let expire = crate::domain::item_time::initialize_expiry(
                None,
                false,
                info.expire_in,
                &mut || {
                    super::lab_clock::utc_now()
                        .map_err(|_| crate::domain::item_time::ItemTimeError::Clock)
                },
            )
            .map_err(|_| fail())?
            .expiry
            .map(|t| {
                t.sql_parameter()
                    .and_then(SqlDateTime::framework_date_time)
                    .map(|t| t.ticks())
            })
            .transpose()
            .map_err(|_| fail())?;
            insert_system_item(
                tx,
                assets,
                id,
                class,
                *count,
                true,
                expire,
                &item_attribute::EMPTY,
                None,
            )
            .await?;
            out.push(Transferred {
                item_id: id,
                class_ex: text(class),
                count: *count,
                colors: [-1; 3],
                reduced: 0,
                bonus: 0,
                bound: false,
                expire_ticks: expire,
            });
        }
    }
    Ok(Ok(out))
}

/// GetCIDByName: SELECT TOP 1 ID FROM CharacterInfo WHERE Name = @Name AND Status = 0
/// (the collation compares case-insensitively).
async fn cid_by_name(
    tx: &mut Transaction<'_, Postgres>,
    name: &str,
) -> Result<Option<i64>, MinimumError> {
    let rows: Vec<(i64, Vec<u8>)> = sqlx::query_as(
        "SELECT id,name_utf16 FROM game_character_info WHERE status=0 AND name_utf16 IS NOT NULL ORDER BY id LIMIT 1001",
    )
    .fetch_all(&mut **tx)
    .await?;
    let wanted = name.to_lowercase();
    Ok(rows
        .into_iter()
        .find(|(_, raw)| text(raw).to_lowercase() == wanted)
        .map(|(id, _)| id))
}
async fn name_in(tx: &mut Transaction<'_, Postgres>, cid: i64) -> Result<String, MinimumError> {
    let raw: Option<Vec<u8>> =
        sqlx::query_scalar("SELECT name_utf16 FROM game_character_info WHERE id=$1")
            .bind(cid)
            .fetch_one(&mut **tx)
            .await?;
    Ok(text(&raw.ok_or_else(fail)?))
}

/// SendMailMessage (ItemServiceProxy: level 10, UseSecondPassword) → SendMailProcessor.
pub async fn send_mail(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    m: &wire::SendMail,
    entity_ids: Vec<i64>,
) -> Result<Outcome, MinimumError> {
    let level: i32 = sqlx::query_scalar("SELECT level FROM game_character_info WHERE id=$1")
        .bind(cid)
        .fetch_one(pool)
        .await?;
    if level < 10 {
        return Ok(Outcome {
            packets: vec![super::minimum_shop::dialog(
                "#GameUI_Heroes_Cannot_Send_Mail_Under_Level_5",
            )?],
            ..Outcome::default()
        });
    }
    let items: Vec<&wire::AttachedTransferItemInfo> =
        m.item_list.iter().flatten().flatten().collect();
    if items.len() > MAX_ATTACHMENTS || entity_ids.len() < items.len() + 1 {
        return Err(fail());
    }
    let to_name = m.to_name.clone().unwrap_or_default();
    let mut ids = entity_ids.into_iter();
    let result = tokio::time::timeout(TIMEOUT, async {
        let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
        bounds(&mut tx).await?;
        let now = utc_now()?;
        let Some(info) = mail_type_info(assets, m.mail_type).cloned() else {
            return Ok((mail::sent::INVALID_MAIL_TYPE, None, Vec::new()));
        };
        if !info.is_correct_user_mail_type(!items.is_empty() || m.gold > 0, m.charged_gold > 0) {
            return Ok((mail::sent::INVALID_MAIL_TYPE, None, Vec::new()));
        }
        let Some(to) = cid_by_name(&mut tx, &to_name).await? else {
            return Ok((mail::sent::CID_QUERY_FAILED, None, Vec::new()));
        };
        if to == cid {
            return Ok((mail::sent::SELF_CID_FAILED, None, Vec::new()));
        }
        let mut requests: Vec<(i64, Vec<u8>, i32)> = items
            .iter()
            .map(|i| {
                (
                    i.item_id,
                    utf16(i.item_class.as_deref().unwrap_or_default()),
                    i.item_count,
                )
            })
            .collect();
        if m.gold > 0 {
            requests.push((-1, assets.inventory.gold_class().to_vec(), m.gold));
        }
        let mut changed = Changed::default();
        let transferred = match transfer_to_system(
            &mut tx,
            assets,
            cid,
            Some(to),
            &requests,
            &mut ids,
            now,
            &mut changed,
        )
        .await?
        {
            Ok(t) => t,
            Err(reason) => return Ok((reason as i32, None, Vec::new())),
        };
        // MailBox.SendMail: Fee plus FeeForItem% of each transferred item's Price.
        let (fee, rate) = info.fees(m.express);
        let mut priced = Vec::new();
        for t in &transferred {
            let (class, _) = item_attribute::parse_class_ex(&t.class_ex).map_err(|_| fail())?;
            let indexed = assets
                .stats_index
                .class(Some(&utf16(&class)))
                .ok_or_else(fail)?;
            let price = assets
                .economy
                .as_ref()
                .and_then(|e| e.get(indexed.source_index))
                .ok_or_else(fail)?
                .price;
            priced.push((price, t.count));
        }
        let fee = mail::send_fee(fee, rate, priced);
        if !destroy_class(
            &mut tx,
            assets,
            cid,
            assets.inventory.gold_class(),
            fee,
            None,
            now,
            &mut changed,
        )
        .await?
        {
            return Ok((mail::sent::TRANSFER_FEE_PAY_ERROR, None, Vec::new()));
        }
        let from_name = name_in(&mut tx, cid).await?;
        let times = Times::make(now, &info, m.express, None);
        let new = NewMail {
            from_cid: cid,
            from_name: &from_name,
            to_cid: to,
            to_name: &to_name,
            title: m.title.as_deref().unwrap_or_default(),
            content: m.content.as_deref().unwrap_or_default(),
            mail_type: m.mail_type,
            times,
            charged_gold: m.charged_gold,
            items: &transferred,
        };
        if to_name.encode_utf16().count() > 20 {
            return Ok((mail::sent::SEND_FAILED, None, Vec::new()));
        }
        let mail_id = insert_mail(&mut tx, &new).await?;
        let ledger: Vec<(String, i64, i32)> = transferred
            .iter()
            .map(|t| {
                (
                    format!(
                        "{}/{}/{}/{}",
                        t.class_ex, t.colors[0], t.colors[1], t.colors[2]
                    ),
                    t.item_id,
                    t.count,
                )
            })
            .collect();
        let task = match m.mail_type {
            mail_type::TEXT => "TextMail",
            mail_type::ITEM => "ItemMail",
            _ => "ItemChargeMail",
        };
        journal(&mut tx, mail_id, cid, to, task, m.charged_gold, &ledger).await?;
        let slots = changed.slots(&mut tx, owner, assets, cid).await?;
        tx.commit().await?;
        Ok::<_, MinimumError>((mail::sent::SUCCESSFUL, Some((to, mail_id)), slots))
    })
    .await
    .map_err(|_| fail())??;
    let (code, notice, slots) = result;
    let mut packets = Vec::new();
    if !slots.is_empty() {
        packets.push(inventory_update(slots)?);
    }
    packets.push(sent(code)?);
    Ok(Outcome {
        packets,
        notices: notice.into_iter().collect(),
        schedule: Vec::new(),
    })
}

/// GetMailItemMessage (level ≥ 10 allows charged mail) → GetMailItemProcessor.
pub async fn get_mail_item(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    mail_id: i64,
    entity_ids: Vec<i64>,
) -> Result<Outcome, MinimumError> {
    let level: i32 = sqlx::query_scalar("SELECT level FROM game_character_info WHERE id=$1")
        .bind(cid)
        .fetch_one(pool)
        .await?;
    let allowed = level >= 10;
    let mut ids = entity_ids.into_iter();
    let outcome = tokio::time::timeout(TIMEOUT, async {
        let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
        bounds(&mut tx).await?;
        let now = utc_now()?;
        type Done = (GetMailItemResult, Vec<Option<inv::Slot>>, Option<(i64, i64)>);
        let done = |r: GetMailItemResult| -> Result<Done, MinimumError> { Ok((r, Vec::new(), None)) };
        let Some(m) = query_mail(&mut tx, mail_id).await? else {
            return done(GetMailItemResult::NoItem);
        };
        let e = m.entry(assets);
        if !e.visible_by(cid, now) || !e.attachments_pending() {
            return done(GetMailItemResult::NoItem);
        }
        if m.row.to_cid != cid || !m.times.visible_by_receiver(now, m.row.is_deleted) {
            return done(GetMailItemResult::QueryMailFail);
        }
        let info = mail_type_info(assets, m.mail_type()).cloned().ok_or_else(fail)?;
        let mut system = Vec::new();
        let mut gm = Vec::new();
        for i in m.items.iter().filter(|i| !i.received) {
            if i.item_id > 0 {
                system.push(i);
            } else if info.system {
                gm.push(i);
            } else {
                return done(GetMailItemResult::GiveItemFail);
            }
        }
        if !system.is_empty() && !gm.is_empty() {
            return done(GetMailItemResult::ItemTypeMixed);
        }
        if system.is_empty() && gm.is_empty() {
            return done(GetMailItemResult::NoItem);
        }
        if system.len() + gm.len() > MAX_ATTACHMENTS {
            return Err(fail());
        }
        let mut changed = Changed::default();
        let mut paid = None;
        if m.row.charged_gold > 0 {
            if !allowed {
                return done(GetMailItemResult::GetItemNotAllowed);
            }
            // TransferSingleItemToSystem(Gold, ChargedGold).
            match transfer_to_system(
                &mut tx,
                assets,
                cid,
                None,
                &[(-1, assets.inventory.gold_class().to_vec(), m.row.charged_gold)],
                &mut ids,
                now,
                &mut changed,
            )
            .await?
            {
                Ok(mut t) => paid = t.pop(),
                Err(_) => return done(GetMailItemResult::FeePayError),
            }
        }
        let applied = if !system.is_empty() {
            let is_account_mail = same_account(&mut tx, cid, m.row.from_cid).await?;
            let user_trade = !info.system;
            let mut gives = Vec::new();
            for i in &system {
                let row: Option<OwnedItem> = sqlx::query_as(
                    "SELECT id,item_class_utf16,count,collection,slot,expire_ticks,tradable,checksum FROM game_items WHERE id=$1 AND owner_id IS NULL FOR UPDATE",
                )
                .bind(i.item_id)
                .fetch_optional(&mut *tx)
                .await?;
                // GetSystemItem: a missing, invalid or located item fails TransferFromSystem.
                let Some(row) = row else {
                    return done(GetMailItemResult::Unknown);
                };
                let class = text(&row.item_class_utf16);
                let units: Vec<u16> = class.encode_utf16().collect();
                if row.collection != 255
                    || row.slot != -1
                    || row.count <= 0
                    || minimum_give::class_info(assets, &row.item_class_utf16)?.is_none()
                    || (assets.inventory.is_gold(&row.item_class_utf16)
                        && row.checksum != Some(gold_checksum(row.id, None, &units, row.count)))
                {
                    return done(GetMailItemResult::Unknown);
                }
                let attributes = attributes_of(&mut tx, row.id).await?;
                let equipment = equip_row(&mut tx, row.id).await?;
                let bound = !row.tradable
                    || (user_trade
                        && user_trade_binds(assets, &row.item_class_utf16, &attributes, is_account_mail)?);
                gives.push((row, attributes, equipment, bound));
            }
            let requests: Vec<Request<'_>> = gives
                .iter()
                .map(|(row, attributes, equipment, bound)| Request {
                    class: &row.item_class_utf16,
                    count: row.count,
                    bound: *bound,
                    equipment: equipment.as_ref().map(|e| Equipment {
                        colors: [e.color1, e.color2, e.color3],
                        reduced: e.reduced_durability,
                        bonus: e.max_durability_bonus,
                    }),
                    attributes,
                })
                .collect();
            let expiries: Vec<Option<i64>> = gives.iter().map(|g| g.0.expire_ticks).collect();
            let applied = minimum_give::apply_expiring(
                &mut tx,
                owner,
                assets,
                cid,
                &requests,
                &expiries,
                false,
                &mut || ids.next().ok_or_else(fail),
            )
            .await?;
            match applied {
                Ok(a) => {
                    let used: Vec<i64> = gives.iter().map(|g| g.0.id).collect();
                    destroy_system_items(&mut tx, &used).await?;
                    a
                }
                Err(e) => return done(give_result(e)),
            }
        } else {
            // GM mail: GiveItem(ItemClassEx, count, colors, IsCharacterBinded true).
            let mut parsed = Vec::new();
            for i in &gm {
                let (class, attributes) =
                    item_attribute::parse_class_ex(&text(&i.item_class_ex_utf16)).map_err(|_| fail())?;
                parsed.push((utf16(&class), i.item_count, attributes));
            }
            let requests: Vec<Request<'_>> = parsed
                .iter()
                .map(|(class, count, attributes)| Request {
                    class,
                    count: *count,
                    bound: true,
                    equipment: None,
                    attributes,
                })
                .collect();
            match minimum_give::apply(
                &mut tx,
                owner,
                assets,
                cid,
                &[],
                &requests,
                &mut || ids.next().ok_or_else(fail),
            )
            .await?
            {
                Ok(a) => a,
                Err(e) => return done(give_result(e)),
            }
        };
        let mut notice = None;
        if let (true, Some(paid)) = (m.row.charged_gold > 0 && m.row.from_cid != -1, paid) {
            // MakeChargePayMail: the paid gold to the sender, express ChargePayMail.
            let pay = mail_type_info(assets, mail_type::CHARGE_PAY).ok_or_else(fail)?;
            let (title, content) = (text(&m.row.title_utf16), text(&m.row.content_utf16));
            let (from_name, to_name) = (text(&m.row.to_name_utf16), text(&m.row.from_name_utf16));
            let new = NewMail {
                from_cid: m.row.to_cid,
                from_name: &from_name,
                to_cid: m.row.from_cid,
                to_name: &to_name,
                title: &mail::charge_pay_title(&title),
                content: &mail::charge_pay_content(&content),
                mail_type: mail_type::CHARGE_PAY,
                times: Times::make(now, pay, true, None),
                charged_gold: 0,
                items: std::slice::from_ref(&paid),
            };
            let id = insert_mail(&mut tx, &new).await?;
            journal(
                &mut tx,
                id,
                m.row.to_cid,
                m.row.from_cid,
                "ChargePayMail",
                0,
                &[(format!("{}/-1/-1/-1", paid.class_ex), paid.item_id, paid.count)],
            )
            .await?;
            notice = Some((m.row.from_cid, id));
        }
        sqlx::query("UPDATE game_mail_items SET received=true,received_units300=$2 WHERE mail_id=$1")
            .bind(mail_id)
            .bind(to_units300(now)?)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE game_mail SET is_read=true WHERE mail_id=$1")
            .bind(mail_id)
            .execute(&mut *tx)
            .await?;
        mail_log(&mut tx, &m, "GetMailItem").await?;
        let mut slots = changed.slots(&mut tx, owner, assets, cid).await?;
        slots.extend(applied.slots);
        tx.commit().await?;
        Ok::<_, MinimumError>((GetMailItemResult::Success, slots, notice))
    })
    .await
    .map_err(|_| fail())??;
    let (result, slots, notice) = outcome;
    if result != GetMailItemResult::Success {
        failure_log(pool, mail_id, -1, cid, "GetMailItemFailed").await;
    }
    let mut packets = Vec::new();
    if !slots.is_empty() {
        packets.push(inventory_update(slots)?);
    }
    packets.push(packet(
        Kind::GetMailItemCompleted,
        Message::GetMailItemCompleted(wire::GetMailItemCompleted {
            mail_id,
            result: result as u8,
        }),
    )?);
    Ok(Outcome {
        packets,
        notices: notice.into_iter().collect(),
        schedule: Vec::new(),
    })
}
/// TransferFromSystem's user-trade binding: IsHighLevelEquipment, FeatureMatrix Bind or
/// TradeOnBind of the class, or an enchant scroll under Macro_EnchantTradeRestriction
/// from another account.
pub(super) fn user_trade_binds(
    assets: &Assets,
    class: &[u8],
    attributes: &Attributes,
    is_account_mail: bool,
) -> Result<bool, MinimumError> {
    let indexed = assets.stats_index.class(Some(class)).ok_or_else(fail)?;
    let catalog = assets
        .catalog
        .classes
        .get(indexed.source_index)
        .ok_or_else(fail)?;
    let policy = &assets.catalog.policy;
    if policy.is_enabled(catalog.bind.as_deref())?
        || policy.is_enabled(catalog.trade_on_bind.as_deref())?
    {
        return Ok(true);
    }
    let name = text(class);
    if !is_account_mail
        && name.starts_with("enchant_scroll")
        && policy.is_enabled(Some("Macro_EnchantTradeRestriction"))?
    {
        return Ok(true);
    }
    high_level_equipment(assets, &name, indexed.equippable, attributes)
}
/// Item.IsHighLevelEquipment.
pub(super) fn high_level_equipment(
    assets: &Assets,
    name: &str,
    equippable: bool,
    attributes: &Attributes,
) -> Result<bool, MinimumError> {
    if name.to_lowercase() == "succubus_wing_upper_cash" {
        return Ok(true);
    }
    if !equippable || name.starts_with("enchant_scroll") {
        return Ok(false);
    }
    let level = |key: &str| {
        attributes
            .get(key)
            .and_then(|a| a.value.trim().parse::<i32>().ok())
            .unwrap_or(0)
    };
    if level("ENHANCE") >= 9 {
        return Ok(true);
    }
    for key in ["PREFIX", "SUFFIX"] {
        if let Some(a) = attributes.get(key) {
            let enchant = assets
                .enhance_contents()
                .and_then(|c| c.enchant(&a.value))
                .map_or(0, |e| e.level);
            if enchant >= 7 {
                return Ok(true);
            }
        }
    }
    if attributes.get("LOOK").is_some() {
        return Ok(true);
    }
    if attributes.get("QUALITY").is_some_and(|a| a.arg >= 5) {
        return Ok(true);
    }
    let slots = assets
        .catalog
        .policy
        .get_integer("BraceletMaxSlotCount")
        .unwrap_or(0);
    for i in 0..slots.clamp(0, 4) {
        if attributes.get(&format!("GS_{i}")).is_some() {
            return Ok(true);
        }
    }
    Ok(false)
}
/// ItemOperationResult → GetMailItemCompletedMessage.ResultEnum.
fn give_result(e: GiveError) -> GetMailItemResult {
    match e {
        GiveError::NoEmptySlot => GetMailItemResult::NoEmptySlot,
        GiveError::UniqueConstraint => GetMailItemResult::UniqueViolation,
        _ => GetMailItemResult::Unknown,
    }
}

/// DeleteMailMessage → DeleteMailProcessor, then UpdateMailList on completion.
pub async fn delete_mail(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    mail_ids: &[i64],
) -> Result<Outcome, MinimumError> {
    if mail_ids.len() > 256 {
        return Err(fail());
    }
    let notices = tokio::time::timeout(TIMEOUT, async {
        let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
        bounds(&mut tx).await?;
        let now = utc_now()?;
        let mut delete = Vec::new();
        let mut returned = Vec::new();
        let mut destroyed = Vec::new();
        for id in mail_ids {
            let Some(m) = query_mail(&mut tx, *id).await? else {
                continue;
            };
            let e = m.entry(assets);
            if !e.visible_by(cid, now) {
                continue;
            }
            if e.returnable().map_err(|_| fail())? {
                returned.push(m);
                continue;
            }
            for i in m.items.iter().filter(|i| !i.received) {
                if i.item_id == -1 {
                    // A GM mail cannot be deleted: the whole operation fails.
                    return Ok(None);
                }
                destroyed.push(i.item_id);
            }
            delete.push(m);
        }
        destroy_system_items(&mut tx, &destroyed).await?;
        for m in &delete {
            sqlx::query("UPDATE game_mail SET is_deleted=true WHERE mail_id=$1")
                .bind(m.row.mail_id)
                .execute(&mut *tx)
                .await?;
        }
        let mut notices = Vec::new();
        for m in &returned {
            notices.push(return_mail(&mut tx, assets, m, now).await?);
        }
        tx.commit().await?;
        Ok::<_, MinimumError>(Some(notices))
    })
    .await
    .map_err(|_| fail())??;
    let Some(notices) = notices else {
        return Ok(Outcome::default());
    };
    let mut out = mail_list(pool, owner, assets, cid).await?;
    out.notices.extend(notices);
    Ok(out)
}

/// ReturnMailMessage → ReturnMailProcessor: MailSent(ReturnSuccessful), or the frontend's
/// MailSent(SendFailed) when the processor fails.
pub async fn return_mail_request(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    mail_id: i64,
) -> Result<Outcome, MinimumError> {
    let returned = tokio::time::timeout(TIMEOUT, async {
        let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
        bounds(&mut tx).await?;
        let now = utc_now()?;
        let Some(m) = query_mail(&mut tx, mail_id).await? else {
            return Ok(None);
        };
        let e = m.entry(assets);
        if !e.visible_by(cid, now)
            || !e.attachments_pending()
            || m.row.to_cid != cid
            || !m.times.visible_by_receiver(now, m.row.is_deleted)
        {
            return Ok(None);
        }
        let notice = return_mail(&mut tx, assets, &m, now).await?;
        tx.commit().await?;
        Ok::<_, MinimumError>(Some(notice))
    })
    .await
    .map_err(|_| fail())??;
    Ok(match returned {
        Some(notice) => Outcome {
            packets: vec![sent(mail::sent::RETURN_SUCCESSFUL)?],
            notices: vec![notice],
            schedule: Vec::new(),
        },
        None => Outcome {
            packets: vec![sent(mail::sent::SEND_FAILED)?],
            ..Outcome::default()
        },
    })
}

/// SendGiveItemFailMail(item): MakeSystemMail(DungeonItemMail, the GiveFailMail title and
/// content, the system item, no charge) to the owner, in the caller's transaction.
pub(super) async fn send_give_fail_mail(
    tx: &mut Transaction<'_, Postgres>,
    assets: &Assets,
    cid: i64,
    item_id: i64,
) -> Result<i64, MinimumError> {
    send_system_item_mail(
        tx,
        assets,
        cid,
        item_id,
        mail_type::DUNGEON_ITEM,
        None,
        None,
    )
    .await
}

/// ItemOwner.SendItemToMail(mailType, item, 0, title, content, expire): MakeSystemMail of a
/// system item to its owner in the caller's transaction (None titles are the GiveFailMail
/// ones). `force_expire` (DateTime ticks) replaces the type's expiry.
pub(super) async fn send_system_item_mail(
    tx: &mut Transaction<'_, Postgres>,
    assets: &Assets,
    cid: i64,
    item_id: i64,
    kind: u8,
    titles: Option<(&str, &str)>,
    force_expire: Option<i64>,
) -> Result<i64, MinimumError> {
    let task = match kind {
        mail_type::DUNGEON_ITEM => "DungeonItemMail",
        mail_type::AUCTION_ITEM => "AuctionItemMail",
        11 => "GMMail",
        12 => "GuildLevelUpItemMail",
        _ => return Err(fail()),
    };
    let info = mail_type_info(assets, kind).cloned().ok_or_else(fail)?;
    let sender = info.sender_name().ok_or_else(fail)?;
    let row: OwnedItem = sqlx::query_as(
        "SELECT id,item_class_utf16,count,collection,slot,expire_ticks,tradable,checksum FROM game_items WHERE id=$1 AND owner_id IS NULL",
    )
    .bind(item_id)
    .fetch_one(&mut **tx)
    .await?;
    let attributes = attributes_of(tx, item_id).await?;
    let equipment = equip_row(tx, item_id).await?;
    let class = text(&row.item_class_utf16);
    let transferred = Transferred {
        item_id,
        class_ex: item_attribute::build_class_ex(&class, &attributes).map_err(|_| fail())?,
        count: row.count,
        colors: equipment
            .as_ref()
            .map_or([-1; 3], |e| [e.color1, e.color2, e.color3]),
        reduced: equipment.as_ref().map_or(0, |e| e.reduced_durability),
        bonus: equipment.as_ref().map_or(0, |e| e.max_durability_bonus),
        bound: !row.tradable,
        expire_ticks: row.expire_ticks,
    };
    let name = name_in(tx, cid).await?;
    let now = utc_now()?;
    let new = NewMail {
        from_cid: -1,
        from_name: &sender,
        to_cid: cid,
        to_name: &name,
        title: &titles.map_or_else(
            || mail::give_fail_title(&class, row.count),
            |t| t.0.to_owned(),
        ),
        content: &titles.map_or_else(
            || mail::give_fail_content(&class, row.count),
            |t| t.1.to_owned(),
        ),
        mail_type: kind,
        times: Times::make(now, &info, true, force_expire),
        charged_gold: 0,
        items: std::slice::from_ref(&transferred),
    };
    let id = insert_mail(tx, &new).await?;
    journal(
        tx,
        id,
        -1,
        cid,
        task,
        0,
        &[(
            format!(
                "{}/{}/{}/{}",
                transferred.class_ex,
                transferred.colors[0],
                transferred.colors[1],
                transferred.colors[2]
            ),
            item_id,
            row.count,
        )],
    )
    .await?;
    Ok(id)
}

/// MakeSystemItem(new GiveItemRequest(class, count)) then SendGiveItemFailMail, in the
/// caller's transaction: the console message, or None when the request is not IsSingle
/// (no mail).
pub(super) async fn mail_system_item(
    tx: &mut Transaction<'_, Postgres>,
    assets: &Assets,
    cid: i64,
    class: &[u8],
    count: i32,
    ids: &mut (dyn FnMut() -> Result<i64, MinimumError> + Send),
) -> Result<Option<OutgoingPacket>, MinimumError> {
    let Some(info) = minimum_give::class_info(assets, class)? else {
        return Ok(None);
    };
    if count <= 0 || count > info.max_stack || info.equippable {
        return Ok(None);
    }
    let id = ids()?;
    let expire =
        crate::domain::item_time::initialize_expiry(None, false, info.expire_in, &mut || {
            super::lab_clock::utc_now().map_err(|_| crate::domain::item_time::ItemTimeError::Clock)
        })
        .map_err(|_| fail())?
        .expiry
        .map(|t| {
            t.sql_parameter()
                .and_then(SqlDateTime::framework_date_time)
                .map(|t| t.ticks())
        })
        .transpose()
        .map_err(|_| fail())?;
    insert_system_item(
        tx,
        assets,
        id,
        class,
        count,
        true,
        expire,
        &item_attribute::EMPTY,
        None,
    )
    .await?;
    send_give_fail_mail(tx, assets, cid, id).await?;
    notice(cid);
    Ok(Some(super::minimum_shop::system_message(
        super::minimum_shop::SYSTEM,
        "GameUI_Heroes_GiveFailMail_Success_Console",
        &[text(class), count.to_string()],
    )?))
}

/// QueuedItemController with a mail title: MakeSystemItems(GiveItemRequest(class, count,
/// attributes, bound)) then SendItemToMail(kind, item, title, content, expire), for a single
/// non-equipment stack; None (nothing written) for another request.
#[allow(clippy::too_many_arguments)]
pub(super) async fn mail_new_system_item(
    tx: &mut Transaction<'_, Postgres>,
    assets: &Assets,
    cid: i64,
    request: (&[u8], i32, &Attributes, bool),
    kind: u8,
    titles: (&str, &str),
    force_expire: Option<i64>,
    ids: &mut (dyn FnMut() -> Result<i64, MinimumError> + Send),
) -> Result<Option<i64>, MinimumError> {
    let (class, count, attributes, bound) = request;
    let Some(info) = minimum_give::class_info(assets, class)? else {
        return Ok(None);
    };
    if count <= 0 || count > info.max_stack || info.equippable {
        return Ok(None);
    }
    let id = ids()?;
    let expire =
        crate::domain::item_time::initialize_expiry(None, false, info.expire_in, &mut || {
            super::lab_clock::utc_now().map_err(|_| crate::domain::item_time::ItemTimeError::Clock)
        })
        .map_err(|_| fail())?
        .expiry
        .map(|t| {
            t.sql_parameter()
                .and_then(SqlDateTime::framework_date_time)
                .map(|t| t.ticks())
        })
        .transpose()
        .map_err(|_| fail())?;
    insert_system_item(
        tx, assets, id, class, count, !bound, expire, attributes, None,
    )
    .await?;
    let mail = send_system_item_mail(tx, assets, cid, id, kind, Some(titles), force_expire).await?;
    notice(cid);
    Ok(Some(mail))
}

/// MailBox.ScheduleAllMail (ItemOwner activation): the times of the QueryMailList rows.
pub async fn schedule_all(
    pool: &PgPool,
    assets: &Assets,
    cid: i64,
) -> Result<Vec<i64>, MinimumError> {
    let mut tx = pool.begin().await?;
    timeouts(&mut tx).await?;
    let now = utc_now()?;
    let mails = query_list(&mut tx, cid).await?;
    tx.commit().await?;
    Ok(mails
        .iter()
        .flat_map(|m| mail::schedule(cid, &m.entry(assets), false, now))
        .collect())
}
/// NoticeMailProcessor → MailBox.NoticeMail: a new mail to this owner is scheduled as
/// just sent (an arrived mail pushes the list at once).
pub async fn schedule_notice(
    pool: &PgPool,
    assets: &Assets,
    cid: i64,
    mail_id: i64,
) -> Result<Vec<i64>, MinimumError> {
    let mut tx = pool.begin().await?;
    timeouts(&mut tx).await?;
    let now = utc_now()?;
    let row: Option<MailRow> = sqlx::query_as(&format!(
        "SELECT {MAIL_COLUMNS} FROM game_mail WHERE mail_id=$1"
    ))
    .bind(mail_id)
    .fetch_optional(&mut *tx)
    .await?;
    let mails = attach(&mut tx, row.into_iter().collect()).await?;
    tx.commit().await?;
    Ok(match mails.first() {
        Some(m) if m.row.to_cid == cid => mail::schedule(cid, &m.entry(assets), true, now),
        _ => Vec::new(),
    })
}
/// The QueryMailList procedure's mail IDs of `cid` (ArriveTime descending).
pub async fn query_mail_ids(pool: &PgPool, cid: i64) -> Result<Vec<i64>, MinimumError> {
    let mut tx = pool.begin().await?;
    timeouts(&mut tx).await?;
    let ids = query_list(&mut tx, cid)
        .await?
        .iter()
        .map(|m| m.row.mail_id)
        .collect();
    tx.commit().await?;
    Ok(ids)
}
/// The DateTime ticks of now, for the connection's TimeLine.
pub fn now_ticks() -> Result<i64, MinimumError> {
    utc_now()
}

#[cfg(test)]
mod tests {
    use super::TimeLine;

    #[test]
    fn timeline_runs_past_jobs_at_once_and_polls_due_ones() {
        let mut t = TimeLine::default();
        assert!(t.add(5, 10));
        assert!(!t.add(20, 10));
        assert!(!t.add(20, 10));
        assert!(!t.add(40, 10));
        assert_eq!(t.poll(20), 0);
        assert_eq!(t.poll(22), 2);
        assert_eq!(t.poll(41), 1);
        assert_eq!(t.poll(100), 0);
    }
}
