use super::name_check_common as common;
use super::{cleanup_game_pool, isolated_game_pool};
use vindictus_server::{
    application::game_users::provision_game_user,
    domain::legacy_user_data::LegacyBytes,
    infrastructure::{
        character_lobby::UserKey,
        character_name_check::{self as storage, NameCheckError, NameCheckResult as R},
        character_name_proof::NameProof,
    },
};
fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
fn key() -> UserKey<'static> {
    UserKey {
        uid: 31,
        name: "Owner",
        revision: 0,
    }
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn name_check_queries_real_state_and_preserves_reserved_consumption_before_later_rejections()
{
    let (admin, pool, schema) = isolated_game_pool().await;
    let content = common::content();
    let proof = common::proof(&content);
    provision_game_user(&pool, 31, "Owner", 42, 0)
        .await
        .unwrap();
    assert_eq!(
        storage::check(&pool, key(), &proof, Some("Murael"))
            .await
            .unwrap(),
        R::Success
    );
    assert_eq!(
        storage::check(&pool, key(), &proof, Some("A!"))
            .await
            .unwrap(),
        R::InvalidName
    );
    assert!(matches!(
        storage::check(&pool, key(), &proof, None).await,
        Err(NameCheckError::Profile)
    ));
    assert!(matches!(
        storage::check(&pool, key(), &proof, Some("OtherName")).await,
        Err(NameCheckError::Profile)
    ));
    sqlx::query("INSERT INTO game_reserved_names(row_id,nexon_id_utf16,nexon_sn,character_name_utf16,created) VALUES(1,$1,-99,$2,false)").bind(utf16("Other")).bind(utf16("Murael")).execute(&pool).await.unwrap();
    assert_eq!(
        storage::check(&pool, key(), &proof, Some("Murael"))
            .await
            .unwrap(),
        R::ReservedName
    );
    // SQL case equality alone is insufficient: the original own-list Contains is ordinal.
    sqlx::query("UPDATE game_reserved_names SET nexon_id_utf16=$1,character_name_utf16=$2")
        .bind(utf16("OWNER "))
        .bind(utf16("MURAEL"))
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        storage::check(&pool, key(), &proof, Some("Murael"))
            .await
            .unwrap(),
        R::ReservedName
    );
    assert!(
        !sqlx::query_scalar::<_, bool>("SELECT created FROM game_reserved_names")
            .fetch_one(&pool)
            .await
            .unwrap()
    );
    sqlx::query("UPDATE game_reserved_names SET character_name_utf16=$1")
        .bind(utf16("Murael"))
        .execute(&pool)
        .await
        .unwrap();
    let mut bundle = vindictus_server::infrastructure::character_export::read_export(
        include_bytes!("../fixtures/character-bundle.json").as_slice(),
    )
    .unwrap();
    for row in &mut bundle.character_info {
        row.name = Some(LegacyBytes::from_bytes(utf16("OtherName")).unwrap());
    }
    let id = bundle.character_info[0].id;
    bundle.character_info[0].name = Some(LegacyBytes::from_bytes(utf16("Murael ")).unwrap());
    bundle.character_info[0].status = 0;
    vindictus_server::infrastructure::characters::import_bundle(&pool, &bundle, false)
        .await
        .unwrap();
    assert_eq!(
        storage::check(&pool, key(), &proof, Some("Murael"))
            .await
            .unwrap(),
        R::DuplicatedName
    );
    assert!(
        sqlx::query_scalar::<_, bool>("SELECT created FROM game_reserved_names")
            .fetch_one(&pool)
            .await
            .unwrap()
    );
    sqlx::query("UPDATE game_character_info SET status=1,delete_time_units300=NULL WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        storage::check(&pool, key(), &proof, Some("Murael"))
            .await
            .unwrap(),
        R::Success
    );
    for (days, expected) in [
        (29, R::DuplicatedName),
        (31, R::Success),
        (-1, R::DuplicatedName),
    ] {
        sqlx::query("UPDATE game_character_info SET delete_time_units300=floor((extract(epoch FROM clock_timestamp())+2208988800)*300+0.5)::BIGINT-$2*25920000 WHERE id=$1").bind(id).bind(i64::from(days)).execute(&pool).await.unwrap();
        assert_eq!(
            storage::check(&pool, key(), &proof, Some("Murael"))
                .await
                .unwrap(),
            expected
        );
    }
    sqlx::query("UPDATE game_character_info SET delete_time_units300=NULL WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO game_character_name_logs(id,old_character_name_utf16,new_character_name_utf16) VALUES(1,$1,$2)").bind(utf16("murael")).bind(utf16("OtherName")).execute(&pool).await.unwrap();
    assert_eq!(
        storage::check(&pool, key(), &proof, Some("Murael"))
            .await
            .unwrap(),
        R::ChangedCharacterName
    );
    sqlx::query(
        "UPDATE game_character_name_logs SET log_date_units300=log_date_units300-31*25920000",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        storage::check(&pool, key(), &proof, Some("Murael"))
            .await
            .unwrap(),
        R::Success
    );
    let mut forbidden = common::value(&content);
    forbidden["forbidden"] = serde_json::json!(true);
    let forbidden =
        NameProof::read(serde_json::to_vec(&forbidden).unwrap().as_slice(), &content).unwrap();
    sqlx::query("UPDATE game_reserved_names SET created=false")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        storage::check(&pool, key(), &forbidden, Some("Murael"))
            .await
            .unwrap(),
        R::ForbiddenName
    );
    assert!(
        !sqlx::query_scalar::<_, bool>("SELECT created FROM game_reserved_names")
            .fetch_one(&pool)
            .await
            .unwrap()
    );
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn name_check_refuses_wrong_owner_revision_unknown_values_and_cancelled_reservation_lock() {
    let (admin, pool, schema) = isolated_game_pool().await;
    let content = common::content();
    let proof = common::proof(&content);
    provision_game_user(&pool, 31, "Owner", 42, 0)
        .await
        .unwrap();
    for k in [
        UserKey { uid: 32, ..key() },
        UserKey {
            name: "Other",
            ..key()
        },
        UserKey {
            revision: 1,
            ..key()
        },
    ] {
        assert!(matches!(
            storage::check(&pool, k, &proof, Some("Murael")).await,
            Err(NameCheckError::Ownership)
        ));
    }
    sqlx::query("INSERT INTO game_reserved_names(row_id,nexon_id_utf16,nexon_sn,character_name_utf16,created) VALUES(1,$1,31,$2,false)").bind(utf16("Owner")).bind(utf16("Unmeasured")).execute(&pool).await.unwrap();
    assert!(matches!(
        storage::check(&pool, key(), &proof, Some("Murael")).await,
        Err(NameCheckError::Profile)
    ));
    sqlx::query("UPDATE game_reserved_names SET character_name_utf16=$1")
        .bind(utf16("Murael"))
        .execute(&pool)
        .await
        .unwrap();
    let mut held = pool.begin().await.unwrap();
    sqlx::query("SELECT row_id FROM game_reserved_names FOR UPDATE")
        .execute(&mut *held)
        .await
        .unwrap();
    assert!(
        tokio::time::timeout(
            std::time::Duration::from_millis(75),
            storage::check(&pool, key(), &proof, Some("Murael"))
        )
        .await
        .is_err()
    );
    held.rollback().await.unwrap();
    assert!(
        !sqlx::query_scalar::<_, bool>("SELECT created FROM game_reserved_names")
            .fetch_one(&pool)
            .await
            .unwrap()
    );
    cleanup_game_pool(admin, pool, schema).await;
}
