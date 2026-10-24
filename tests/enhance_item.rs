use vindictus_server::domain::{
    character_stats::Arithmetic,
    enhance::{EnhanceInfo, EnhanceStat, Recipe, Rune, RuneExtra, StatRows},
    enhance_item::{
        Data, Features, Outcome, Random, Refusal, Request, Target, Unsupported, prepare, resolve,
    },
};

fn info(level: i32, on_fail: &str) -> EnhanceInfo {
    EnhanceInfo {
        level,
        material1: Some("stone".into()),
        num1: 3,
        material2: Some("elixir".into()),
        num2: 0,
        gold: 6500,
        success_ratio: 1.0,
        on_fail: Some(on_fail.into()),
        on_fail_recipe_ratio: if on_fail == "Destroy" { 0.5 } else { 0.0 },
        on_fail_material_num1: if on_fail == "Destroy" { 21 } else { 0 },
        effect: level,
    }
}
fn units(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}
struct Fake {
    infos: Vec<EnhanceInfo>,
    rows: Vec<(i32, Vec<EnhanceStat>)>,
    recipe: Recipe,
}
impl Fake {
    fn new() -> Self {
        let durability = |v: f32| EnhanceStat {
            stat: Some(units("MaxDurability")),
            value_type: Some("Add".into()),
            value: v,
            base: Some(units("MaxDurability")),
        };
        Self {
            infos: vec![
                info(4, "RankDown"),
                info(5, "RankDown"),
                info(6, "RankReset"),
                info(9, "Destroy"),
                info(7, "NoPenalty"),
                info(3, "RankDown"),
            ],
            rows: vec![
                (4, vec![durability(600.0)]),
                (5, vec![durability(850.0)]),
                (6, vec![durability(1100.0)]),
            ],
            recipe: Recipe {
                materials: vec![("ingot".into(), 6), ("gold".into(), 900)],
            },
        }
    }
}
impl Data for Fake {
    fn info(&self, level: i32) -> Option<&EnhanceInfo> {
        self.infos.iter().find(|i| i.level == level)
    }
    fn stat_rows(&self, level: i32) -> Option<StatRows<'_>> {
        self.info(level)?;
        Some(
            self.rows
                .iter()
                .find(|(l, _)| *l == level)
                .map(|(_, r)| r.as_slice()),
        )
    }
    fn server_ratio(&self, level: i32) -> Option<f32> {
        (level <= 15).then_some(if level <= 2 { 1.0 } else { 0.5 })
    }
    fn rune(&self, class: &str) -> Option<Rune> {
        match class {
            "protect" => Some(Rune {
                relation: 1,
                min_level: 4,
                max_level: 15,
            }),
            "premium" => Some(Rune {
                relation: 2,
                min_level: 1,
                max_level: 15,
            }),
            _ => None,
        }
    }
    fn rune_extra(&self, relation: i32) -> Option<RuneExtra> {
        Some(match relation {
            1 => RuneExtra {
                success_ratio: 0.9,
                use_item_success_ratio: false,
                destroy_on_success: false,
                reset_rank_on_fail: true,
                force_rank_down_on_fail: 0,
            },
            2 => RuneExtra {
                success_ratio: 0.1,
                use_item_success_ratio: true,
                destroy_on_success: true,
                reset_rank_on_fail: false,
                force_rank_down_on_fail: 3,
            },
            _ => return None,
        })
    }
    fn bonus(&self, material: &str, _: i32) -> Option<f64> {
        (material == "gevne").then_some(3.3)
    }
    fn instead(&self, material: &str, _: i32) -> Option<(String, i32)> {
        (material == "gevne").then(|| ("gevne".to_owned(), 1))
    }
    fn recipe(&self) -> Option<&Recipe> {
        Some(&self.recipe)
    }
    fn price(&self, _: &str, num: i32) -> Option<i32> {
        Some(num)
    }
}
struct Script {
    doubles: Vec<f64>,
    ints: Vec<i32>,
    calls: Vec<i32>,
}
impl Random for Script {
    fn next_double(&mut self) -> f64 {
        self.doubles.remove(0)
    }
    fn next(&mut self, maximum: i32) -> Option<i32> {
        self.calls.push(maximum);
        (maximum >= 0).then(|| self.ints.remove(0).min(maximum.max(1) - 1))
    }
}
fn features() -> Features {
    Features {
        event_enhance_a_shot: false,
        event_change_ratio: false,
        success_bonus: Some(0),
        success_bonus_limit: Some(10),
        gold_class: "gold".into(),
    }
}
fn request<'a>(m1: &'a str, m2: &'a str, add: &'a str) -> Request<'a> {
    Request {
        material1: m1,
        material2: m2,
        additional: add,
        event: false,
    }
}
fn target(level: i32) -> Target {
    Target {
        level,
        max_armor_broken: false,
        guild_price_bonus: 0,
    }
}
fn run(
    req: &Request<'_>,
    level: i32,
    doubles: Vec<f64>,
    ints: Vec<i32>,
) -> (Result<Outcome, Unsupported>, Vec<i32>) {
    let fake = Fake::new();
    let prepared = prepare(req, &target(level), &fake, &features(), &|_| 1)
        .unwrap()
        .unwrap();
    let mut random = Script {
        doubles,
        ints,
        calls: Vec::new(),
    };
    let outcome = resolve(
        &prepared,
        req,
        &target(level),
        &fake,
        &mut random,
        Arithmetic::FrameworkX64,
    );
    (outcome, random.calls)
}

