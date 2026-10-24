use vindictus_server::domain::{
    feature_policy::FeaturePolicy,
    npc_script::{DefaultScript, Element, NpcScripts, Progress, StoryLineInfo},
    npc_talk::{Facts, Output, Session, StoryState, TalkError, Talker},
};

fn el(name: &str, attributes: &[(&str, &str)], text: &str, children: Vec<Element>) -> Element {
    Element {
        name: name.to_owned(),
        attributes: attributes
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect(),
        text: text.to_owned(),
        children,
        tail: String::new(),
    }
}
fn leaf(name: &str, text: &str) -> Element {
    el(name, &[], text, Vec::new())
}
struct Fiona {
    level: i32,
}
impl Facts for Fiona {
    fn class(&self) -> i32 {
        1
    }
    fn level(&self) -> i32 {
        self.level
    }
    fn title_acquired(&self, id: i32) -> bool {
        id == 7
    }
    fn base_skill_rank(&self, skill: &str) -> i32 {
        if skill == "skill_fiona_guard" { 2 } else { -1 }
    }
    fn items(&self) -> Vec<(String, i32)> {
        vec![
            ("gold".to_owned(), 500),
            ("token_wolf_tooth".to_owned(), 3),
            ("token_wolf_claw".to_owned(), 2),
        ]
    }
    fn use_preset(&self) -> bool {
        true
    }
    fn utc_seconds(&self) -> i32 {
        1_000_000
    }
    fn item_count(&self, class: &str) -> i32 {
        self.items()
            .into_iter()
            .find(|(c, _)| c == class)
            .map_or(0, |(_, n)| n)
    }
    fn manufacture(&self, _: &str) -> Option<(i32, i32)> {
        None
    }
    fn quests(&self) -> Option<&vindictus_server::domain::quest::QuestContents> {
        None
    }
    fn repair_price(&self) -> i32 {
        0
    }
    fn gold(&self) -> i32 {
        500
    }
}
fn scripts() -> NpcScripts {
    scripts_with(vec![])
}
fn scripts_with(lines: Vec<(StoryLineInfo, Option<&Element>)>) -> NpcScripts {
    let menu = el(
        "ButtonSet",
        &[("id", "shop_menu")],
        "",
        vec![
            el(
                "Button",
                &[("caption", "buy"), ("command", "shop")],
                "",
                vec![],
            ),
            el(
                "Button",
                &[("caption", "ask")],
                "",
                vec![el("Do", &[], "", vec![leaf("talk", "ANSWER")])],
            ),
            el(
                "Button",
                &[("caption", "bye"), ("command", "endtalk")],
                "",
                vec![],
            ),
        ],
    );
    let root = el(
        "DefaultPhase",
        &[],
        "",
        vec![
            el(
                "ButtonSet",
                &[("id", "base_buttonset")],
                "",
                vec![el(
                    "Button",
                    &[("caption", "bye"), ("command", "endtalk")],
                    "",
                    vec![],
                )],
            ),
            menu,
            el(
                "Talk",
                &[
                    ("id", "FERGHUS"),
                    ("location", "t01_bs"),
                    ("command", "greeting"),
                ],
                "",
                vec![el(
                    "Do",
                    &[],
                    "",
                    vec![
                        el(
                            "If",
                            &[],
                            "",
                            vec![
                                el(
                                    "Condition",
                                    &[],
                                    "",
                                    vec![leaf("level_min", "10"), leaf("has_title", "7")],
                                ),
                                el("Then", &[], "", vec![leaf("talk", "VETERAN")]),
                                el("Else", &[], "", vec![leaf("talk", "ROOKIE")]),
                            ],
                        ),
                        leaf("button_set", "shop_menu"),
                    ],
                )],
            ),
            el(
                "Talk",
                &[
                    ("id", "FERGHUS"),
                    ("location", "t01_bs"),
                    ("command", "shop"),
                ],
                "",
                vec![el(
                    "Do",
                    &[],
                    "",
                    vec![
                        leaf("talk", "BEFORE"),
                        leaf("give_quest", "quest_x"),
                        leaf("talk", "AFTER"),
                    ],
                )],
            ),
            el(
                "Talk",
                &[
                    ("id", "AIDAN"),
                    ("location", "t01_inn"),
                    ("command", "greeting"),
                ],
                "",
                vec![leaf("talk", "HIDDEN")],
            ),
        ],
    );
    let other = el("DefaultPhase", &[], "", vec![]);
    NpcScripts::load(
        &[
            DefaultScript {
                feature: "",
                root: &root,
            },
            DefaultScript {
                feature: "x",
                root: &other,
            },
        ],
        lines,
    )
    .unwrap()
}
fn policy() -> FeaturePolicy {
    FeaturePolicy::npc_source([("CurrentVer".to_owned(), Some("3.09.00".to_owned()))]).unwrap()
}
fn texts(m: &[vindictus_server::domain::npc_talk::TalkEntity]) -> Vec<(String, String, String)> {
    m.iter()
        .map(|e| (e.command.clone(), e.arg1.clone(), e.arg2.clone()))
        .collect()
}
fn t(c: &str, a: &str, b: &str) -> (String, String, String) {
    (c.to_owned(), a.to_owned(), b.to_owned())
}

