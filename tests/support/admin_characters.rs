//! Panel character views (B4.3a) against PostgreSQL: the owner and staff read the sheet with
//! gold, items (English name, attributes, equipped part), skills, effects and titles; another
//! account cannot. Part of the postgres test target; needs TEST_DATABASE_URL.
use super::admin_panel::isolated_pool;
use vindictus_server::{
    application::{
        accounts::create_account,
        admin_characters,
        admin_session::{AdminError, PanelAuth, set_role_from_cli},
    },
    domain::{Username, admin::PanelRole},
    infrastructure::admin_characters::EnglishNames,
};
use zeroize::Zeroizing;

const PASSWORD: &str = "senha de teste do painel";

fn utf16(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn character_sheet_for_owner_and_staff_only() {
    let (admin_pool, pool, schema) = isolated_pool().await;
    for name in ["Dono", "Outro", "Equipe"] {
        create_account(
            &pool,
            Username::parse(name).expect("name"),
            Zeroizing::new(PASSWORD.as_bytes().to_vec()),
        )
        .await
        .expect("account");
    }
    set_role_from_cli(&pool, "equipe", Some(PanelRole::Leitura), "leitura")
        .await
        .expect("role");
    sqlx::raw_sql(
        "INSERT INTO game_users(id,name,next_character_sn) VALUES(501,'Dono',1);
         INSERT INTO game_characters(id) VALUES(9001);
         INSERT INTO game_character_info(id,uid,character_sn,class,level,exp,str,dex,int,will,luck,hp,stamina,dirty,play_time,ap,total_used_ap,last_ap_update_units300,permission,status,region_code,is_connected,create_time_ticks,free_match_win_count,free_match_lose_count,use_preset,is_premium,map_state_info,selected_title)
           VALUES(9001,501,0,1,20,12345,10,11,12,13,14,300,100,false,7200,5,40,0,0,0,0,false,0,0,0,false,false,0,410001);
         INSERT INTO game_item_collections(owner_id) VALUES(9001);
         INSERT INTO game_equippables(id,part,color1,color2,color3,reduced_durability,max_durability_bonus) VALUES(71,1,0,0,0,3,0);
         INSERT INTO game_titles(cid,title_id,acquired) VALUES(9001,410001,true),(9001,410002,false);",
    )
    .execute(&pool)
    .await
    .expect("character");
    for (id, class, count, collection, slot) in [
        (70i64, "gold", 1234, 101i16, -1),
        (71, "longsword", 1, 0, 0),
    ] {
        sqlx::query("INSERT INTO game_items(id,item_class_utf16,owner_id,count,collection,slot,tradable) VALUES($1,$2,9001,$3,$4,$5,true)")
            .bind(id).bind(utf16(class)).bind(count).bind(collection).bind(slot)
            .execute(&pool).await.expect("item");
    }
    sqlx::query("INSERT INTO game_item_attributes(id,item_id,attribute_bytes,value_bytes,arg,arg2) VALUES(1,71,$1,$2,0,0)")
        .bind(b"ENHANCE".to_vec()).bind(b"6".to_vec())
        .execute(&pool).await.expect("attribute");
    sqlx::query(
        "INSERT INTO game_skills(row_id,cid,skill_utf16,rank,locked) VALUES(1,9001,$1,3,0)",
    )
    .bind(utf16("skill_fiona_guard"))
    .execute(&pool)
    .await
    .expect("skill");
    sqlx::query("INSERT INTO game_status_effects(row_id,cid,type_utf16,level) VALUES(1,9001,$1,1)")
        .bind(utf16("account_homecafe_bonus"))
        .execute(&pool)
        .await
        .expect("effect");
    sqlx::query("UPDATE game_character_info SET name_utf16=$1 WHERE id=9001")
        .bind(utf16("Murael"))
        .execute(&pool)
        .await
        .expect("name");

    let names = EnglishNames::read(
        br#"{"items":{"LONGSWORD":["Long Sword","A sword."]},"skills":{"SKILL_FIONA_GUARD":"Guard"},"status_effects":{}}"#.as_slice(),
    )
    .expect("names");
    let peer = None;
    let auth = PanelAuth::new(pool.clone());
    let login = |name: &'static str| {
        let auth = auth.clone();
        async move {
            auth.login(name, Zeroizing::new(PASSWORD.as_bytes().to_vec()), peer)
                .await
                .expect("login")
                .session
        }
    };
    let owner = login("dono").await;
    let other = login("outro").await;
    let staff = login("equipe").await;

    let mine = admin_characters::mine(&pool, &owner).await.expect("mine");
    assert_eq!(
        (mine.len(), mine[0].name.as_str(), mine[0].cid.as_str()),
        (1, "Murael", "9001")
    );
    assert!(
        admin_characters::mine(&pool, &other)
            .await
            .expect("other")
            .is_empty()
    );

    let sheet = admin_characters::sheet(&pool, &names, None, &owner, "9001")
        .await
        .expect("sheet");
    assert_eq!(
        (sheet.gold, sheet.owner.as_str(), sheet.stats.str),
        (1234, "Dono", 10)
    );
    let sword = sheet
        .items
        .iter()
        .find(|i| i.class == "longsword")
        .expect("sword");
    assert_eq!(sword.name.as_deref(), Some("Long Sword"));
    assert_eq!(
        (sword.equipped_part, sword.durability_lost),
        (Some(1), Some(3))
    );
    assert_eq!(
        (
            sword.attributes[0].name.as_str(),
            sword.attributes[0].value.as_str()
        ),
        ("ENHANCE", "6")
    );
    assert_eq!(sheet.skills[0].name.as_deref(), Some("Guard"));
    assert_eq!(sheet.status_effects[0].kind, "account_homecafe_bonus");
    assert!(
        sheet
            .titles
            .iter()
            .any(|t| t.title_id == 410001 && t.selected && t.acquired)
    );

    // Another account cannot read it, nor list other accounts; staff can.
    assert!(matches!(
        admin_characters::sheet(&pool, &names, None, &other, "9001").await,
        Err(AdminError::NotFound)
    ));
    assert!(matches!(
        admin_characters::of_account(&pool, &other, "dono").await,
        Err(AdminError::Forbidden)
    ));
    assert_eq!(
        admin_characters::of_account(&pool, &staff, "dono")
            .await
            .expect("staff list")
            .len(),
        1
    );
    admin_characters::sheet(&pool, &names, None, &staff, "9001")
        .await
        .expect("staff sheet");
    assert!(matches!(
        admin_characters::sheet(&pool, &names, None, &staff, "x").await,
        Err(AdminError::NotFound)
    ));

    // Changes (B4.3b): reading roles and players are refused; a GM passes the permission check
    // (no item content loaded here), but not on a character whose account has a panel role.
    use vindictus_server::application::admin_character_edits::{Change, change_item};
    let edit = |session| Change {
        session,
        peer: None,
        operation_id: uuid::Uuid::new_v4(),
        reason: "teste",
    };
    for session in [&owner, &staff] {
        assert!(matches!(
            change_item(&pool, None, &edit(session), "9001", "gold", 1, true).await,
            Err(AdminError::Forbidden)
        ));
    }
    set_role_from_cli(&pool, "equipe", Some(PanelRole::Gm), "gm")
        .await
        .expect("gm");
    let gm = login("equipe").await;
    assert!(matches!(
        change_item(&pool, None, &edit(&gm), "9001", "gold", 1, true).await,
        Err(AdminError::ContentUnavailable)
    ));
    set_role_from_cli(&pool, "dono", Some(PanelRole::Leitura), "leitura")
        .await
        .expect("role");
    assert!(matches!(
        change_item(&pool, None, &edit(&gm), "9001", "gold", 1, true).await,
        Err(AdminError::Forbidden)
    ));

    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&admin_pool)
        .await
        .expect("cleanup");
    admin_pool.close().await;
}