#[test]
fn materials_ratios_and_refusals_follow_the_processor() {
    let fake = Fake::new();
    let check =
        |req: &Request<'_>, level: i32| prepare(req, &target(level), &fake, &features(), &|_| 1);
    // Gold, then Material1 and Material2 when the request names them.
    let p = check(&request("stone", "elixir", ""), 3).unwrap().unwrap();
    assert_eq!(
        p.materials,
        [
            ("gold".to_owned(), 6500),
            ("stone".to_owned(), 3),
            ("elixir".to_owned(), 0)
        ]
    );
    assert_eq!(
        (p.next_level, p.success_ratio, p.bonus_ratio),
        (4, 0.5, 0.0)
    );
    // A substitute material adds its bonus (×0.01f) and replaces the slot's material.
    let p = check(&request("gevne", "", ""), 3).unwrap().unwrap();
    assert_eq!(
        p.materials,
        [("gold".to_owned(), 6500), ("gevne".to_owned(), 1)]
    );
    assert_eq!(p.bonus_ratio, 3.3 * 0.009999999776482582);
    // An unknown first material, a missing rune and the rune range are refusals.
    assert_eq!(
        check(&request("other", "", ""), 3).unwrap(),
        Err(Refusal::NoEnhanceInfo)
    );
    assert_eq!(
        check(&request("stone", "", "unknown"), 3).unwrap(),
        Err(Refusal::NotEnoughMaterial)
    );
    assert_eq!(
        check(&request("stone", "", "protect"), 2).unwrap(),
        Err(Refusal::NotEnoughMaterial)
    );
    assert_eq!(
        prepare(
            &request("stone", "", "protect"),
            &target(3),
            &fake,
            &features(),
            &|_| 0
        )
        .unwrap(),
        Err(Refusal::NotEnoughMaterial)
    );
    // A rune without item ratio replaces the server ratio, capped at 0.4f.
    let p = check(&request("stone", "", "protect"), 3).unwrap().unwrap();
    assert_eq!(p.success_ratio, 0.4000000059604645);
    let p = check(&request("stone", "", "premium"), 3).unwrap().unwrap();
    assert_eq!(p.success_ratio, 0.5);
    // Broken max armor, the event request and a missing next EnhanceInfo.
    let broken = Target {
        max_armor_broken: true,
        ..target(3)
    };
    assert_eq!(
        prepare(
            &request("stone", "", ""),
            &broken,
            &fake,
            &features(),
            &|_| 1
        )
        .unwrap(),
        Err(Refusal::MaxArmorBroken)
    );
    let event = Request {
        event: true,
        ..request("stone", "", "")
    };
    assert_eq!(check(&event, 3).unwrap(), Err(Refusal::System));
    assert_eq!(check(&request("stone", "", ""), 10), Err(Unsupported));
    // Guild discount: Gold × (100 - bonus) / 100.
    let guild = Target {
        guild_price_bonus: 10,
        ..target(3)
    };
    let p = prepare(
        &request("stone", "", ""),
        &guild,
        &fake,
        &features(),
        &|_| 1,
    )
    .unwrap()
    .unwrap();
    assert_eq!(p.materials[0], ("gold".to_owned(), 5850));
}

