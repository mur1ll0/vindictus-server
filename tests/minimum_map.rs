use serde_json::Value;
use vindictus_server::{
    domain::{
        legacy_user_data::SqlDateTime2,
        map_partition::{
            Channel, MapFile, OutgoingPacket, Topology, login_ok_region_code, recommend_channel,
            utility_epoch_seconds,
        },
    },
    protocol::{
        DEFAULT_MAX_BODY, Frame,
        character_list::{Costume, Summary},
        minimum_map::{self as codec, ActionSync, Kind, Message, MinimumMapWireError, Vector3D},
        transport::PlainReceiver,
    },
};

fn costume(v: &Value) -> Costume {
    Costume {
        costume_type_info: (!v["CostumeTypeInfo"].is_null()).then(|| {
            v["CostumeTypeInfo"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| {
                    (
                        r["key"].as_i64().unwrap() as i32,
                        r["value"].as_i64().unwrap() as i32,
                    )
                })
                .collect()
        }),
        color_info: (!v["ColorInfo"].is_null()).then(|| {
            v["ColorInfo"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| {
                    (
                        r["key"].as_i64().unwrap() as i32,
                        r["value"].as_i64().unwrap() as i32,
                    )
                })
                .collect()
        }),
        avatar_info: (!v["AvatarInfo"].is_null()).then(|| {
            v["AvatarInfo"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| {
                    (
                        r["key"].as_i64().unwrap() as i32,
                        r["value"].as_bool().unwrap(),
                    )
                })
                .collect()
        }),
        avatar_hide_info: (!v["AvatarHideInfo"].is_null()).then(|| {
            v["AvatarHideInfo"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| {
                    (
                        r["key"].as_i64().unwrap() as i32,
                        r["value"].as_i64().unwrap() as i32,
                    )
                })
                .collect()
        }),
        pollution_info: (!v["PollutionInfo"].is_null()).then(|| {
            v["PollutionInfo"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| {
                    (
                        r["key"].as_i64().unwrap() as i32,
                        r["value"].as_u64().unwrap() as u8,
                    )
                })
                .collect()
        }),
        effect_info: (!v["EffectInfo"].is_null()).then(|| {
            v["EffectInfo"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| {
                    (
                        r["key"].as_i64().unwrap() as i32,
                        r["value"].as_i64().unwrap() as i32,
                    )
                })
                .collect()
        }),
        decoration_info: (!v["DecorationInfo"].is_null()).then(|| {
            v["DecorationInfo"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| {
                    (
                        r["key"].as_i64().unwrap() as i32,
                        r["value"].as_i64().unwrap() as i32,
                    )
                })
                .collect()
        }),
        decoration_color_info: (!v["DecorationColorInfo"].is_null()).then(|| {
            v["DecorationColorInfo"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| {
                    (
                        r["key"].as_i64().unwrap() as i32,
                        r["value"].as_i64().unwrap() as i32,
                    )
                })
                .collect()
        }),
        shineness: v["Shineness"].as_i64().unwrap() as i32,
        height: v["Height"].as_i64().unwrap() as i32,
        bust: v["Bust"].as_i64().unwrap() as i32,
        painting_pos_x: v["PaintingPosX"].as_i64().unwrap() as i32,
        painting_pos_y: v["PaintingPosY"].as_i64().unwrap() as i32,
        painting_rotation: v["PaintingRotation"].as_i64().unwrap() as i32,
        painting_size: v["PaintingSize"].as_i64().unwrap() as i32,
        hide_head_costume: v["HideHeadCostume"].as_bool().unwrap(),
        cafe_type: v["CafeType"].as_i64().unwrap() as i32,
        is_return: v["IsReturn"].as_bool().unwrap(),
        vip_code: v["VIPCode"].as_i64().unwrap() as i32,
        body_painting_pos_x: v["BodyPaintingPosX"].as_i64().unwrap() as i32,
        body_painting_pos_y: v["BodyPaintingPosY"].as_i64().unwrap() as i32,
        body_painting_rotation: v["BodyPaintingRotation"].as_i64().unwrap() as i32,
        body_painting_size: v["BodyPaintingSize"].as_i64().unwrap() as i32,
        body_painting_side: v["BodyPaintingSide"].as_i64().unwrap() as i32,
        body_painting_clip: v["BodyPaintingClip"].as_i64().unwrap() as i32,
        body_painting_mode: v["BodyPaintingMode"].as_i64().unwrap() as i32,
        body_shape_info: (!v["BodyShapeInfo"].is_null()).then(|| {
            v["BodyShapeInfo"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| {
                    (
                        r["key"].as_i64().unwrap() as i32,
                        r["value"]["bits"].as_u64().unwrap() as u32,
                    )
                })
                .collect()
        }),
        town_effect: v["TownEffect"].as_i64().unwrap() as i32,
    }
}

