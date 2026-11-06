//! PvP matches hosted by a player through the relay (B5 PV2c): Arena channels, registration,
//! BindPlay's WhoIs and JoinP2P's ConnectionRequest, and the removal.
use super::{cleanup_game_pool, initial_inventory, isolated_game_pool, minimum_character};
use std::{sync::Arc, time::Duration};
use tokio::sync::mpsc::Receiver;
use vindictus_server::{
    domain::{character_stats::Arithmetic, map_partition::OutgoingPacket, pvp::PvpContents},
    infrastructure::{
        character_lobby::QueryConfig,
        party_hub::{Notice, PartyHub},
        pvp_match::{Binder, Entry, PvpHub, Rules},
        relay::Relay,
    },
    protocol::{
        DEFAULT_MAX_BODY,
        mission::{self as mwire, Kind as MKind},
        pvp::{self as wire, Kind, register_state},
    },
};

fn contents() -> PvpContents {
    let row = |k: &str, v: &str| {
        (
            "p05".to_owned(),
            k.to_owned(),
            Some(v.to_owned()),
            None::<String>,
        )
    };
    PvpContents::load(
        &[
            row("GameRule", "Arena"),
            row("MatchingSystem", "Arena"),
            row("MaxSize", "32"),
            row("TeamSize", "1"),
            row("LevelLimitMin", "40"),
        ],
        &mut |_| Ok(true),
    )
    .unwrap()
}

fn rules() -> Rules {
    Rules {
        config: contents().get("p05").cloned(),
        spectator_min: 1,
        double_event: false,
        character_rank: true,
    }
}

fn register(channel: i32) -> wire::Register {
    wire::Register {
        pvp_mode: Some("p05".to_owned()),
        cheat: 0,
        channel_id: channel,
    }
}

/// The PvP messages pushed to a character until `kind` arrives (or the wait ends).
async fn until(rx: &mut Receiver<Notice>, kind: i32) -> Vec<OutgoingPacket> {
    let mut out = Vec::new();
    let _ = tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(n) = rx.recv().await {
            if let Notice::Push(p) = n {
                let done = p.iter().any(|p| p.category == kind);
                out.extend(p);
                if done {
                    return;
                }
            }
        }
    })
    .await;
    out
}

fn result(p: &OutgoingPacket) -> (i32, String) {
    match wire::decode(Kind::RegisterResult, &p.body, DEFAULT_MAX_BODY).unwrap() {
        Some(wire::Message::RegisterResult(r)) => (
            r.state,
            r.message.and_then(|m| m.format).unwrap_or_default(),
        ),
        _ => panic!("no PvpRegisterResultMessage"),
    }
}

