use serde_json::Value;
use vindictus_server::protocol::{
    DEFAULT_MAX_BODY,
    guild::{self as wire, GuildInfo, GuildMember, Kind, Message},
};

fn bytes(v: &Value) -> Vec<u8> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u8)
        .collect()
}
fn text(v: &Value) -> Option<String> {
    v.as_str().map(str::to_owned)
}
fn int(v: &Value) -> i64 {
    v.as_i64().unwrap()
}
fn check_info(i: &Option<GuildInfo>, f: &Value) {
    let Some(i) = i else {
        assert!(f.is_null());
        return;
    };
    assert_eq!(i64::from(i.guild_sn), int(&f["GuildSN"]));
    assert_eq!(i.guild_name, text(&f["GuildName"]));
    assert_eq!(i64::from(i.guild_level), int(&f["GuildLevel"]));
    assert_eq!(i64::from(i.member_count), int(&f["MemberCount"]));
    assert_eq!(i.master_name, text(&f["MasterName"]));
    assert_eq!(i64::from(i.max_member_count), int(&f["MaxMemberCount"]));
    assert_eq!(
        Some(i.is_newbie_recommend),
        f["IsNewbieRecommend"].as_bool()
    );
    assert_eq!(i.guild_point, int(&f["GuildPoint"]));
    assert_eq!(i.guild_notice, text(&f["GuildNotice"]));
    let theirs = &f["DailyGainGP"];
    assert_eq!(i.daily_gain_gp.0.is_none(), theirs.is_null());
    let rows = i.daily_gain_gp.0.clone().unwrap_or_default();
    let theirs = theirs.as_array().map(Vec::as_slice).unwrap_or_default();
    assert_eq!(rows.len(), theirs.len());
    for ((k, v), t) in rows.iter().zip(theirs) {
        assert_eq!(i64::from(*k), int(&t["key"]));
        assert_eq!(i64::from(*v), int(&t["value"]));
    }
}
fn check_member(m: &Option<GuildMember>, f: &Value) {
    let m = m.as_ref().unwrap();
    assert_eq!(m.character_name, text(&f["CharacterName"]));
    assert_eq!(i64::from(m.game_level), int(&f["GameLevel"]));
    assert_eq!(i64::from(m.rank), int(&f["Rank"]));
    assert_eq!(m.point, int(&f["Point"]));
    assert_eq!(i64::from(m.last_login_time), int(&f["LastLoginTime"]));
    assert_eq!(Some(m.is_online), f["IsOnline"].as_bool());
}