fn summary(v: &Value) -> Summary<'_> {
    assert!(v["Pet"].is_null());
    Summary {
        cid: v["CID"].as_i64().unwrap(),
        nexon_sn: v["NexonSN"].as_i64().unwrap() as i32,
        character_id: v["CharacterID"].as_str(),
        character_sn: v["CharacterSN"].as_i64().unwrap() as i32,
        base_character: v["BaseCharacter"].as_u64().unwrap() as u8,
        level: v["Level"].as_i64().unwrap() as i32,
        exp: v["Exp"].as_i64().unwrap() as i32,
        level_up_exp: v["LevelUpExp"].as_i64().unwrap() as i32,
        title: v["Title"].as_i64().unwrap() as i32,
        title_count: v["TitleCount"].as_i64().unwrap() as i32,
        costume: (!v["Costume"].is_null()).then(|| costume(&v["Costume"])),
        daily_micro_play_count: v["DailyMicroPlayCount"].as_i64().unwrap() as i32,
        daily_free_play_count: v["DailyFreePlayCount"].as_i64().unwrap() as i32,
        quote: v["Quote"].as_str(),
        total_used_ap: v["TotalUsedAP"].as_i64().unwrap() as i32,
        guild_id: v["GuildId"].as_i64().unwrap() as i32,
        guild_name: v["GuildName"].as_str(),
        cafe_type: v["CafeType"].as_i64().unwrap() as i32,
        is_premium: v["IsPremium"].as_bool().unwrap(),
        has_bonus_effect: v["HasBonusEffect"].as_bool().unwrap(),
        is_return: v["IsReturn"].as_bool().unwrap(),
        is_deleting: v["IsDeleting"].as_bool().unwrap(),
        delete_wait_left_sec: v["DeleteWaitLeftSec"].as_i64().unwrap() as i32,
        is_should_name_change: v["IsShouldNameChange"].as_bool().unwrap(),
        vip_code: v["VIPCode"].as_i64().unwrap() as i32,
        vocation_class: v["VocationClass"].as_i64().unwrap() as i32,
        vocation_level: v["VocationLevel"].as_i64().unwrap() as i32,
        vocation_exp: v["VocationExp"].as_i64().unwrap() as i32,
        vocation_level_up_exp: v["VocationLevelUpExp"].as_i64().unwrap() as i32,
        vocation_skill_point_available: v["VocationSkillPointAvailable"].as_i64().unwrap() as i32,
        free_match_win_count: v["FreeMatchWinCount"].as_i64().unwrap() as i32,
        free_match_lose_count: v["FreeMatchLoseCount"].as_i64().unwrap() as i32,
        is_event_jumping: v["IsEventJumping"].as_bool().unwrap(),
        free_title_name: v["FreeTitleName"].as_str(),
        pattern: v["Pattern"].as_i64().unwrap() as i32,
        arena_win_count: v["ArenaWinCount"].as_i64().unwrap() as i32,
        arena_lose_count: v["ArenaLoseCount"].as_i64().unwrap() as i32,
        arena_successive_win_count: v["ArenaSuccessiveWinCount"].as_i64().unwrap() as i32,
    }
}

fn vector(v: &Value) -> Vector3D {
    Vector3D {
        x_bits: v["x_bits"].as_u64().unwrap() as u32,
        y_bits: v["y_bits"].as_u64().unwrap() as u32,
        z_bits: v["z_bits"].as_u64().unwrap() as u32,
    }
}

fn action(v: &Value) -> ActionSync {
    ActionSync {
        position: vector(&v["position"]),
        velocity: vector(&v["velocity"]),
        yaw_bits: v["yaw_bits"].as_u64().unwrap() as u32,
        sequence: v["sequence"].as_i64().unwrap() as i16,
        action_state_index: v["action_state_index"].as_i64().unwrap() as i16,
        start_time_bits: v["start_time_bits"].as_u64().unwrap() as u32,
        state: v["state"].as_i64().unwrap() as i32,
    }
}