#[test]
fn success_binds_from_nine_and_failures_take_their_branch() {
    // Success at the boundary roll; +9 binds the item.
    let (o, _) = run(&request("stone", "", ""), 3, vec![0.5], vec![]);
    let o = o.unwrap();
    assert_eq!(
        (o.success, o.level, o.bind, o.result.as_deref()),
        (true, 4, false, Some("Success"))
    );
    let (o, _) = run(&request("stone", "", ""), 8, vec![0.0], vec![]);
    let o = o.unwrap();
    assert_eq!((o.level, o.bind), (9, true));
    // RankDown: one level down, then Next(100) >= 30 - 2×(5-1) loses max durability:
    // Next((250 × 2 + 99) / 100) → (1 + draw) × 100.
    let (o, calls) = run(&request("stone", "", ""), 4, vec![0.9], vec![22, 3]);
    let o = o.unwrap();
    assert_eq!(
        (o.success, o.level, o.result.as_deref()),
        (false, 3, Some("RankDown"))
    );
    assert_eq!(calls, [100, 5]);
    assert_eq!(o.max_durability_loss, 400);
    // Below the threshold nothing is lost.
    let (o, calls) = run(&request("stone", "", ""), 4, vec![0.9], vec![21]);
    assert_eq!((o.unwrap().max_durability_loss, calls), (0, vec![100]));
    // RankReset to 0; Destroy marks the item and gives back material + half the recipe.
    let (o, _) = run(&request("stone", "", ""), 5, vec![0.9], vec![0]);
    assert_eq!(o.unwrap().level, 0);
    let (o, _) = run(&request("stone", "", ""), 8, vec![0.9], vec![]);
    let o = o.unwrap();
    assert!(o.destroyed);
    assert_eq!(o.result.as_deref(), Some("Destroy"));
    assert_eq!(o.give, [("stone".to_owned(), 21), ("ingot".to_owned(), 3)]);
    assert_eq!(o.fail_reward, o.give);
    // NoPenalty keeps the level and still answers with its OnFail name.
    let (o, _) = run(&request("stone", "", ""), 6, vec![0.9], vec![]);
    let o = o.unwrap();
    assert_eq!(
        (o.level, o.result.as_deref(), o.destroyed),
        (6, Some("NoPenalty"), false)
    );
}

#[test]
fn runes_protect_the_item_and_void_the_fail_rewards() {
    // Reset rune: level 0, rune destroyed, no reward.
    let (o, _) = run(&request("stone", "", "protect"), 8, vec![0.99], vec![]);
    let o = o.unwrap();
    assert_eq!(
        (o.level, o.destroy_rune, o.destroyed, o.result.as_deref()),
        (0, true, false, Some("ProtectRune_Fail"))
    );
    assert!(o.give.is_empty() && o.fail_reward.is_empty());
    // Rank-down rune: level - 3 (not below 0); on success the rune is consumed.
    let (o, _) = run(&request("stone", "", "premium"), 5, vec![0.99], vec![]);
    assert_eq!(o.unwrap().level, 2);
    let (o, _) = run(&request("stone", "", "premium"), 2, vec![0.99], vec![]);
    assert_eq!(o.unwrap().level, 0);
    let (o, _) = run(&request("stone", "", "premium"), 2, vec![0.0], vec![]);
    let o = o.unwrap();
    assert_eq!(o.destroy.last(), Some(&("premium".to_owned(), 1)));
}
