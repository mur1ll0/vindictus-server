//! Um armazém pequeno montado com o mesmo crate do servidor (assinado com uma chave de teste) e lido
//! de uma pasta; o mesmo caminho do launcher, sem rede.
use super::*;
use ed25519_dalek::SigningKey;
use std::sync::atomic::AtomicUsize;
use vindictus_client_dist::manifest::{Delta, FORMAT, ObjectRef};
use vindictus_client_dist::sha256_hex;

struct DirFetch {
    root: PathBuf,
    requests: AtomicUsize,
}

impl Fetch for DirFetch {
    fn bytes(&self, path: &str) -> Result<Vec<u8>, UpdateError> {
        self.requests.fetch_add(1, Ordering::Relaxed);
        fs::read(self.root.join(path)).map_err(|_| UpdateError::Server(404))
    }

    fn download(
        &self,
        path: &str,
        dest: &Path,
        size: u64,
        _cancel: &AtomicBool,
        progress: &dyn Fn(u64),
    ) -> Result<(), UpdateError> {
        let data = self.bytes(path)?;
        let have = fs::metadata(dest).map(|m| m.len()).unwrap_or(0).min(size);
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dest)?;
        file.write_all(&data[have as usize..])?;
        progress(size - have);
        Ok(())
    }
}

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "launcher-update-{name}-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).ok();
    dir
}

fn keystream() -> [u8; codec::KEYSTREAM_SIZE] {
    std::array::from_fn(|i| (i * 13 + 7) as u8)
}

fn noise(seed: u32, len: usize) -> Vec<u8> {
    let mut state = seed | 1;
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state as u8
        })
        .collect()
}

struct Store {
    root: PathBuf,
    key: SigningKey,
}

impl Store {
    fn new() -> Self {
        let root = temp("store");
        let key = SigningKey::from_bytes(&[42u8; 32]);
        let ks = keystream();
        let (object, _) = codec::compress(&ks).unwrap_or_default();
        Self::put(&root, &layout::object(&sha256_hex(&ks)), &object);
        Self { root, key }
    }

    fn put(root: &Path, path: &str, data: &[u8]) {
        let file = root.join(path);
        fs::create_dir_all(file.parent().unwrap_or(root)).ok();
        fs::write(file, data).ok();
    }

    fn sign(&self, path: &str, data: &[u8]) {
        Self::put(&self.root, path, data);
        Self::put(
            &self.root,
            &layout::signature(path),
            &sign::sign(&self.key, data),
        );
    }

    /// Publica `files` (caminho, conteúdo) como `version`, com deltas da anterior.
    fn publish(
        &self,
        version: u32,
        files: &[(&str, Vec<u8>)],
        previous: Option<&Manifest>,
    ) -> Manifest {
        let ks = keystream();
        let mut entries = Vec::new();
        let mut deltas = Vec::new();
        for (path, data) in files {
            let transform = vindictus_client_dist::rules::transform_of(path);
            let mut form = data.clone();
            codec::apply(transform, &ks, 0, &mut form);
            let (object, _) = codec::compress(&form).unwrap_or_default();
            let sha = sha256_hex(data);
            Self::put(&self.root, &layout::object(&sha), &object);
            if let Some(old) =
                previous.and_then(|m| m.files.iter().find(|f| f.path == *path && f.sha256 != sha))
            {
                let old_data = fs::read(self.root.join(layout::object(&old.sha256)))
                    .ok()
                    .and_then(|o| codec::decompress(&o, old.size).ok())
                    .unwrap_or_default();
                let patch = codec::delta(&old_data, &form).unwrap_or_default();
                Self::put(&self.root, &layout::delta(&old.sha256, &sha), &patch);
                deltas.push(Delta {
                    path: path.to_string(),
                    from_sha256: old.sha256.clone(),
                    from_size: old.size,
                    to_sha256: sha.clone(),
                    delta_size: patch.len() as u64,
                });
            }
            entries.push(FileEntry {
                path: path.to_string(),
                sha256: sha,
                size: data.len() as u64,
                transform,
                object_size: object.len() as u64,
                preserve: vindictus_client_dist::rules::preserved(path),
            });
        }
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        let ks_object = fs::metadata(self.root.join(layout::object(&sha256_hex(&ks))))
            .map(|m| m.len())
            .unwrap_or(0);
        let manifest = Manifest {
            format: FORMAT,
            version,
            title: format!("v{version}"),
            notes: String::new(),
            created_unix: 0,
            keystream: ObjectRef {
                sha256: sha256_hex(&ks),
                size: 4096,
                object_size: ks_object,
            },
            files: entries,
            deltas,
        };
        let bytes = manifest.to_bytes().unwrap_or_default();
        self.sign(&layout::manifest(version), &bytes);
        let pointer = VersionPointer {
            format: FORMAT,
            current: version,
            minimum: 1,
            manifest_sha256: sha256_hex(&bytes),
            launcher: None,
        };
        self.sign(layout::POINTER, &pointer.to_bytes().unwrap_or_default());
        manifest
    }