fn kind_from_name(name: &str) -> Kind {
    match name {
        "notice_game_environment" => Kind::NoticeGameEnvironment,
        "identify" => Kind::Identify,
        "enter_channel" => Kind::EnterChannel,
        "channel_server_address" => Kind::ChannelServerAddress,
        "identify_failed" => Kind::IdentifyFailed,
        "move_partition" => Kind::MovePartition,
        "update_action" => Kind::UpdateAction,
        "disappeared" => Kind::Disappeared,
        "notify_look" => Kind::NotifyLook,
        "notify_action" => Kind::NotifyAction,
        "select_character" => Kind::SelectCharacter,
        "login_ok" => Kind::LoginOk,
        "character_common_info" => Kind::CharacterCommonInfo,
        "update_stat" => Kind::UpdateStat,
        "level_up" => Kind::LevelUp,
        other => panic!("unexpected kind {other}"),
    }
}

fn model<'a>(kind: Kind, v: &'a Value) -> Option<Message<'a>> {
    if v.is_null() {
        return None;
    }
    Some(match kind {
        Kind::NoticeGameEnvironment => Message::NoticeGameEnvironment {
            cafe_type: v["CafeType"].as_i64().unwrap() as i32,
            is_otp: v["IsOTP"].as_bool().unwrap(),
        },
        Kind::Identify => Message::Identify {
            id: v["ID"].as_i64().unwrap(),
            key: v["Key"].as_i64().unwrap() as i32,
        },
        Kind::EnterChannel => Message::EnterChannel {
            channel_id: v["ChannelID"].as_i64().unwrap(),
            partition_id: v["PartitionID"].as_i64().unwrap(),
            action: action(&v["Action"]),
        },
        Kind::ChannelServerAddress => Message::ChannelServerAddress {
            channel_id: v["ChannelID"].as_i64().unwrap(),
            address: v["Address"].as_str(),
            port: v["Port"].as_i64().unwrap() as i32,
            key: v["Key"].as_i64().unwrap() as i32,
        },
        Kind::IdentifyFailed => Message::IdentifyFailed(v["ErrorMessage"].as_str()),
        Kind::MovePartition => Message::MovePartition {
            target_partition_id: v["TargetPartitionID"].as_i64().unwrap(),
        },
        Kind::UpdateAction => Message::UpdateAction {
            data: action(&v["Data"]),
        },
        Kind::Disappeared => Message::Disappeared {
            id: v["ID"].as_i64().unwrap(),
        },
        Kind::NotifyLook => Message::NotifyLook {
            id: v["ID"].as_i64().unwrap(),
            look: (!v["Look"].is_null()).then(|| summary(&v["Look"])),
        },
        Kind::NotifyAction => Message::NotifyAction {
            id: v["ID"].as_i64().unwrap(),
            action: action(&v["Action"]),
        },
        Kind::SelectCharacter => Message::SelectCharacter {
            index: v["Index"].as_i64().unwrap() as i32,
        },
        Kind::LoginOk => Message::LoginOk {
            region_code: v["RegionCode"].as_i64().unwrap() as i32,
            time: v["Time"].as_i64().unwrap() as i32,
            limited: v["Limited"].as_i64().unwrap() as i32,
            facebook_token: v["FacebookToken"].as_str(),
            user_care_type: v["UserCareType"].as_i64().unwrap() as i32,
            user_care_next_state: v["UserCareNextState"].as_i64().unwrap() as i32,
            map_state_info: v["MapStateInfo"].as_u64().unwrap() as u8,
        },
        Kind::CharacterCommonInfo => Message::CharacterCommonInfo {
            query_id: v["QueryID"].as_i64().unwrap(),
            info: (!v["Info"].is_null()).then(|| summary(&v["Info"])),
        },
        Kind::LevelUp => Message::LevelUp {
            level: v["Level"].as_i64().unwrap() as i32,
            stat_increased: (!v["StatIncreased"].is_null()).then(|| {
                v["StatIncreased"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|r| {
                        (
                            r["key"].as_str().unwrap(),
                            r["value"].as_i64().unwrap() as i32,
                        )
                    })
                    .collect()
            }),
        },
        Kind::UpdateStat => Message::UpdateStat {
            stat: (!v["Stat"].is_null()).then(|| {
                v["Stat"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|r| {
                        (
                            r["key"].as_str().unwrap(),
                            r["value"].as_i64().unwrap() as i32,
                        )
                    })
                    .collect()
            }),
        },
    })
}

