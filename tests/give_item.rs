use vindictus_server::domain::{
    give_item::{ClassInfo, Created, Destroy, Give, GiveError, Held, Inventory, plan},
    item_location::ItemLocation,
};

fn class(name: &str, storage: u8, max_stack: i32, unique: i32, equippable: bool) -> ClassInfo {
    ClassInfo {
        class: name.encode_utf16().flat_map(u16::to_le_bytes).collect(),
        max_stack,
        storage,
        unique,
        equippable,
        expire_in: None,
        tircoin: false,
    }
}
fn at(collection: u8, slot: i32) -> ItemLocation {
    ItemLocation { collection, slot }
}
fn held(info: &ClassInfo, id: i64, count: i32, location: ItemLocation) -> (Vec<u8>, Held) {
    (
        info.class.clone(),
        Held {
            id,
            count,
            location,
            bound: false,
            expires: false,
            attributes: 0,
        },
    )
}
fn inventory(items: &[(Vec<u8>, Held)]) -> Inventory<'_> {
    Inventory {
        items,
        bags: &[0, 1, 6],
        bundle_exceed: 0,
        story_tab: false,
        item_impl: true,
    }
}
fn give(class: &ClassInfo, count: i32) -> Give<'_> {
    Give {
        class,
        count,
        bound: false,
        attributes: 0,
    }
}

#[test]
fn gold_goes_to_the_account_location_and_then_stacks_on_it() {
    let gold = class("gold", 3, 2_100_000_000, 0, false);
    let empty = plan(&inventory(&[]), &[], &[give(&gold, 500)]).unwrap();
    assert!(empty.modified.is_empty());
    assert_eq!(
        empty.created,
        [Created {
            class: gold.class.clone(),
            count: 500,
            bound: false,
            location: at(101, -1),
            request: 0,
        }]
    );
    let items = [held(&gold, 9, 500, at(101, -1))];
    let more = plan(&inventory(&items), &[], &[give(&gold, 250)]).unwrap();
    assert_eq!(more.modified, [(9, 750)]);
    assert!(more.created.is_empty());
    // Destroying everything and giving again reuses the destroyed account item.
    let pay = plan(
        &inventory(&items),
        &[Destroy {
            class: &gold,
            count: 500,
        }],
        &[],
    )
    .unwrap();
    assert_eq!(pay.modified, [(9, 0)]);
    assert_eq!(
        plan(
            &inventory(&items),
            &[Destroy {
                class: &gold,
                count: 501
            }],
            &[]
        ),
        Err(GiveError::NotEnoughItem)
    );
}

#[test]
fn stacks_fill_the_fullest_item_first_then_new_stacks_use_free_slots() {
    let potion = class("potion", 1, 10, 0, false);
    let items = [held(&potion, 1, 4, at(0, 0)), held(&potion, 2, 7, at(0, 2))];
    let p = plan(&inventory(&items), &[], &[give(&potion, 17)]).unwrap();
    // StackComparer orders by Num descending: 7→10 first, then 4→10.
    assert_eq!(p.modified, [(2, 10), (1, 10)]);
    assert_eq!(
        p.created
            .iter()
            .map(|c| (c.count, c.location))
            .collect::<Vec<_>>(),
        [(8, at(0, 1))]
    );
    // Duplicated requests merge before stacking.
    let merged = plan(
        &inventory(&items),
        &[],
        &[give(&potion, 3), give(&potion, 3)],
    )
    .unwrap();
    assert_eq!(merged.modified, [(2, 10), (1, 7)]);
}

#[test]
fn equipment_never_stacks_and_bags_fill_in_source_collection_order() {
    let sword = class("sword", 1, 1, 0, true);
    let items: Vec<_> = (0..48)
        .map(|slot| held(&sword, 100 + i64::from(slot), 1, at(0, slot)))
        .collect();
    let p = plan(&inventory(&items), &[], &[give(&sword, 2)]).unwrap();
    assert!(p.modified.is_empty());
    assert_eq!(
        p.created.iter().map(|c| c.location).collect::<Vec<_>>(),
        [at(1, 0), at(1, 1)]
    );
    let all = plan(&inventory(&items), &[], &[give(&sword, 97)]);
    assert_eq!(all, Err(GiveError::NoEmptySlot));
}