fn channels(p: &OutgoingPacket) -> Vec<String> {
    match wire::decode(Kind::ChannelList, &p.body, DEFAULT_MAX_BODY).unwrap() {
        Some(wire::Message::ChannelList(l)) => l
            .channels
            .unwrap()
            .into_iter()
            .flatten()
            .map(|c| format!("{}:{}", c.channel_id, c.desc.unwrap_or_default()))
            .collect(),
        _ => panic!("no PvpChannelListMessage"),
    }
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn pvp_arena_channels_register_bind_and_remove() {
    let (a, p, s) = isolated_game_pool().await;
    minimum_character::setup(&p).await;
    minimum_character::initialize(&p).await.unwrap();
    let assets = Arc::new(super::minimum_give::skill_assets(&p, 0).await);
    vindictus_server::infrastructure::minimum_selection::select(
        &p,
        &initial_inventory::key(),
        &assets,
        0,
        QueryConfig {
            max_free: 5,
            care_clock: None,
        },
        Arithmetic::FrameworkX64,
    )
    .await
    .unwrap();
    let owner = initial_inventory::key();
    let party = Arc::new(PartyHub::new(false, false));
    let hub = Arc::new(PvpHub::new(
        Arc::clone(&party),
        Arc::new(vindictus_server::infrastructure::guild::GuildHub::default()),
    ));
    hub.set_relay(Some(Relay::new("127.0.0.1:27006".parse().unwrap())));
    let (tx, mut rx) = tokio::sync::mpsc::channel(64);
    party.register(1, "Fiona", tx);
    let binder = Binder {
        pool: p.clone(),
        assets: Arc::clone(&assets),
        config: QueryConfig {
            max_free: 5,
            care_clock: None,
        },
        arithmetic: Arithmetic::FrameworkX64,
        ids: None,
        guild_levels: None,
    };
    let query = wire::QueryChannelList {
        pvp_mode: Some("p05".to_owned()),
        key: Some("k".to_owned()),
    };
    let config = rules().config;
    let list = hub.channel_list(config.as_ref(), &query).unwrap();
    assert_eq!(list.category, Kind::ChannelList.category());
    assert_eq!(channels(&list), ["0:0/32", "1:0/32", "2:0/32"]);
    // Another mode lists nothing; an unknown channel and a frontend refusal cancel.
    assert!(channels(&hub.channel_list(None, &query).unwrap()).is_empty());
    let entry = || Entry {
        cid: 1,
        owner: &owner,
        level: 1,
        name: "Fiona",
        guild_id: 0,
    };
    hub.register(binder.clone(), entry(), None, rules(), &register(5))
        .map(|own| assert_eq!(result(&own[0]), (register_state::CANCELED, "GameUI_Heroes_PvpJoinFail_InvalidChannel".to_owned())))
        .unwrap();
    let own = hub
        .register(binder.clone(), entry(), Some("GameUI_Heroes_PvpJoinFail_InMicroPlay"), rules(), &register(0))
        .unwrap();
    assert_eq!(result(&own[0]).0, register_state::CANCELED);
    let mut no_mode = rules();
    no_mode.config = None;
    let own = hub.register(binder.clone(), entry(), None, no_mode, &register(0)).unwrap();
    assert_eq!(result(&own[0]).1, "GameUI_Heroes_PvpJoinFail_InvalidPvpMode");
    // Level 1 is below LevelLimitMin 40: the Arena takes it as a spectator.
    assert!(hub.register(binder.clone(), entry(), None, rules(), &register(0)).unwrap().is_empty());
    let got = until(&mut rx, MKind::ConnectionRequest.category()).await;
    let results: Vec<(i32, String)> = got
        .iter()
        .filter(|p| p.category == Kind::RegisterResult.category())
        .map(result)
        .collect();
    assert_eq!(
        results,
        [
            (register_state::WAIT, "GameUI_Heroes_PvpJoin_WaitingHost".to_owned()),
            (register_state::WAIT, "GameUI_Heroes_PvpJoin_WhoisComplete".to_owned()),
        ]
    );
    let request = got.last().unwrap();
    match mwire::decode(MKind::ConnectionRequest, &request.body, DEFAULT_MAX_BODY).unwrap() {
        Some(mwire::Message::ConnectionRequest(c)) => {
            assert_eq!(c.category.as_deref(), Some("PvpService.PvpService"));
            assert_eq!((c.port, c.ping_host_cid, c.group_id), (27006, -1, 0));
        }
        _ => panic!("no ConnectionRequestMessage"),
    }
    assert_eq!(channels(&hub.channel_list(config.as_ref(), &query).unwrap())[0], "0:1/32");
    // Registering again while registered is a duplicate (and clears the stale entry).
    let own = hub.register(binder.clone(), entry(), None, rules(), &register(0)).unwrap();
    assert_eq!(result(&own[0]).1, "GameUI_Heroes_PvpJoinFail_PlayerDuplicated");
    // Not a PvP host: RegisterServer belongs to a mission.
    assert_eq!(hub.register_server(1, None), None);
    // Level 1 is below LevelLimitMin 40 for a bout (Arena_Register_Level_Fail).
    let refused = hub.waiting_queue(1, 0);
    assert_eq!(refused.len(), 1);
    assert_ne!(refused[0].category, Kind::Command.category());
    hub.remove(1);
    assert!(hub.proxy(1).is_none());
    cleanup_game_pool(a, p, s).await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn pvp_arena_bound_player_is_kicked_and_the_channel_empties() {
    let (a, p, s) = isolated_game_pool().await;
    minimum_character::setup(&p).await;
    minimum_character::initialize(&p).await.unwrap();
    let assets = Arc::new(super::minimum_give::skill_assets(&p, 0).await);
    vindictus_server::infrastructure::minimum_selection::select(
        &p,
        &initial_inventory::key(),
        &assets,
        0,
        QueryConfig {
            max_free: 5,
            care_clock: None,
        },
        Arithmetic::FrameworkX64,
    )
    .await
    .unwrap();
    let owner = initial_inventory::key();
    let party = Arc::new(PartyHub::new(false, false));
    let hub = Arc::new(PvpHub::new(
        Arc::clone(&party),
        Arc::new(vindictus_server::infrastructure::guild::GuildHub::default()),
    ));
    hub.set_relay(Some(Relay::new("127.0.0.1:27006".parse().unwrap())));
    let (tx, mut rx) = tokio::sync::mpsc::channel(64);
    party.register(1, "Fiona", tx);
    let binder = Binder {
        pool: p.clone(),
        assets,
        config: QueryConfig {
            max_free: 5,
            care_clock: None,
        },
        arithmetic: Arithmetic::FrameworkX64,
        ids: None,
        guild_levels: None,
    };
    let entry = Entry {
        cid: 1,
        owner: &owner,
        level: 50,
        name: "Fiona",
        guild_id: 0,
    };
    hub.register(binder, entry, None, rules(), &register(1)).unwrap();
    until(&mut rx, MKind::ConnectionRequest.category()).await;
    // The host queues for bout 0: the game's commands go to the host (itself).
    assert!(hub.waiting_queue(1, 0).is_empty());
    let commands: Vec<(i32, String)> = until(&mut rx, Kind::Command.category())
        .await
        .iter()
        .filter(|p| p.category == Kind::Command.category())
        .map(|p| match wire::decode(Kind::Command, &p.body, DEFAULT_MAX_BODY).unwrap() {
            Some(wire::Message::Command(c)) => (c.command, c.arg.unwrap_or_default()),
            _ => panic!("no PvpCommandMessage"),
        })
        .collect();
    assert_eq!(
        commands[0],
        (6, "0,GameUI_Heroes_Pvp_Arena_Registerd_Waiting_Queue,Fiona".to_owned())
    );
    // An unknown bout is the processor's Unknown, as a ForceDialog.
    assert_eq!(hub.waiting_queue(1, 9).len(), 1);
    hub.remove(1);
    let kicked = until(&mut rx, Kind::Kicked.category()).await;
    assert!(kicked.iter().any(|p| p.category == Kind::Kicked.category()));
    let query = wire::QueryChannelList {
        pvp_mode: Some("p05".to_owned()),
        key: None,
    };
    let list = hub.channel_list(rules().config.as_ref(), &query).unwrap();
    assert_eq!(channels(&list)[1], "1:0/32");
    cleanup_game_pool(a, p, s).await;
}

fn full_rules() -> Rules {
    let row = |k: &str, v: &str| ("p04".to_owned(), k.to_owned(), Some(v.to_owned()), None::<String>);
    let contents = PvpContents::load(
        &[
            row("GameRule", "GodMatch"),
            row("MatchingSystem", "Full"),
            row("TeamSize", "1"),
            row("IsGuild", "FALSE"),
        ],
        &mut |_| Ok(true),
    )
    .unwrap();
    Rules {
        config: contents.get("p04").cloned(),
        spectator_min: 1,
        double_event: false,
        character_rank: true,
    }
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn pvp_full_matching_pairs_confirms_and_dodges() {
    let (a, p, s) = isolated_game_pool().await;
    minimum_character::setup(&p).await;
    minimum_character::initialize(&p).await.unwrap();
    let assets = Arc::new(super::minimum_give::skill_assets(&p, 0).await);
    let config = QueryConfig {
        max_free: 5,
        care_clock: None,
    };
    vindictus_server::infrastructure::minimum_selection::select(&p, &initial_inventory::key(), &assets, 0, config, Arithmetic::FrameworkX64)
        .await
        .unwrap();
    let owner = initial_inventory::key();
    let party = Arc::new(PartyHub::new(false, false));
    let hub = Arc::new(PvpHub::new(
        Arc::clone(&party),
        Arc::new(vindictus_server::infrastructure::guild::GuildHub::default()),
    ));
    hub.set_relay(Some(Relay::new("127.0.0.1:27006".parse().unwrap())));
    let (tx1, mut rx1) = tokio::sync::mpsc::channel(64);
    party.register(1, "Fiona", tx1);
    let (tx2, mut rx2) = tokio::sync::mpsc::channel(64);
    party.register(2, "Evie", tx2);
    let binder = Binder {
        pool: p.clone(),
        assets,
        config,
        arithmetic: Arithmetic::FrameworkX64,
        ids: None,
        guild_levels: None,
    };
    let entry = |cid: i64, name: &'static str| Entry {
        cid,
        owner: &owner,
        level: 50,
        name,
        guild_id: 0,
    };
    let request = wire::Register {
        pvp_mode: Some("p04".to_owned()),
        cheat: 0,
        channel_id: 0,
    };
    let formats = |got: &[OutgoingPacket]| -> Vec<(i32, String)> {
        got.iter().filter(|p| p.category == Kind::RegisterResult.category()).map(result).collect()
    };
    hub.register(binder.clone(), entry(1, "Fiona"), None, full_rules(), &request).unwrap();
    let got = until(&mut rx1, Kind::RegisterResult.category()).await;
    assert_eq!(formats(&got), [(register_state::WAIT, "GameUI_Heroes_PvpJoin_WaitingOpponent".to_owned())]);
    hub.register(binder.clone(), entry(2, "Evie"), None, full_rules(), &request).unwrap();
    for rx in [&mut rx1, &mut rx2] {
        let mut got = until(rx, Kind::RegisterResult.category()).await;
        got.extend(until(rx, Kind::RegisterResult.category()).await);
        assert_eq!(
            formats(&got),
            [
                (register_state::WAIT, "GameUI_Heroes_PvpJoin_OpponentFound".to_owned()),
                (register_state::JOINABLE, "GameUI_Heroes_PvpJoin_DSReady".to_owned()),
            ]
        );
    }
    // The play opens when both confirm; the first player binds and joins the relay.
    hub.confirm_join(binder.clone(), 1, full_rules());
    hub.confirm_join(binder.clone(), 2, full_rules());
    let got = until(&mut rx1, MKind::ConnectionRequest.category()).await;
    assert!(got.iter().any(|p| p.category == MKind::ConnectionRequest.category()));
    hub.remove(1);
    hub.remove(2);
    // A waiting player who unregisters is Dodged.
    hub.register(binder.clone(), entry(1, "Fiona"), None, full_rules(), &request).unwrap();
    until(&mut rx1, Kind::RegisterResult.category()).await;
    hub.unregister(1);
    let got = until(&mut rx1, Kind::RegisterResult.category()).await;
    assert_eq!(formats(&got)[0].1, "GameUI_Heroes_PvpJoinFail_Dodged");
    cleanup_game_pool(a, p, s).await;
}
