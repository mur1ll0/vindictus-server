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
    let hub = Arc::new(PvpHub::new(Arc::clone(&party)));
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
    let hub = Arc::new(PvpHub::new(Arc::clone(&party)));
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
    };
    let entry = Entry {
        cid: 1,
        owner: &owner,
        level: 50,
    };
    hub.register(binder, entry, None, rules(), &register(1)).unwrap();
    until(&mut rx, MKind::ConnectionRequest.category()).await;
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