#[test]
fn cash_slots_skip_occupied_ones_including_destroyed_items() {
    let ticket = class("ticket", 2, 5, 0, false);
    let other = class("other", 2, 1, 0, false);
    let items = [
        held(&other, 1, 1, at(100, 0)),
        held(&other, 2, 1, at(100, 2)),
    ];
    let p = plan(
        &inventory(&items),
        &[Destroy {
            class: &other,
            count: 1,
        }],
        &[give(&ticket, 12)],
    )
    .unwrap();
    assert_eq!(
        p.created
            .iter()
            .map(|c| (c.count, c.location))
            .collect::<Vec<_>>(),
        [(5, at(100, 1)), (5, at(100, 3)), (2, at(100, 4))]
    );
    // DestroyComparer: same Num, higher Slot first.
    assert_eq!(p.modified, [(2, 0)]);
}

#[test]
fn unique_bundle_and_unsupported_branches_refuse_without_a_plan() {
    let rare = class("rare", 1, 5, 3, false);
    let items = [held(&rare, 1, 2, at(0, 0))];
    assert_eq!(
        plan(&inventory(&items), &[], &[give(&rare, 2)]),
        Err(GiveError::UniqueConstraint)
    );
    assert_eq!(
        plan(&inventory(&items), &[], &[give(&rare, 1)])
            .unwrap()
            .modified,
        [(1, 3)]
    );
    let stone = class("stone", 1, 10, 0, false);
    let bundled = Inventory {
        bundle_exceed: 2,
        ..inventory(&[])
    };
    assert_eq!(
        plan(&bundled, &[], &[give(&stone, 30)]),
        Err(GiveError::ExceedBundleCount)
    );
    assert!(plan(&bundled, &[], &[give(&stone, 29)]).is_ok());
    // An ExpireIn class is planned like any other; the expiry is set on creation.
    let mut expiring = class("timed", 1, 1, 0, false);
    expiring.expire_in = Some(60);
    assert_eq!(
        plan(&inventory(&[]), &[], &[give(&expiring, 1)])
            .unwrap()
            .created
            .len(),
        1
    );
    let restore = class("restore", 4, 1, 0, false);
    assert_eq!(
        plan(&inventory(&[]), &[], &[give(&restore, 1)]),
        Err(GiveError::NoEmptySlot)
    );
    assert_eq!(
        plan(&inventory(&[]), &[], &[give(&stone, -1)]),
        Err(GiveError::InvalidRequest)
    );
}

#[test]
fn story_items_take_the_first_free_story_slot_like_story_storage() {
    let necklace = class("break_lower", 5, 1000, 0, false);
    let other = class("story_letter", 5, 1, 0, false);
    let items = [
        held(&other, 7, 1, at(103, 0)),
        held(&other, 8, 1, at(103, 2)),
    ];
    let mut inv = inventory(&items);
    // Without StoryTabInInventory the storage is Invalid: nothing can hold it.
    assert_eq!(
        plan(&inv, &[], &[give(&necklace, 1)]).err(),
        Some(GiveError::NoEmptySlot)
    );
    inv.story_tab = true;
    let p = plan(&inv, &[], &[give(&necklace, 1)]).unwrap();
    assert_eq!(p.created[0].location, at(103, 1));
    // A story item destroyed in the same request still holds its slot (EmptySlots
    // ignores the destroyed set).
    let p = plan(
        &inv,
        &[Destroy {
            class: &other,
            count: 1,
        }],
        &[give(&necklace, 1)],
    )
    .unwrap();
    assert_eq!(p.created[0].location, at(103, 1));
    // A full story tab has no empty slot.
    let full: Vec<_> = (0..48)
        .map(|s| held(&other, 100 + i64::from(s), 1, at(103, s)))
        .collect();
    let mut inv = inventory(&full);
    inv.story_tab = true;
    assert_eq!(
        plan(&inv, &[], &[give(&necklace, 1)]).err(),
        Some(GiveError::NoEmptySlot)
    );
}

