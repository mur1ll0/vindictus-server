use vindictus_server::domain::{
    feature_policy::FeaturePolicy,
    npc_script::{DefaultScript, Element, NpcScripts},
    npc_talk::{Facts, Session, TalkError, Talker},
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
    fn quest_progress(&self, _: &str) -> i32 {
        -1
    }
}
fn scripts() -> NpcScripts {
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
        vec![],
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
    let pushed = Talker::new(&s, &p, &facts, &[], &mut pick, 0, &mut session)
        .talk_to(Some("t01_bs"), "FERGHUS", None, Some("greeting"))
        .unwrap();
    assert_eq!(pushed.messages.len(), 1);
    assert_eq!(
        texts(&pushed.messages[0]),
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
    let pushed = Talker::new(&s, &p, &facts, &[], &mut pick, 0, &mut session)
        .select_button(1)
        .unwrap();
    assert_eq!(
        texts(&pushed.messages[0]),
        [
            t("talk", "ANSWER", ""),
            t("clear_button", "", ""),
            t("add_button", "bye", "endtalk"),
            t("finish_talk", "", ""),
        ]
    );
    let veteran = Fiona { level: 10 };
    let pushed = Talker::new(&s, &p, &veteran, &[], &mut pick, 0, &mut Session::default())
        .talk_to(Some("t01_bs"), "FERGHUS", None, Some("greeting"))
        .unwrap();
    assert_eq!(texts(&pushed.messages[0])[1], t("talk", "VETERAN", ""));
}

#[test]
fn server_commands_wait_for_execute_and_unmigrated_ones_change_nothing() {
    let s = scripts();
    let p = policy();
    let facts = Fiona { level: 1 };
    let mut pick = |_| 0;
    let mut session = Session::default();
    let pushed = Talker::new(&s, &p, &facts, &[], &mut pick, 0, &mut session)
        .talk_to(Some("t01_bs"), "FERGHUS", None, Some("shop"))
        .unwrap();
    assert_eq!(
        texts(&pushed.messages[0]),
        [
            t("change_npc", "FERGHUS", ""),
            t("talk", "BEFORE", ""),
            t("has_server_command", "", ""),
        ]
    );
    assert!(session.last_server_command.is_some());
    let result =
        Talker::new(&s, &p, &facts, &[], &mut pick, 0, &mut session).execute_server_command();
    assert_eq!(result, Err(TalkError::Unsupported("give_quest".to_owned())));
    // Unknown commands and NPCs answer with the source error talk and default buttons.
    let pushed = Talker::new(&s, &p, &facts, &[], &mut pick, 0, &mut Session::default())
        .talk_to(None, "FERGHUS", None, Some("missing"))
        .unwrap();
    assert_eq!(
        texts(&pushed.messages[0])[1],
        t("Error", "No Talk For FERGHUS/missing", "")
    );
}
