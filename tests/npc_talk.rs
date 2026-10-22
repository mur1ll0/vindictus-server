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
    fn item_count(&self, class: &str) -> i32 {
        if class == "gold" { 500 } else { 0 }
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
        el("OnEvent", &[], "", conditions.into_iter().chain(commands).collect())
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
                        vec![leaf("QuestCleared_Host", "quest_x"), leaf("MonsterKilled", "boss,2")],
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
    assert_eq!((out.changed, out.story_sector, out.skipped_on_enter), (true, 905, 0));
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
    assert_eq!((rows[0].phase.as_deref(), out.skipped_on_enter), (Some("C"), 1));
}
