use serde::Deserialize;
use vindictus_server::domain::equipment_slot::{Effect, Item, Operation, Slot};

#[derive(Deserialize)]
struct ItemInput {
    id: i64,
    category: Vec<u16>,
    equippable: bool,
    bad_status: bool,
    enhance_other: bool,
    equipped_part: Option<u8>,
    resolved_part: Option<i32>,
}
impl ItemInput {
    fn value(&self) -> Item {
        Item {
            id: self.id,
            category: bytes(&self.category),
            equippable: self.equippable,
            equipped_part: self.equipped_part,
            resolved_part: self.resolved_part,
            bad_status: self.bad_status,
            enhance_other: self.enhance_other,
        }
    }
}
fn bytes(v: &[u16]) -> Vec<u8> {
    v.iter().flat_map(|v| v.to_le_bytes()).collect()
}
#[derive(Deserialize)]
struct Row {
    category: Vec<u16>,
    expanded: bool,
}
#[derive(Deserialize)]
struct Step {
    action: String,
    item: Option<ItemInput>,
    part: i32,
    bad_status: bool,
}
#[derive(Deserialize)]
struct Input {
    part: u8,
    rows: Vec<Row>,
    steps: Vec<Step>,
}
#[derive(Deserialize)]
struct Outcome {
    result: Option<String>,
    loaded: Option<bool>,
    locked: bool,
    disabled: bool,
    id: Option<i64>,
    equipped_part: Option<u8>,
    cleared_id: Option<i64>,
    cleared_part: Option<u8>,
    effects: Vec<String>,
}
#[derive(Deserialize)]
struct Case {
    input: Input,
    outcomes: Vec<Outcome>,
}
#[derive(Deserialize)]
struct Fixture {
    x86: Vec<Case>,
    x64: Vec<Case>,
}
fn result_name(result: Operation) -> &'static str {
    match result {
        Operation::Success => "Success",
        Operation::InvalidItemClass => "InvalidItemClass",
        Operation::EquipSystem => "EquipItemFail_System",
        Operation::InvalidSlot => "EquipItemFail_InvalidSlot",
        Operation::InvalidCategory => "EquipItemFail_InvalidItemCategory",
        Operation::EnhanceSystem => "EnhanceItemFail_System",
    }
}
fn effect_name(effect: Effect) -> &'static str {
    match effect {
        Effect::CheckExpired => "CheckExpired",
        Effect::RegisterChange => "RegisterChange",
        Effect::EnhanceOther => "EnhanceOther",
        Effect::UpdateStat => "UpdateStat",
        Effect::UpdateItem => "UpdateItem",
        Effect::AddExpandedSlot => "AddExpandedSlot",
        Effect::UpdateLockedParts => "UpdateLockedParts",
    }
}
fn verify(cases: Vec<Case>) {
    assert_eq!(cases.len(), 128);
    for (i, case) in cases.into_iter().enumerate() {
        let mut slot = Slot::new(
            case.input.part,
            case.input
                .rows
                .iter()
                .map(|r| (Some(bytes(&r.category)), r.expanded)),
        )
        .unwrap();
        assert_eq!(slot.part(), case.input.part);
        assert_eq!(case.input.steps.len(), case.outcomes.len());
        for (j, (step, expected)) in case.input.steps.iter().zip(case.outcomes).enumerate() {
            let item = step.item.as_ref().map(ItemInput::value);
            let (mut result, mut loaded, mut cleared) = (None, None, None);
            let mut effects = Vec::new();
            match step.action.as_str() {
                "check" => result = Some(slot.check(item.as_ref()).unwrap()),
                "load" => loaded = Some(slot.load(item.unwrap()).unwrap()),
                "set" => {
                    let (r, e) = slot.set(item).unwrap();
                    result = Some(r);
                    effects = e;
                }
                "clear" => (cleared, effects) = slot.clear(),
                "reset" => {
                    let (r, e) = slot.reset(step.bad_status);
                    result = Some(r);
                    effects = e;
                }
                "load_expansion" => effects = slot.load_expansion(step.part),
                "unlock" => effects = slot.unlock(),
                _ => panic!("unknown fixture step"),
            }
            assert_eq!(
                result.map(result_name),
                expected.result.as_deref(),
                "{i}/{j}"
            );
            assert_eq!(loaded, expected.loaded, "{i}/{j}");
            assert_eq!(slot.locked(), expected.locked, "{i}/{j}");
            assert_eq!(slot.disabled(), expected.disabled, "{i}/{j}");
            assert_eq!(slot.value().map(|v| v.id), expected.id, "{i}/{j}");
            assert_eq!(
                slot.value().and_then(|v| v.equipped_part),
                expected.equipped_part,
                "{i}/{j}"
            );
            assert_eq!(
                cleared.as_ref().map(|v| v.id),
                expected.cleared_id,
                "{i}/{j}"
            );
            assert_eq!(
                cleared.as_ref().and_then(|v| v.equipped_part),
                expected.cleared_part,
                "{i}/{j}"
            );
            assert_eq!(
                effects.into_iter().map(effect_name).collect::<Vec<_>>(),
                expected.effects,
                "{i}/{j}"
            );
        }
    }
}
#[test]
fn original_framework_slot_transitions_and_effect_order_match_both_architectures() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("fixtures/framework-equipment-slot.json")).unwrap();
    verify(fixture.x86);
    verify(fixture.x64);
}
#[test]
fn invalid_metadata_or_category_refuses_before_mutation_and_never_repairs_rows() {
    assert!(Slot::new(0, []).is_err());
    assert!(Slot::new(0, [(None, false)]).is_err());
    assert!(Slot::new(0, [(Some(vec![1]), false)]).is_err());
    assert!(Slot::new(0, [(Some(vec![0; 1026]), false)]).is_err());
    assert!(Slot::new(0, [(Some(vec![]), false), (Some(vec![]), true)]).is_err());
    let mut slot = Slot::new(255, [(Some(vec![]), false)]).unwrap();
    let mut item = Item {
        id: 17,
        category: vec![0],
        equippable: true,
        equipped_part: None,
        resolved_part: None,
        bad_status: false,
        enhance_other: false,
    };
    assert!(slot.load(item.clone()).is_err());
    assert!(slot.set(Some(item.clone())).is_err());
    assert!(slot.value().is_none());
    item.category.clear();
    assert_eq!(slot.set(Some(item)).unwrap().0, Operation::Success);
    assert_eq!(slot.value().unwrap().id, 17);
    assert_eq!(slot.value().unwrap().equipped_part, Some(255));
}
