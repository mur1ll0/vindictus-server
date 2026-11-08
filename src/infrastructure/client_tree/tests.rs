use super::*;
use crate::infrastructure::client_dist_store::{PublishRequest, keygen, publish, verify};

struct Fixture {
    root: PathBuf,
    paths: StorePaths,
    staging: PathBuf,
    public: String,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// Version 1 published from a small tree: an exe, a pack (hfs-xor), a player file and an excluded file.
fn fixture(name: &str) -> Fixture {
    let root = std::env::temp_dir().join(format!("client-tree-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let client = root.join("client");
    fs::create_dir_all(client.join("hfs")).ok();
    fs::create_dir_all(client.join("cfg")).ok();
    let ks: Vec<u8> = (0..4096u32).map(|i| (i * 31 + 5) as u8).collect();
    fs::write(root.join("ks.bin"), &ks).ok();
    fs::write(client.join("Vindictus.exe"), b"MZ exe").ok();
    fs::write(client.join("nmcogame.dll"), messenger(1)).ok();
    fs::write(
        client.join("hfs/0FABE22A68C0451A7EF97F6E8E682A448BB8122A.hfs"),
        b"HF\x01\x02 pack".repeat(500),
    )
    .ok();
    fs::write(client.join("cfg/config_env.cfg"), b"player").ok();
    fs::write(client.join("start.bat"), b"excluded").ok();
    let paths = StorePaths {
        store: root.join("store"),
        keys: root.join("keys"),
    };
    let public = keygen(&paths.keys).unwrap_or_default();
    let first = publish(&PublishRequest {
        store: &paths.store,
        keys: &paths.keys,
        tree: &client,
        keystream: &root.join("ks.bin"),
        title: "v1",
        notes: "",
        minimum: None,
        threads: 1,
    });
    assert!(first.is_ok(), "{first:?}");
    let staging = root.join("staging");
    Fixture {
        root,
        paths,
        staging,
        public,
    }
}

/// A 40 KB DLL-like file that does not compress; `version` changes 16 bytes of it (a delta is worth it).
fn messenger(version: u8) -> Vec<u8> {
    let mut state = 0x9E37_79B9u32;
    let mut data: Vec<u8> = (0..40_000)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state as u8
        })
        .collect();
    data[1_000..1_016].fill(version);
    data
}

fn put(f: &Fixture, path: &str, data: &[u8]) -> Change {
    let (sha256, size) = stage(&f.staging, data).unwrap_or_default();
    Change::Put {
        path: path.into(),
        sha256,
        size,
    }
}

fn manifest(f: &Fixture, version: u32) -> Option<Manifest> {
    fs::read(f.paths.store.join(layout::manifest(version)))
        .ok()
        .and_then(|b| Manifest::parse(&b).ok())
}

#[test]
fn a_draft_adds_replaces_and_removes_files_without_the_tree() {
    let f = fixture("publish");
    let changes = vec![
        // Replace (other spelling: the published one wins), add a new folder, remove a file.
        put(&f, "NMCOGAME.DLL", &messenger(2)),
        put(&f, "scaleform/item_icon/new_icon.tga", b"icon"),
        put(
            &f,
            "hfs/564E36A2A4C3B8F0C5E1D2F3A4B5C6D7E8F90123.hfs",
            &b"HF\x01\x02 new".repeat(64),
        ),
        Change::Remove {
            path: "Vindictus.exe".into(),
        },
    ];
    let report = publish_changes(&f.paths, &f.staging, 1, &changes, " Conteúdo ", "notas");
    assert_eq!(
        report
            .as_ref()
            .ok()
            .map(|r| (r.version, r.files, r.objects_written)),
        Some((2, 5, 3)),
        "{report:?}"
    );
    let v2 = manifest(&f, 2);
    let listed: Option<Vec<(String, Transform)>> = v2.as_ref().map(|m| {
        m.files
            .iter()
            .map(|e| (e.path.clone(), e.transform))
            .collect()
    });
    assert_eq!(
        listed,
        Some(vec![
            ("cfg/config_env.cfg".into(), Transform::None),
            (
                "hfs/0FABE22A68C0451A7EF97F6E8E682A448BB8122A.hfs".into(),
                Transform::HfsXor
            ),
            (
                "hfs/564E36A2A4C3B8F0C5E1D2F3A4B5C6D7E8F90123.hfs".into(),
                Transform::HfsXor
            ),
            ("nmcogame.dll".into(), Transform::None),
            ("scaleform/item_icon/new_icon.tga".into(), Transform::None),
        ])
    );
    assert_eq!(v2.as_ref().map(|m| m.title.as_str()), Some("Conteúdo"));
    // The replaced file got a delta from version 1; everything restores with the right SHA-256.
    assert_eq!(v2.map(|m| m.deltas.len()), Some(1));
    let checked = verify(&f.paths.store, &f.public, true);
    assert_eq!(
        checked.as_ref().ok().map(|r| (r.current, r.problems.len())),
        Some((2, 0)),
        "{checked:?}"
    );
}

#[test]
fn a_draft_is_refused_when_the_store_moved_or_nothing_changes() {
    let f = fixture("refuse");
    let same = put(&f, "Vindictus.exe", b"MZ exe");
    assert!(publish_changes(&f.paths, &f.staging, 1, &[same], "t", "").is_err());
    let missing = Change::Remove {
        path: "bin/none.dll".into(),
    };
    assert!(publish_changes(&f.paths, &f.staging, 1, &[missing], "t", "").is_err());
    assert!(publish_changes(&f.paths, &f.staging, 1, &[], "t", "").is_err());
    for path in ["../x.dll", "start-lab.bat", "C:/x.dll", "lab-backup/a.hfs"] {
        assert!(check_path(path).is_err(), "{path}");
    }
    let ok = put(&f, "bin\\new.dll", b"MZ new");
    assert!(publish_changes(&f.paths, &f.staging, 1, std::slice::from_ref(&ok), "t", "").is_ok());
    // The draft started on version 1; the store is now at 2.
    let late = put(&f, "bin/other.dll", b"MZ other");
    assert!(publish_changes(&f.paths, &f.staging, 1, &[late], "t", "").is_err());
    // A staged file that changed on disk is refused.
    if let Change::Put { sha256, .. } = &ok {
        fs::write(f.staging.join(sha256), b"tampered").ok();
    }
    assert!(publish_changes(&f.paths, &f.staging, 2, &[ok], "t", "").is_err());
}

#[test]
fn the_server_tree_follows_the_published_version_and_lists_its_changes() {
    let f = fixture("tree");
    let work = f.root.join("work");
    let mut cache = HashCache::default();
    let Some(v1) = manifest(&f, 1) else {
        panic!("no version 1");
    };
    // An empty tree gets every file; a build backup in it is never touched.
    fs::create_dir_all(work.join("lab-backup")).ok();
    fs::write(work.join("lab-backup/keep.hfs"), b"backup").ok();
    fs::write(work.join("stray.txt"), b"not published").ok();
    let synced = sync_tree(&f.paths.store, &v1, &work, &mut cache);
    assert_eq!(
        synced.as_ref().ok().map(|r| (r.written, r.deleted)),
        Some((4, 1)),
        "{synced:?}"
    );
    assert!(work.join("lab-backup/keep.hfs").exists());
    let again = diff_tree(&work, &v1, &mut cache);
    assert_eq!(
        again
            .as_ref()
            .ok()
            .map(|d| (d.put.len(), d.removed.len(), d.unchanged)),
        Some((0, 0, 4))
    );
    // The content step changes the tree: a changed file, a new one, a removed one; the player's file
    // changed locally is never proposed.
    fs::write(work.join("nmcogame.dll"), b"MZ rebuilt").ok();
    fs::write(work.join("sql.db3"), b"new").ok();
    fs::remove_file(work.join("Vindictus.exe")).ok();
    fs::write(work.join("cfg/config_env.cfg"), b"changed by the build").ok();
    let diff = diff_tree(&work, &v1, &mut cache).ok();
    let summary = diff.as_ref().map(|d| {
        (
            d.put
                .iter()
                .map(|(p, _, _, before)| (p.clone(), before.is_some()))
                .collect::<Vec<_>>(),
            d.removed.iter().map(|(p, _)| p.clone()).collect::<Vec<_>>(),
        )
    });
    assert_eq!(
        summary,
        Some((
            vec![("nmcogame.dll".into(), true), ("sql.db3".into(), false)],
            vec!["Vindictus.exe".into()]
        ))
    );
    // Staged from the tree and published, then the tree is the new version.
    let mut changes = Vec::new();
    for (path, sha, size, _) in diff.map(|d| d.put).unwrap_or_default() {
        assert!(stage_from_tree(&f.staging, &work, &path, &sha).is_ok());
        changes.push(Change::Put {
            path,
            sha256: sha,
            size,
        });
    }
    changes.push(Change::Remove {
        path: "Vindictus.exe".into(),
    });
    assert!(publish_changes(&f.paths, &f.staging, 1, &changes, "conteúdo", "").is_ok());
    let Some(v2) = manifest(&f, 2) else {
        panic!("no version 2");
    };
    let clean = diff_tree(&work, &v2, &mut cache);
    assert_eq!(
        clean.as_ref().ok().map(|d| (d.put.len(), d.removed.len())),
        Some((0, 0))
    );
    // A later withdraw (back to version 1) is followed by the tree.
    assert!(sync_tree(&f.paths.store, &v1, &work, &mut cache).is_ok());
    assert_eq!(
        fs::read(work.join("Vindictus.exe")).ok(),
        Some(b"MZ exe".to_vec())
    );
    assert!(!work.join("sql.db3").exists());
    let cache_file = f.root.join("hashes.json");
    assert!(cache.save(&cache_file).is_ok());
    assert!(!HashCache::load(&cache_file).files.is_empty());
}
