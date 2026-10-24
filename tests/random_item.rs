use vindictus_server::domain::random_item::{
    Owner, Random, Recipe, RecipeClass, Unsupported, open,
};

struct Script(Vec<f64>, Vec<i32>);
impl Random for Script {
    fn next_double(&mut self) -> f64 {
        self.0.remove(0)
    }
    fn next(&mut self, maximum: i32) -> Option<i32> {
        let v = self.1.remove(0);
        assert!(v < maximum.max(1), "roll {v} >= {maximum}");
        Some(v)
    }
}
fn class(name: &str, probability: f64, number: i32, rule: Option<&str>) -> RecipeClass {
    RecipeClass {
        name: name.into(),
        expected: None,
        probability,
        number,
        rule: rule.map(str::to_owned),
    }
}
fn recipe(class: &str, probability: f64, character: Option<i32>, notify: &str) -> Recipe {
    Recipe {
        character,
        item_class: class.into(),
        number: 2,
        probability,
        colors: [None, Some(3), None],
        notify: Some(notify.into()),
        variable: 0,
        have: 0.0,
        dont_have: 0.0,
    }
}

#[test]
fn boxes_draw_a_class_then_its_items_like_open_bundle() {
    let classes = [
        class("a", 0.25, 1, None),
        class("b", 0.75, 1, None),
        class("extra", 0.0, 1, Some("x additional{0.5} y")),
    ];
    let recipes = |name: &str| match name {
        "b" => vec![
            recipe("potion", 0.5, None, "0"),
            recipe("sword", 0.25, Some(1), "1"),
            recipe("axe", 0.25, Some(3), "custom"),
        ],
        "extra" => vec![recipe("gem", 1.0, None, "custom")],
        _ => vec![recipe("stone", 1.0, None, "0")],
    };
    // NextDouble 0.3 skips a (0.25) and picks b; for Fiona (1) b has potion and sword:
    // Next(7_500_000) = 6_000_000 passes potion's 5_000_000 and takes the sword. The extra
    // class rolls 0.4 < 0.5 and gives its gem (all its items: count equals ItemNumber).
    let (rewards, _) = open(
        &classes,
        &recipes,
        1,
        None,
        true,
        &mut Script(vec![0.3, 0.4], vec![6_000_000]),
    )
    .unwrap();
    let got: Vec<_> = rewards
        .iter()
        .map(|r| {
            (
                r.item_class.as_str(),
                r.number,
                r.colors,
                r.notify.as_deref(),
                r.broadcast,
            )
        })
        .collect();
    assert_eq!(
        got,
        [
            (
                "sword",
                2,
                [-1, 3, -1],
                None,
                Some("GameUI_Heroes_RandomItem_GetItem_Plural_Telepathy")
            ),
            ("gem", 2, [-1, 3, -1], Some("custom"), None),
        ]
    );
    // Open (no bundle) ignores the additional classes and the owner.
    let (rewards, _) = open(
        &classes,
        &recipes,
        1,
        None,
        false,
        &mut Script(vec![0.1], vec![]),
    )
    .unwrap();
    assert_eq!(rewards.len(), 1);
    assert_eq!(rewards[0].item_class, "stone");
    // Rare and '$' bundle classes need the appearance/open tables.
    let rare = [RecipeClass {
        expected: Some(5),
        ..class("r", 1.0, 1, None)
    }];
    assert_eq!(
        open(&rare, &recipes, 1, None, true, &mut Script(vec![], vec![])),
        Err(Unsupported::RareClass)
    );
    let dollar = [class("$b{BundleNum%3}", 1.0, 1, None)];
    assert_eq!(
        open(
            &dollar,
            &recipes,
            1,
            None,
            true,
            &mut Script(vec![0.0], vec![])
        ),
        Err(Unsupported::BundleClass)
    );
}

#[test]
fn variable_recipes_follow_the_owned_items() {
    let classes = [class("v", 1.0, 1, None)];
    let recipes = |_: &str| {
        vec![
            Recipe {
                variable: 1,
                have: 0.1,
                dont_have: 0.6,
                ..recipe("cash_rare", 0.0, None, "0")
            },
            recipe("normal", 0.8, None, "0"),
        ]
    };
    let holds = |c: &str| c == "cash_rare";
    let storage = |_: &str| Some(1u8);
    let owner = Owner {
        holds: &holds,
        storage: &storage,
        constraint_check: true,
        variable_probability: true,
    };
    // Held: 0.1 × 1e7 for the variable recipe, 0.8 × (1 − 0.1) / 0.8 × 1e7 for the other.
    let (rewards, notes) = open(
        &classes,
        &recipes,
        1,
        Some(&owner),
        true,
        &mut Script(vec![0.0], vec![1_000_000]),
    )
    .unwrap();
    assert_eq!(rewards[0].item_class, "normal");
    // A variable recipe outside the cash/account storages answers the dialog.
    assert!(notes.variable_dialog);
}

#[test]
fn v2_sessions_pick_one_gachapon_recipe_and_each_package() {
    use vindictus_server::domain::random_item::{V2Data, V2Recipe, V2Unsupported, open_v2};
    let r = |class: Option<&str>, count: i32, probability: f64| V2Recipe {
        item_class: class.map(str::to_owned),
        count,
        probability,
        rare: None,
        character: None,
        item: None,
        feature_enabled: true,
        colors: [-1; 3],
        bind: false,
        expire: None,
        announce: None,
        message: Some("m".into()),
    };
    let top = V2Data {
        gachapon: vec![
            r(Some("a"), 1, 1.0),
            V2Recipe {
                character: Some("3".into()),
                ..r(Some("evie"), 1, 5.0)
            },
            r(Some("b"), 2, 3.0),
        ],
        package: vec![
            r(Some("#inner"), 1, 1.0),
            V2Recipe {
                item: Some("!owned".into()),
                ..r(Some("skipped"), 1, 1.0)
            },
            r(Some("lucky"), 1, 0.5),
        ],
        sequence: vec![],
    };
    let inner = V2Data {
        package: vec![r(None, 1, 1.0)],
        ..V2Data::default()
    };
    let data = |id: &str| match id {
        "top" => Some(&top),
        "inner" => Some(&inner),
        _ => None,
    };
    let holds = |c: &str| c == "owned";
    // Fiona's gachapon sums 4.0: 0.5 × 4 = 2.0 passes a (1.0) and takes b. The '#inner'
    // package rolls 0.0, lucky rolls 0.6 (kept out) and then inner opens: a null class gives
    // metal_clamp × 0.
    let opened = open_v2(
        &data,
        "top",
        1,
        &holds,
        &mut Script(vec![0.5, 0.0, 0.6, 0.0], vec![]),
    )
    .unwrap();
    let grants: Vec<_> = opened
        .grants
        .iter()
        .map(|g| (g.item_class.as_str(), g.count))
        .collect();
    assert_eq!(grants, [("b", 2), ("metal_clamp", 0)]);
    assert_eq!(
        opened.messages,
        [("m".into(), "b".into(), 2), ("m".into(), String::new(), 1)]
    );
    let rare = V2Data {
        package: vec![V2Recipe {
            rare: Some(7),
            ..r(Some("x"), 1, 1.0)
        }],
        ..V2Data::default()
    };
    assert_eq!(
        open_v2(
            &|_| Some(&rare),
            "r",
            1,
            &holds,
            &mut Script(vec![], vec![])
        ),
        Err(V2Unsupported::Rare)
    );
}
