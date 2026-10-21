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
        },
    )
}
fn inventory(items: &[(Vec<u8>, Held)]) -> Inventory<'_> {
    Inventory {
        items,
        bags: &[0, 1, 6],
        bundle_exceed: 0,
        story_tab: false,
    }
}
fn give(class: &ClassInfo, count: i32) -> Give<'_> {
    Give {
        class,
        count,
        bound: false,
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
    let mut expiring = class("timed", 1, 1, 0, false);
    expiring.expire_in = Some(60);
    assert_eq!(
        plan(&inventory(&[]), &[], &[give(&expiring, 1)]),
        Err(GiveError::Unsupported)
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