#[test]
fn greeting_runs_conditions_and_shows_the_menu_then_buttons_continue_the_query() {
    let s = scripts();
    let p = policy();
    let mut session = Session::default();
    let mut pick = |_| 0;
    let facts = Fiona { level: 1 };
    let pushed = Talker::new(
        &s,
        &p,
        &facts,
        &mut StoryState::default(),
        &mut pick,
        0,
        &mut session,
    )
    .talk_to(Some("t01_bs"), "FERGHUS", None, Some("greeting"))
    .unwrap();
    assert_eq!(pushed.messages().len(), 1);
    assert_eq!(
        texts(pushed.messages()[0]),
        [
            t("change_npc", "FERGHUS", ""),
            t("talk", "ROOKIE", ""),
            t("clear_button", "", ""),
            t("add_button", "buy", "shop"),
            t("add_button", "ask", ""),
            t("add_button", "bye", "endtalk"),
            t("finish_talk", "", ""),
        ]
    );
    assert!(session.can_start_query && session.last_button_set.is_some());
    // The action button continues the open query; the default button set closes it.
    let pushed = Talker::new(
        &s,
        &p,
        &facts,
        &mut StoryState::default(),
        &mut pick,
        0,
        &mut session,
    )
    .select_button(1)
    .unwrap();
    assert_eq!(
        texts(pushed.messages()[0]),
        [
            t("talk", "ANSWER", ""),
            t("clear_button", "", ""),
            t("add_button", "bye", "endtalk"),
            t("finish_talk", "", ""),
        ]
    );
    let veteran = Fiona { level: 10 };
    let pushed = Talker::new(
        &s,
        &p,
        &veteran,
        &mut StoryState::default(),
        &mut pick,
        0,
        &mut Session::default(),
    )
    .talk_to(Some("t01_bs"), "FERGHUS", None, Some("greeting"))
    .unwrap();
    assert_eq!(texts(pushed.messages()[0])[1], t("talk", "VETERAN", ""));
}

#[test]
fn server_commands_wait_for_execute_and_unmigrated_ones_change_nothing() {
    let s = scripts();
    let p = policy();
    let facts = Fiona { level: 1 };
    let mut pick = |_| 0;
    let mut session = Session::default();
    let pushed = Talker::new(
        &s,
        &p,
        &facts,
        &mut StoryState::default(),
        &mut pick,
        0,
        &mut session,
    )
    .talk_to(Some("t01_bs"), "FERGHUS", None, Some("shop"))
    .unwrap();
    assert_eq!(
        texts(pushed.messages()[0]),
        [
            t("change_npc", "FERGHUS", ""),
            t("talk", "BEFORE", ""),
            t("has_server_command", "", ""),
        ]
    );
    assert!(session.last_server_command.is_some());
    let result = Talker::new(
        &s,
        &p,
        &facts,
        &mut StoryState::default(),
        &mut pick,
        0,
        &mut session,
    )
    .execute_server_command();
    assert_eq!(result, Err(TalkError::Unsupported("give_quest".to_owned())));
    // Unknown commands and NPCs answer with the source error talk and default buttons.
    let pushed = Talker::new(
        &s,
        &p,
        &facts,
        &mut StoryState::default(),
        &mut pick,
        0,
        &mut Session::default(),
    )
    .talk_to(None, "FERGHUS", None, Some("missing"))
    .unwrap();
    assert_eq!(
        texts(pushed.messages()[0])[1],
        t("Error", "No Talk For FERGHUS/missing", "")
    );
}

fn story_scripts() -> NpcScripts {
    let talk = |npc: &str, children: Vec<Element>| {
        el(
            "Talk",
            &[("id", npc)],
            "",
            vec![el("Do", &[], "", children)],
        )
    };
    let story = el(
        "StoryLine",
        &[],
        "",
        vec![
            el(
                "Phase",
                &[("id", "A")],
                "",
                vec![talk(
                    "AIDAN",
                    vec![
                        leaf("talk", "WELCOME"),
                        leaf("reveal_storyline", ""),
                        leaf("set_guide", "landmark_inn,GUIDE_A"),
                        leaf("set_token", "count,2"),
                        leaf("increase_token", "count,3"),
                        leaf("give_item", "potion,2"),
                        leaf("goto_phase", "B"),
                    ],
                )],
            ),
            el(
                "Phase",
                &[("id", "B")],
                "",
                vec![talk(
                    "AIDAN",
                    vec![
                        el(
                            "If",
                            &[],
                            "",
                            vec![
                                el("Condition", &[], "", vec![leaf("token_is", "count,5")]),
                                el("Then", &[], "", vec![leaf("talk", "FIVE")]),
                                el("Else", &[], "", vec![leaf("talk", "OTHER")]),
                            ],
                        ),
                        leaf("take_item", "potion,1"),
                        leaf("clear_guide", ""),
                        leaf("set_token", "count,0"),
                        leaf("complete_storyline", ""),
                    ],
                )],
            ),
        ],
    );
    let info = StoryLineInfo {
        id: "Guide".to_owned(),
        xml_path: Some("QuestSet01".to_owned()),
        class_restriction: 1023,
        reward_gold: 40,
        reward_title: 410017,
        ..Default::default()
    };
    scripts_with(vec![(info, Some(&story))])
}
fn story_policy() -> FeaturePolicy {
    FeaturePolicy::npc_source([
        ("CurrentVer".to_owned(), Some("3.09.00".to_owned())),
        ("GoldRate_Story".to_owned(), Some("150".to_owned())),
    ])
    .unwrap()
}
/// Runs the query and every server command it stops at, collecting the outputs.
fn run(
    s: &NpcScripts,
    p: &FeaturePolicy,
    state: &mut StoryState,
    session: &mut Session,
    npc: &str,
) -> Vec<Output> {
    let facts = Fiona { level: 1 };
    let mut pick = |_| 0;
    let mut outputs = Talker::new(s, p, &facts, state, &mut pick, 0, session)
        .talk_to(None, npc, Some("Guide"), None)
        .unwrap()
        .outputs;
    while session.last_server_command.is_some() {
        let more = Talker::new(s, p, &facts, state, &mut pick, 0, session)
            .execute_server_command()
            .unwrap();
        outputs.extend(more.outputs);
    }
    outputs
}

