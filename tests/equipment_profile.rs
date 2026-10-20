mod fixture {
    include!("fixtures/fiona_inventory_support.rs");
    include!("fixtures/equipment_profile_support.rs");
}
use vindictus_server::infrastructure::{equipment_profile::Profile, item_catalog};
#[test]
fn equipment_metadata_is_bound_to_catalog_source_indices_and_implementations() {
    let (c, _, _, _) = fixture::fixtures();
    let catalog = item_catalog::read_package(serde_json::to_vec(&c).unwrap().as_slice()).unwrap();
    let v = fixture::equipment_profile_fixture();
    let profile = Profile::read(serde_json::to_vec(&v).unwrap().as_slice()).unwrap();
    profile.bind(&catalog).unwrap();
    let key = "synthetic_bag"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let m = profile.metadata(&catalog, &key).unwrap();
    assert_eq!(
        (
            m.category.as_str(),
            m.level,
            m.equip_class.as_str(),
            m.serial
        ),
        ("WEAPON", 7, "LONGSWORD", 29)
    );
    assert_eq!(m.sets.len(), 1);
    for field in ["source_index", "item_utf16", "feature"] {
        let mut bad = v.clone();
        bad["classes"][0][field] = match field {
            "source_index" => 99.into(),
            "item_utf16" => "4100".into(),
            _ => "changed".into(),
        };
        assert!(
            Profile::read(serde_json::to_vec(&bad).unwrap().as_slice())
                .unwrap()
                .bind(&catalog)
                .is_err()
        );
    }
    let mut bad = v.clone();
    bad["implementations"]["gear"] = "0".repeat(64).into();
    assert!(Profile::read(serde_json::to_vec(&bad).unwrap().as_slice()).is_err());
    let mut bad = v.clone();
    bad["database_sha256"] = "0".repeat(64).into();
    assert!(
        Profile::read(serde_json::to_vec(&bad).unwrap().as_slice())
            .unwrap()
            .bind(&catalog)
            .is_err()
    );
    let mut bad = v.clone();
    bad["classes"][0]["category_utf16"] = "00d8".into();
    assert!(Profile::read(serde_json::to_vec(&bad).unwrap().as_slice()).is_err());
}