#[test]
fn framework_guild_messages_match_x86_x64() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/framework-guild-wire.json")).unwrap();
    assert_eq!(fixture["original_services_executed"], false);
    let profiles = fixture["profiles"].as_array().unwrap();
    assert_eq!(profiles[0]["wire_cases"], profiles[1]["wire_cases"]);
    let cases = profiles[0]["wire_cases"].as_array().unwrap();
    assert_eq!(cases.len(), 141);
    for (n, case) in cases.iter().enumerate() {
        let kind = match case["kind"].as_str().unwrap() {
            "open_guild" => Kind::OpenGuild,
            "guild_result" => Kind::GuildResult,
            "guild_info" => Kind::GuildInfo,
            "guild_member_list" => Kind::GuildMemberList,
            "query_guild_list" => Kind::QueryGuildList,
            "guild_list" => Kind::GuildList,
            "join_guild" => Kind::JoinGuild,
            "leave_guild" => Kind::LeaveGuild,
            "close_guild" => Kind::CloseGuild,
            "reconnect_guild" => Kind::ReconnectGuild,
            "guild_operation" => Kind::GuildOperation,
            "change_master" => Kind::ChangeMaster,
            "invite_guild" => Kind::InviteGuild,
            "guild_notice_edit" => Kind::GuildNoticeEdit,
            "guild_story_request" => Kind::GuildStoryRequest,
            _ => panic!("unknown fixture type"),
        };
        let category = case["category"].as_i64().unwrap() as i32;
        if case["mode"] == "registered-typed" {
            assert_eq!(category, kind.category());
        }
        let body = bytes(&case["body"]);
        let typed = bytes(&case["typed_body"]);
        let decoded = wire::decode(kind, &typed, DEFAULT_MAX_BODY).unwrap();
        assert_eq!(
            &*wire::encode(kind, decoded.as_ref(), DEFAULT_MAX_BODY).unwrap(),
            typed.as_slice(),
            "case {n}"
        );
        if Kind::INGRESS.contains(&kind) {
            assert_eq!(
                wire::decode_ingress(kind, category, &body, DEFAULT_MAX_BODY).unwrap(),
                decoded,
                "ingress case {n}"
            );
        }
        let f = &case["fields"];
        assert_eq!(decoded.is_none(), f.is_null(), "case {n}");
        match decoded {
            None => {}
            Some(Message::OpenGuild(o)) => {
                assert_eq!(o.guild_name, text(&f["GuildName"]));
                assert_eq!(o.guild_name_id, text(&f["GuildNameID"]));
                assert_eq!(o.guild_intro, text(&f["GuildIntro"]));
            }
            Some(Message::GuildResult(r)) => {
                assert_eq!(i64::from(r.result), int(&f["Result"]));
                assert_eq!(r.arg, text(&f["Arg"]));
                assert_eq!(r.guild_id, int(&f["GuildID"]));
            }
            Some(Message::GuildInfo(i)) => check_info(&i, &f["GuildInfo"]),
            Some(Message::GuildMemberList(l)) => {
                assert_eq!(Some(l.is_full_update), f["IsFullUpdate"].as_bool());
                let theirs = f["Members"].as_array().map(Vec::as_slice);
                assert_eq!(l.members.is_none(), theirs.is_none());
                for (m, t) in l
                    .members
                    .unwrap_or_default()
                    .iter()
                    .zip(theirs.unwrap_or_default())
                {
                    check_member(m, t);
                }
            }
            Some(Message::QueryGuildList(q)) => {
                assert_eq!(i64::from(q.query_type), int(&f["QueryType"]));
                assert_eq!(q.search_key, text(&f["SearchKey"]));
                assert_eq!(i64::from(q.page), int(&f["Page"]));
                assert_eq!(i64::from(q.page_size), int(&f["PageSize"]));
            }
            Some(Message::GuildList(l)) => {
                assert_eq!(i64::from(l.page), int(&f["Page"]));
                assert_eq!(i64::from(l.total_page), int(&f["TotalPage"]));
                let theirs = f["GuildList"].as_array().map(Vec::as_slice);
                assert_eq!(l.guild_list.is_none(), theirs.is_none());
                for (i, t) in l
                    .guild_list
                    .unwrap_or_default()
                    .iter()
                    .zip(theirs.unwrap_or_default())
                {
                    check_info(i, t);
                }
            }
            Some(Message::JoinGuild(sn)) => assert_eq!(i64::from(sn), int(&f["GuildSN"])),
            Some(Message::GuildOperation(ops)) => {
                let theirs = f["Operations"].as_array().map(Vec::as_slice);
                assert_eq!(ops.is_none(), theirs.is_none());
                for (o, t) in ops
                    .unwrap_or_default()
                    .iter()
                    .zip(theirs.unwrap_or_default())
                {
                    let o = o.as_ref().unwrap();
                    assert_eq!(o.command, text(&t["Command"]));
                    assert_eq!(o.target, text(&t["Target"]));
                    assert_eq!(i64::from(o.value), int(&t["Value"]));
                }
            }
            Some(Message::ChangeMaster(name)) => assert_eq!(name, text(&f["NewMasterName"])),
            Some(Message::InviteGuild(name)) => assert_eq!(name, text(&f["Name"])),
            Some(Message::GuildNoticeEdit(t)) => assert_eq!(t, text(&f["Text"])),
            Some(Message::LeaveGuild)
            | Some(Message::CloseGuild)
            | Some(Message::ReconnectGuild)
            | Some(Message::GuildStoryRequest) => {}
        }
    }
}