#[test]
fn only_equippable_lvl_classes_need_the_enhance_attribute() {
    // ItemImpl.GiveItem: an equippable "<class>_lvlN" becomes <class> with ENHANCE N*2.
    let sword = class("long_sword_lvl1", 1, 1, 0, true);
    let items = [];
    assert_eq!(
        plan(&inventory(&items), &[], &[give(&sword, 1)]),
        Err(GiveError::Unsupported)
    );
    // CraftItem (random boxes) gives the "_lvlN" class as it is.
    let craft = Inventory {
        item_impl: false,
        ..inventory(&items)
    };
    assert_eq!(
        plan(&craft, &[], &[give(&sword, 1)]).unwrap().created.len(),
        1
    );
    // A non-equippable class keeps its name (GuidePlay_5 gives enhance_stone_lvl1).
    let stone = class("enhance_stone_lvl1", 1, 99, 0, false);
    let out = plan(&inventory(&items), &[], &[give(&stone, 1)]).unwrap();
    assert_eq!(out.created.len(), 1);
    assert_eq!(out.created[0].location, at(0, 0));
}

#[test]
fn expiring_classes_never_stack_onto_items_with_an_expiry() {
    // ItemTraitExtension.IsStackableWith compares ExpireDateTime: a request has none,
    // and every item of an ExpireIn class got its own on creation.
    let mut pass = class("cash_freepass_7_hair_Expire", 1, 10, 0, false);
    pass.expire_in = Some(604_800);
    let mut items = [held(&pass, 5, 1, at(0, 0))];
    items[0].1.expires = true;
    let out = plan(&inventory(&items), &[], &[give(&pass, 1)]).unwrap();
    assert!(out.modified.is_empty());
    assert_eq!(out.created.len(), 1);
    assert_eq!(out.created[0].location, at(0, 1));
    // DestroyComparer orders by ExpireDateTime, which is not compared here.
    assert_eq!(
        plan(
            &inventory(&items),
            &[Destroy {
                class: &pass,
                count: 1
            }],
            &[]
        ),
        Err(GiveError::Unsupported)
    );
}

#[test]
fn attributes_split_stacks_and_merges_like_is_stackable_with() {
    use vindictus_server::domain::item_attribute::{StackKeys, parse_class_ex};
    let scroll = class("enchant_scroll", 1, 10, 0, false);
    let mut keys = StackKeys::default();
    let (name, bitter) = parse_class_ex("enchant_scroll[PREFIX:bitter]").unwrap();
    assert_eq!(name, "enchant_scroll");
    let bitter = keys.intern(&bitter);
    let hard = keys.intern(&parse_class_ex("enchant_scroll[PREFIX:hard]").unwrap().1);
    // Arg2 is not compared by IsStackableWith; Arg is.
    let bitter2 = keys.intern(
        &parse_class_ex("enchant_scroll[PREFIX:bitter<7>]")
            .unwrap()
            .1,
    );
    let bitter_arg = keys.intern(
        &parse_class_ex("enchant_scroll[PREFIX:bitter(1)]")
            .unwrap()
            .1,
    );
    assert_eq!(bitter, bitter2);
    assert!(bitter != hard && bitter != bitter_arg && bitter != 0);
    let mut items = [held(&scroll, 1, 3, at(0, 0)), held(&scroll, 2, 4, at(0, 1))];
    items[0].1.attributes = bitter;
    // A plain request never stacks on the attributed item.
    let plain = plan(&inventory(&items), &[], &[give(&scroll, 2)]).unwrap();
    assert_eq!(plain.modified, [(2, 6)]);
    let attributed = |n, attributes| Give {
        class: &scroll,
        count: n,
        bound: false,
        attributes,
    };
    let same = plan(&inventory(&items), &[], &[attributed(2, bitter2)]).unwrap();
    assert_eq!(same.modified, [(1, 5)]);
    assert!(same.created.is_empty());
    // MergeDuplicatedRequest sums only stackable requests; each keeps its request index.
    let merged = plan(
        &inventory(&[]),
        &[],
        &[
            attributed(1, hard),
            attributed(1, bitter),
            attributed(2, hard),
        ],
    )
    .unwrap();
    assert_eq!(
        merged
            .created
            .iter()
            .map(|c| (c.count, c.request, c.location))
            .collect::<Vec<_>>(),
        [(3, 0, at(0, 0)), (1, 1, at(0, 1))]
    );
    // DestroyComparer would weigh GetItemValue; destroying an attributed class is refused.
    assert_eq!(
        plan(
            &inventory(&items),
            &[Destroy {
                class: &scroll,
                count: 1
            }],
            &[]
        ),
        Err(GiveError::Unsupported)
    );
    // Attributed equipment is planned like any equipment (the caller validates its
    // Item.Stat); two requests never merge.
    let sword = class("longsword", 1, 1, 0, true);
    let enhanced = keys.intern(&parse_class_ex("longsword[ENHANCE:3]").unwrap().1);
    let sword_give = || Give {
        class: &sword,
        count: 1,
        bound: false,
        attributes: enhanced,
    };
    assert_eq!(
        plan(&inventory(&[]), &[], &[sword_give(), sword_give()])
            .unwrap()
            .created
            .iter()
            .map(|c| (c.request, c.location))
            .collect::<Vec<_>>(),
        [(0, at(0, 0)), (1, at(0, 1))]
    );
}

