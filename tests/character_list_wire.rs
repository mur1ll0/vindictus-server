use serde_json::Value;
use vindictus_server::protocol::character_list::{
    self as codec, CharacterListError, Costume, Message, Summary,
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
        pet: None,
        is_event_jumping: v["IsEventJumping"].as_bool().unwrap(),
        free_title_name: v["FreeTitleName"].as_str(),
        pattern: v["Pattern"].as_i64().unwrap() as i32,
        arena_win_count: v["ArenaWinCount"].as_i64().unwrap() as i32,
        arena_lose_count: v["ArenaLoseCount"].as_i64().unwrap() as i32,
        arena_successive_win_count: v["ArenaSuccessiveWinCount"].as_i64().unwrap() as i32,
    }
}
fn model(v: &Value) -> Option<Message<'_>> {
    if v.is_null() {
        return None;
    }
    Some(Message {
        characters: (!v["Characters"].is_null()).then(|| {
            v["Characters"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| (!r.is_null()).then(|| summary(r)))
                .collect()
        }),
        max_free: v["MaxFreeCharacterCount"].as_i64().unwrap() as i32,
        max_purchased: v["MaxPurchasedCharacterCount"].as_i64().unwrap() as i32,
        max_premium: v["MaxPremiumCharacters"].as_i64().unwrap() as i32,
        prologue_played: v["ProloguePlayed"].as_u64().unwrap() as u8,
        preset_used: v["PresetUsedCharacterCount"].as_i64().unwrap() as i32,
        party_states: (!v["LoginPartyState"].is_null()).then(|| {
            v["LoginPartyState"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| r.as_i64().unwrap() as i32)
                .collect()
        }),
    })
}
fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "fixtures/framework-full-character-list-wire.json"
    ))
    .unwrap()
}
fn bytes(v: &Value) -> Vec<u8> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|b| b.as_u64().unwrap() as u8)
        .collect()
}
#[test]
fn filled_character_list_matches_recovered_fields_and_bytes_for_each_architecture_and_envelope() {
    let f = fixture();
    assert_eq!(f["profiles"].as_array().unwrap().len(), 2);
    for profile in f["profiles"].as_array().unwrap() {
        let cases = profile["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 273);
        for case in cases {
            let body = bytes(&case["body"]);
            let typed = bytes(&case["typed_body"]);
            let expected = model(&case["fields"]);
            let encoded = codec::encode(expected.as_ref(), 65536).unwrap();
            assert_eq!(encoded.as_slice(), typed.as_slice());
            let observed =
                codec::decode_ingress(case["category"].as_i64().unwrap() as i32, &body, 65536)
                    .unwrap();
            assert!(observed == expected);
        }
    }
}
#[test]
fn filled_character_list_rejects_truncation_trailing_bytes_size_and_duplicate_dictionary_keys() {
    let f = fixture();
    let c = &f["profiles"][0]["cases"][9];
    let mut expected = model(&c["fields"]);
    let body = codec::encode(expected.as_ref(), 65536).unwrap();
    for length in 0..body.len() {
        assert!(codec::decode(&body[..length], 65536).is_err());
    }
    let mut trailing = body.to_vec();
    trailing.push(0);
    assert!(codec::decode(&trailing, 65536).is_err());
    assert!(codec::decode(&body, body.len() - 1).is_err());
    assert!(codec::encode(expected.as_ref(), body.len() - 1).is_err());
    let costume = expected.as_mut().unwrap().characters.as_mut().unwrap()[0]
        .as_mut()
        .unwrap()
        .costume
        .as_mut()
        .unwrap();
    costume.color_info = Some(vec![(1, 2), (1, 3)]);
    assert!(matches!(
        codec::encode(expected.as_ref(), 65536),
        Err(CharacterListError::DuplicateKey)
    ));
}