#[test]
fn story_commands_change_phase_tokens_guide_and_items_in_source_order() {
    let s = story_scripts();
    let p = story_policy();
    let mut state = StoryState {
        progress: vec![Progress {
            story_line: "Guide".to_owned(),
            phase: Some("A".to_owned()),
            status: 255,
        }],
        guide: Some((String::new(), String::new())),
        ..Default::default()
    };
    let mut session = Session::default();
    let outputs = run(&s, &p, &mut state, &mut session, "AIDAN");
    // reveal_storyline makes the storyline Normal; goto_phase moves to B.
    assert_eq!(
        state.progress[0],
        Progress {
            story_line: "Guide".to_owned(),
            phase: Some("B".to_owned()),
            status: 0,
        }
    );
    assert_eq!(
        state.guide,
        Some(("landmark_inn".to_owned(), "GUIDE_A".to_owned()))
    );
    assert_eq!(
        state.variables,
        [("Guide".to_owned(), "count".to_owned(), 5)]
    );
    assert_eq!(state.item_delta, [("potion".to_owned(), 2)]);
    assert!(state.saves.is_empty());
    let guide = Output::Guide("landmark_inn".to_owned(), "GUIDE_A".to_owned());
    assert_eq!(outputs.iter().filter(|o| **o == guide).count(), 2);
    assert!(outputs.contains(&Output::Effect(
        vindictus_server::domain::npc_talk::Effect::GiveItem {
            class: "potion".to_owned(),
            count: 2
        }
    )));
    assert!(matches!(outputs.last(), Some(Output::SyncStory(rows)) if rows == &state.progress));
    let talk: Vec<_> = outputs
        .iter()
        .filter_map(|o| match o {
            Output::Talk(t) => Some(texts(t)),
            _ => None,
        })
        .flatten()
        .map(|e| e.0)
        .collect();
    assert!(talk.contains(&"reveal_storyline_successful".to_owned()));
    assert!(talk.contains(&"give_item_successful".to_owned()));
    // Phase B reads the token, takes the item, clears the guide and completes.
    let mut session = Session::default();
    let outputs = run(&s, &p, &mut state, &mut session, "AIDAN");
    assert_eq!(state.progress[0].phase, None);
    assert!(state.variables.is_empty());
    assert_eq!(state.item_delta, [("potion".to_owned(), 1)]);
    assert_eq!(state.guide, Some((String::new(), String::new())));
    let talk: Vec<_> = outputs
        .iter()
        .filter_map(|o| match o {
            Output::Talk(t) => Some(texts(t)),
            _ => None,
        })
        .flatten()
        .collect();
    assert!(talk.contains(&t("talk", "FIVE", "")));
    assert!(talk.contains(&t("take_item_successful", "potion", "1")));
    // RewardGold 40 at GoldRate_Story 150.
    assert!(talk.contains(&t("give_item_successful", "gold", "60")));
    assert!(talk.contains(&t("complete_storyline_successful", "Guide", "")));
    // RewardTitle: GiveReward(0, 0, null, {410017: true}) acquires the title.
    assert!(talk.contains(&t("acquire_title_successful", "410017", "")));
    assert!(outputs.contains(&Output::Effect(
        vindictus_server::domain::npc_talk::Effect::AcquireTitle(410017)
    )));
}

#[test]
fn take_item_without_the_item_answers_the_source_error() {
    let s = story_scripts();
    let p = story_policy();
    let mut state = StoryState {
        progress: vec![Progress {
            story_line: "Guide".to_owned(),
            phase: Some("B".to_owned()),
            status: 0,
        }],
        ..Default::default()
    };
    let mut session = Session::default();
    let outputs = run(&s, &p, &mut state, &mut session, "AIDAN");
    let talk: Vec<_> = outputs
        .iter()
        .filter_map(|o| match o {
            Output::Talk(t) => Some(texts(t)),
            _ => None,
        })
        .flatten()
        .collect();
    assert!(talk.contains(&t(
        "Error",
        "GameUI_Heroes_NPCTalkEntity_Talk_NullError,take_item",
        ""
    )));
    assert!(state.item_delta.is_empty());
    assert_eq!(state.progress[0].phase.as_deref(), Some("B"));
}

struct QuestFiona(vindictus_server::domain::quest::QuestContents);
impl Facts for QuestFiona {
    fn class(&self) -> i32 {
        1
    }
    fn level(&self) -> i32 {
        1
    }
    fn title_acquired(&self, _: i32) -> bool {
        false
    }
    fn base_skill_rank(&self, _: &str) -> i32 {
        -1
    }
    fn items(&self) -> Vec<(String, i32)> {
        Vec::new()
    }
    fn use_preset(&self) -> bool {
        false
    }
    fn utc_seconds(&self) -> i32 {
        0
    }
    fn item_count(&self, _: &str) -> i32 {
        0
    }
    fn manufacture(&self, _: &str) -> Option<(i32, i32)> {
        None
    }
    fn quests(&self) -> Option<&vindictus_server::domain::quest::QuestContents> {
        Some(&self.0)
    }
    fn repair_price(&self) -> i32 {
        0
    }
    fn gold(&self) -> i32 {
        0
    }
}