    fn fetch(&self) -> DirFetch {
        DirFetch {
            root: self.root.clone(),
            requests: AtomicUsize::new(0),
        }
    }
}

const PACK: &str = "hfs/0FABE22A68C0451A7EF97F6E8E682A448BB8122A.hfs";

fn v1() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("Vindictus.exe", noise(1, 5_000)),
        ("bin/client.dll", noise(2, 80_000)),
        (PACK, noise(3, 60_000)),
        ("cfg/config_env.cfg", b"default options".to_vec()),
    ]
}

fn v2() -> Vec<(&'static str, Vec<u8>)> {
    let mut files = v1();
    files[1].1[40_000..40_010].fill(9);
    files[2].1[100..120].fill(1);
    files.push((
        "hfs/AAAA000000000000000000000000000000000000.hfs",
        noise(4, 30_000),
    ));
    files
}

fn run_mode(
    game: &Path,
    store: &Store,
    mode: Mode,
    version: Option<u32>,
) -> Result<Outcome, UpdateError> {
    let fetch = store.fetch();
    run(
        game,
        &fetch,
        &store.key.verifying_key(),
        mode,
        version,
        &AtomicBool::new(false),
        &|| false,
        &|_| {},
    )
}

fn matches(game: &Path, files: &[(&str, Vec<u8>)]) -> bool {
    files
        .iter()
        .all(|(path, data)| fs::read(game.join(path)).ok().as_ref() == Some(data))
}

#[test]
fn installs_into_an_empty_folder_and_records_the_state() {
    let store = Store::new();
    let m1 = store.publish(1, &v1(), None);
    store.publish(2, &v2(), Some(&m1));
    let game = temp("install");
    let outcome = run_mode(&game, &store, Mode::Full, None).ok();
    assert_eq!(outcome.map(|o| (o.version, o.downloaded)), Some((2, 5)));
    assert!(matches(&game, &v2()));
    assert_eq!(local_state(&game).map(|s| s.version), Some(2));
    assert!(!game.join(WORK).join("parts").exists());
}

#[test]
fn an_existing_client_downloads_only_what_differs() {
    let store = Store::new();
    let m1 = store.publish(1, &v1(), None);
    store.publish(2, &v2(), Some(&m1));
    let game = temp("existing");
    for (path, data) in v1() {
        Store::put(&game, path, &data);
    }
    Store::put(&game, "cfg/config_env.cfg", b"my options");
    let outcome = run_mode(&game, &store, Mode::Full, None).ok();
    // client.dll, the pack and the new pack; the player's options stay.
    assert_eq!(
        outcome.map(|o| (o.downloaded, o.checked, o.preserved_kept)),
        Some((3, 4, 1))
    );
    assert_eq!(
        fs::read(game.join("cfg/config_env.cfg")).ok(),
        Some(b"my options".to_vec())
    );
}

#[test]
fn an_update_uses_the_deltas() {
    let store = Store::new();
    let m1 = store.publish(1, &v1(), None);
    let game = temp("update");
    assert!(run_mode(&game, &store, Mode::Full, None).is_ok());
    store.publish(2, &v2(), Some(&m1));
    let outcome = run_mode(&game, &store, Mode::Changes, None).ok();
    assert_eq!(
        outcome.map(|o| (o.version, o.deltas, o.downloaded, o.delta_fallbacks)),
        Some((2, 2, 1, 0))
    );
    assert!(matches(&game, &v2()));
}

