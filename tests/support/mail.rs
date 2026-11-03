use super::{cleanup_game_pool, isolated_game_pool, minimum_character};
use vindictus_server::{
    infrastructure::{minimum_give, minimum_mail},
    protocol::{
        DEFAULT_MAX_BODY,
        mail::{self as wire, Kind, Message},
    },
};

fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
fn decode(out: &minimum_mail::Outcome, kind: Kind) -> Message {
    let p = out
        .packets
        .iter()
        .find(|p| p.category == kind.category())
        .unwrap_or_else(|| panic!("{kind:?} missing"));
    wire::decode(kind, &p.body, DEFAULT_MAX_BODY)
        .unwrap()
        .unwrap()
}
fn sent(out: &minimum_mail::Outcome) -> i32 {
    match decode(out, Kind::MailSent) {
        Message::MailSent(code) => code,
        _ => unreachable!(),
    }
}
fn list(out: &minimum_mail::Outcome) -> wire::MailList {
    match decode(out, Kind::MailList) {
        Message::MailList(l) => l,
        _ => unreachable!(),
    }
}
async fn gold(p: &sqlx::PgPool) -> i32 {
    sqlx::query_scalar("SELECT COALESCE(sum(count),0)::int FROM game_items WHERE owner_id=1 AND item_class_utf16=$1")
        .bind(utf16("synthetic_gold"))
        .fetch_one(p)
        .await
        .unwrap()
}
/// A second character of the same account named "Evie" (the mail recipient).
async fn recipient(p: &sqlx::PgPool) {
    sqlx::raw_sql(
        "INSERT INTO game_characters SELECT (jsonb_populate_record(NULL::game_characters,to_jsonb(c)||'{\"id\":2}')).* FROM game_characters c WHERE id=1;
         INSERT INTO game_character_info SELECT (jsonb_populate_record(NULL::game_character_info,to_jsonb(c)||'{\"id\":2,\"character_sn\":7}')).* FROM game_character_info c WHERE id=1;",
    )
    .execute(p)
    .await
    .unwrap();
    sqlx::query("UPDATE game_character_info SET name_utf16=$1 WHERE id=2")
        .bind(utf16("Evie"))
        .execute(p)
        .await
        .unwrap();
}
/// The last mail sent by 1 to 2 becomes one from 2 to 1 that has arrived.
async fn turn_around(p: &sqlx::PgPool) -> i64 {
    sqlx::query_scalar("UPDATE game_mail SET from_cid=2,to_cid=1,from_name_utf16=$1,to_name_utf16=$2,arrive_units300=posted_units300 WHERE mail_id=(SELECT max(mail_id) FROM game_mail WHERE from_cid=1) RETURNING mail_id")
        .bind(utf16("Evie"))
        .bind(utf16("Fiona"))
        .fetch_one(p)
        .await
        .unwrap()
}
fn send(mail_type: u8, to: &str, gold: i32) -> wire::SendMail {
    wire::SendMail {
        mail_type,
        to_name: Some(to.to_owned()),
        title: Some("hello".to_owned()),
        content: Some("body".to_owned()),
        item_list: Some(Vec::new()),
        gold,
        express: false,
        charged_gold: 0,
    }
}
fn ids(base: i64) -> Vec<i64> {
    (0..16).map(|n| base + n).collect()
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn mail_sends_receives_returns_deletes_and_mails_a_full_inventory() {
    let (a, p, s) = isolated_game_pool().await;
    minimum_character::setup(&p).await;
    minimum_character::initialize(&p).await.unwrap();
    sqlx::query("UPDATE game_character_info SET name_utf16=$1 WHERE id=1")
        .bind(utf16("Fiona"))
        .execute(&p)
        .await
        .unwrap();
    recipient(&p).await;
    let assets = super::minimum_give::mail_assets();
    let owner = super::initial_inventory::key();
    // Under level 10 the frontend answers the dialog only.
    sqlx::query("UPDATE game_character_info SET level=9 WHERE id=1")
        .execute(&p)
        .await
        .unwrap();
    let out = minimum_mail::send_mail(&p, &owner, &assets, 1, &send(1, "Evie", 0), ids(900))
        .await
        .unwrap();
    assert!(
        out.packets
            .iter()
            .all(|p| p.category != Kind::MailSent.category())
    );
    sqlx::query("UPDATE game_character_info SET level=10 WHERE id=1")
        .execute(&p)
        .await
        .unwrap();
    assert_eq!(
        minimum_give::lab_grant(&p, &assets, Some(1), "synthetic_gold", 5000, 9)
            .await
            .unwrap(),
        Ok(1)
    );
    // Refusals: unknown name, self, a type that does not match the attachments.
    for (m, code) in [
        (send(1, "Nobody", 0), 8),
        (send(1, "fiona", 0), 12),
        (send(1, "Evie", 10), 9),
        (send(2, "Evie", 0), 9),
        (send(9, "Evie", 10), 9),
    ] {
        let out = minimum_mail::send_mail(&p, &owner, &assets, 1, &m, ids(900))
            .await
            .unwrap();
        assert_eq!(sent(&out), code);
    }
    assert_eq!(gold(&p).await, 5000);
    // Item mail with 1000 gold: the gold becomes a system item, the fee is 100 + 2% of the
    // gold's Price (0 in the fixture); a recipient name matches case-insensitively.
    let out = minimum_mail::send_mail(&p, &owner, &assets, 1, &send(2, "EVIE", 1000), ids(900))
        .await
        .unwrap();
    assert_eq!(sent(&out), -1);
    assert_eq!(out.notices.len(), 1);
    assert_eq!(out.notices[0].0, 2);
    assert_eq!(gold(&p).await, 3900);
    let (to, kind, posted, arrive, expire): (i64, i16, i64, i64, Option<i64>) = sqlx::query_as(
        "SELECT to_cid,mail_type,posted_units300,arrive_units300,expire_units300 FROM game_mail WHERE mail_id=$1",
    )
    .bind(out.notices[0].1)
    .fetch_one(&p)
    .await
    .unwrap();
    assert_eq!((to, kind), (2, 2));
    assert_eq!(arrive - posted, 60 * 60 * 300);
    assert_eq!(expire.map(|e| e - arrive), Some(10080 * 60 * 300));
    let system: (i64, i32, Option<i64>) = sqlx::query_as(
        "SELECT i.id,i.count,i.owner_id FROM game_mail_items m JOIN game_items i ON i.id=m.item_id WHERE m.mail_id=$1",
    )
    .bind(out.notices[0].1)
    .fetch_one(&p)
    .await
    .unwrap();
    assert_eq!((system.1, system.2), (1000, None));
    let journal: i64 =
        sqlx::query_scalar("SELECT count(*) FROM game_postal_journals WHERE mail_id=$1")
            .bind(out.notices[0].1)
            .fetch_one(&p)
            .await
            .unwrap();
    assert_eq!(journal, 1);
    // The sender sees it in the sent list; it has not arrived for anyone yet.
    let l = list(
        &minimum_mail::mail_list(&p, &owner, &assets, 1)
            .await
            .unwrap(),
    );
    assert_eq!(l.sent_mail_list.as_ref().map(Vec::len), Some(1));
    assert_eq!(l.received_mail_list.as_ref().map(Vec::len), Some(0));
    // Received: MailInfo reads it, GetMailItem gives the gold and destroys the system item.
    let mail = turn_around(&p).await;
    let l = list(
        &minimum_mail::mail_list(&p, &owner, &assets, 1)
            .await
            .unwrap(),
    );
    assert_eq!(l.new_mail_count, 1);
    let brief = l.received_mail_list.unwrap()[0].clone().unwrap();
    assert_eq!(
        (brief.mail_id, brief.item_attached, brief.is_read),
        (mail, 1, false)
    );
    // ToBriefMailInfo leaves the gold out of AttachedItemInfo.
    assert_eq!(brief.attached_item_info.map(|v| v.len()), Some(0));
    let info = minimum_mail::mail_info(&p, &owner, 1, mail).await.unwrap();
    let Message::MailInfo(info) = decode(&info, Kind::MailInfo) else {
        unreachable!()
    };
    assert_eq!(info.items.as_ref().map(Vec::len), Some(1));
    assert_eq!(info.from_name.as_deref(), Some("Evie"));
    let out = minimum_mail::get_mail_item(&p, &owner, &assets, 1, mail, ids(950))
        .await
        .unwrap();
    let Message::GetMailItemCompleted(done) = decode(&out, Kind::GetMailItemCompleted) else {
        unreachable!()
    };
    assert_eq!((done.mail_id, done.result), (mail, 0));
    assert!(out.packets[0].category != Kind::GetMailItemCompleted.category());
    assert_eq!(gold(&p).await, 4900);
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM game_items WHERE id=$1")
        .bind(system.0)
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(left, 0);
    let (read, received): (Option<bool>, bool) = sqlx::query_as(
        "SELECT m.is_read,i.received FROM game_mail m JOIN game_mail_items i ON i.mail_id=m.mail_id WHERE m.mail_id=$1",
    )
    .bind(mail)
    .fetch_one(&p)
    .await
    .unwrap();
    assert_eq!((read, received), (Some(true), true));
    // A second GetMailItem finds nothing to take.
    let again = minimum_mail::get_mail_item(&p, &owner, &assets, 1, mail, ids(970))
        .await
        .unwrap();
    let Message::GetMailItemCompleted(done) = decode(&again, Kind::GetMailItemCompleted) else {
        unreachable!()
    };
    assert_eq!(done.result, 1);
    assert_eq!(gold(&p).await, 4900);
    // ReturnMail: the attachment goes back to the sender as ReturnedMail.
    let out = minimum_mail::send_mail(&p, &owner, &assets, 1, &send(2, "Evie", 200), ids(1000))
        .await
        .unwrap();
    assert_eq!(sent(&out), -1);
    let mail = turn_around(&p).await;
    let item: i64 = sqlx::query_scalar("SELECT item_id FROM game_mail_items WHERE mail_id=$1")
        .bind(mail)
        .fetch_one(&p)
        .await
        .unwrap();
    let out = minimum_mail::return_mail_request(&p, &owner, &assets, 1, mail)
        .await
        .unwrap();
    assert_eq!(sent(&out), -2);
    assert_eq!(out.notices.len(), 1);
    let (to, kind, title): (i64, i16, Vec<u8>) =
        sqlx::query_as("SELECT to_cid,mail_type,title_utf16 FROM game_mail WHERE mail_id=$1")
            .bind(out.notices[0].1)
            .fetch_one(&p)
            .await
            .unwrap();
    assert_eq!((to, kind), (2, 5));
    assert_eq!(
        title,
        utf16("GameUI_Heroes_Server_MailFactory_MakeReturnMail_Title,hello")
    );
    let moved: i64 = sqlx::query_scalar("SELECT item_id FROM game_mail_items WHERE mail_id=$1")
        .bind(out.notices[0].1)
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(moved, item);
    let gone: i64 = sqlx::query_scalar("SELECT count(*) FROM game_mail WHERE mail_id=$1")
        .bind(mail)
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(gone, 0);
    // DeleteMail of a text mail marks it deleted; the list leaves it out.
    let out = minimum_mail::send_mail(&p, &owner, &assets, 1, &send(1, "Evie", 0), ids(1100))
        .await
        .unwrap();
    assert_eq!(sent(&out), -1);
    let mail = turn_around(&p).await;
    let out = minimum_mail::delete_mail(&p, &owner, &assets, 1, &[mail])
        .await
        .unwrap();
    let l = list(&out);
    assert!(
        l.received_mail_list
            .unwrap()
            .iter()
            .flatten()
            .all(|b| b.mail_id != mail)
    );
    let deleted: bool = sqlx::query_scalar("SELECT is_deleted FROM game_mail WHERE mail_id=$1")
        .bind(mail)
        .fetch_one(&p)
        .await
        .unwrap();
    assert!(deleted);
    // GiveItem ForceMail: what does not fit becomes DungeonItemMail from the system.
    let before: i64 = sqlx::query_scalar("SELECT count(*) FROM game_mail WHERE to_cid=1")
        .fetch_one(&p)
        .await
        .unwrap();
    let (slots, mails) =
        minimum_give::lab_grant_force_mail(&p, &assets, Some(1), "synthetic_bag", 200, 9)
            .await
            .unwrap()
            .unwrap();
    assert!(slots > 0 && mails > 0, "{slots} {mails}");
    let after: i64 = sqlx::query_scalar("SELECT count(*) FROM game_mail WHERE to_cid=1")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(after - before, mails as i64);
    let (from, kind, sender, title, arrive, posted): (i64, i16, Vec<u8>, Vec<u8>, i64, i64) = sqlx::query_as(
        "SELECT from_cid,mail_type,from_name_utf16,title_utf16,arrive_units300,posted_units300 FROM game_mail WHERE to_cid=1 ORDER BY mail_id DESC LIMIT 1",
    )
    .fetch_one(&p)
    .await
    .unwrap();
    assert_eq!((from, kind, arrive), (-1, 9, posted));
    assert_eq!(sender, utf16("Heroes_GameUI_MailSender_SYSTEM"));
    assert_eq!(
        title,
        utf16("#GameUI_Heroes_GiveFailMail_Title,synthetic_bag,1")
    );
    assert!(minimum_mail::take_notice(1));
    // The inventory is full: taking a mailed bag answers NoEmptySlot and changes nothing.
    let mailed: i64 =
        sqlx::query_scalar("SELECT max(mail_id) FROM game_mail WHERE to_cid=1 AND from_cid=-1")
            .fetch_one(&p)
            .await
            .unwrap();
    let out = minimum_mail::get_mail_item(&p, &owner, &assets, 1, mailed, ids(1200))
        .await
        .unwrap();
    let Message::GetMailItemCompleted(done) = decode(&out, Kind::GetMailItemCompleted) else {
        unreachable!()
    };
    assert_eq!(done.result, 2);
    let still: bool = sqlx::query_scalar("SELECT received FROM game_mail_items WHERE mail_id=$1")
        .bind(mailed)
        .fetch_one(&p)
        .await
        .unwrap();
    assert!(!still);
    // DeleteMail of a system mail (not Returnable): deleted, its system item destroyed.
    let system_item: i64 =
        sqlx::query_scalar("SELECT item_id FROM game_mail_items WHERE mail_id=$1")
            .bind(mailed)
            .fetch_one(&p)
            .await
            .unwrap();
    minimum_mail::delete_mail(&p, &owner, &assets, 1, &[mailed])
        .await
        .unwrap();
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM game_items WHERE id=$1")
        .bind(system_item)
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(left, 0);
    // RequiresDelete: a mail posted over 30 days ago with an expiry is cleansed by the list.
    sqlx::query("UPDATE game_mail SET posted_units300=posted_units300-31*86400*300 WHERE to_cid=1 AND from_cid=-1 AND mail_id<>$1")
        .bind(mailed)
        .execute(&p)
        .await
        .unwrap();
    let old: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM game_mail WHERE to_cid=1 AND from_cid=-1 AND mail_id<>$1",
    )
    .bind(mailed)
    .fetch_one(&p)
    .await
    .unwrap();
    assert!(old > 0);
    minimum_mail::mail_list(&p, &owner, &assets, 1)
        .await
        .unwrap();
    let old: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM game_mail WHERE to_cid=1 AND from_cid=-1 AND mail_id<>$1",
    )
    .bind(mailed)
    .fetch_one(&p)
    .await
    .unwrap();
    assert_eq!(old, 0);
    // ScheduleAllMail: the pending expiries of what is left.
    let times = minimum_mail::schedule_all(&p, &assets, 1).await.unwrap();
    assert!(!times.is_empty());
    cleanup_game_pool(a, p, s).await;
}