#[test]
fn give_quest_reveals_the_quest_pushes_progress_and_feeds_quest_conditions() {
    use vindictus_server::domain::quest::{QuestContents, QuestInfo, QuestTables};
    let cond = |name: &str, arg: &str, yes: &str, no: &str| {
        el(
            "If",
            &[],
            "",
            vec![
                el("Condition", &[], "", vec![leaf(name, arg)]),
                el("Then", &[], "", vec![leaf("talk", yes)]),
                el("Else", &[], "", vec![leaf("talk", no)]),
            ],
        )
    };
    let story = el(
        "StoryLine",
        &[],
        "",
        vec![el(
            "Phase",
            &[("id", "A")],
            "",
            vec![el(
                "Talk",
                &[("id", "AIDAN")],
                "",
                vec![el(
                    "Do",
                    &[],
                    "",
                    vec![
                        cond("has_quest", "quest_x", "OWNED", "NONE"),
                        leaf("give_quest", "quest_x"),
                        cond("has_quest", "quest_x", "OWNED", "NONE"),
                        cond("quest_progress", "quest_x,1", "P1", "P0"),
                        cond("quest_available", "quest_x", "FREE", "LOCKED"),
                        leaf("give_quest", "missing"),
                    ],
                )],
            )],
        )],
    );
    let info = StoryLineInfo {
        id: "Guide".to_owned(),
        xml_path: Some("QuestSet01".to_owned()),
        class_restriction: 1023,
        ..Default::default()
    };
    let s = scripts_with(vec![(info, Some(&story))]);
    let p = story_policy();
    let quest = QuestInfo {
        id: "quest_x".to_owned(),
        auto_give_level: 0,
        quest_set: 1,
        service_coin: 0,
        max_member_count: 4,
        max_play_count: None,
        enable_today_quest: None,
        season3_today_group: None,
        auto_reveal: false,
        feature: None,
        is_season2: false,
    };
    let contents = QuestContents::load(
        QuestTables {
            quests: vec![quest],
            ..Default::default()
        },
        &p,
    )
    .unwrap();
    let facts = QuestFiona(contents);
    let mut state = StoryState {
        progress: vec![Progress {
            story_line: "Guide".to_owned(),
            phase: Some("A".to_owned()),
            status: 0,
        }],
        ..Default::default()
    };
    state.quests.level = 1;
    let mut session = Session::default();
    let mut pick = |_| 0;
    let mut outputs = Talker::new(&s, &p, &facts, &mut state, &mut pick, 0, &mut session)
        .talk_to(None, "AIDAN", Some("Guide"), None)
        .unwrap()
        .outputs;
    while session.last_server_command.is_some() {
        outputs.extend(
            Talker::new(&s, &p, &facts, &mut state, &mut pick, 0, &mut session)
                .execute_server_command()
                .unwrap()
                .outputs,
        );
    }
    let talk: Vec<_> = outputs
        .iter()
        .filter_map(|o| match o {
            Output::Talk(t) => Some(texts(t)),
            _ => None,
        })
        .flatten()
        .collect();
    let said: Vec<&str> = talk
        .iter()
        .filter(|e| e.0 == "talk")
        .map(|e| e.1.as_str())
        .collect();
    assert_eq!(said, ["NONE", "OWNED", "P0", "FREE"]);
    assert!(talk.contains(&t("give_quest_successful", "quest_x", "")));
    assert!(talk.contains(&t(
        "Error",
        "GameUI_Heroes_NPCTalkEntity_Talk_NullError,give_quest",
        ""
    )));
    let owned = state.quests.owned("quest_x").unwrap();
    assert_eq!((owned.revealed, owned.row_id), (1, None));
    assert!(matches!(
        outputs.iter().find(|o| matches!(o, Output::QuestProgress(_))),
        Some(Output::QuestProgress(q)) if q.owned("quest_x").is_some()
    ));
}

#[test]
fn quest_finished_events_move_the_phase_and_name_the_story_sector() {
    use vindictus_server::domain::npc_script::QuestFinished;
    let on_event = |conditions: Vec<Element>, commands: Vec<Element>| {
        el(
            "OnEvent",
            &[],
            "",
            conditions.into_iter().chain(commands).collect(),
        )
    };
    let story = el(
        "StoryLine",
        &[],
        "",
        vec![
            el(
                "Phase",
                &[("id", "A")],
                "",
                vec![
                    on_event(
                        vec![
                            leaf("QuestCleared_Host", "quest_x"),
                            leaf("MonsterKilled", "boss,2"),
                        ],
                        vec![leaf("goto_phase", "C")],
                    ),
                    on_event(
                        vec![leaf("QuestCleared", " quest_x ")],
                        vec![leaf("goto_phase", "B"), leaf("story_sector", "905")],
                    ),
                ],
            ),
            el("Phase", &[("id", "B")], "", vec![]),
            el(
                "Phase",
                &[("id", "C")],
                "",
                vec![el("OnEnterPhase", &[], "", vec![leaf("talk", "x")])],
            ),
        ],
    );
    let info = StoryLineInfo {
        id: "Guide".to_owned(),
        xml_path: Some("QuestSet01".to_owned()),
        class_restriction: 1023,
        ..Default::default()
    };
    let s = scripts_with(vec![(info, Some(&story))]);
    let progress = || {
        vec![Progress {
            story_line: "Guide".to_owned(),
            phase: Some("A".to_owned()),
            status: 0,
        }]
    };
    let kills = [("boss".to_owned(), 1)];
    let event = |quest, success, is_host| QuestFinished {
        quest,
        success,
        boss_killed: &kills,
        is_host,
    };
    // The first handler needs two boss kills: the second handler moves to B.
    let mut rows = progress();
    let out = s.quest_finished(&mut rows, &event("quest_x", true, true));
    assert_eq!(
        (out.changed, out.story_sector, out.skipped_on_enter),
        (true, 905, 0)
    );
    assert_eq!(rows[0].phase.as_deref(), Some("B"));
    // A failed or other quest moves nothing.
    let mut rows = progress();
    let out = s.quest_finished(&mut rows, &event("quest_x", false, true));
    assert_eq!((out.changed, out.story_sector), (false, -1));
    // With enough boss kills the host handler goes to C (its OnEnterPhase is counted).
    let kills = [("boss".to_owned(), 2)];
    let mut rows = progress();
    let host = QuestFinished {
        quest: "quest_x",
        success: true,
        boss_killed: &kills,
        is_host: true,
    };
    let out = s.quest_finished(&mut rows, &host);
    assert_eq!(
        (rows[0].phase.as_deref(), out.skipped_on_enter),
        (Some("C"), 1)
    );
}