fn bytes(v: &Value) -> Vec<u8> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|b| b.as_u64().unwrap() as u8)
        .collect()
}

fn synth_dat_bytes(map: &Value) -> Vec<u8> {
    let mut out = vec![0u8; 24];
    let parts = map["partitions"].as_array().unwrap();
    let links = map["links"].as_array().unwrap();
    out.extend_from_slice(&(parts.len() as i32).to_le_bytes());
    for p in parts {
        let pid: i64 = p.as_str().unwrap().parse().unwrap();
        out.extend_from_slice(&pid.to_le_bytes());
    }
    for l in links {
        let lhs: i64 = l["lhs"].as_str().unwrap().parse().unwrap();
        let rhs: i64 = l["rhs"].as_str().unwrap().parse().unwrap();
        out.extend_from_slice(&lhs.to_le_bytes());
        out.extend_from_slice(&rhs.to_le_bytes());
    }
    assert_eq!(out.len() as u64, map["file_length"].as_u64().unwrap());
    out
}

#[test]
fn minimum_map_wire_and_plain_transport_match_framework_oracle() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/framework-minimum-map-wire.json")).unwrap();
    let profiles = fixture["profiles"].as_array().unwrap();
    assert_eq!(profiles.len(), 2);

    for profile in profiles {
        let cases = profile["wire_cases"].as_array().unwrap();
        assert_eq!(cases.len(), 201);
        let mut concat_stream = Vec::new();
        let mut expected_frames = Vec::new();

        for case in cases {
            let kind = kind_from_name(case["kind"].as_str().unwrap());
            let category = case["category"].as_i64().unwrap() as i32;
            let body = bytes(&case["body"]);
            let typed = bytes(&case["typed_body"]);
            let expected = model(kind, &case["fields"]);

            let encoded = codec::encode(kind, expected.as_ref(), DEFAULT_MAX_BODY).unwrap();
            assert_eq!(encoded.as_slice(), typed.as_slice());
            assert!(codec::decode(kind, &typed, DEFAULT_MAX_BODY).unwrap() == expected);
            assert!(
                codec::decode_ingress(kind, category, &body, DEFAULT_MAX_BODY).unwrap() == expected
            );

            let frame_bytes = Frame {
                instance_id: 0,
                category_id: category,
                body: &body,
            }
            .encode(DEFAULT_MAX_BODY)
            .unwrap();
            concat_stream.extend_from_slice(&frame_bytes);
            expected_frames.push((category, body));
        }

        let mut rx = PlainReceiver::new(DEFAULT_MAX_BODY);
        let mut actual_frames = Vec::new();
        for chunk in concat_stream.chunks(11) {
            for mut f in rx.push(chunk).unwrap() {
                assert_eq!(f.instance_id, 0);
                actual_frames.push((f.category_id, std::mem::take(&mut f.body)));
            }
        }
        assert_eq!(rx.buffered_len(), 0);
        assert_eq!(actual_frames, expected_frames);

        for epoch_case in profile["epoch_cases"].as_array().unwrap() {
            let ticks: i64 = epoch_case["ticks"].as_str().unwrap().parse().unwrap();
            let dt = SqlDateTime2::from_ticks(ticks).unwrap();
            assert_eq!(
                utility_epoch_seconds(dt),
                epoch_case["seconds"].as_i64().unwrap() as i32
            );
        }
    }

    assert_eq!(login_ok_region_code(1), 0);
    assert_eq!(login_ok_region_code(0), 0);
    assert_eq!(login_ok_region_code(2), 2);

    assert_eq!(
        codec::encode(
            Kind::Identify,
            Some(&Message::SelectCharacter { index: 0 }),
            DEFAULT_MAX_BODY
        ),
        Err(MinimumMapWireError::Kind)
    );
    assert_eq!(
        codec::encode(
            Kind::UpdateStat,
            Some(&Message::UpdateStat {
                stat: Some(vec![("ATK", 1), ("ATK", 2)])
            }),
            DEFAULT_MAX_BODY
        ),
        Err(MinimumMapWireError::DuplicateKey)
    );
}

