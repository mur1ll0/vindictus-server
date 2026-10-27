//! Model kits from the laboratory client copy (.cache/play-client, never versioned). Ignored by
//! default: `cargo test --test model_kit -- --ignored` with the copy and its hfs-keystream.bin.
use std::path::Path;
use vindictus_server::infrastructure::{client_files::ClientFiles, model_kit};

const LAB: &str = ".cache/play-client";

fn lab() -> ClientFiles {
    ClientFiles::open(Path::new(LAB), &Path::new(LAB).join("hfs-keystream.bin"))
        .expect("lab client copy")
}

#[test]
#[ignore = "needs the laboratory client copy"]
fn kit_matches_the_python_model_kit() {
    let files = lab();
    let kit = model_kit::build(&files, "models/player/fiona/fiona_inner_fringe.mdl").expect("kit");
    let names: Vec<&str> = kit.iter().map(|(n, _)| n.as_str()).collect();
    for expected in [
        "models/player/fiona/fiona_inner_fringe.mdl",
        "models/player/fiona/fiona_inner_fringe.vvd",
        "models/player/fiona/fiona_inner_fringe.dx90.vtx",
        "materials/models/player/fiona/pc_fiona_inner_fringe.vmt",
        "materials/models/player/fiona/pc_fiona_inner_fringe.vtf",
        "kit.json",
    ] {
        assert!(names.contains(&expected), "{expected}");
    }
    let python = Path::new(".cache/private-assets/kits/fiona_inner_fringe");
    if python.is_dir() {
        for (name, data) in &kit {
            if name != "kit.json" {
                assert_eq!(
                    &std::fs::read(python.join(name)).expect(name),
                    data,
                    "{name}"
                );
            }
        }
    }
    let out = Path::new(".cache/verification/fiona_inner_lab.zip");
    let lab_kit =
        model_kit::build(&files, "models/player/fiona/fiona_inner_lab.mdl").expect("lab item kit");
    std::fs::write(out, model_kit::zip(&lab_kit)).expect("write zip");
}

#[test]
#[ignore = "needs the laboratory client copy"]
fn items_resolve_to_their_costume_models() {
    let files = lab();
    let text = files
        .read("scripts/player_costume.txt")
        .expect("read")
        .expect("player_costume.txt");
    assert_eq!(
        model_kit::ModelScript::Costume("inner_armor")
            .model(&text, 16)
            .as_deref(),
        Some("models/player/fiona/fiona_inner_fringe.mdl")
    );
    assert_eq!(
        model_kit::ModelScript::Costume("inner_armor")
            .model(&text, 9101)
            .as_deref(),
        Some("models/player/fiona/fiona_inner_lab.mdl")
    );
}

#[test]
#[ignore = "needs the laboratory client copy"]
fn weapons_and_shields_resolve_and_build_the_python_kits() {
    let files = lab();
    let longsword = model_kit::ModelScript::of("WEAPON", Some("LONGSWORD")).expect("weapon");
    let weapon = files
        .read(&longsword.file())
        .expect("read")
        .expect("weapon script");
    assert_eq!(longsword.keys(&weapon).len(), 72);
    assert_eq!(
        longsword.model(&weapon, 1).as_deref(),
        Some("models/weapon/longsword/longsword_swordwind.mdl")
    );
    let shields = model_kit::ModelScript::of("LARGESHIELD", Some("LARGESHIELD")).expect("shield");
    let text = files
        .read(&shields.file())
        .expect("read")
        .expect("shields.txt");
    assert_eq!(shields.keys(&text).len(), 76);
    assert_eq!(
        shields.model(&text, 15).as_deref(),
        Some("models/weapon/largeshield/largeshield_kite.mdl")
    );
    assert_eq!(
        shields.model(&text, 2).as_deref(),
        Some("models/weapon/shield/shield_swordwind.mdl")
    );
    // Same files as scripts/client/model_kit.py (.cache/kits/weapons, made before the test).
    for model in [
        "models/weapon/longsword/longsword_swordwind.mdl",
        "models/weapon/largeshield/largeshield_kite.mdl",
        "models/weapon/shield/shield_swordwind.mdl",
    ] {
        let kit = model_kit::build(&files, model).expect("kit");
        let name = model.rsplit('/').next().unwrap().trim_end_matches(".mdl");
        let python = Path::new(".cache/kits/weapons").join(name);
        if python.is_dir() {
            for (path, data) in &kit {
                if path != "kit.json" {
                    assert_eq!(
                        &std::fs::read(python.join(path)).expect(path),
                        data,
                        "{path}"
                    );
                }
            }
        }
    }
}