#[test]
fn notice_title_answers_successful_and_defers_the_title_effect() {
    use vindictus_server::domain::npc_talk::Effect;
    let story = el(
        "StoryLine",
        &[],
        "",
        vec![el(
            "Phase",
            &[("id", "A")],
            "",
            vec![el(
                "Talk",
                &[("id", "MARREC")],
                "",
                vec![el(
                    "Do",
                    &[],
                    "",
                    vec![
                        leaf("talk", "BEFORE"),
                        leaf("notice_title", " 410018 "),
                        leaf("talk", "AFTER"),
                    ],
                )],
            )],
        )],
    );
    let info = StoryLineInfo {
        id: "Guide".to_owned(),
        xml_path: Some("QuestSet01".to_owned()),
        class_restriction: 1023,
        ..Default::default()
    };
    let s = scripts_with(vec![(info, Some(&story))]);
    let p = story_policy();
    let mut state = StoryState {
        progress: vec![Progress {
            story_line: "Guide".to_owned(),
            phase: Some("A".to_owned()),
            status: 0,
        }],
        ..Default::default()
    };
    let mut session = Session::default();
    let outputs = run(&s, &p, &mut state, &mut session, "MARREC");
    let talk: Vec<_> = outputs
        .iter()
        .filter_map(|o| match o {
            Output::Talk(t) => Some(texts(t)),
            _ => None,
        })
        .flatten()
        .collect();
    // The dialog continues after the command (it used to stop as unsupported).
    assert!(talk.contains(&t("talk", "AFTER", "")));
    // titleID.ToString() of the script argument, as written.
    let noticed: Vec<_> = talk
        .iter()
        .filter(|e| e.0 == "notice_title_successful")
        .collect();
    assert_eq!(noticed.len(), 1, "{talk:?}");
    assert_eq!(noticed[0].1.trim(), "410018");
    assert!(outputs.contains(&Output::Effect(Effect::NoticeTitle(410018))));
}

#[test]
fn skill_conditions_read_the_base_rank_like_data_cache() {
    let cond = |name: &str, arg: &str, yes: &str, no: &str| {
        el(
            "If",
            &[],
            "",
            vec![
                el("Condition", &[], "", vec![leaf(name, arg)]),
                el("Then", &[], "", vec![leaf("talk", yes)]),
                el("Else", &[], "", vec![leaf("talk", no)]),
            ],
        )
    };
    let story = el(
        "StoryLine",
        &[],
        "",
        vec![el(
            "Phase",
            &[("id", "A")],
            "",
            vec![el(
                "Talk",
                &[("id", "CEARA")],
                "",
                vec![el(
                    "Do",
                    &[],
                    "",
                    vec![
                        cond("has_skill", "skill_fiona_guard", "HAS", "NOT"),
                        cond("has_skill", "skill_missing", "HAS", "NOT"),
                        cond("skill_rank_min", "skill_fiona_guard,2", "MIN_OK", "MIN_LOW"),
                        cond("skill_rank_min", "skill_fiona_guard,3", "MIN_OK", "MIN_LOW"),
                        cond(
                            "skill_rank_max",
                            "skill_fiona_guard,1",
                            "MAX_OK",
                            "MAX_HIGH",
                        ),
                        // A missing skill is rank -1: below any minimum, within any maximum.
                        cond("skill_rank_min", "skill_missing,0", "MIN_OK", "MIN_LOW"),
                        cond("skill_rank_max", "skill_missing,0", "MAX_OK", "MAX_HIGH"),
                    ],
                )],
            )],
        )],
    );
    let info = StoryLineInfo {
        id: "Guide".to_owned(),
        xml_path: Some("QuestSet01".to_owned()),
        class_restriction: 1023,
        ..Default::default()
    };
    let s = scripts_with(vec![(info, Some(&story))]);
    let p = story_policy();
    let mut state = StoryState {
        progress: vec![Progress {
            story_line: "Guide".to_owned(),
            phase: Some("A".to_owned()),
            status: 0,
        }],
        ..Default::default()
    };
    let mut session = Session::default();
    let outputs = run(&s, &p, &mut state, &mut session, "CEARA");
    let said: Vec<String> = outputs
        .iter()
        .filter_map(|o| match o {
            Output::Talk(t) => Some(texts(t)),
            _ => None,
        })
        .flatten()
        .filter(|e| e.0 == "talk")
        .map(|e| e.1)
        .collect();
    assert_eq!(
        said,
        [
            "HAS", "NOT", "MIN_OK", "MIN_LOW", "MAX_HIGH", "MIN_LOW", "MAX_OK"
        ]
    );
}

