//! Model kits from the laboratory client copy (.cache/play-client, never versioned). Ignored by
//! default: `cargo test --test model_kit -- --ignored` with the copy and its hfs-keystream.bin.
use std::path::Path;
use vindictus_server::infrastructure::{client_files::ClientFiles, model_kit};

const LAB: &str = ".cache/play-client";

fn lab() -> ClientFiles {
    ClientFiles::open(Path::new(LAB), &Path::new(LAB).join("hfs-keystream.bin")).expect("lab client copy")
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
                assert_eq!(&std::fs::read(python.join(name)).expect(name), data, "{name}");
            }
        }
    }
    let out = Path::new(".cache/verification/fiona_inner_lab.zip");
    let lab_kit = model_kit::build(&files, "models/player/fiona/fiona_inner_lab.mdl").expect("lab item kit");
    std::fs::write(out, model_kit::zip(&lab_kit)).expect("write zip");
}

#[test]
#[ignore = "needs the laboratory client copy"]
fn items_resolve_to_their_costume_models() {
    let files = lab();
    let text = files.read("scripts/player_costume.txt").expect("read").expect("player_costume.txt");
    assert_eq!(
        model_kit::costume_model(&text, "inner_armor", 16).as_deref(),
        Some("models/player/fiona/fiona_inner_fringe.mdl")
    );
    assert_eq!(
        model_kit::costume_model(&text, "inner_armor", 9101).as_deref(),
        Some("models/player/fiona/fiona_inner_lab.mdl")
    );
}
