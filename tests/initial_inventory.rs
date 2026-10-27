use serde::Deserialize;
use vindictus_server::domain::{
    initial_inventory::*,
    item_colors::{ColorRandom, Materials},
    item_durability::EquipmentDurability,
    item_stats::ItemStat,
    item_time::ItemTimeError,
    legacy_user_data::SqlDateTime2,
};
#[derive(Deserialize)]
struct Fixture {
    cases: Cases,
}
#[derive(Deserialize)]
struct Cases {
    validity: Vec<Valid>,
    checks: Vec<Check>,
    plans: Vec<Plan>,
}
#[derive(Deserialize)]
struct Valid {
    input: [i32; 5],
    output: [i32; 5],
}
#[derive(Deserialize)]
struct Check {
    units: Vec<u16>,
    id: String,
    owner: Option<String>,
    count: i32,
    value: i32,
}
#[derive(Deserialize)]
struct Plan {
    input: Vec<[i32; 4]>,
    bundle: i32,
    error: Option<String>,
    output: Option<Vec<[i32; 4]>>,
}
fn fixture() -> Cases {
    serde_json::from_str::<Fixture>(include_str!("fixtures/framework-initial-inventory.json"))
        .unwrap()
        .cases
}
fn stat(max: i32) -> ItemStat {
    ItemStat::from_raw(&[], &[], [max, 0, 0, 0]).unwrap()
}
pub struct Clock {
    pub calls: usize,
}
impl InventoryClock for Clock {
    fn allocate(&mut self) -> Result<i64, InventoryError> {
        self.calls += 1;
        Ok(638448480000000000 + 65536 * self.calls as i64)
    }
    fn utc(&mut self) -> Result<SqlDateTime2, ItemTimeError> {
        SqlDateTime2::from_ticks(638448480000000000).map_err(|_| ItemTimeError::Clock)
    }
}
#[test]
fn new_matching_equipment_validity_matches_original_corrections() {
    let rows = fixture().validity;
    assert_eq!(rows.len(), 2000);
    for (i, row) in rows.into_iter().enumerate() {
        let [base, modified, bonus, reduced, limit] = row.input;
        let mut d = EquipmentDurability::load(&stat(base), &stat(modified), reduced, bonus);
        validate_durability(&mut d, limit);
        assert_eq!(
            [1, d.maximum(), d.current(), d.reduced(), d.bonus()],
            row.output,
            "case {i}"
        );
    }
}
#[test]
fn gold_checksum_matches_original_crc_utf8_signed_values_and_surrogates() {
    let rows = fixture().checks;
    assert_eq!(rows.len(), 900);
    for (i, r) in rows.into_iter().enumerate() {
        assert_eq!(
            gold_checksum(
                r.id.parse().unwrap(),
                r.owner.map(|v| v.parse().unwrap()),
                &r.units,
                r.count
            ),
            r.value,
            "case {i}"
        );
    }
}
#[test]
fn empty_cash_and_bag_order_bundle_unique_splitting_and_capacity_match_original() {
    let plans = fixture().plans;
    assert_eq!(plans.len(), 96);
    let base = stat(0);
    for (i, p) in plans.into_iter().enumerate() {
        let keys = (0..p.input.len())
            .map(|j| {
                format!("synthetic_{j}")
                    .encode_utf16()
                    .flat_map(u16::to_le_bytes)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let requests = p
            .input
            .iter()
            .zip(&keys)
            .map(|(r, k)| InitialRequest {
                class: k,
                count: r[2],
                max_stack: r[1],
                unique: r[3],
                storage: r[0] as u8,
                expire_in: None,
                equipment: false,
                base: &base,
                materials: [None; 3],
                gold: false,
            })
            .collect::<Vec<_>>();
        let mut clock = Clock { calls: 0 };
        let result = compose(
            &requests,
            1,
            &Materials::default(),
            &mut ColorRandom::from_seed(0),
            30000,
            p.bundle,
            &mut clock,
        );
        match p.error.as_deref() {
            Some("Limit") => {
                assert!(matches!(result, Err(InventoryError::Limit)), "plan {i}");
                assert_eq!(clock.calls, 0);
            }
            Some("Full") => {
                assert!(matches!(result, Err(InventoryError::Full)), "plan {i}");
                assert_eq!(clock.calls, 0);
            }
            None => {
                let rows = result.unwrap();
                let actual = rows
                    .items()
                    .iter()
                    .map(|r| {
                        let class = String::from_utf16(
                            &r.class()
                                .as_chunks::<2>()
                                .0
                                .iter()
                                .map(|b| u16::from_le_bytes(*b))
                                .collect::<Vec<_>>(),
                        )
                        .unwrap();
                        [
                            class[10..].parse().unwrap(),
                            r.count(),
                            i32::from(r.location().collection),
                            r.location().slot,
                        ]
                    })
                    .collect::<Vec<_>>();
                assert_eq!(actual, p.output.unwrap(), "plan {i}");
                assert_eq!(clock.calls, rows.items().len());
            }
            x => panic!("source error {x:?}"),
        }
    }
}
#[test]
fn late_equipment_count_failure_consumes_id_and_no_color_draws_while_profile_bounds_fail_early() {
    let base = stat(100);
    let key = "synthetic"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let r = InitialRequest {
        class: &key,
        count: 2,
        max_stack: 2,
        unique: 0,
        storage: 1,
        expire_in: None,
        equipment: true,
        base: &base,
        materials: [None; 3],
        gold: false,
    };
    let mut clock = Clock { calls: 0 };
    let mut rng = ColorRandom::from_seed(17);
    assert!(matches!(
        compose(
            &[r],
            1,
            &Materials::default(),
            &mut rng,
            30000,
            0,
            &mut clock
        ),
        Err(InventoryError::Bound)
    ));
    assert_eq!(clock.calls, 1);
    assert_eq!(
        rng.next(1000000).unwrap(),
        ColorRandom::from_seed(17).next(1000000).unwrap()
    );
}
#[test]
fn account_storage_item_binds_first_at_account_location_with_one_stack() {
    let base = stat(0);
    let keys = ["gold", "potion"].map(|n| {
        n.encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>()
    });
    fn request<'a>(
        key: &'a [u8],
        base: &'a ItemStat,
        storage: u8,
        count: i32,
        max_stack: i32,
    ) -> InitialRequest<'a> {
        InitialRequest {
            class: key,
            count,
            max_stack,
            unique: 0,
            storage,
            expire_in: None,
            equipment: false,
            base,
            materials: [None; 3],
            gold: storage == 3,
        }
    }
    let rows = compose(
        &[
            request(&keys[1], &base, 1, 20, 100),
            request(&keys[0], &base, 3, 10000, 2100000000),
        ],
        1,
        &Materials::default(),
        &mut ColorRandom::from_seed(0),
        30000,
        0,
        &mut Clock { calls: 0 },
    )
    .unwrap();
    let placed = rows
        .items()
        .iter()
        .map(|r| {
            (
                r.class() == keys[0].as_slice(),
                r.count(),
                r.location().collection,
                r.location().slot,
                r.checksum().is_some(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        placed,
        [(true, 10000, 101, -1, true), (false, 20, 0, 0, false)]
    );
    // AccountStorage keeps a single item per class: no second stack.
    let mut clock = Clock { calls: 0 };
    assert!(matches!(
        compose(
            &[request(&keys[0], &base, 3, 101, 100)],
            1,
            &Materials::default(),
            &mut ColorRandom::from_seed(0),
            30000,
            0,
            &mut clock,
        ),
        Err(InventoryError::Storage)
    ));
    assert_eq!(clock.calls, 0);
}
