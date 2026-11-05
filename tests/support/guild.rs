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
    let list = guilds::search(&p, &hub, &search("night", 0, 1, 10))
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
    let page2 = guilds::search(&p, &hub, &search("night", 0, 2, 1))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(page2.total_page, 2);
    let by_master = guilds::search(&p, &hub, &search("carol", 1, 1, 10))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(by_master.guild_list.unwrap().len(), 1);
    assert!(
        guilds::search(&p, &hub, &search("n", 0, 1, 10))
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        guilds::search(&p, &hub, &search("night", 0, 1, 0))
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
    let names: Vec<_> = guilds::search(&p, &hub, &search("night", 0, 1, 10))
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

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn guild_gp_levels_and_level_up_rewards() {
    use vindictus_server::domain::guild::{Levels, gain};
    use vindictus_server::infrastructure::guild::GuildConfig;
    let (admin, p, schema) = isolated_game_pool().await;
    character(&p, 1, 10, "Alice", 25).await;
    character(&p, 2, 20, "Bob", 30).await;
    sqlx::query("UPDATE game_queue_identity SET initialized=true")
        .execute(&p)
        .await
        .unwrap();
    let party = PartyHub::new(false, false);
    let hub = GuildHub::new(GuildConfig {
        level_enabled: true,
        ap_gain_ratio: 100,
        daily_limit: "200;60".to_owned(),
        reset_hours: 7,
    });
    let levels = Levels::new([
        (1, 100, Some("capsule_ap[VALUE:100]".to_owned())),
        (2, 130, Some("capsule_ap[VALUE:100]".to_owned())),
    ])
    .unwrap();
    let sn = guilds::open(&p, 1, "Night_Owls", "nightowls", None)
        .await
        .unwrap()
        .unwrap();
    sqlx::query("INSERT INTO game_guild_user(cid,guild_sn,intro,created_seconds,modified_seconds,group_user_type) VALUES(2,$1,'',0,0,155)")
        .bind(sn)
        .execute(&p)
        .await
        .unwrap();
    // Offline: no GP.
    assert!(
        guilds::give_ap_gp(&p, &party, &hub, &levels, 1, 500)
            .await
            .unwrap()
            .is_empty()
    );
    let (a_tx, _a_rx) = tokio::sync::mpsc::channel(64);
    party.register(1, "Alice", a_tx);
    let (b_tx, mut b_rx) = tokio::sync::mpsc::channel(64);
    party.register(2, "Bob", b_tx);
    guilds::enter(&p, &party, &hub, 1).await;
    guilds::enter(&p, &party, &hub, 2).await;
    pushed(&mut b_rx);
    // A waiting member gives nothing.
    assert!(
        guilds::give_ap_gp(&p, &party, &hub, &levels, 2, 500)
            .await
            .unwrap()
            .is_empty()
    );
    // 1200 AP: GuildGainGP, a level up and the info; the member keeps the given GP.
    let out = guilds::give_ap_gp(&p, &party, &hub, &levels, 1, 1200)
        .await
        .unwrap();
    let Message::GuildGainGp(g) = wire::decode(Kind::GuildGainGp, &out[0].body, DEFAULT_MAX_BODY)
        .unwrap()
        .unwrap()
    else {
        panic!("no GuildGainGP")
    };
    assert_eq!(g.guild_point, 0);
    assert_eq!(g.daily_gain_gp.0.unwrap(), [(gain::BY_PVE, 1200)]);
    assert_eq!(out[1].category, Kind::GuildLevelUp.category());
    let info = out
        .iter()
        .filter(|p| p.category == Kind::GuildInfo.category())
        .map(|p| info_of(p).unwrap())
        .next_back()
        .unwrap();
    assert_eq!((info.guild_level, info.guild_point), (2, 200));
    assert_eq!(info.daily_gain_gp.0.unwrap(), [(gain::BY_PVE, 1200)]);
    // Everyone online gets GuildGainGP and GuildLevelUp, waiting members too.
    let to_bob: Vec<i32> = pushed(&mut b_rx).iter().map(|p| p.category).collect();
    assert!(to_bob.contains(&Kind::GuildGainGp.category()));
    assert!(to_bob.contains(&Kind::GuildLevelUp.category()));
    let (level, exp): (i32, i64) =
        sqlx::query_as("SELECT level,exp FROM game_guild WHERE oid_guild=$1")
            .bind(sn)
            .fetch_one(&p)
            .await
            .unwrap();
    assert_eq!((level, exp), (2, 200));
    let point: i64 = sqlx::query_scalar("SELECT point FROM game_guild_character_info WHERE cid=1")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(point, 1200);
    // The level-1 reward is queued once for the regular member, not for the waiting one.
    let queued: Vec<i64> =
        sqlx::query_scalar("SELECT cid FROM game_queued_items WHERE NOT processed ORDER BY cid")
            .fetch_all(&p)
            .await
            .unwrap();
    assert_eq!(queued, [1]);
    let max: i32 =
        sqlx::query_scalar("SELECT max_level FROM game_guild_character_max_level WHERE cid=1")
            .fetch_one(&p)
            .await
            .unwrap();
    assert_eq!(max, 2);
    // The daily limit (2000): 800 more, then nothing.
    guilds::give_ap_gp(&p, &party, &hub, &levels, 1, 5000)
        .await
        .unwrap();
    let daily: i32 = sqlx::query_scalar(
        "SELECT point FROM game_guild_daily_gain_gp WHERE oid_guild=$1 AND gp_gain_type=1",
    )
    .bind(sn)
    .fetch_one(&p)
    .await
    .unwrap();
    assert_eq!(daily, 2000);
    assert!(
        guilds::give_ap_gp(&p, &party, &hub, &levels, 1, 100)
            .await
            .unwrap()
            .is_empty()
    );
    // A reset older than the previous reset time clears today's gains.
    sqlx::query("UPDATE game_guild SET last_daily_gp_reset_seconds=0 WHERE oid_guild=$1")
        .bind(sn)
        .execute(&p)
        .await
        .unwrap();
    let out = guilds::give_ap_gp(&p, &party, &hub, &levels, 1, 100)
        .await
        .unwrap();
    assert!(!out.is_empty());
    cleanup_game_pool(admin, p, schema).await;
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn guild_brief_and_membership_for_other_systems() {
    let (admin, p, schema) = isolated_game_pool().await;
    character(&p, 1, 10, "Alice", 25).await;
    character(&p, 2, 20, "Bob", 30).await;
    character(&p, 3, 30, "Carol", 40).await;
    let sn = guilds::open(&p, 1, "Night_Owls", "nightowls", None)
        .await
        .unwrap()
        .unwrap();
    sqlx::query("UPDATE game_guild SET level=13 WHERE oid_guild=$1")
        .bind(sn)
        .execute(&p)
        .await
        .unwrap();
    sqlx::query("INSERT INTO game_guild_user(cid,guild_sn,intro,created_seconds,modified_seconds,group_user_type) VALUES(2,$1,'',0,0,155)")
        .bind(sn)
        .execute(&p)
        .await
        .unwrap();
    let mut c = p.acquire().await.unwrap();
    assert_eq!(
        guilds::brief_on(&mut c, 1).await.unwrap(),
        (sn, "Night_Owls".to_owned(), 13)
    );
    // A waiting member shows no guild, but still counts as a member.
    assert_eq!(
        guilds::brief_on(&mut c, 2).await.unwrap(),
        (0, String::new(), 0)
    );
    assert!(guilds::is_member_on(&mut c, 2).await.unwrap());
    assert!(!guilds::is_member_on(&mut c, 3).await.unwrap());
    drop(c);
    cleanup_game_pool(admin, p, schema).await;
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn rank_basis_update_follows_the_procedure() {
    use vindictus_server::domain::rank::ScoreType;
    use vindictus_server::infrastructure::rank;
    let (admin, p, schema) = isolated_game_pool().await;
    let day = 86_400;
    let now = 10 * day + 3600;
    // Count: insert, then add.
    assert!(
        rank::basis_update(&p, 1, 50001, ScoreType::Count, false, 1, 0, now)
            .await
            .unwrap()
    );
    assert!(
        rank::basis_update(&p, 1, 50001, ScoreType::Count, false, 1, 0, now + 1)
            .await
            .unwrap()
    );
    // Time: insert, a worse time is kept out, a better one replaces; the next day rolls over.
    assert!(
        rank::basis_update(&p, 1, 20001, ScoreType::Time, true, 9000, 0, now)
            .await
            .unwrap()
    );
    assert!(
        !rank::basis_update(&p, 1, 20001, ScoreType::Time, true, 9500, 0, now + 5)
            .await
            .unwrap()
    );
    assert!(
        rank::basis_update(&p, 1, 20001, ScoreType::Time, true, 8000, 7, now + 9)
            .await
            .unwrap()
    );
    assert!(
        rank::basis_update(&p, 1, 20001, ScoreType::Time, true, 12000, 8, now + day)
            .await
            .unwrap()
    );
    let rows: Vec<(i32, i64, Option<i64>)> = sqlx::query_as(
        "SELECT event_id,score,entity_id FROM game_rank_basis WHERE cid=1 ORDER BY event_id",
    )
    .fetch_all(&p)
    .await
    .unwrap();
    assert_eq!(rows, [(20001, 12000, Some(8)), (50001, 2, Some(0))]);
    let history: Vec<(i64, i64)> =
        sqlx::query_as("SELECT score,last_update_seconds FROM game_rank_basis_history")
            .fetch_all(&p)
            .await
            .unwrap();
    assert_eq!(history, [(8000, now + 9)]);
    cleanup_game_pool(admin, p, schema).await;
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn rank_publication_by_period() {
    use vindictus_server::domain::rank::ScoreType;
    use vindictus_server::infrastructure::rank;
    let (admin, p, schema) = isolated_game_pool().await;
    character(&p, 1, 10, "Alice", 25).await;
    character(&p, 2, 20, "Bob", 30).await;
    let wed = 1_793_786_400; // 2026-11-04 10:00 UTC
    let day = 86_400;
    let events = [
        (50001, ScoreType::Count, false),
        (20001, ScoreType::Time, true),
    ];
    // Yesterday: Alice 3 battles, Bob 1; times 9000 (Alice) and 8000 (Bob).
    let y = wed - day;
    for _ in 0..3 {
        rank::basis_update(&p, 1, 50001, ScoreType::Count, false, 1, 0, y)
            .await
            .unwrap();
    }
    rank::basis_update(&p, 2, 50001, ScoreType::Count, false, 1, 0, y)
        .await
        .unwrap();
    rank::basis_update(&p, 1, 20001, ScoreType::Time, true, 9000, 0, y)
        .await
        .unwrap();
    rank::basis_update(&p, 2, 20001, ScoreType::Time, true, 8000, 0, y)
        .await
        .unwrap();
    assert!(rank::publish(&p, &events, wed).await.unwrap());
    assert!(!rank::publish(&p, &events, wed + 60).await.unwrap());
    let rows: Vec<(i16, i32, i64, i64, i32, i32)> = sqlx::query_as(
        "SELECT period_type,event_id,cid,score,n4_rank,n4_rank_prev FROM game_rank_result \
         WHERE period_type=1 ORDER BY event_id,n4_order",
    )
    .fetch_all(&p)
    .await
    .unwrap();
    assert_eq!(
        rows,
        [
            (1, 20001, 2, 8000, 1, 0),
            (1, 20001, 1, 9000, 2, 0),
            (1, 50001, 1, 3, 1, 0),
            (1, 50001, 2, 1, 2, 0),
        ]
    );
    // The next day: Bob 5 more battles; the daily count is the gain since the snapshot.
    for _ in 0..5 {
        rank::basis_update(&p, 2, 50001, ScoreType::Count, false, 1, 0, wed + 3600)
            .await
            .unwrap();
    }
    assert!(rank::publish(&p, &events, wed + day).await.unwrap());
    let daily: Vec<(i64, i64, i32, i32)> = sqlx::query_as(
        "SELECT cid,score,n4_rank,n4_rank_prev FROM game_rank_result WHERE period_type=1 AND event_id=50001",
    )
    .fetch_all(&p)
    .await
    .unwrap();
    assert_eq!(daily, [(2, 5, 1, 2)]);
    // The week to date keeps both (no snapshot at the week start: cumulative counts).
    let weekly: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT cid,score FROM game_rank_result WHERE period_type=2 AND event_id=50001 ORDER BY n4_order",
    )
    .fetch_all(&p)
    .await
    .unwrap();
    assert_eq!(weekly, [(2, 6), (1, 3)]);
    cleanup_game_pool(admin, p, schema).await;
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn rank_queries_read_the_published_results() {
    use vindictus_server::domain::rank::ScoreType;
    use vindictus_server::infrastructure::rank;
    let (admin, p, schema) = isolated_game_pool().await;
    character(&p, 1, 10, "Alice", 25).await;
    character(&p, 2, 20, "Bob", 30).await;
    let wed = 1_793_786_400;
    let events = [
        (50001, ScoreType::Count, false),
        (20001, ScoreType::Time, true),
    ];
    rank::basis_update(&p, 1, 50001, ScoreType::Count, false, 3, 0, wed - 86_400)
        .await
        .unwrap();
    rank::basis_update(&p, 2, 50001, ScoreType::Count, false, 1, 0, wed - 86_400)
        .await
        .unwrap();
    rank::basis_update(&p, 2, 20001, ScoreType::Time, true, 8000, 0, wed - 86_400)
        .await
        .unwrap();
    sqlx::query("INSERT INTO game_rank_favorite VALUES(1,50001,0),(2,50001,0),(2,20001,0)")
        .execute(&p)
        .await
        .unwrap();
    rank::publish(&p, &events, wed).await.unwrap();
    let mine = rank::by_cid(&p, 2, 1).await.unwrap();
    let rows: Vec<_> = mine
        .iter()
        .map(|r| {
            let r = r.as_ref().unwrap();
            (r.event_id, r.rank, r.score, r.character_name.clone())
        })
        .collect();
    assert_eq!(
        rows,
        [
            (20001, 1, 8000, Some(String::new())),
            (50001, 2, 1, Some(String::new()))
        ]
    );
    let top = rank::by_event(&p, 50001, 1).await.unwrap();
    assert_eq!(
        top[0].as_ref().unwrap().character_name.as_deref(),
        Some("Alice")
    );
    let firsts = rank::firsts(&p, 1).await.unwrap();
    assert_eq!(firsts.len(), 2);
    let favorites = rank::favorites(&p, 1, 1).await.unwrap();
    assert_eq!(favorites[0].as_ref().unwrap().rank, 1);
    let heavy = rank::heavy_favorites(&p).await.unwrap();
    assert_eq!(heavy[0].as_ref().unwrap().event_id, 50001);
    // Entering the first 100 from no rank: one alarm per row, then read.
    assert_eq!(
        rank::alarms(&p, 1).await.unwrap(),
        [(1, 50001, 1), (2, 50001, 1), (3, 50001, 1)]
    );
    assert!(rank::alarms(&p, 1).await.unwrap().is_empty());
    cleanup_game_pool(admin, p, schema).await;
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn rank_titles_are_given_selected_and_reset() {
    use vindictus_server::domain::{
        progression::RankTitleInfo,
        rank::{self as rules, TitleUpdate},
    };
    use vindictus_server::infrastructure::rank_title;
    let (admin, p, schema) = isolated_game_pool().await;
    character(&p, 1, 10, "Alice", 25).await;
    // Weekly rows: Alice second of 60003, guild 7 first of 60001, a row past the limit.
    sqlx::query(
        "INSERT INTO game_rank_result(period_type,event_id,cid,score,n4_order,n4_rank,n4_rank_prev) VALUES \
         (2,60003,2,9,1,1,0),(2,60003,1,5,2,2,0),(2,60003,3,1,4,4,0),(2,60001,7,3,1,1,0),(1,60003,1,5,1,1,0)",
    )
    .execute(&p)
    .await
    .unwrap();
    let high = rank_title::load_rows(&p, 3).await.unwrap();
    let rows: Vec<_> = high
        .iter()
        .map(|h| (h.event_id, h.rank, h.entity))
        .collect();
    assert_eq!(rows, [(60001, 1, 7), (60003, 1, 2), (60003, 2, 1)]);
    let titles = [(60001, 1, 420034), (60003, 2, 420041)];
    let list = rules::rank_titles(&high, &titles, |e| e == 60001, 1, Some(7));
    assert_eq!(list, [420034, 420041]);
    let is_guild = |t: i32| t == 420034;
    let is_rank = |t: i32| t == 420034 || t == 420041;
    let change = rules::update_rank_titles(
        &[],
        &list,
        |_| true,
        is_guild,
        is_rank,
        TitleUpdate::Both,
        0,
    );
    // The guild title's class restriction refuses GiveTitle; the rank title is still held.
    let info = |t: i32| {
        Some(RankTitleInfo {
            available: t != 420034,
            goals: vec![],
            hide_until: 0,
        })
    };
    let mut tx = p.begin().await.unwrap();
    let packets = rank_title::apply_on(&mut tx, 77, 1, &change, info, 0)
        .await
        .unwrap();
    assert!(packets.is_empty());
    let selected = rank_title::select_on(&mut tx, 77, 1, &change.held, 420041)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(selected.category, 565);
    assert!(
        rank_title::select_on(&mut tx, 77, 1, &change.held, 999)
            .await
            .unwrap()
            .is_none()
    );
    tx.commit().await.unwrap();
    let owned: Vec<(i32, bool)> =
        sqlx::query_as("SELECT title_id,acquired FROM game_titles WHERE cid=1 ORDER BY title_id")
            .fetch_all(&p)
            .await
            .unwrap();
    assert_eq!(owned, [(420041, false)]);
    let logs: Vec<(i32, i32)> = sqlx::query_as(
        "SELECT event,title_id FROM (SELECT * FROM game_character_title_log01 UNION ALL \
         SELECT * FROM game_character_title_log02) l WHERE journal_id=77 ORDER BY time_ticks,event,title_id",
    )
    .fetch_all(&p)
    .await
    .unwrap();
    assert_eq!(logs.len(), 4);
    for event in [(8, 420034), (2, 420041), (8, 420041), (3, 420041)] {
        assert!(logs.contains(&event), "{event:?}");
    }
    // The rank title is lost: SelectedTitle returns to 0 with SelectTitle(0) and a Telepathy.
    let lost = rules::update_rank_titles(
        &change.held,
        &[],
        |_| true,
        is_guild,
        is_rank,
        TitleUpdate::Character,
        420041,
    );
    assert!(lost.unselect);
    let mut tx = p.begin().await.unwrap();
    let packets = rank_title::apply_on(&mut tx, 77, 1, &lost, info, 420041)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(packets.len(), 2);
    assert_eq!(packets[0], rank_title::select_title_packet(0).unwrap());
    let now: i32 = sqlx::query_scalar("SELECT selected_title FROM game_character_info WHERE id=1")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(now, 0);
    cleanup_game_pool(admin, p, schema).await;
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn free_match_counts_increase_up_to_the_int_maximum() {
    use vindictus_server::infrastructure::pvp;
    let (admin, p, schema) = isolated_game_pool().await;
    character(&p, 1, 10, "Alice", 25).await;
    character(&p, 2, 20, "Bob", 30).await;
    assert!(pvp::increase_count(&p, 1, true).await.unwrap());
    assert!(pvp::increase_count(&p, 2, false).await.unwrap());
    assert!(pvp::increase_count(&p, 2, false).await.unwrap());
    sqlx::query("UPDATE game_character_info SET free_match_win_count=2147483647 WHERE id=2")
        .execute(&p)
        .await
        .unwrap();
    assert!(pvp::increase_count(&p, 2, true).await.unwrap());
    assert!(!pvp::increase_count(&p, 9, true).await.unwrap());
    let counts: Vec<(i64, i32, i32)> = sqlx::query_as(
        "SELECT id,free_match_win_count,free_match_lose_count FROM game_character_info ORDER BY id",
    )
    .fetch_all(&p)
    .await
    .unwrap();
    assert_eq!(counts, [(1, 1, 0), (2, 2_147_483_647, 2)]);
    cleanup_game_pool(admin, p, schema).await;
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn guild_storage_session_settings_and_listeners() {
    use vindictus_server::infrastructure::guild_storage::{self as storage, StorageConfig};
    use vindictus_server::protocol::guild_storage::{self as swire, Kind as SKind};
    let (admin, p, schema) = isolated_game_pool().await;
    character(&p, 1, 10, "Alice", 25).await;
    character(&p, 2, 20, "Bob", 30).await;
    let assets = super::minimum_character::assets();
    let party = PartyHub::new(false, false);
    let hub = GuildHub::default();
    let sn = guilds::open(&p, 1, "Night_Owls", "nightowls", None)
        .await
        .unwrap()
        .unwrap();
    sqlx::query(
        "INSERT INTO game_guild_user(cid,guild_sn,intro,created_seconds,modified_seconds,group_user_type) VALUES(2,$1,NULL,0,0,125)",
    )
    .bind(sn)
    .execute(&p)
    .await
    .unwrap();
    let (a_tx, _a_rx) = tokio::sync::mpsc::channel(64);
    party.register(1, "Alice", a_tx);
    let (b_tx, mut b_rx) = tokio::sync::mpsc::channel(64);
    party.register(2, "Bob", b_tx);
    let (state, _) = guilds::enter(&p, &party, &hub, 1).await;
    assert_eq!(state, State::InGuild { guild_sn: sn });
    let (state, _) = guilds::enter(&p, &party, &hub, 2).await;
    assert_eq!(state, State::InGuild { guild_sn: sn });
    while b_rx.try_recv().is_ok() {}
    assert_eq!(storage::regular_members(&p, sn).await.unwrap(), 2);

    let off = StorageConfig::default();
    assert!(
        storage::session(&p, &assets, off, &hub, sn, 2, rank::MEMBER, true)
            .await
            .unwrap()
            .is_empty()
    );
    let config = StorageConfig {
        feature: true,
        required_member: 2,
    };
    // The first load initializes the setting and keeps 0/0; then the stored 898779.
    let out = storage::session(&p, &assets, config, &hub, sn, 2, rank::MEMBER, true)
        .await
        .unwrap();
    assert_eq!(out.len(), 3);
    let info = |packet: &OutgoingPacket| {
        assert_eq!(packet.category, SKind::GuildInventoryInfo.category());
        match swire::decode(SKind::GuildInventoryInfo, &packet.body, DEFAULT_MAX_BODY).unwrap() {
            Some(swire::Message::GuildInventoryInfo(i)) => i,
            _ => panic!("no GuildInventoryInfoMessage"),
        }
    };
    let first = info(&out[0]);
    assert!(first.is_enabled);
    assert_eq!(
        (
            first.storage_count,
            first.gold_limit,
            first.access_limit_tag
        ),
        (0, 0, 0)
    );
    assert_eq!(first.slot_infos.as_ref().map(Vec::len), Some(0));
    for (packet, today) in [(&out[1], true), (&out[2], false)] {
        assert_eq!(packet.category, 0);
        assert_eq!(
            &packet.body[..16],
            SKind::GuildStorageLogs.type_id().as_bytes()
        );
        let Some(swire::Message::GuildStorageLogs(logs)) = swire::decode(
            SKind::GuildStorageLogs,
            &packet.body[16..],
            DEFAULT_MAX_BODY,
        )
        .unwrap() else {
            panic!("no GuildStorageLogsMessage")
        };
        assert_eq!(logs.is_today_log, today);
    }
    let again = storage::load(&p, &assets, sn).await.unwrap();
    assert_eq!(again.access_limit_tag, storage::INITIAL_ACCESS_LIMIT_TAG);

    // Only the master changes the setting; listening members get the new inventory info.
    let dialog =
        storage::update_setting(&p, &assets, config, &hub, &party, sn, 2, rank::MEMBER, 5, 7)
            .await
            .unwrap();
    assert_eq!(dialog.len(), 1);
    assert_ne!(dialog[0].category, SKind::GuildInventoryInfo.category());
    let own = storage::update_setting(
        &p,
        &assets,
        config,
        &hub,
        &party,
        sn,
        1,
        rank::MASTER,
        5000,
        7,
    )
    .await
    .unwrap();
    assert!(own.is_empty(), "the master is not listening");
    let got = pushed(&mut b_rx);
    assert_eq!(got.len(), 1);
    let changed = info(&got[0]);
    assert_eq!((changed.gold_limit, changed.access_limit_tag), (5000, 7));
    // Below the required members the storage is stopped.
    let stopped = StorageConfig {
        feature: true,
        required_member: 3,
    };
    let refused = storage::update_setting(
        &p,
        &assets,
        stopped,
        &hub,
        &party,
        sn,
        1,
        rank::MASTER,
        1,
        1,
    )
    .await
    .unwrap();
    assert_eq!(refused.len(), 1);
    // Ending the session stops the broadcast.
    assert!(
        storage::session(&p, &assets, config, &hub, sn, 2, rank::MEMBER, false)
            .await
            .unwrap()
            .is_empty()
    );
    storage::update_setting(&p, &assets, config, &hub, &party, sn, 1, rank::MASTER, 9, 9)
        .await
        .unwrap();
    assert!(pushed(&mut b_rx).is_empty());
    cleanup_game_pool(admin, p, schema).await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn guild_storage_deposits_picks_and_arranges_items_and_gold() {
    use vindictus_server::infrastructure::guild_storage::{
        self as storage, Buyer, Member, StorageConfig,
    };
    use vindictus_server::infrastructure::minimum_give;
    use vindictus_server::protocol::guild_storage::{self as swire, ItemMove, Kind as SKind};
    let (admin, p, schema) = isolated_game_pool().await;
    super::minimum_character::setup(&p).await;
    super::minimum_character::initialize(&p).await.unwrap();
    sqlx::query("UPDATE game_character_info SET name_utf16=$1,level=25 WHERE id=1")
        .bind(utf16("Fiona"))
        .execute(&p)
        .await
        .unwrap();
    let assets = super::minimum_give::marketplace_assets();
    let owner = super::initial_inventory::key();
    for (class, n) in [("synthetic_gold", 10_000), ("synthetic_ore", 50)] {
        assert_eq!(
            minimum_give::lab_grant(&p, &assets, Some(1), class, n, 9)
                .await
                .unwrap(),
            Ok(1)
        );
    }
    let party = PartyHub::new(false, false);
    let hub = GuildHub::default();
    let sn = guilds::open(&p, 1, "Night_Owls", "nightowls", None)
        .await
        .unwrap()
        .unwrap();
    let (tx, _rx) = tokio::sync::mpsc::channel(64);
    party.register(1, "Fiona", tx);
    let (state, _) = guilds::enter(&p, &party, &hub, 1).await;
    assert_eq!(state, State::InGuild { guild_sn: sn });
    let config = StorageConfig {
        feature: true,
        required_member: 1,
    };
    let member = || Member {
        owner: &owner,
        cid: 1,
        rank: rank::MASTER,
    };
    let held = |class: &'static str| {
        let p = p.clone();
        async move {
            let rows: Vec<(Option<i64>, i32, i16, i32)> = sqlx::query_as(
                "SELECT owner_id,count,collection,slot FROM game_items WHERE item_class_utf16=$1 AND (owner_id=1 OR id IN (SELECT id FROM game_guild_storage_items)) ORDER BY owner_id NULLS FIRST,slot",
            )
            .bind(utf16(class))
            .fetch_all(&p)
            .await
            .unwrap();
            rows
        }
    };
    let mv = |item_id: i64, class: &str, amount: i32, tab: u8, slot: i32| ItemMove {
        item_id,
        item_class: Some(class.to_owned()),
        amount,
        target_tab: tab,
        target_slot: slot,
    };
    let info = |packets: &[OutgoingPacket]| {
        let last = packets.last().expect("packets");
        assert_eq!(last.category, SKind::GuildInventoryInfo.category());
        match swire::decode(SKind::GuildInventoryInfo, &last.body, DEFAULT_MAX_BODY).unwrap() {
            Some(swire::Message::GuildInventoryInfo(i)) => i,
            _ => panic!("no GuildInventoryInfoMessage"),
        }
    };
    let ids = |base: i64| (0..4).map(|n| base + n).collect::<Vec<i64>>();
    // Listening, so the own copy of the broadcast comes back.
    storage::session(&p, &assets, config, &hub, sn, 1, rank::MASTER, true)
        .await
        .unwrap();
    // Without slots nothing moves.
    let out = storage::add_item(
        &p,
        &assets,
        config,
        &hub,
        &party,
        sn,
        member(),
        &mv(0, "synthetic_gold", 100, 255, -1),
        ids(9000),
    )
    .await
    .unwrap();
    assert_eq!(out.len(), 1);
    assert_ne!(out[0].category, SKind::GuildInventoryInfo.category());
    // Without a FeatureMatrix the first purchase costs no gold: 24 slots.
    let bought = storage::purchase(
        &p,
        &assets,
        config,
        &hub,
        &party,
        sn,
        Buyer {
            owner: &owner,
            cid: 1,
            log: vindictus_server::infrastructure::minimum_item_logs::Context {
                flags: vindictus_server::infrastructure::minimum_item_logs::flags(&assets).unwrap(),
                civil_offset_seconds: 0,
            },
        },
        -1,
    )
    .await;
    assert_eq!(info(&bought.unwrap()).storage_count, 24);

    // Gold in and out.
    let out = storage::add_item(
        &p,
        &assets,
        config,
        &hub,
        &party,
        sn,
        member(),
        &mv(0, "synthetic_gold", 3000, 255, -1),
        ids(9010),
    )
    .await
    .unwrap();
    assert_eq!(info(&out).slot_infos.as_ref().map(Vec::len), Some(1));
    assert_eq!(
        held("synthetic_gold").await,
        vec![(None, 3000, 101, -1), (Some(1), 7000, 101, -1)]
    );
    storage::add_item(
        &p,
        &assets,
        config,
        &hub,
        &party,
        sn,
        member(),
        &mv(0, "synthetic_gold", 500, 255, -1),
        ids(9020),
    )
    .await
    .unwrap();
    let out = storage::pick_item(
        &p,
        &assets,
        config,
        &hub,
        &party,
        sn,
        member(),
        &mv(0, "synthetic_gold", 1500, 255, -1),
        ids(9030),
    )
    .await
    .unwrap();
    assert_eq!(info(&out).slot_infos.as_ref().map(Vec::len), Some(1));
    assert_eq!(
        held("synthetic_gold").await,
        vec![(None, 2000, 101, -1), (Some(1), 8000, 101, -1)]
    );
    // Picking more than the storage holds fails without a change.
    let out = storage::pick_item(
        &p,
        &assets,
        config,
        &hub,
        &party,
        sn,
        member(),
        &mv(0, "synthetic_gold", 2001, 255, -1),
        ids(9040),
    )
    .await
    .unwrap();
    assert_eq!(out.len(), 1);
    assert_eq!(
        held("synthetic_gold").await,
        vec![(None, 2000, 101, -1), (Some(1), 8000, 101, -1)]
    );

    // An item into a target slot, stacked by a second deposit, arranged, then picked.
    let ore: i64 =
        sqlx::query_scalar("SELECT id FROM game_items WHERE owner_id=1 AND item_class_utf16=$1")
            .bind(utf16("synthetic_ore"))
            .fetch_one(&p)
            .await
            .unwrap();
    storage::add_item(
        &p,
        &assets,
        config,
        &hub,
        &party,
        sn,
        member(),
        &mv(ore, "synthetic_ore", 20, 0, 5),
        ids(9050),
    )
    .await
    .unwrap();
    storage::add_item(
        &p,
        &assets,
        config,
        &hub,
        &party,
        sn,
        member(),
        &mv(ore, "synthetic_ore", 10, 0, -1),
        ids(9060),
    )
    .await
    .unwrap();
    let rows = held("synthetic_ore").await;
    assert_eq!(rows[0], (None, 30, 0, 5));
    assert_eq!(rows[1].1, 20);
    let stored: i64 = sqlx::query_scalar("SELECT id FROM game_guild_storage_items g JOIN game_items i USING(id) WHERE i.item_class_utf16=$1")
        .bind(utf16("synthetic_ore"))
        .fetch_one(&p)
        .await
        .unwrap();
    let out = storage::arrange_item(
        &p,
        &assets,
        config,
        &hub,
        &party,
        sn,
        1,
        rank::MASTER,
        stored,
        7,
    )
    .await
    .unwrap();
    let moved = info(&out);
    let slot = moved
        .slot_infos
        .unwrap()
        .into_iter()
        .flatten()
        .find(|s| s.id == stored)
        .unwrap();
    assert_eq!(slot.slot, 7);
    // Outside the purchased slots the arrangement fails.
    let out = storage::arrange_item(
        &p,
        &assets,
        config,
        &hub,
        &party,
        sn,
        1,
        rank::MASTER,
        stored,
        30,
    )
    .await
    .unwrap();
    assert_eq!(out.len(), 1);
    // A member of a lower rank than the tab's limit is refused.
    let low = Member {
        owner: &owner,
        cid: 1,
        rank: 4,
    };
    let out = storage::pick_item(
        &p,
        &assets,
        config,
        &hub,
        &party,
        sn,
        low,
        &mv(stored, "synthetic_ore", 5, 255, -1),
        ids(9070),
    )
    .await
    .unwrap();
    assert_eq!(out.len(), 1);
    storage::pick_item(
        &p,
        &assets,
        config,
        &hub,
        &party,
        sn,
        member(),
        &mv(stored, "synthetic_ore", 30, 255, -1),
        ids(9080),
    )
    .await
    .unwrap();
    let rows = held("synthetic_ore").await;
    assert_eq!(rows.iter().map(|r| r.1).sum::<i32>(), 50);
    assert!(rows.iter().all(|r| r.0 == Some(1)));
    let links: i64 = sqlx::query_scalar("SELECT count(*) FROM game_guild_storage_items")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(links, 1, "only the gold stays");
    cleanup_game_pool(admin, p, schema).await;
}
