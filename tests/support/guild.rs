use super::{cleanup_game_pool, isolated_game_pool};
use vindictus_server::{
    domain::{guild::open_error, map_partition::OutgoingPacket},
    infrastructure::{
        guild::{self as guilds, GuildHub, State},
        party_hub::{Notice, PartyHub},
    },
    protocol::{
        DEFAULT_MAX_BODY,
        guild::{self as wire, GuildInfo, Kind, Message, QueryGuildList, rank, result},
    },
};

fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
async fn character(p: &sqlx::PgPool, id: i64, uid: i32, name: &str, level: i32) {
    sqlx::query("INSERT INTO game_characters(id) VALUES($1)")
        .bind(id)
        .execute(p)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO game_character_info(id,uid,character_sn,class,level,exp,str,dex,int,will,luck,hp,stamina,dirty,play_time,ap,total_used_ap,last_ap_update_units300,permission,status,region_code,is_connected,create_time_ticks,free_match_win_count,free_match_lose_count,use_preset,is_premium,map_state_info,name_utf16) VALUES($1,$2,$1,0,$3,0,0,0,0,0,0,0,0,false,0,0,0,0,0,0,0,false,0,0,0,false,false,0,$4)",
    )
    .bind(id)
    .bind(uid)
    .bind(level)
    .bind(utf16(name))
    .execute(p)
    .await
    .unwrap();
}
fn decode(p: &OutgoingPacket) -> Message {
    let kind = [
        Kind::GuildResult,
        Kind::GuildInfo,
        Kind::GuildMemberList,
        Kind::GuildList,
    ]
    .into_iter()
    .find(|k| k.category() == p.category)
    .expect("guild packet");
    wire::decode(kind, &p.body, DEFAULT_MAX_BODY)
        .unwrap()
        .unwrap()
}
fn result_of(p: &OutgoingPacket) -> (i32, String, i64) {
    match decode(p) {
        Message::GuildResult(r) => (r.result, r.arg.unwrap_or_default(), r.guild_id),
        _ => panic!("not a GuildResult"),
    }
}
fn info_of(p: &OutgoingPacket) -> Option<GuildInfo> {
    match decode(p) {
        Message::GuildInfo(i) => i,
        _ => panic!("not a GuildInfo"),
    }
}
fn search(key: &str, query_type: i32, page: i32, size: u8) -> QueryGuildList {
    QueryGuildList {
        query_type,
        search_key: Some(key.to_owned()),
        page,
        page_size: size,
    }
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn guild_open_connect_search_and_presence() {
    let (admin, p, schema) = isolated_game_pool().await;
    character(&p, 1, 10, "Alice", 25).await;
    character(&p, 2, 20, "Bob", 30).await;
    character(&p, 3, 30, "Carol", 40).await;
    let party = PartyHub::new(false, false);
    let hub = GuildHub::default();

    // OutGuild on entry: ConnectSuccess with GuildID 0, then a null GuildInfo.
    let (state, out) = guilds::enter(&p, &party, &hub, 1).await;
    assert_eq!(state, State::OutGuild);
    assert_eq!(out.len(), 2);
    assert_eq!(
        result_of(&out[0]),
        (result::CONNECT_SUCCESS, String::new(), 0)
    );
    assert_eq!(info_of(&out[1]), None);

    // OpenGuild validation order: ID, then name, then the duplicates.
    for (name, id, err) in [
        ("Night_Owls", "", open_error::EMPTY_ID),
        ("abc", "abc", open_error::NAMING_RULE),
        (
            &"A".repeat(21) as &str,
            "long",
            open_error::NAMING_RULE_MAX_BYTES,
        ),
    ] {
        assert_eq!(guilds::open(&p, 1, name, id, None).await.unwrap(), Err(err));
    }
    let sn = guilds::open(&p, 1, "Night_Owls", "nightowls", Some("Olá"))
        .await
        .unwrap()
        .unwrap();
    assert!(sn > 0);
    assert_eq!(
        guilds::open(&p, 2, "night_owls", "other", None)
            .await
            .unwrap(),
        Err(open_error::DUPLICATED_NAME)
    );
    assert_eq!(
        guilds::open(&p, 2, "Day_Larks", "NIGHTOWLS", None)
            .await
            .unwrap(),
        Err(open_error::DUPLICATED_ID)
    );
    // The master already has a GuildUser row.
    assert_eq!(
        guilds::open(&p, 1, "Day_Larks", "daylarks", None)
            .await
            .unwrap(),
        Err(open_error::REPEATED_CHARACTERS)
    );
    assert_eq!(
        guilds::membership(&p, 1).await.unwrap(),
        Some((sn, rank::MASTER))
    );

    // The master connects: GuildInfo, the full list, ConnectSuccess with the GuildSN.
    let (state, out) = guilds::enter(&p, &party, &hub, 1).await;
    assert_eq!(
        state,
        State::InGuild {
            guild_sn: sn,
            rank: rank::MASTER
        }
    );
    assert_eq!(out.len(), 3);
    let info = info_of(&out[0]).unwrap();
    assert_eq!(info.guild_name.as_deref(), Some("Night_Owls"));
    assert_eq!(info.master_name.as_deref(), Some("Alice"));
    assert_eq!(
        (info.guild_level, info.member_count, info.max_member_count),
        (1, 1, 100)
    );
    assert_eq!(info.guild_notice.as_deref(), Some(""));
    let Message::GuildMemberList(list) = decode(&out[1]) else {
        panic!("no member list")
    };
    assert!(list.is_full_update);
    let members = list.members.unwrap();
    assert_eq!(members.len(), 1);
    let alice = members[0].as_ref().unwrap();
    assert_eq!(alice.character_name.as_deref(), Some("Alice"));
    assert_eq!(
        (alice.game_level, alice.rank, alice.point),
        (25, rank::MASTER, 0)
    );
    assert!(alice.is_online);
    assert_eq!(
        result_of(&out[2]),
        (result::CONNECT_SUCCESS, String::new(), i64::from(sn))
    );

    // A second regular member online sees the master leave.
    sqlx::query("INSERT INTO game_guild_user(cid,guild_sn,intro,created_seconds,modified_seconds,group_user_type) VALUES(2,$1,'',0,0,125)")
        .bind(sn)
        .execute(&p)
        .await
        .unwrap();
    let (b_tx, mut b_rx) = tokio::sync::mpsc::channel(8);
    party.register(2, "Bob", b_tx);
    let (_, out) = guilds::enter(&p, &party, &hub, 2).await;
    assert_eq!(out.len(), 3);
    guilds::leave(&p, &party, &hub, 1).await;
    let Ok(Notice::Packets(pushed)) = b_rx.try_recv() else {
        panic!("no presence update")
    };
    let Message::GuildMemberList(partial) = decode(&pushed[0]) else {
        panic!("not a member list")
    };
    assert!(!partial.is_full_update);
    let line = partial.members.unwrap()[0].clone().unwrap();
    assert_eq!(line.character_name.as_deref(), Some("Alice"));
    assert!(!line.is_online);

    // QueryGuildList by name (prefix, case-insensitive), by master, and the failures.
    guilds::open(&p, 3, "Night_Larks", "nightlarks", None)
        .await
        .unwrap()
        .unwrap();
    let list = guilds::search(&p, &search("night", 0, 1, 10))
        .await
        .unwrap()
        .unwrap();
    let names: Vec<_> = list
        .guild_list
        .unwrap()
        .into_iter()
        .map(|g| g.unwrap().guild_name.unwrap())
        .collect();
    assert_eq!(names, ["Night_Owls", "Night_Larks"]);
    assert_eq!((list.page, list.total_page), (1, 1));
    let page2 = guilds::search(&p, &search("night", 0, 2, 1))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(page2.total_page, 2);
    assert_eq!(
        page2.guild_list.unwrap()[0]
            .as_ref()
            .unwrap()
            .guild_name
            .as_deref(),
        Some("Night_Larks")
    );
    let by_master = guilds::search(&p, &search("carol", 1, 1, 10))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(by_master.guild_list.unwrap().len(), 1);
    assert_eq!(by_master.total_page, 1);
    assert!(
        guilds::search(&p, &search("n", 0, 1, 10))
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        guilds::search(&p, &search("night", 0, 1, 0))
            .await
            .unwrap()
            .is_none()
    );
    cleanup_game_pool(admin, p, schema).await;
}