#[test]
fn story_commands_and_conditions_follow_process_command_func() {
    use vindictus_server::domain::npc_talk::Effect;
    let cond = |name: &str, arg: &str, yes: &str, no: &str| {
        el(
            "If",
            &[],
            "",
            vec![
                el("Condition", &[], "", vec![leaf(name, arg)]),
                el("Then", &[], "", vec![leaf("talk", yes)]),
                el("Else", &[], "", vec![leaf("talk", no)]),
            ],
        )
    };
    let guide = el(
        "StoryLine",
        &[],
        "",
        vec![el(
            "Phase",
            &[("id", "A")],
            "",
            vec![el(
                "Talk",
                &[("id", "MARREC")],
                "",
                vec![el(
                    "Do",
                    &[],
                    "",
                    vec![
                        cond("has_item_regex", "token_wolf_.*,5", "REGEX5", "REGEX_LOW"),
                        cond(
                            "has_item_regex",
                            "token_wolf_tooth,4",
                            "REGEX4",
                            "REGEX_LOW",
                        ),
                        cond("has_guild", "", "GUILD", "NO_GUILD"),
                        cond("is_cafe", "", "CAFE", "NO_CAFE"),
                        cond("is_account_secured", "", "OTP", "NO_OTP"),
                        cond("is_preset_character", "", "PRESET", "NOT_PRESET"),
                        leaf("set_timer", "t1"),
                        cond("time_passed", "t1,10", "PASSED", "WAIT"),
                        leaf("acquire_title", "410017"),
                        leaf("give_exp", "100"),
                        leaf("take_item_silent", "token_wolf_claw"),
                        cond("has_item_regex", "token_wolf_claw,1", "CLAW", "NO_CLAW"),
                        leaf("give_item_failmail", "potion,2"),
                        leaf("take_item_enhanced", "token_wolf_tooth,1"),
                        leaf("save_point", ""),
                        leaf("change_phase", "Side,Y"),
                        cond("storyline_is", "Side,Y", "SIDE_Y", "SIDE_X"),
                        leaf("delete_storyline", "Side"),
                        cond("has_storyline", "Side", "SIDE", "NO_SIDE"),
                        leaf("complete_storyline_and_delete", ""),
                        cond("has_storyline", "Guide", "GUIDE", "NO_GUIDE"),
                    ],
                )],
            )],
        )],
    );
    let side = el(
        "StoryLine",
        &[],
        "",
        vec![
            el("Phase", &[("id", "X")], "", vec![]),
            el("Phase", &[("id", "Y")], "", vec![]),
        ],
    );
    let line = |id: &str| StoryLineInfo {
        id: id.to_owned(),
        xml_path: Some("QuestSet01".to_owned()),
        class_restriction: 1023,
        ..Default::default()
    };
    let s = scripts_with(vec![
        (line("Guide"), Some(&guide)),
        (line("Side"), Some(&side)),
    ]);
    let p = story_policy();
    let mut state = StoryState {
        progress: vec![
            Progress {
                story_line: "Guide".to_owned(),
                phase: Some("A".to_owned()),
                status: 0,
            },
            Progress {
                story_line: "Side".to_owned(),
                phase: Some("X".to_owned()),
                status: 0,
            },
        ],
        ..Default::default()
    };
    let mut session = Session::default();
    let outputs = run(&s, &p, &mut state, &mut session, "MARREC");
    let talk: Vec<(String, String, String)> = outputs
        .iter()
        .filter_map(|o| match o {
            Output::Talk(t) => Some(texts(t)),
            _ => None,
        })
        .flatten()
        .collect();
    let said: Vec<&str> = talk
        .iter()
        .filter(|e| e.0 == "talk")
        .map(|e| e.1.as_str())
        .collect();
    assert_eq!(
        said,
        [
            "REGEX5",
            "REGEX_LOW",
            "NO_GUILD",
            "NO_CAFE",
            "NO_OTP",
            "PRESET",
            "WAIT",
            "NO_CLAW",
            "SIDE_Y",
            "NO_SIDE",
            "NO_GUIDE"
        ]
    );
    for expected in [
        t("acquire_title_successful", "410017", ""),
        t("give_item_successful", "potion", "2"),
        t("take_item_successful", "token_wolf_tooth", "1"),
        t("delete_storyline_successful", "Side", ""),
        t("complete_storyline_successful", "Guide", ""),
    ] {
        assert!(talk.contains(&expected), "{expected:?} in {talk:?}");
    }
    let effects: Vec<&Effect> = outputs
        .iter()
        .filter_map(|o| match o {
            Output::Effect(e) => Some(e),
            _ => None,
        })
        .collect();
    assert_eq!(
        effects,
        [
            &Effect::AcquireTitle(410017),
            &Effect::Reward { exp: 100, ap: 0 },
            &Effect::TakeItem {
                class: "token_wolf_claw".to_owned(),
                count: 2
            },
            &Effect::GiveItem {
                class: "potion".to_owned(),
                count: 2
            },
            &Effect::TakeItem {
                class: "token_wolf_tooth".to_owned(),
                count: 1
            },
        ]
    );
    // Both storylines are gone (deleted, and completed then deleted).
    assert!(state.progress.is_empty());
}

#[test]
fn take_item_enhanced_with_an_enhance_attribute_is_refused() {
    let story = el(
        "StoryLine",
        &[],
        "",
        vec![el(
            "Phase",
            &[("id", "A")],
            "",
            vec![el(
                "Talk",
                &[("id", "FERGHUS")],
                "",
                vec![el(
                    "Do",
                    &[],
                    "",
                    vec![leaf("take_item_enhanced", "longsword[ENHANCE:3],1")],
                )],
            )],
        )],
    );
    let info = StoryLineInfo {
        id: "Guide".to_owned(),
        xml_path: Some("QuestSet01".to_owned()),
        class_restriction: 1023,
        ..Default::default()
    };
    let s = scripts_with(vec![(info, Some(&story))]);
    let p = story_policy();
    let mut state = StoryState {
        progress: vec![Progress {
            story_line: "Guide".to_owned(),
            phase: Some("A".to_owned()),
            status: 0,
        }],
        ..Default::default()
    };
    let mut session = Session::default();
    let facts = Fiona { level: 1 };
    let mut pick = |_| 0;
    let first = Talker::new(&s, &p, &facts, &mut state, &mut pick, 0, &mut session).talk_to(
        None,
        "FERGHUS",
        Some("Guide"),
        None,
    );
    let result = match first {
        Ok(_) if session.last_server_command.is_some() => {
            Talker::new(&s, &p, &facts, &mut state, &mut pick, 0, &mut session)
                .execute_server_command()
                .map(|_| ())
        }
        other => other.map(|_| ()),
    };
    assert!(matches!(result, Err(TalkError::Unsupported(_))));
}

