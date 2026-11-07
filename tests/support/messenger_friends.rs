use super::{cleanup_game_pool, isolated_game_pool};
use vindictus_server::infrastructure::messenger_friends::{self as db, Requested, State};

#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn messenger_friends_request_confirm_block_and_categories() {
    let (a, p, s) = isolated_game_pool().await;
    sqlx::query("INSERT INTO game_users(id,name,next_character_sn) VALUES(11,'alpha',0),(12,'Beta',0),(13,'gamma',0)")
        .execute(&p)
        .await
        .unwrap();
    assert_eq!(db::find(&p, "BETA").await.unwrap(), Some(12));
    assert_eq!(db::find(&p, "nobody").await.unwrap(), None);
    let beta = db::profile(&p, 12).await.unwrap().unwrap();
    assert_eq!((beta.login.as_str(), beta.character.as_str()), ("Beta", ""));

    // Request, refusal of a repeat, acceptance.
    assert_eq!(db::request(&p, 11, 12).await.unwrap(), Requested::Sent);
    assert!(db::request(&p, 11, 12).await.is_err());
    assert!(db::request(&p, 11, 11).await.is_err());
    assert_eq!(db::state(&p, 12, 11).await.unwrap(), Some(State::Waiting));
    assert!(db::confirm(&p, 11, 12, true).await.is_err());
    db::confirm(&p, 12, 11, true).await.unwrap();
    assert_eq!(db::state(&p, 11, 12).await.unwrap(), Some(State::Friend));
    assert_eq!(db::friends(&p, 12).await.unwrap()[0].profile.login, "alpha");

    // A refused request leaves nothing; crossing requests accept.
    db::request(&p, 13, 11).await.unwrap();
    db::confirm(&p, 11, 13, false).await.unwrap();
    assert_eq!(db::state(&p, 13, 11).await.unwrap(), None);
    db::request(&p, 13, 12).await.unwrap();
    assert_eq!(db::request(&p, 12, 13).await.unwrap(), Requested::Accepted);
    assert_eq!(db::state(&p, 13, 12).await.unwrap(), Some(State::Friend));

    // Block: only the blocker's row; the blocked cannot ask again.
    db::block(&p, 12, 13, true).await.unwrap();
    assert_eq!(db::state(&p, 12, 13).await.unwrap(), Some(State::Blocked));
    assert_eq!(db::state(&p, 13, 12).await.unwrap(), None);
    assert!(db::request(&p, 13, 12).await.is_err());
    db::block(&p, 12, 13, false).await.unwrap();
    assert_eq!(db::state(&p, 12, 13).await.unwrap(), None);

    // Categories and moves.
    let c = db::add_category(&p, 11, "  Guilda  ").await.unwrap();
    assert_eq!(c, 1);
    assert!(db::add_category(&p, 11, "   ").await.is_err());
    db::rename_category(&p, 11, c, "Raid").await.unwrap();
    db::move_friend(&p, 11, 12, c).await.unwrap();
    assert!(db::move_friend(&p, 11, 12, 9).await.is_err());
    assert_eq!(db::friends(&p, 11).await.unwrap()[0].category_id, c);
    db::delete_category(&p, 11, c).await.unwrap();
    assert_eq!(db::categories(&p, 11).await.unwrap(), vec![]);
    assert_eq!(db::friends(&p, 11).await.unwrap()[0].category_id, 0);
    assert!(db::remove(&p, 11, 12).await.unwrap());
    assert_eq!(db::state(&p, 12, 11).await.unwrap(), None);
    cleanup_game_pool(a, p, s).await;
}