#[test]
fn a_bad_delta_falls_back_to_the_object() {
    let store = Store::new();
    let m1 = store.publish(1, &v1(), None);
    let game = temp("fallback");
    assert!(run_mode(&game, &store, Mode::Full, None).is_ok());
    let m2 = store.publish(2, &v2(), Some(&m1));
    let delta = &m2.deltas[0];
    Store::put(
        &store.root,
        &layout::delta(&delta.from_sha256, &delta.to_sha256),
        b"garbage",
    );
    let outcome = run_mode(&game, &store, Mode::Changes, None).ok();
    assert_eq!(
        outcome.map(|o| (o.deltas, o.delta_fallbacks, o.downloaded)),
        Some((1, 1, 2))
    );
    assert!(matches(&game, &v2()));
}

#[test]
fn a_wrong_signature_installs_nothing() {
    let store = Store::new();
    store.publish(1, &v1(), None);
    let game = temp("signature");
    let fetch = store.fetch();
    let other = SigningKey::from_bytes(&[7u8; 32]).verifying_key();
    let result = run(
        &game,
        &fetch,
        &other,
        Mode::Full,
        None,
        &AtomicBool::new(false),
        &|| false,
        &|_| {},
    );
    assert!(matches!(result, Err(UpdateError::Signature)));
    assert!(!game.join("Vindictus.exe").exists());
}

#[test]
fn a_running_game_keeps_the_files_staged_until_the_next_run() {
    let store = Store::new();
    store.publish(1, &v1(), None);
    let game = temp("running");
    let fetch = store.fetch();
    let key = store.key.verifying_key();
    let result = run(
        &game,
        &fetch,
        &key,
        Mode::Full,
        None,
        &AtomicBool::new(false),
        &|| true,
        &|_| {},
    );
    assert!(matches!(result, Err(UpdateError::GameRunning)));
    assert!(!game.join("Vindictus.exe").exists());
    assert!(game.join("Vindictus.exe.novo").exists());
    // The next run checks the staged files again only through the published state.
    assert!(
        run(
            &game,
            &fetch,
            &key,
            Mode::Full,
            None,
            &AtomicBool::new(false),
            &|| false,
            &|_| {}
        )
        .is_ok()
    );
    assert!(matches(&game, &v1()));
}

#[test]
fn an_interrupted_apply_is_finished() {
    let game = temp("journal");
    fs::create_dir_all(game.join(WORK)).ok();
    Store::put(&game, "old/gone.dat", b"old file");
    Store::put(&game, "bin/client.dll", b"old");
    Store::put(&game, "bin/client.dll.novo", b"new");
    let journal = Journal {
        state: LocalState {
            version: 3,
            manifest_sha256: "a".repeat(64),
            local: Default::default(),
        },
        paths: vec!["bin/client.dll".into()],
        remove: vec!["old/gone.dat".into(), "never/there.dat".into()],
    };
    assert!(write_json(&game.join(WORK).join(JOURNAL), &journal).is_ok());
    assert_eq!(finish_interrupted(&game).ok(), Some(Some(3)));
    assert_eq!(
        fs::read(game.join("bin/client.dll")).ok(),
        Some(b"new".to_vec())
    );
    assert_eq!(local_state(&game).map(|s| s.version), Some(3));
    assert!(!game.join(WORK).join(JOURNAL).exists());
    assert!(!game.join("old/gone.dat").exists());
}

fn v3() -> Vec<(&'static str, Vec<u8>)> {
    // v2 without the new pack: a version that removes a file.
    v2().into_iter()
        .filter(|(path, _)| !path.starts_with("hfs/AAAA"))
        .collect()
}

