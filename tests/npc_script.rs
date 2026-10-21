use vindictus_server::domain::npc_script::{
    Arena, DefaultScript, Element, Node, NpcScripts, StoryLineInfo,
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
fn base_buttons() -> Element {
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
    )
}
fn entity(
    arena: &Arena,
    id: usize,
) -> (
    &str,
    Option<&str>,
    Option<&str>,
    Option<&str>,
    Option<usize>,
) {
    let Node::Entity {
        command,
        arg1,
        arg2,
        talking_npc,
        next,
        ..
    } = &arena.nodes[id]
    else {
        panic!("entity expected")
    };
    (
        command,
        arg1.as_deref(),
        arg2.as_deref(),
        talking_npc.as_deref(),
        *next,
    )
}

#[test]
fn entities_parse_source_arguments_and_do_blocks_chain_their_commands() {
    let mut arena = Arena::default();
    let block = el(
        "Do",
        &[],
        "",
        vec![
            leaf("talk", "A\\nB, C"),
            el("goto_phase", &[("arg2", " x\\ny ")], " p1 , q , r", vec![]),
            leaf("change_npc", "GERENS"),
            leaf("talk", "after"),
        ],
    );
    let id = arena.load_do(&block).unwrap();
    arena.set_talking_npc(id, "FERGHUS");
    let Node::Do { commands, .. } = arena.nodes[id].clone() else {
        panic!("do expected")
    };
    assert_eq!(
        entity(&arena, commands[0]),
        (
            "talk",
            Some("A\nB, C"),
            None,
            Some("FERGHUS"),
            Some(commands[1])
        )
    );
    // Attributes win over the split value; arg2 keeps "\n" unescaped from text only.
    assert_eq!(
        entity(&arena, commands[1]),
        (
            "goto_phase",
            Some("p1"),
            Some("x\ny"),
            Some("FERGHUS"),
            Some(commands[2])
        )
    );
    assert_eq!(entity(&arena, commands[2]).3, Some("GERENS"));
    assert_eq!(entity(&arena, commands[3]).3, Some("GERENS"));
    // Do.Next reaches its last command; a trailing ButtonSet keeps its own Next.
    let tail = arena.load_command(&leaf("endtalk", "")).unwrap();
    arena.set_next(id, Some(tail));
    assert_eq!(entity(&arena, commands[3]).4, Some(tail));
    assert!(
        Arena::default()
            .load_command(&el("talk", &[("bogus", "1")], "", vec![]))
            .is_err()
    );
}

#[test]
fn base_info_keeps_insertion_order_features_and_rejects_source_load_errors() {
    let a = el(
        "DefaultPhase",
        &[],
        "",
        vec![
            base_buttons(),
            el(
                "Talk",
                &[
                    ("id", "FERGHUS"),
                    ("location", "t01_bs"),
                    ("command", "greeting"),
                ],
                "",
                vec![leaf("talk", "hi")],
            ),
            el(
                "Talk",
                &[("id", "AIDAN"), ("command", "greeting")],
                "",
                vec![leaf("talk", "hey")],
            ),
        ],
    );
    let b = el(
        "DefaultPhase",
        &[],
        "",
        vec![el(
            "Talk",
            &[
                ("id", "BRYN"),
                ("location", "t01_bs"),
                ("command", "greeting"),
            ],
            "",
            vec![el("talk", &[("id", "cmd1")], "x", vec![])],
        )],
    );
    let story = el(
        "StoryLine",
        &[],
        "",
        vec![el(
            "Phase",
            &[("id", "p0")],
            "",
            vec![
                el(
                    "Talk",
                    &[("id", "AIDAN")],
                    "",
                    vec![el("Do", &[], "", vec![leaf("reveal_storyline", "")])],
                ),
                el("DeleteNpc", &[("id", "BRYN")], "", vec![]),
            ],
        )],
    );
    let info = |id: &str, level: i32| StoryLineInfo {
        id: id.to_owned(),
        xml_path: Some("QuestSet01".to_owned()),
        auto_give_level: level,
        class_restriction: 1023,
        ..Default::default()
    };
    let scripts = NpcScripts::load(
        &[
            DefaultScript {
                feature: "",
                root: &a,
            },
            DefaultScript {
                feature: "flag",
                root: &b,
            },
        ],
        vec![
            (info("late", 3), Some(&story)),
            (info("first", 1), Some(&story)),
            (info("none", 0), Some(&story)),
        ],
    )
    .unwrap();
    let names: Vec<(&str, Vec<&str>)> = scripts
        .base
        .default_npcs
        .iter()
        .map(|(b, n)| (b.as_str(), n.iter().map(|(n, _)| n.as_str()).collect()))
        .collect();
    // Missing location while NpcBaseInfo is built is "t01_inn".
    assert_eq!(
        names,
        [
            ("t01_bs", vec!["FERGHUS", "BRYN"]),
            ("t01_inn", vec!["AIDAN"])
        ]
    );
    assert_eq!(
        (scripts.base.feature("BRYN"), scripts.base.feature("NOBODY")),
        ("flag", "")
    );
    assert!(
        scripts
            .base
            .command_talk("t01_bs", "FERGHUS", "greeting")
            .is_some()
    );
    let line = scripts.story_line("first").unwrap();
    let phase = line.initial_phase().unwrap();
    assert!(phase.talks[0].is_revealing);
    // After the base is built, storyline talks use GetDefaultBuilding.
    assert_eq!(phase.talks[0].key.0, "t01_inn");
    assert_eq!(
        phase.deleted_npcs,
        [("t01_bs".to_owned(), "BRYN".to_owned())]
    );
    // AutoGiveStoryLines keys in first-seen order, filtered by level.
    let ids = |level| {
        scripts
            .auto_give_for_login(level)
            .iter()
            .map(|l| l.info.id.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(ids(1), ["first"]);
    assert_eq!(ids(3), ["late", "first"]);
    // Duplicated server command IDs and a missing base_buttonset refuse the load.
    assert!(
        NpcScripts::load(
            &[
                DefaultScript {
                    feature: "",
                    root: &a
                },
                DefaultScript {
                    feature: "",
                    root: &b
                },
                DefaultScript {
                    feature: "",
                    root: &b
                },
            ],
            vec![],
        )
        .is_err()
    );
    assert!(
        NpcScripts::load(
            &[DefaultScript {
                feature: "",
                root: &b
            }],
            vec![]
        )
        .is_err()
    );
}
