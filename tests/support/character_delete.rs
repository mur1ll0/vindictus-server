use super::{cleanup_game_pool, isolated_game_pool, minimum_character as fixture};
use sqlx::PgPool;
use vindictus_server::{
    application::creation_lifecycle::{MinimumOutcome, MinimumRequest},
    domain::{character_stats::Arithmetic, legacy_user_data::SqlDateTime2},
    infrastructure::{
        character_lobby::{QueryConfig, UserKey},
        minimum_character::MinimumError,
        minimum_character_delete as delete, minimum_character_list as list, minimum_selection,
    },
    protocol::{
        DEFAULT_MAX_BODY,
        character_delete::{CancelResult, DeleteResult},
        character_list,
    },
};
const OWNER: &str = "LifecycleOwner";
const DAY: i64 = 86_400 * 10_000_000;
fn config() -> QueryConfig {
    QueryConfig {
        max_free: 5,
        care_clock: None,
    }
}
fn at(ticks: i64) -> impl FnMut() -> Result<SqlDateTime2, MinimumError> {
    move || Ok(SqlDateTime2::from_ticks(ticks).unwrap())
}
const T0: i64 = 638448480000000000;
/// Two characters of one lab account created by the native lifecycle: (owner uid, revision,
/// [(cid, CharacterSN)] in creation order).
async fn two_characters(p: &PgPool) -> (i32, i64, Vec<(i64, i32)>) {
    fixture::setup(p).await;
    sqlx::raw_sql("DELETE FROM game_characters;DELETE FROM game_my_settings")
        .execute(p)
        .await
        .unwrap();
    let (session, revision, uid) = super::creation_lifecycle::setup(p).await;
    let runtime = super::creation_lifecycle::runtime(p, &super::creation_lifecycle::package());
    let mut created = Vec::new();
    for name in ["Murael", "Lucanor"] {
        let outcome = runtime
            .run_minimum_admitted(
                &session,
                MinimumRequest {
                    revision,
                    name,
                    assets: fixture::assets(),
                    body: fixture::body_named(name),
                    offset: 0,
                    config: config(),
                    delete_delay: 1440,
                },
            )
            .await
            .unwrap();
        let MinimumOutcome::Created(c) = outcome else {
            panic!("create failed")
        };
        let sn: i32 =
            sqlx::query_scalar("SELECT character_sn FROM game_character_info WHERE id=$1")
                .bind(c.cid)
                .fetch_one(p)
                .await
                .unwrap();
        created.push((c.cid, sn));
    }
    runtime.shutdown().await.unwrap();
    (uid, revision, created)
}
/// The list as the client receives it: (cid, name, IsDeleting, DeleteWaitLeftSec) in row order.
async fn rows(
    p: &PgPool,
    owner: &UserKey<'_>,
    delay: i32,
    now: i64,
) -> Vec<(i64, String, bool, i32)> {
    let assets = fixture::assets();
    let body = list::query(
        p,
        owner,
        assets.initializers(),
        None,
        config(),
        delay,
        at(now),
    )
    .await
    .unwrap();
    character_list::decode(&body, DEFAULT_MAX_BODY)
        .unwrap()
        .unwrap()
        .characters
        .unwrap()
        .into_iter()
        .map(|c| {
            let c = c.unwrap();
            (
                c.cid,
                c.character_id.unwrap().to_owned(),
                c.is_deleting,
                c.delete_wait_left_sec,
            )
        })
        .collect()
}
async fn status(p: &PgPool, cid: i64) -> (i32, Option<i64>) {
    sqlx::query_as("SELECT status,delete_time_units300 FROM game_character_info WHERE id=$1")
        .bind(cid)
        .fetch_one(p)
        .await
        .unwrap()
}
async fn ledger(p: &PgPool) -> Vec<(String, i32)> {
    let rows: Vec<(Vec<u8>, i32)> = sqlx::query_as("SELECT target_utf16,value FROM (SELECT journal_id,event_utf16,target_utf16,value FROM game_character_ledger01 UNION ALL SELECT journal_id,event_utf16,target_utf16,value FROM game_character_ledger02) l WHERE event_utf16=$1 ORDER BY journal_id")
        .bind("DeleteCharacter".encode_utf16().flat_map(u16::to_le_bytes).collect::<Vec<_>>())
        .fetch_all(p)
        .await
        .unwrap();
    rows.into_iter()
        .map(|(t, v)| {
            let units = t
                .chunks(2)
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .collect::<Vec<_>>();
            (String::from_utf16(&units).unwrap(), v)
        })
        .collect()
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn character_delete_with_delay_waits_cancels_and_deletes_after_the_term() {
    let (a, p, s) = isolated_game_pool().await;
    let (uid, revision, created) = two_characters(&p).await;
    let owner = UserKey {
        uid,
        name: OWNER,
        revision,
    };
    let [(murael, murael_sn), (lucanor, _)] = created[..] else {
        panic!("two characters")
    };
    assert_eq!(
        delete::listed_named(&p, &owner, Some("Murael"))
            .await
            .unwrap(),
        Some(murael)
    );
    assert_eq!(
        delete::listed_named(&p, &owner, Some("murael"))
            .await
            .unwrap(),
        None
    );
    assert!(!delete::remain_cash_items(&p, &owner, murael).await.unwrap());
    // Unknown name or CharacterSN: nothing is journaled or changed.
    for (name, sn) in [
        (Some("Nobody"), murael_sn),
        (Some("Murael"), murael_sn + 100),
        (None, murael_sn),
    ] {
        assert_eq!(
            delete::delete(&p, &owner, name, sn, 1440, at(T0)).await,
            (DeleteResult::FailNoSuchCharacter, 0)
        );
        assert_eq!(
            delete::cancel(&p, &owner, name, sn).await,
            CancelResult::FailNoSuchCharacter
        );
    }
    // A cancel of a character that is not being deleted.
    assert_eq!(
        delete::cancel(&p, &owner, Some("Murael"), murael_sn).await,
        CancelResult::FailInvalidStatus
    );
    // 0 → 2 with the delay; the list keeps it with its remaining time.
    assert_eq!(
        delete::delete(&p, &owner, Some("Murael"), murael_sn, 1440, at(T0)).await,
        (DeleteResult::SuccessDeleting, 0)
    );
    assert_eq!(status(&p, murael).await.0, 2);
    assert!(status(&p, murael).await.1.is_some());
    let listed = rows(&p, &owner, 1440, T0 + 60 * 10_000_000).await;
    assert_eq!(
        listed,
        vec![
            (murael, "Murael".to_owned(), true, 86_340),
            (lucanor, "Lucanor".to_owned(), false, 86_400),
        ]
    );
    // A character waiting for deletion is not selectable.
    assert!(
        minimum_selection::select(
            &p,
            &owner,
            &fixture::assets(),
            0,
            config(),
            Arithmetic::FrameworkX64
        )
        .await
        .is_err()
    );
    // Before the term: Fail_NeedMoreWaiting with the remaining seconds, nothing changed.
    assert_eq!(
        delete::delete(
            &p,
            &owner,
            Some("Murael"),
            murael_sn,
            1440,
            at(T0 + 3_600 * 10_000_000)
        )
        .await,
        (DeleteResult::FailNeedMoreWaiting, 82_800)
    );
    assert_eq!(status(&p, murael).await.0, 2);
    // Cancel: 2 → 0, DeleteTime null.
    assert_eq!(
        delete::cancel(&p, &owner, Some("Murael"), murael_sn).await,
        CancelResult::Success
    );
    assert_eq!(status(&p, murael).await, (0, None));
    // Delete again and complete it after the term: 2 → 1, the list no longer has it.
    assert_eq!(
        delete::delete(&p, &owner, Some("Murael"), murael_sn, 1440, at(T0)).await,
        (DeleteResult::SuccessDeleting, 0)
    );
    assert_eq!(
        delete::delete(&p, &owner, Some("Murael"), murael_sn, 1440, at(T0 + DAY)).await,
        (DeleteResult::SuccessDeleted, 0)
    );
    assert_eq!(status(&p, murael).await.0, 1);
    assert_eq!(
        rows(&p, &owner, 1440, T0 + DAY).await,
        vec![(lucanor, "Lucanor".to_owned(), false, 86_400)]
    );
    assert_eq!(
        delete::listed_named(&p, &owner, Some("Murael"))
            .await
            .unwrap(),
        None
    );
    // A deleted row is found by name and CharacterSN but its status is invalid.
    assert_eq!(
        delete::delete(&p, &owner, Some("Murael"), murael_sn, 1440, at(T0 + DAY)).await,
        (DeleteResult::FailInvalidStatus, 0)
    );
    assert_eq!(
        ledger(&p).await,
        vec![
            ("Deleting".to_owned(), 2),
            ("DeleteCancel".to_owned(), 0),
            ("Deleting".to_owned(), 2),
            ("Deleted".to_owned(), 1),
        ]
    );
    cleanup_game_pool(a, p, s).await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn character_delete_without_delay_cash_items_and_slot_order() {
    let (a, p, s) = isolated_game_pool().await;
    let (uid, revision, created) = two_characters(&p).await;
    let owner = UserKey {
        uid,
        name: OWNER,
        revision,
    };
    let [(murael, murael_sn), (lucanor, lucanor_sn)] = created[..] else {
        panic!("two characters")
    };
    // SelectOrdersChange: OrderNum per character in list order; the list and the selection
    // index follow it.
    assert!(delete::select_orders(&p, &owner, &[1, 0]).await.unwrap());
    let orders: Vec<(i64, Option<i16>)> =
        sqlx::query_as("SELECT id,order_num FROM game_character_info WHERE uid=$1 ORDER BY id")
            .bind(uid)
            .fetch_all(&p)
            .await
            .unwrap();
    assert_eq!(orders, vec![(murael, Some(1)), (lucanor, Some(0))]);
    let names = |r: Vec<(i64, String, bool, i32)>| r.into_iter().map(|r| r.0).collect::<Vec<_>>();
    assert_eq!(names(rows(&p, &owner, 0, T0).await), vec![lucanor, murael]);
    // A short collection writes nothing.
    assert!(!delete::select_orders(&p, &owner, &[0]).await.unwrap());
    assert!(delete::select_orders(&p, &owner, &[0, 1, 9]).await.unwrap());
    assert_eq!(names(rows(&p, &owner, 0, T0).await), vec![lucanor, murael]);
    let selected = minimum_selection::select(
        &p,
        &owner,
        &fixture::assets(),
        1,
        config(),
        Arithmetic::FrameworkX64,
    )
    .await
    .unwrap();
    assert_eq!(selected.cid, murael);
    // Open cash orders of the character other than the adventure/grand packages.
    let order = |class: &str| {
        sqlx::query("INSERT INTO game_cash_orders(uid,cid,product_no,item_class_utf16,pieces,expire_days,remain_quantity,price) VALUES($1,$2,1,$3,1,0,1,0)")
            .bind(uid)
            .bind(lucanor)
            .bind(class.encode_utf16().flat_map(u16::to_le_bytes).collect::<Vec<_>>())
    };
    order("package_grand").execute(&p).await.unwrap();
    assert!(
        !delete::remain_cash_items(&p, &owner, lucanor)
            .await
            .unwrap()
    );
    order("hair_dye").execute(&p).await.unwrap();
    assert!(
        delete::remain_cash_items(&p, &owner, lucanor)
            .await
            .unwrap()
    );
    assert!(!delete::remain_cash_items(&p, &owner, murael).await.unwrap());
    // DeleteCharacterDelay 0: 0 → 1 at once.
    assert_eq!(
        delete::delete(&p, &owner, Some("Lucanor"), lucanor_sn, 0, at(T0)).await,
        (DeleteResult::SuccessDeleted, 0)
    );
    assert_eq!(status(&p, lucanor).await.0, 1);
    assert_eq!(
        delete::delete(&p, &owner, Some("Lucanor"), lucanor_sn, 0, at(T0)).await,
        (DeleteResult::FailInvalidStatus, 0)
    );
    // The last character deleted: the list is empty, not refused.
    sqlx::query("UPDATE game_character_info SET is_connected=false WHERE id=$1")
        .bind(murael)
        .execute(&p)
        .await
        .unwrap();
    assert_eq!(
        delete::delete(&p, &owner, Some("Murael"), murael_sn, 0, at(T0)).await,
        (DeleteResult::SuccessDeleted, 0)
    );
    assert!(rows(&p, &owner, 0, T0).await.is_empty());
    cleanup_game_pool(a, p, s).await;
}
