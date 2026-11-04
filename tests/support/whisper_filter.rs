use super::{cleanup_game_pool, isolated_game_pool};
use vindictus_server::{
    domain::map_partition::OutgoingPacket,
    infrastructure::minimum_whisper::{self as whispers, update_type},
    protocol::{
        DEFAULT_MAX_BODY,
        frontend_control::{self as ctl, Kind as ControlKind, Message as ControlMessage},
        whisper::{self as wire, Kind, Message, UpdateFilter},
    },
};

fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
async fn character(p: &sqlx::PgPool, id: i64, uid: i32, name: &str, status: i32) {
    sqlx::query("INSERT INTO game_characters(id) VALUES($1)")
        .bind(id)
        .execute(p)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO game_character_info(id,uid,character_sn,class,level,exp,str,dex,int,will,luck,hp,stamina,dirty,play_time,ap,total_used_ap,last_ap_update_units300,permission,status,region_code,is_connected,create_time_ticks,free_match_win_count,free_match_lose_count,use_preset,is_premium,map_state_info,name_utf16) VALUES($1,$2,$1,0,1,0,0,0,0,0,0,0,0,false,0,0,0,0,0,$3,0,false,0,0,0,false,false,0,$4)",
    )
    .bind(id)
    .bind(uid)
    .bind(status)
    .bind(utf16(name))
    .execute(p)
    .await
    .unwrap();
}
fn update(op: i32, target: Option<&str>) -> UpdateFilter {
    UpdateFilter {
        operation_type: op,
        target_id: target.map(str::to_owned),
        level: 99,
    }
}
/// The SystemMessage text key and parameters of a packet.
fn system(p: &OutgoingPacket) -> (String, Vec<String>) {
    assert_eq!(p.category, ControlKind::System.category());
    match ctl::decode(ControlKind::System, &p.body, DEFAULT_MAX_BODY)
        .unwrap()
        .unwrap()
    {
        ControlMessage::System { category, text } => {
            assert_eq!(category, 0);
            let text = text.unwrap();
            (
                text.format.unwrap().to_owned(),
                text.parameters
                    .unwrap_or_default()
                    .into_iter()
                    .map(|v| v.unwrap_or_default().to_owned())
                    .collect(),
            )
        }
        _ => unreachable!(),
    }
}
fn list(p: &OutgoingPacket) -> Vec<(String, i32)> {
    assert_eq!(p.category, Kind::FilterList.category());
    match wire::decode(Kind::FilterList, &p.body, DEFAULT_MAX_BODY)
        .unwrap()
        .unwrap()
    {
        Message::FilterList(d) => d.0.unwrap(),
        _ => unreachable!(),
    }
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn whisper_filter_adds_removes_and_lists_blocked_characters() {
    let (admin, p, schema) = isolated_game_pool().await;
    character(&p, 1, 10, "Alice", 0).await;
    character(&p, 2, 20, "Bob", 0).await;
    character(&p, 3, 10, "AliceAlt", 0).await;
    character(&p, 4, 30, "Ghost", 1).await;
    character(&p, 5, 40, "Carol", 0).await;
    let first = whispers::filter_list_message(&p, 1).await.unwrap();
    assert!(list(&first).is_empty());

    // Unknown, null and deleted targets: Invalid_Character_Name, nothing stored.
    for target in [Some("Nobody"), None, Some("Ghost")] {
        let out = whispers::update_filter(&p, 1, 10, &update(update_type::ADD, target))
            .await
            .unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(system(&out[0]).0, whispers::INVALID_NAME);
    }
    // Any character of the own account is refused.
    let out = whispers::update_filter(&p, 1, 10, &update(update_type::ADD, Some("alicealt")))
        .await
        .unwrap();
    assert_eq!(system(&out[0]).0, whispers::CANNOT_FILTER_YOURSELF);

    // Add (case-insensitive name; the level is always 1): Success_Add with the typed
    // name, then the list.
    let out = whispers::update_filter(&p, 1, 10, &update(update_type::ADD, Some("bob")))
        .await
        .unwrap();
    assert_eq!(out.len(), 2);
    assert_eq!(
        system(&out[0]),
        (whispers::SUCCESS_ADD.to_owned(), vec!["bob".to_owned()])
    );
    assert_eq!(list(&out[1]), [("Bob".to_owned(), 1)]);
    // A second add of the same pair fails.
    let out = whispers::update_filter(&p, 1, 10, &update(update_type::ADD, Some("Bob")))
        .await
        .unwrap();
    assert_eq!(out.len(), 1);
    assert_eq!(system(&out[0]).0, whispers::CANNOT_ADD);
    let out = whispers::update_filter(&p, 1, 10, &update(update_type::ADD, Some("Carol")))
        .await
        .unwrap();
    assert_eq!(
        list(&out[1]),
        [("Bob".to_owned(), 1), ("Carol".to_owned(), 1)]
    );
    // Update (and any other type) fails without a message.
    for op in [update_type::UPDATE, 7] {
        let out = whispers::update_filter(&p, 1, 10, &update(op, Some("Bob")))
            .await
            .unwrap();
        assert!(out.is_empty());
    }
    // The list is per character and survives a new login.
    let relogin = whispers::filter_list_message(&p, 1).await.unwrap();
    assert_eq!(list(&relogin).len(), 2);
    assert!(list(&whispers::filter_list_message(&p, 2).await.unwrap()).is_empty());

    // Remove: Success_Remove and the list; a second remove fails.
    let out = whispers::update_filter(&p, 1, 10, &update(update_type::REMOVE, Some("Bob")))
        .await
        .unwrap();
    assert_eq!(
        system(&out[0]),
        (whispers::SUCCESS_REMOVE.to_owned(), vec!["Bob".to_owned()])
    );
    assert_eq!(list(&out[1]), [("Carol".to_owned(), 1)]);
    let out = whispers::update_filter(&p, 1, 10, &update(update_type::REMOVE, Some("Bob")))
        .await
        .unwrap();
    assert_eq!(out.len(), 1);
    assert_eq!(system(&out[0]).0, whispers::CANNOT_REMOVE);

    // A blocked character deleted later stays in the list (the LINQ join has no status
    // filter) and can no longer be removed by name.
    sqlx::query("UPDATE game_character_info SET status=1 WHERE id=5")
        .execute(&p)
        .await
        .unwrap();
    let after = whispers::filter_list_message(&p, 1).await.unwrap();
    assert_eq!(list(&after), [("Carol".to_owned(), 1)]);
    cleanup_game_pool(admin, p, schema).await;
}
