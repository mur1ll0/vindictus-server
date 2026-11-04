use super::{cleanup_game_pool, isolated_game_pool};
use tokio::sync::mpsc::Receiver;
use vindictus_server::{
    domain::{guild::open_error, map_partition::OutgoingPacket},
    infrastructure::{
        guild::{self as guilds, GuildHub, State, change_master, join_error},
        party_hub::{Notice, PartyHub},
    },
    protocol::{
        DEFAULT_MAX_BODY,
        guild::{
            self as wire, GuildInfo, GuildMember, Kind, Message, Operation, QueryGuildList, rank,
            result,
        },
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
        Kind::GuildChat,
        Kind::GuildChatMemberInfo,
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
/// (IsFullUpdate, lines) of a GuildMemberList.
fn list_of(p: &OutgoingPacket) -> (bool, Vec<GuildMember>) {
    match decode(p) {
        Message::GuildMemberList(l) => (
            l.is_full_update,
            l.members.unwrap().into_iter().map(Option::unwrap).collect(),
        ),
        _ => panic!("not a GuildMemberList"),
    }
}
/// Short shape of packets: I(nfo), F(ull list), U(pdate list), R(esult code).
fn shape(packets: &[OutgoingPacket]) -> Vec<String> {
    packets
        .iter()
        .map(|p| match decode(p) {
            Message::GuildInfo(_) => "I".to_owned(),
            Message::GuildMemberList(l) if l.is_full_update => "F".to_owned(),
            Message::GuildMemberList(_) => "U".to_owned(),
            Message::GuildResult(r) => format!("R{}", r.result),
            Message::GuildChat(_) => "C".to_owned(),
            Message::GuildChatMemberInfo(i) => format!("M{}", u8::from(i.is_online)),
            _ => "?".to_owned(),
        })
        .collect()
}
fn pushed(rx: &mut Receiver<Notice>) -> Vec<OutgoingPacket> {
    let mut out = Vec::new();
    while let Ok(n) = rx.try_recv() {
        if let Notice::Push(p) = n {
            out.extend(p);
        }
    }
    out
}
fn op(command: &str, target: &str, value: i32) -> Option<Operation> {
    Some(Operation {
        command: Some(command.to_owned()),
        target: Some(target.to_owned()),
        value,
    })
}
fn search(key: &str, query_type: i32, page: i32, size: u8) -> QueryGuildList {
    QueryGuildList {
        query_type,
        search_key: Some(key.to_owned()),
        page,
        page_size: size,
    }
}
async fn user_type(p: &sqlx::PgPool, cid: i64) -> Option<i16> {
    sqlx::query_scalar("SELECT group_user_type FROM game_guild_user WHERE cid=$1")
        .bind(cid)
        .fetch_optional(p)
        .await
        .unwrap()
}
async fn real_count(p: &sqlx::PgPool, sn: i32) -> i32 {
    sqlx::query_scalar("SELECT real_user_count FROM game_guild_info WHERE guild_sn=$1")
        .bind(sn)
        .fetch_one(p)
        .await
        .unwrap()
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
    assert_eq!(shape(&out), ["R0", "I"]);
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

    // The master connects: Connect + Sync, UpdateGroupMemberInfo + Sync, ConnectSuccess.
    let (a_tx, mut a_rx) = tokio::sync::mpsc::channel(64);
    party.register(1, "Alice", a_tx);
    let (state, out) = guilds::enter(&p, &party, &hub, 1).await;
    assert_eq!(state, State::InGuild { guild_sn: sn });
    assert_eq!(shape(&out), ["I", "F", "F", "U", "F", "R0"]);
    let info = info_of(&out[0]).unwrap();
    assert_eq!(info.guild_name.as_deref(), Some("Night_Owls"));
    assert_eq!(info.master_name.as_deref(), Some("Alice"));
    assert_eq!(
        (info.guild_level, info.member_count, info.max_member_count),
        (1, 1, 100)
    );
    let (_, members) = list_of(&out[1]);
    assert_eq!(members.len(), 1);
    assert_eq!(members[0].character_name.as_deref(), Some("Alice"));
    assert_eq!(
        (members[0].game_level, members[0].rank, members[0].point),
        (25, rank::MASTER, 0)
    );
    assert!(members[0].is_online);
    assert_eq!(
        result_of(&out[5]),
        (result::CONNECT_SUCCESS, String::new(), i64::from(sn))
    );

    // Bob joins (waiting): the online master sees his line.
    assert_eq!(
        guilds::join(&p, &party, &hub, 2, 9999).await.unwrap(),
        Err(join_error::NOT_INITIALIZE)
    );
    assert_eq!(
        guilds::join(&p, &party, &hub, 1, sn).await.unwrap(),
        Err(open_error::HAS_GUILD)
    );
    assert_eq!(guilds::join(&p, &party, &hub, 2, sn).await.unwrap(), Ok(sn));
    assert_eq!(user_type(&p, 2).await, Some(155));
    let to_alice = pushed(&mut a_rx);
    assert_eq!(shape(&to_alice), ["U"]);
    assert_eq!(list_of(&to_alice[0]).1[0].rank, rank::WAIT);
    // A waiting member connects: GuildInfo and the full lists of a changed member.
    let (b_tx, mut b_rx) = tokio::sync::mpsc::channel(64);
    party.register(2, "Bob", b_tx);
    let (state, out) = guilds::enter(&p, &party, &hub, 2).await;
    assert_eq!(state, State::InGuild { guild_sn: sn });
    assert_eq!(shape(&out), ["I", "F", "F", "R0"]);
    pushed(&mut a_rx);

    // Accept the join: Bob becomes a Member, the info changes for everyone.
    let (all, synced) = guilds::operate(&p, &party, &hub, 1, sn, &[op("Join", "Bob", 1)])
        .await
        .unwrap();
    assert!(all && !synced.kicked);
    assert_eq!(shape(&synced.packets), ["I", "U"]);
    assert_eq!(info_of(&synced.packets[0]).unwrap().member_count, 2);
    assert_eq!(shape(&pushed(&mut b_rx)), ["I", "U", "F"]);
    assert_eq!(
        (user_type(&p, 2).await, real_count(&p, sn).await),
        (Some(125), 2)
    );
    // Promote Bob; an unknown target and a second promotion fail.
    let (all, _) = guilds::operate(
        &p,
        &party,
        &hub,
        1,
        sn,
        &[op("Rank", "Bob", rank::OPERATOR)],
    )
    .await
    .unwrap();
    assert!(all);
    assert_eq!(user_type(&p, 2).await, Some(15));
    let (all, _) = guilds::operate(
        &p,
        &party,
        &hub,
        1,
        sn,
        &[op("Rank", "Bob", rank::OPERATOR), op("Rank", "Nobody", 0)],
    )
    .await
    .unwrap();
    assert!(!all);
    // Carol asks to join and is rejected by the operator.
    guilds::join(&p, &party, &hub, 3, sn)
        .await
        .unwrap()
        .unwrap();
    let (all, _) = guilds::operate(&p, &party, &hub, 2, sn, &[op("Join", "Carol", 0)])
        .await
        .unwrap();
    assert!(all);
    assert_eq!(user_type(&p, 3).await, None);
    pushed(&mut a_rx);
    pushed(&mut b_rx);

    // ChangeMaster: only the master may hand the guild over, and only to an Operator.
    let (code, _) = guilds::change_master(&p, &party, &hub, 2, sn, Some("Alice"))
        .await
        .unwrap();
    assert_eq!(code, change_master::NOT_MATCH_OLD_MASTER);
    let (code, _) = guilds::change_master(&p, &party, &hub, 1, sn, Some("Nobody"))
        .await
        .unwrap();
    assert_eq!(code, change_master::NOT_FOUND_NEW_MASTER);
    let (code, _) = guilds::change_master(&p, &party, &hub, 1, sn, Some("Alice"))
        .await
        .unwrap();
    assert_eq!(code, change_master::EQUAL_NEW_AND_OLD_ID);
    let (code, synced) = guilds::change_master(&p, &party, &hub, 1, sn, Some("Bob"))
        .await
        .unwrap();
    assert_eq!(code, change_master::SUCCESS);
    assert_eq!(
        info_of(&synced.packets[0]).unwrap().master_name.as_deref(),
        Some("Bob")
    );
    assert_eq!(
        (user_type(&p, 1).await, user_type(&p, 2).await),
        (Some(15), Some(5))
    );
    pushed(&mut b_rx);

    // Any member edits the notice; everyone online gets the new info.
    let synced = guilds::edit_notice(&p, &party, &hub, 1, sn, Some("Raide às 21h"))
        .await
        .unwrap();
    assert_eq!(
        info_of(&synced.packets[0]).unwrap().guild_notice.as_deref(),
        Some("Raide às 21h")
    );
    let to_bob = pushed(&mut b_rx);
    assert_eq!(
        info_of(&to_bob[0]).unwrap().guild_notice.as_deref(),
        Some("Raide às 21h")
    );

    // Guild chat: both join the room; the members already there see the newcomer.
    assert!(guilds::join_chat(&p, &party, &hub, 1, sn).await.unwrap());
    assert!(guilds::join_chat(&p, &party, &hub, 2, sn).await.unwrap());
    assert_eq!(shape(&pushed(&mut a_rx)), ["M1"]);
    let (failed, own) = guilds::chat(&p, &party, &hub, 1, sn, Some("Olá, guilda"))
        .await
        .unwrap();
    assert_eq!((failed, shape(&own)), (None, vec!["C".to_owned()]));
    let to_bob = pushed(&mut b_rx);
    let Message::GuildChat(said) = decode(&to_bob[0]) else {
        panic!("not a GuildChat")
    };
    assert_eq!(said.sender.as_deref(), Some("Alice"));
    assert_eq!(said.message.as_deref(), Some("Olá, guilda"));
    // Not online in the guild: Unknown.
    assert_eq!(
        guilds::chat(&p, &party, &hub, 3, sn, Some("x"))
            .await
            .unwrap()
            .0,
        Some(guilds::chat_error::UNKNOWN)
    );

    // The PlayerConn closes: LeaveChatRoom, Disconnect and Sync at once.
    guilds::leave(&p, &party, &hub, 1).await;
    let to_bob = pushed(&mut b_rx);
    assert_eq!(shape(&to_bob), ["M0", "U"]);
    let (_, lines) = list_of(&to_bob[1]);
    assert_eq!(lines[0].character_name.as_deref(), Some("Alice"));
    assert!(!lines[0].is_online);
    // The notice stays when the text is null.
    let synced = guilds::edit_notice(&p, &party, &hub, 2, sn, None)
        .await
        .unwrap();
    assert_eq!(shape(&synced.packets), ["I"]);
    assert_eq!(
        info_of(&synced.packets[0]).unwrap().guild_notice.as_deref(),
        Some("Raide às 21h")
    );

    // The new master kicks Alice while she is online: GuildKicked reaches her frontend.
    guilds::enter(&p, &party, &hub, 1).await;
    pushed(&mut a_rx);
    let (all, _) = guilds::operate(&p, &party, &hub, 2, sn, &[op("Rank", "Alice", 0)])
        .await
        .unwrap();
    assert!(all);
    let kicked = std::iter::from_fn(|| a_rx.try_recv().ok()).any(
        |n| matches!(n, Notice::GuildKicked { result: r } if r == result::LEAVE_GUILD_SUCCESS),
    );
    assert!(kicked);
    assert_eq!(
        (user_type(&p, 1).await, real_count(&p, sn).await),
        (None, 1)
    );

    // Alice joins again and leaves by herself: the sync kicks the caller.
    guilds::join(&p, &party, &hub, 1, sn)
        .await
        .unwrap()
        .unwrap();
    guilds::enter(&p, &party, &hub, 1).await;
    // A waiting member may not chat.
    assert_eq!(
        guilds::chat(&p, &party, &hub, 1, sn, Some("x"))
            .await
            .unwrap()
            .0,
        Some(guilds::chat_error::NOT_ENOUGH_PERMISSION)
    );
    let left = guilds::leave_guild(&p, &party, &hub, 1, sn)
        .await
        .unwrap()
        .unwrap();
    assert!(left.kicked);
    assert_eq!(user_type(&p, 1).await, None);
    assert_eq!(real_count(&p, sn).await, 1);

    // CloseGuild: one member left; the name is free again, the ID stays taken.
    let night_larks = guilds::open(&p, 3, "Night_Larks", "nightlarks", None)
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
    let page2 = guilds::search(&p, &search("night", 0, 2, 1))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(page2.total_page, 2);
    let by_master = guilds::search(&p, &search("carol", 1, 1, 10))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(by_master.guild_list.unwrap().len(), 1);
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
    // Not online in its guild: a FailMessage (null Arg).
    assert_eq!(
        guilds::close(&p, &party, &hub, 3, night_larks)
            .await
            .unwrap(),
        Err(None)
    );
    assert_eq!(
        guilds::close(&p, &party, &hub, 2, sn).await.unwrap(),
        Ok(())
    );
    assert_eq!(guilds::membership(&p, 2).await.unwrap(), None);
    let names: Vec<_> = guilds::search(&p, &search("night", 0, 1, 10))
        .await
        .unwrap()
        .unwrap()
        .guild_list
        .unwrap()
        .into_iter()
        .map(|g| g.unwrap().guild_name.unwrap())
        .collect();
    assert_eq!(names, ["Night_Larks"]);
    assert!(
        guilds::open(&p, 2, "Night_Owls", "owls2", None)
            .await
            .unwrap()
            .is_ok()
    );
    assert_eq!(
        guilds::open(&p, 1, "Owl_Again", "nightowls", None)
            .await
            .unwrap(),
        Err(open_error::DUPLICATED_ID)
    );
    cleanup_game_pool(admin, p, schema).await;
}
