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
        println!(
            "class {class}: {} item groups, {} default skills ({} active), {} presets",
            requests.len(),
            profile.skills().len(),
            profile.skills().iter().filter(|s| s.active()).count(),
            profile.preset_ids().len()
        );
    }
}