#[test]
fn files_a_version_drops_are_removed_but_not_the_players() {
    let store = Store::new();
    let m1 = store.publish(1, &v1(), None);
    let m2 = store.publish(2, &v2(), Some(&m1));
    let game = temp("remove");
    assert!(run_mode(&game, &store, Mode::Full, None).is_ok());
    Store::put(&game, "SAVE/mine.dat", b"player data");
    store.publish(3, &v3(), Some(&m2));
    let outcome = run_mode(&game, &store, Mode::Changes, None).ok();
    assert_eq!(outcome.map(|o| (o.version, o.removed)), Some((3, 1)));
    assert!(
        !game
            .join("hfs/AAAA000000000000000000000000000000000000.hfs")
            .exists()
    );
    assert!(game.join("SAVE/mine.dat").exists() && matches(&game, &v3()));
}

#[test]
fn verify_removes_what_the_launcher_installed_and_the_version_dropped() {
    let store = Store::new();
    let m1 = store.publish(1, &v1(), None);
    let m2 = store.publish(2, &v2(), Some(&m1));
    let game = temp("verify-remove");
    assert!(run_mode(&game, &store, Mode::Full, None).is_ok());
    store.publish(3, &v3(), Some(&m2));
    let outcome = run_mode(&game, &store, Mode::Full, None).ok();
    assert_eq!(outcome.map(|o| o.removed), Some(1));
    // Without a state (a copy the launcher did not install) nothing is removed.
    let other = temp("verify-no-state");
    for (path, data) in v2() {
        Store::put(&other, path, &data);
    }
    let outcome = run_mode(&other, &store, Mode::Full, None).ok();
    assert_eq!(outcome.map(|o| o.removed), Some(0));
    assert!(
        other
            .join("hfs/AAAA000000000000000000000000000000000000.hfs")
            .exists()
    );
}

#[test]
fn the_pinned_key_is_the_servers() {
    assert!(pinned_key().is_ok());
}

#[test]
fn the_bootstrap_follows_the_update_server_host() {
    assert_eq!(
        bootstrap_url("http://192.168.1.13:8090").as_deref(),
        Some("http://192.168.1.13:8081/en-eu-x-gm/EndPoint.txt")
    );
    assert_eq!(
        bootstrap_url("https://jogo.example").as_deref(),
        Some("http://jogo.example:8081/en-eu-x-gm/EndPoint.txt")
    );
    assert!(bootstrap_url("ftp://x").is_none());
}

fn with_database(store: &Store, url: &str) -> Vec<(&'static str, Vec<u8>)> {
    let dir = temp("db");
    let path = dir.join("heroes.db3");
    let db = rusqlite::Connection::open(&path).ok();
    if let Some(db) = db {
        let _ = db.execute_batch(&format!(
            "CREATE TABLE FeatureMatrix(Feature TEXT, [EN-EU-X-GM] TEXT); INSERT INTO FeatureMatrix VALUES('EndPoints', '{url}');"
        ));
    }
    let _ = store;
    let mut files = v1();
    files.push(("sql/heroes.db3", fs::read(&path).unwrap_or_default()));
    files
}

#[test]
fn the_adjusted_database_is_not_repaired_back() {
    let store = Store::new();
    let files = with_database(&store, "http://127.0.0.1:8081/en-eu-x-gm/EndPoint.txt");
    store.publish(1, &files, None);
    let game = temp("endpoint");
    assert!(run_mode(&game, &store, Mode::Full, None).is_ok());
    let url = "http://192.168.1.13:8081/en-eu-x-gm/EndPoint.txt";
    assert_eq!(apply_endpoint(&game, url).ok(), Some(true));
    assert_eq!(apply_endpoint(&game, url).ok(), Some(false));
    let read = rusqlite::Connection::open(game.join(DATABASE))
        .ok()
        .and_then(|db| {
            db.query_row(
                "SELECT [EN-EU-X-GM] FROM FeatureMatrix WHERE Feature='EndPoints'",
                [],
                |r| r.get::<_, String>(0),
            )
            .ok()
        });
    assert_eq!(read.as_deref(), Some(url));
    assert!(local_state(&game).is_some_and(|s| s.local.contains_key(DATABASE)));
    // Verifying downloads nothing: the adjusted database is the known local change.
    let outcome = run_mode(&game, &store, Mode::Full, None).ok();
    assert_eq!(outcome.map(|o| o.downloaded), Some(0));
}