struct SkillFiona(
    std::collections::BTreeMap<String, vindictus_server::domain::skill_list::SkillContent>,
);
impl Facts for SkillFiona {
    fn class(&self) -> i32 {
        1
    }
    fn level(&self) -> i32 {
        1
    }
    fn title_acquired(&self, _: i32) -> bool {
        false
    }
    fn base_skill_rank(&self, skill: &str) -> i32 {
        if skill == "skill_fiona_guard" { 2 } else { -1 }
    }
    fn items(&self) -> Vec<(String, i32)> {
        Vec::new()
    }
    fn use_preset(&self) -> bool {
        false
    }
    fn utc_seconds(&self) -> i32 {
        0
    }
    fn item_count(&self, _: &str) -> i32 {
        0
    }
    fn manufacture(&self, _: &str) -> Option<(i32, i32)> {
        None
    }
    fn quests(&self) -> Option<&vindictus_server::domain::quest::QuestContents> {
        None
    }
    fn repair_price(&self) -> i32 {
        0
    }
    fn gold(&self) -> i32 {
        0
    }
    fn skill_contents(
        &self,
    ) -> Option<
        &std::collections::BTreeMap<String, vindictus_server::domain::skill_list::SkillContent>,
    > {
        Some(&self.0)
    }
}

#[test]
fn give_skill_grants_once_and_story_rewards_report_the_learn_result() {
    use vindictus_server::domain::{
        npc_talk::Effect,
        skill_list::{NewSkill, RankInfo, SkillContent},
    };
    let skill = |restriction: i32| {
        let mut c = SkillContent {
            class_restriction: Some(restriction),
            price: Some(-1),
            ..Default::default()
        };
        c.ranks.insert(
            1,
            RankInfo {
                level: 1,
                required_ap: 0,
                require_unlock: false,
                feature_enabled: true,
            },
        );
        c
    };
    let facts = SkillFiona(std::collections::BTreeMap::from([
        ("skill_fiona_smash3".to_owned(), skill(2)),
        ("skill_fiona_guard".to_owned(), skill(2)),
        ("skill_fiona_reward".to_owned(), skill(2)),
        ("skill_lethita_smash4".to_owned(), skill(1)),
    ]));
    let talk = |children| {
        el(
            "Talk",
            &[("id", "AIDAN")],
            "",
            vec![el("Do", &[], "", children)],
        )
    };
    let story = el(
        "StoryLine",
        &[],
        "",
        vec![
            el(
                "Phase",
                &[("id", "A")],
                "",
                vec![talk(vec![
                    leaf("give_skill", "skill_fiona_smash3"),
                    leaf("give_skill", "skill_fiona_smash3"),
                    leaf("give_skill", "skill_fiona_guard"),
                    leaf("complete_storyline", ""),
                ])],
            ),
            el(
                "Phase",
                &[("id", "B")],
                "",
                vec![talk(vec![leaf("give_skill", "skill_lethita_smash4")])],
            ),
        ],
    );
    let info = StoryLineInfo {
        id: "Guide".to_owned(),
        xml_path: Some("QuestSet01".to_owned()),
        class_restriction: 1023,
        reward_skill: Some("skill_fiona_reward".to_owned()),
        reward_skill_rank: 1,
        ..Default::default()
    };
    let s = scripts_with(vec![(info, Some(&story))]);
    let p = story_policy();
    let run = |phase: &str| {
        let mut state = StoryState {
            progress: vec![Progress {
                story_line: "Guide".to_owned(),
                phase: Some(phase.to_owned()),
                status: 0,
            }],
            ..Default::default()
        };
        let mut session = Session::default();
        let mut pick = |_| 0;
        let mut outputs = Talker::new(&s, &p, &facts, &mut state, &mut pick, 0, &mut session)
            .talk_to(None, "AIDAN", Some("Guide"), None)
            .unwrap()
            .outputs;
        while session.last_server_command.is_some() {
            let more = Talker::new(&s, &p, &facts, &mut state, &mut pick, 0, &mut session)
                .execute_server_command()
                .unwrap();
            outputs.extend(more.outputs);
        }
        let talk: Vec<_> = outputs
            .iter()
            .filter_map(|o| match o {
                Output::Talk(t) => Some(texts(t)),
                _ => None,
            })
            .flatten()
            .collect();
        let effects: Vec<_> = outputs
            .into_iter()
            .filter_map(|o| match o {
                Output::Effect(e) => Some(e),
                _ => None,
            })
            .collect();
        (talk, effects, state)
    };
    let new = NewSkill {
        price: 0,
        locked: 0,
        active: false,
    };
    // The second grant and an owned skill answer Already_Learnt: success, no effect.
    let (talk, effects, state) = run("A");
    assert_eq!(
        effects,
        [
            Effect::GiveSkill {
                skill: "skill_fiona_smash3".to_owned(),
                new,
                result: false
            },
            Effect::GiveSkill {
                skill: "skill_fiona_reward".to_owned(),
                new,
                result: true
            },
        ]
    );
    let successes = talk
        .iter()
        .filter(|e| e.0 == "give_skill_successful")
        .count();
    assert_eq!(successes, 3);
    assert_eq!(state.progress[0].phase, None);
    // A class the skill does not allow: OnError("give_skill"), the phase stays.
    let (talk, effects, state) = run("B");
    assert!(effects.is_empty());
    assert!(talk.contains(&t(
        "Error",
        "GameUI_Heroes_NPCTalkEntity_Talk_NullError,give_skill",
        ""
    )));
    assert_eq!(state.progress[0].phase.as_deref(), Some("B"));
}