#[test]
fn map_partition_parser_topology_and_channel_match_framework_oracle() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/framework-minimum-map-wire.json")).unwrap();
    let profile = &fixture["profiles"][0];

    let mut maps = Vec::new();
    let mut expected_build = Vec::new();
    for m in profile["maps"].as_array().unwrap() {
        let bsp = m["bsp"].as_str().unwrap().to_owned();
        let raw = synth_dat_bytes(m);
        let parsed = MapFile::parse(&bsp, &raw).unwrap();
        assert_eq!(parsed.bsp(), bsp);
        assert_eq!(
            parsed.partitions().len(),
            m["partitions"].as_array().unwrap().len()
        );
        assert_eq!(
            parsed.visible_links().len(),
            m["links"].as_array().unwrap().len()
        );
        expected_build.push((bsp, m["built"].as_bool().unwrap()));
        maps.push(parsed);
    }

    let topology = Topology::from_maps(&maps);
    assert_eq!(topology.build_results(), expected_build.as_slice());

    for n in profile["neighbors"].as_array().unwrap() {
        let pid: i64 = n["pid"].as_str().unwrap().parse().unwrap();
        let expected: Vec<i64> = n["visible"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().parse().unwrap())
            .collect();
        assert_eq!(
            topology.neighbor_partitions(pid),
            expected.as_slice(),
            "pid={pid}"
        );
    }

    let mut channel = Channel::new(topology);
    let steps = profile["channel_steps"].as_array().unwrap();
    assert_eq!(steps.len(), 8);

    // Extract look1, look2, actions[0], actions[1] from the step0 and step1 oracle packets!
    let step0_cam1 = steps[0]["cam1"].as_array().unwrap();
    let look1_pkt = OutgoingPacket {
        category: step0_cam1[0]["category"].as_i64().unwrap() as i32,
        body: bytes(&step0_cam1[0]["body"]),
    };
    let Some(Message::NotifyAction {
        action: action0, ..
    }) = codec::decode(
        Kind::NotifyAction,
        &bytes(&step0_cam1[1]["body"]),
        DEFAULT_MAX_BODY,
    )
    .unwrap()
    else {
        panic!("expected NotifyAction")
    };

    let step1_cam1 = steps[1]["cam1"].as_array().unwrap();
    let look2_pkt = OutgoingPacket {
        category: step1_cam1[0]["category"].as_i64().unwrap() as i32,
        body: bytes(&step1_cam1[0]["body"]),
    };
    let Some(Message::NotifyAction {
        action: action1, ..
    }) = codec::decode(
        Kind::NotifyAction,
        &bytes(&step1_cam1[1]["body"]),
        DEFAULT_MAX_BODY,
    )
    .unwrap()
    else {
        panic!("expected NotifyAction")
    };

    let p_colhen1 = 6479215318592913461_i64;
    let p_colhen2 = 6479215318592913462_i64;
    let p_colhen3 = 6479215318592913463_i64;
    let p_far: i64 = steps[4]["cam2_watching"].as_str().unwrap().parse().unwrap();

    for (idx, step) in steps.iter().enumerate() {
        let name = step["step"].as_str().unwrap();
        let out = match idx {
            0 => channel
                .enter_with_look_packet(1001, p_colhen1, look1_pkt.clone(), action0)
                .unwrap(),
            1 => channel
                .enter_with_look_packet(1002, p_colhen2, look2_pkt.clone(), action1)
                .unwrap(),
            2 => channel.update_action(1001, action1).unwrap(),
            3 => channel.move_partition(1002, p_colhen3).unwrap(),
            4 => channel.move_partition(1002, p_far).unwrap(),
            5 => channel.move_partition(1002, p_colhen1).unwrap(),
            6 => channel.leave(1001),
            7 => channel.leave(1002),
            _ => unreachable!(),
        };
        for (cid, key) in [(1001_i64, "cam1"), (1002_i64, "cam2")] {
            let expected: Vec<OutgoingPacket> = step[key]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| OutgoingPacket {
                    category: p["category"].as_i64().unwrap() as i32,
                    body: bytes(&p["body"]),
                })
                .collect();
            let actual = out.get(&cid).cloned().unwrap_or_default();
            assert_eq!(actual, expected, "step={name} camera={key}");
        }
    }

    let mut counts = std::collections::BTreeMap::new();
    assert_eq!(recommend_channel(1, 5, 5, &counts), 1);
    assert_eq!(recommend_channel(0, 5, 5, &counts), 1);
    counts.insert(1, 4);
    assert_eq!(recommend_channel(6, 5, 5, &counts), 2);
}
