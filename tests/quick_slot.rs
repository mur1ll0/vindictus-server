use vindictus_server::domain::quick_slot::{Change, QuickSlots, Refusal};

fn slots() -> QuickSlots {
    QuickSlots::new([
        (3, false, vec!["SUBWEAPON".to_owned()]),
        (100, false, vec!["POTION".to_owned(), "FEATHER".to_owned()]),
        (101, false, vec!["POTION".to_owned()]),
        (102, true, vec!["POTION".to_owned()]),
    ])
    .unwrap()
}

#[test]
fn info_lists_unlocked_slots_in_content_order_with_empty_values() {
    let mut s = slots();
    assert_eq!(
        s.info(),
        [
            (3, Some(String::new())),
            (100, Some(String::new())),
            (101, Some(String::new()))
        ]
    );
    s.load(100, "life_potion_lvl0".to_owned()).unwrap();
    // A locked slot keeps its row but its Value is null and it is not listed.
    s.load(102, "life_potion_lvl0".to_owned()).unwrap();
    assert_eq!(s.info()[1], (100, Some("life_potion_lvl0".to_owned())));
    assert_eq!(s.info().len(), 3);
    assert_eq!(s.equipped("life_potion_lvl0"), Some(100));
    // Unknown parts and second rows for a part are refused instead of hidden.
    assert!(s.load(250, "x".to_owned()).is_err());
    assert!(s.load(100, "y".to_owned()).is_err());
}

#[test]
fn set_and_clear_follow_stackable_slot_rules() {
    let mut s = slots();
    assert_eq!(
        s.set(100, "life_potion_lvl0", "POTION"),
        Ok(Change::Insert {
            part: 100,
            class: "life_potion_lvl0".to_owned()
        })
    );
    assert_eq!(
        s.set(100, "phoenix_feather_lvl0", "FEATHER"),
        Ok(Change::Update {
            part: 100,
            class: "phoenix_feather_lvl0".to_owned()
        })
    );
    assert_eq!(
        s.set(101, "repair_kit_lvl0", "KIT"),
        Err(Refusal::InvalidItemCategory)
    );
    assert_eq!(
        s.set(102, "life_potion_lvl0", "POTION"),
        Err(Refusal::InvalidSlot)
    );
    assert_eq!(
        s.set(55, "life_potion_lvl0", "POTION"),
        Err(Refusal::InvalidSlot)
    );
    assert_eq!(s.clear(100), Some(Some(Change::Delete { part: 100 })));
    assert_eq!(s.clear(100), Some(None));
    assert_eq!(s.clear(55), None);
    assert_eq!(Refusal::InvalidSlot.name(), "EquipItemFail_InvalidSlot");
}

#[test]
fn skill_slots_check_only_the_lock() {
    let mut s = slots();
    // StackableSlot.SetSkill has no category check, unlike Set.
    assert_eq!(
        s.set_skill(101, "skill_fiona_guard"),
        Ok(Change::Insert {
            part: 101,
            class: "skill_fiona_guard".to_owned()
        })
    );
    assert_eq!(s.equipped("skill_fiona_guard"), Some(101));
    assert_eq!(
        s.set_skill(102, "skill_fiona_guard"),
        Err(Refusal::InvalidSlot)
    );
    assert_eq!(
        s.set_skill(55, "skill_fiona_guard"),
        Err(Refusal::InvalidSlot)
    );
}

#[test]
fn artifact_helpers_follow_gear_enumeration() {
    let mut s = QuickSlots::new([
        (100, false, vec!["POTION".to_owned(), "ARTIFACT".to_owned()]),
        (101, false, vec!["POTION".to_owned(), "ARTIFACT".to_owned()]),
        (102, true, vec!["ARTIFACT".to_owned()]),
    ])
    .unwrap();
    // GetEquippableEmptySlots skips locked and occupied slots.
    assert_eq!(s.empty_for("ARTIFACT"), [100, 101]);
    s.set(100, "artifact_a", "ARTIFACT").unwrap();
    assert_eq!(s.empty_for("ARTIFACT"), [101]);
    assert_eq!(s.max_part(), Some(102));
    assert_eq!(
        s.values().collect::<Vec<_>>(),
        [(100, "artifact_a"), (101, "")]
    );
}