struct CraftFiona(
    vindictus_server::domain::manufacture::Contents,
    std::cell::RefCell<vindictus_server::domain::manufacture::State>,
);
impl Facts for CraftFiona {
    fn class(&self) -> i32 {
        1
    }
    fn level(&self) -> i32 {
        1
    }
    fn title_acquired(&self, _: i32) -> bool {
        false
    }
    fn base_skill_rank(&self, _: &str) -> i32 {
        -1
    }
    fn items(&self) -> Vec<(String, i32)> {
        Vec::new()
    }
    fn use_preset(&self) -> bool {
        false
    }
    fn utc_seconds(&self) -> i32 {
        0
    }
    fn item_count(&self, _: &str) -> i32 {
        0
    }
    fn manufacture(&self, id: &str) -> Option<(i32, i32)> {
        self.1.borrow().skill(id).map(|s| (s.grade, s.experience))
    }
    fn quests(&self) -> Option<&vindictus_server::domain::quest::QuestContents> {
        None
    }
    fn repair_price(&self) -> i32 {
        0
    }
    fn gold(&self) -> i32 {
        0
    }
    fn manufacture_contents(&self) -> Option<&vindictus_server::domain::manufacture::Contents> {
        Some(&self.0)
    }
    fn manufacture_state(&self) -> vindictus_server::domain::manufacture::State {
        self.1.borrow().clone()
    }
}

#[test]
fn manufacture_commands_change_the_manager_and_feed_its_conditions() {
    use vindictus_server::domain::{
        manufacture::{Contents, Interval, RecipeRow},
        npc_talk::{Effect, ManufactureChange},
    };
    let contents = Contents::build(
        [RecipeRow {
            manufacture_id: "metal".into(),
            recipe_id: "sword".into(),
            experience_required: 50,
            grade_required: 0,
            base_reward_exp: 1,
            always_listed: 0,
            feature: None,
        }],
        [],
        [(
            "metal".to_owned(),
            Interval {
                grade: 0,
                min: 0,
                max: 99,
            },
        )],
        [],
        |_| true,
    )
    .unwrap();
    let story = el(
        "StoryLine",
        &[],
        "",
        vec![el(
            "Phase",
            &[("id", "A")],
            "",
            vec![el(
                "Talk",
                &[("id", "FERGHUS")],
                "",
                vec![el(
                    "Do",
                    &[],
                    "",
                    vec![
                        leaf("acquire_manufacture", "metal"),
                        el(
                            "If",
                            &[],
                            "",
                            vec![
                                el(
                                    "Condition",
                                    &[],
                                    "",
                                    vec![leaf("is_manufacture_level", "metal,0")],
                                ),
                                el("Then", &[], "", vec![leaf("talk", "LEARNT")]),
                                el("Else", &[], "", vec![leaf("talk", "UNKNOWN")]),
                            ],
                        ),
                        leaf("acquire_manufacture_recipe", "sword"),
                        leaf("levelup_manufacture", "metal"),
                        leaf("talk", "AFTER"),
                    ],
                )],
            )],
        )],
    );
    let info = StoryLineInfo {
        id: "Guide".to_owned(),
        xml_path: Some("QuestSet01".to_owned()),
        class_restriction: 1023,
        ..Default::default()
    };
    let s = scripts_with(vec![(info, Some(&story))]);
    let p = story_policy();
    let mut state = StoryState {
        progress: vec![Progress {
            story_line: "Guide".to_owned(),
            phase: Some("A".to_owned()),
            status: 0,
        }],
        ..Default::default()
    };
    let mut session = Session::default();
    let facts = CraftFiona(contents, Default::default());
    // Each server command is its own request; the item service saved the previous one.
    let save = |outputs: &[Output]| {
        let mut m = facts.1.borrow_mut();
        for o in outputs {
            match o {
                Output::Effect(Effect::Manufacture(ManufactureChange::Acquired(id))) => {
                    m.skills.push(vindictus_server::domain::manufacture::Skill {
                        id: id.clone(),
                        grade: 0,
                        experience: 0,
                    })
                }
                Output::Effect(Effect::Manufacture(ManufactureChange::Recipe { id, .. })) => {
                    m.recipes.push(id.clone())
                }
                _ => {}
            }
        }
    };
    let mut pick = |_| 0;
    let mut outputs = Talker::new(&s, &p, &facts, &mut state, &mut pick, 0, &mut session)
        .talk_to(None, "FERGHUS", Some("Guide"), None)
        .unwrap()
        .outputs;
    save(&outputs);
    while session.last_server_command.is_some() {
        let more = Talker::new(&s, &p, &facts, &mut state, &mut pick, 0, &mut session)
            .execute_server_command()
            .unwrap();
        save(&more.outputs);
        outputs.extend(more.outputs);
    }
    let effects: Vec<_> = outputs
        .iter()
        .filter_map(|o| match o {
            Output::Effect(Effect::Manufacture(c)) => Some(c.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        effects,
        [
            ManufactureChange::Acquired("metal".into()),
            ManufactureChange::Recipe {
                id: "sword".into(),
                new: true
            }
        ]
    );
    let talk: Vec<_> = outputs
        .iter()
        .filter_map(|o| match o {
            Output::Talk(t) => Some(texts(t)),
            _ => None,
        })
        .flatten()
        .collect();
    // The condition reads the acquired skill; the level-up below 99 experience fails.
    assert!(talk.contains(&t("talk", "LEARNT", "")), "{talk:?}");
    assert!(
        talk.iter()
            .any(|e| e.0 == "Error" && e.1.ends_with("levelup_manufacture")),
        "{talk:?}"
    );
    // Without manufacture contents the command is not migrated.
    let mut session = Session::default();
    let mut state2 = StoryState {
        progress: state.progress.clone(),
        ..Default::default()
    };
    let plain = Fiona { level: 1 };
    let mut result = Talker::new(&s, &p, &plain, &mut state2, &mut pick, 0, &mut session).talk_to(
        None,
        "FERGHUS",
        Some("Guide"),
        None,
    );
    while result.is_ok() {
        if session.last_server_command.is_none() {
            break;
        }
        result = Talker::new(&s, &p, &plain, &mut state2, &mut pick, 0, &mut session)
            .execute_server_command();
    }
    assert!(matches!(result, Err(TalkError::Unsupported(_))));
}
