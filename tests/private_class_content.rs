//! Local check of the exported per-class creation contents against the private cache.
//! Run explicitly: `cargo test --test private_class_content -- --ignored`.
use std::{fs::File, path::Path};
use vindictus_server::infrastructure::{
    class_initial, fiona_appearance, fiona_initializer_profile::Profile, fiona_inventory_profile,
    item_catalog, item_colors, item_stats,
};

fn open(name: &str) -> File {
    File::open(Path::new(".cache/private-content").join(name)).unwrap()
}

#[test]
#[ignore = "requires the private content cache"]
fn every_class_has_initializer_items_and_inner_armor() {
    let catalog = item_catalog::read_package(open("item-catalog.json")).unwrap();
    let stats = item_stats::Package::read(open("item-stats.json")).unwrap();
    let colors = item_colors::Package::read(open("item-colors.json")).unwrap();
    let mut inventory =
        fiona_inventory_profile::Profile::read(open("fiona-inventory.json")).unwrap();
    let mut appearance =
        fiona_appearance::Profile::read(open("fiona-appearance-inventory.json")).unwrap();
    let package = class_initial::Package::read(open("class-initial.json")).unwrap();
    package.bind(&catalog).unwrap();
    assert_eq!(package.classes.len(), 10);
    for (class, initial) in package.classes {
        if class != 1 {
            inventory.add_class(class, initial.requests);
            appearance.add_class(class, initial.inner, initial.inner_colors);
        }
    }
    // The first non-cash skin and eye colors of the region (the defaults the client offers).
    let (skin, eye) = (
        std::env::var("LAB_SKIN")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
        std::env::var("LAB_EYE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
    );
    let bound_stats = stats.bind(&catalog).unwrap();
    let bound_colors = colors.bind(&catalog).unwrap();
    for class in 0..=9u8 {
        let requests = inventory
            .requests(class, &catalog, &bound_stats, &bound_colors)
            .unwrap_or_else(|e| panic!("class {class} requests: {e}"));
        assert!(!requests.is_empty(), "class {class}");
        assert!(appearance.has_class(class), "class {class}");
        let profile = if class == 1 {
            Profile::read(
                open("fiona-initializer.json"),
                open("fiona-initializer-reference-pairs.json"),
            )
        } else {
            Profile::read(
                open(&format!("class-{class}-initializer.json")),
                open(&format!("class-{class}-initializer-reference-pairs.json")),
            )
        }
        .unwrap_or_else(|e| panic!("class {class} initializer: {e}"));
        profile.bind(&catalog).unwrap();
        assert_eq!(profile.character_class(), class);
        // The inventory CreateCharacter delivers and the appearance with no chosen item.
        let mut authority =
            vindictus_server::application::character_creation::EntityAuthority::new(17);
        let mut random = vindictus_server::domain::item_colors::ColorRandom::from_seed(0);
        let mut utc = || {
            vindictus_server::domain::legacy_user_data::SqlDateTime2::from_ticks(638448480000000000)
                .map_err(|_| vindictus_server::domain::item_time::ItemTimeError::Clock)
        };
        let composed = vindictus_server::application::fiona_inventory::compose(
            vindictus_server::application::fiona_inventory::Inputs {
                class,
                profile: &inventory,
                catalog: &catalog,
                stats: &stats,
                colors: &colors,
            },
            1,
            &mut authority,
            &mut random,
            &mut utc,
        )
        .unwrap_or_else(|e| panic!("class {class} inventory: {e}"));
        let collections: std::collections::BTreeSet<_> = composed
            .items()
            .iter()
            .map(|r| r.location().collection)
            .collect();
        println!("class {class} collections {collections:?}");
        let template = vindictus_server::protocol::character_creation::Template {
            character_class: class,
            items: Some(Vec::new()),
            skin_color: skin,
            shineness: 0,
            eye_color: eye,
            height: 0,
            bust: 0,
            painting_x: 0,
            painting_y: 0,
            painting_rotation: 0,
            painting_size: 0,
            body_shape: None,
        };
        appearance
            .prepare(&template, &inventory, &catalog, &colors, None)
            .unwrap_or_else(|e| panic!("class {class} appearance: {e}"));
        // Each non-cash customization item the creation screen offers, one at a time.
        // The creation screen's defaults together: first free item of each category.
        if let Ok(list) =
            std::fs::read_to_string(".cache/private-content/default-items-by-class.txt")
        {
            let names = list
                .lines()
                .filter_map(|l| l.strip_prefix(&format!("{class}	")))
                .collect::<Vec<_>>();
            let together = vindictus_server::protocol::character_creation::Template {
                items: Some(
                    names
                        .iter()
                        .map(|n| {
                            Some(
                                vindictus_server::protocol::character_creation::CustomizeItem {
                                    item_class: Some(n),
                                    category: None,
                                    colors: [0, 0, 0],
                                    duration: 0,
                                    price: None,
                                    coupon: 0,
                                },
                            )
                        })
                        .collect(),
                ),
                character_class: class,
                skin_color: skin,
                shineness: 0,
                eye_color: eye,
                height: 0,
                bust: 0,
                painting_x: 0,
                painting_y: 0,
                painting_rotation: 0,
                painting_size: 0,
                body_shape: None,
            };
            println!(
                "class {class} defaults {:?}: {:?}",
                names,
                appearance
                    .prepare(&together, &inventory, &catalog, &colors, None)
                    .map(|c| c.len())
                    .map_err(|e| e.to_string())
            );
        }
        // Lines: class, item class, category, feature (CustomizeItemInfo, IsCash False).
        if let Ok(list) =
            std::fs::read_to_string(".cache/private-content/custom-items-by-class.txt")
        {
            let mut failed = 0;
            let mut seen = std::collections::BTreeSet::new();
            for name in list.lines().filter_map(|l| {
                let mut f = l.split('\t');
                (f.next() == Some(&class.to_string()))
                    .then(|| f.next())
                    .flatten()
            }) {
                if !seen.insert(name) {
                    continue;
                }
                let one = vindictus_server::protocol::character_creation::Template {
                    items: Some(vec![Some(
                        vindictus_server::protocol::character_creation::CustomizeItem {
                            item_class: Some(name),
                            category: None,
                            colors: [0, 0, 0],
                            duration: 0,
                            price: None,
                            coupon: 0,
                        },
                    )]),
                    character_class: class,
                    skin_color: skin,
                    shineness: 0,
                    eye_color: eye,
                    height: 0,
                    bust: 0,
                    painting_x: 0,
                    painting_y: 0,
                    painting_rotation: 0,
                    painting_size: 0,
                    body_shape: None,
                };
                if appearance
                    .prepare(&one, &inventory, &catalog, &colors, None)
                    .is_err()
                {
                    failed += 1;
                    println!("class {class} refuses {name}");
                }
            }
            println!("class {class} refused items {failed}");
        }
        println!(
            "class {class}: {} item groups, {} default skills ({} active), {} presets",
            requests.len(),
            profile.skills().len(),
            profile.skills().iter().filter(|s| s.active()).count(),
            profile.preset_ids().len()
        );
    }
}