#[test]
fn class_ex_parsing_builds_back_and_rejects_the_unstored_profile() {
    use vindictus_server::domain::item_attribute::{
        Attribute, AttributeError, Attributes, build_class_ex, parse_class_ex, stackable,
    };
    let (class, attributes) = parse_class_ex("frostheart_foot[QUALITY:(5)]").unwrap();
    assert_eq!(class, "frostheart_foot");
    assert_eq!(
        attributes.as_slice(),
        [Attribute {
            name: "QUALITY".into(),
            value: String::new(),
            arg: 5,
            arg2: 0
        }]
    );
    assert_eq!(
        build_class_ex(&class, &attributes).unwrap(),
        "frostheart_foot[QUALITY:(5)]"
    );
    let (_, two) = parse_class_ex(" scroll [ PREFIX : a (2)<3>, SUFFIX:b]").unwrap();
    assert_eq!(
        build_class_ex("scroll", &two).unwrap(),
        "scroll[PREFIX:a(2)<3>,SUFFIX:b]"
    );
    assert_eq!(
        parse_class_ex("x[A:1,A:2]").unwrap_err(),
        AttributeError::ClassEx(
            vindictus_server::domain::item_class_ex::ItemClassExError::DuplicateAttribute
        )
    );
    // Dates in an argument need the legacy resolver.
    assert!(parse_class_ex("x[A:v(2014/1/1)]").is_err());
    assert_eq!(
        parse_class_ex(&format!("x[A:{}]", "v".repeat(51))).unwrap_err(),
        AttributeError::Bound
    );
    assert!(Attribute::from_stored(b"PREFIX", b"bitter", 0, 0).is_ok());
    assert_eq!(
        Attribute::from_stored(b"", b"x", 0, 0).unwrap_err(),
        AttributeError::Bound
    );
    assert_eq!(
        Attribute::from_stored(b"P", &[0xe9], 0, 0).unwrap_err(),
        AttributeError::Bound
    );
    // Dictionary keys are ordinal: case differs, so both are kept and never stack.
    let mixed = Attributes::new(vec![
        Attribute::from_stored(b"prefix", b"a", 0, 0).unwrap(),
        Attribute::from_stored(b"PREFIX", b"a", 0, 0).unwrap(),
    ])
    .unwrap();
    assert_eq!(mixed.as_slice().len(), 2);
    assert!(stackable([("A", "x", 1)], [("A", "x", 1)]));
    assert!(!stackable([("A", "x", 1)], [("a", "x", 1)]));
    assert!(!stackable([("A", "x", 1)], [("A", "x", 2)]));
    assert!(!stackable([("A", "x", 1)], []));
}
