//! Store of the client distribution on disk (`${CLIENT_DIST_DIR}`, docs/future/game-launcher.md 4.3 item 7):
//! key generation, publishing a client tree as version N+1 (objects only for content the store lacks),
//! and verification. The private key lives outside the served store (`--keys`); nothing here logs it.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use vindictus_client_dist::manifest::{
    Delta, FORMAT, FileEntry, Manifest, ObjectRef, Transform, VersionPointer,
};
use vindictus_client_dist::{DistError, codec, layout, paths, rules, sha256_hex, sign};

/// Private key file inside `--keys` (32-byte Ed25519 seed).
pub const PRIVATE_KEY: &str = "updates.ed25519";

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error(transparent)]
    Dist(#[from] DistError),
    #[error("client distribution I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Refused(String),
}

fn refused(message: impl Into<String>) -> StoreError {
    StoreError::Refused(message.into())
}

/// Writes through a temporary name in the same folder and renames, so a reader never sees half a file.
fn write_atomic(path: &Path, data: &[u8]) -> Result<(), StoreError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    // Unique per write: two workers may store the same object (identical files) at once.
    static WRITES: AtomicUsize = AtomicUsize::new(0);
    let partial = path.with_extension(format!(
        "partial-{}-{}",
        std::process::id(),
        WRITES.fetch_add(1, Ordering::Relaxed)
    ));
    {
        let mut file = fs::File::create(&partial)?;
        file.write_all(data)?;
        file.sync_all()?;
    }
    fs::rename(&partial, path)?;
    Ok(())
}

/// Creates a new key pair in `keys` (refuses to replace one). Returns the public key (hex).
pub fn keygen(keys: &Path) -> Result<String, StoreError> {
    let private = keys.join(PRIVATE_KEY);
    if private.exists() {
        return Err(refused(
            "a key already exists; publishing with a new one breaks every pinned launcher",
        ));
    }
    let key = ed25519_dalek::SigningKey::generate(&mut rand_core::OsRng);
    fs::create_dir_all(keys)?;
    write_atomic(&private, key.as_bytes())?;
    let public = sign::public_hex(&key);
    write_atomic(&keys.join(layout::PUBLIC_KEY), public.as_bytes())?;
    Ok(public)
}

fn load_key(keys: &Path) -> Result<ed25519_dalek::SigningKey, StoreError> {
    let seed = zeroize::Zeroizing::new(fs::read(keys.join(PRIVATE_KEY))?);
    Ok(sign::signing_key(&seed)?)
}

/// Every file of a client tree that a version holds, relative with `/`, sorted. Links are refused.
pub fn tree_files(root: &Path) -> Result<Vec<String>, StoreError> {
    let mut out = Vec::new();
    let mut pending = vec![PathBuf::new()];
    while let Some(relative) = pending.pop() {
        for entry in fs::read_dir(root.join(&relative))? {
            let entry = entry?;
            let kind = entry.file_type()?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| refused("a file name is not valid text"))?;
            let path = if relative.as_os_str().is_empty() {
                PathBuf::from(&name)
            } else {
                relative.join(&name)
            };
            let text = path.to_string_lossy().replace('\\', "/");
            if kind.is_symlink() {
                return Err(refused(format!("{text} is a link")));
            }
            if kind.is_dir() {
                if !rules::EXCLUDED_DIRS
                    .iter()
                    .any(|d| d.eq_ignore_ascii_case(&name))
                {
                    pending.push(path);
                }
            } else if !rules::excluded(&text) {
                out.push(text);
            }
        }
    }
    out.sort();
    paths::check_all(out.iter().map(String::as_str))?;
    Ok(out)
}

fn read_pointer(
    store: &Path,
    key: &ed25519_dalek::VerifyingKey,
) -> Result<Option<VersionPointer>, StoreError> {
    let path = store.join(layout::POINTER);
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(&path)?;
    sign::verify(
        key,
        &bytes,
        &fs::read(store.join(layout::signature(layout::POINTER)))?,
        "versao.json",
    )?;
    Ok(Some(VersionPointer::parse(&bytes)?))
}

/// What a publish asks for.
pub struct PublishRequest<'a> {
    pub store: &'a Path,
    pub keys: &'a Path,
    pub tree: &'a Path,
    pub keystream: &'a Path,
    pub title: &'a str,
    pub notes: &'a str,
    pub minimum: Option<u32>,
    pub threads: usize,
}

/// What a publish did (printed as JSON).
#[derive(Debug, Serialize)]
pub struct PublishReport {
    pub version: u32,
    pub minimum: u32,
    pub files: usize,
    pub bytes: u64,
    pub objects_written: usize,
    pub objects_written_bytes: u64,
    pub objects_reused: usize,
    pub install_download_bytes: u64,
    pub levels: BTreeMap<i32, usize>,
    pub by_transform: BTreeMap<String, (usize, u64, u64)>,
    pub seconds: u64,
    /// Deltas from the previous version written for this one, and their bytes.
    pub deltas: usize,
    pub delta_bytes: u64,
    /// Changed files that got no delta (too large, the index, or a delta not worth it).
    pub changed_without_delta: usize,
    /// Download to update from the previous version: deltas where there are, whole objects elsewhere.
    pub update_download_bytes: Option<u64>,
}

/// The previous version's manifest, checked against the key.
fn read_manifest(
    store: &Path,
    version: u32,
    key: &ed25519_dalek::VerifyingKey,
) -> Result<Manifest, StoreError> {
    let name = layout::manifest(version);
    let bytes = fs::read(store.join(&name))?;
    sign::verify(
        key,
        &bytes,
        &fs::read(store.join(layout::signature(&name)))?,
        "manifest",
    )?;
    Ok(Manifest::parse(&bytes)?)
}

struct DeltaPlan {
    deltas: Vec<Delta>,
    bytes: u64,
    changed_without_delta: usize,
    update_download_bytes: u64,
}

/// Deltas from `previous` for the files that changed at the same path (docs/future/game-launcher.md 4.3
/// item 4): old and new up to codec::DELTA_MAX_FILE, not the HFS index (its zlib changes everywhere), kept
/// only below codec::DELTA_KEEP_PERCENT of the object. The old form comes from the store's object.
fn plan_deltas(
    store: &Path,
    tree: &Path,
    previous: &Manifest,
    files: &[FileEntry],
    keystream: &[u8; codec::KEYSTREAM_SIZE],
) -> Result<DeltaPlan, StoreError> {
    let before: HashMap<&str, &FileEntry> = previous
        .files
        .iter()
        .map(|f| (f.path.as_str(), f))
        .collect();
    let had: std::collections::HashSet<&str> =
        previous.files.iter().map(|f| f.sha256.as_str()).collect();
    let mut plan = DeltaPlan {
        deltas: Vec::new(),
        bytes: 0,
        changed_without_delta: 0,
        update_download_bytes: 0,
    };
    let mut counted = std::collections::HashSet::new();
    for file in files {
        if had.contains(file.sha256.as_str())
            || file.preserve
            || !counted.insert(file.sha256.as_str())
        {
            continue;
        }
        let old = before.get(file.path.as_str()).filter(|old| {
            old.size <= codec::DELTA_MAX_FILE
                && file.size <= codec::DELTA_MAX_FILE
                && !file.path.eq_ignore_ascii_case(rules::HFS_INDEX)
        });
        let Some(old) = old else {
            plan.changed_without_delta += usize::from(before.contains_key(file.path.as_str()));
            plan.update_download_bytes += file.object_size;
            continue;
        };
        let old_form = codec::decompress(
            &fs::read(store.join(layout::object(&old.sha256)))?,
            old.size,
        )?;
        let mut new_form = fs::read(tree.join(&file.path))?;
        codec::apply(file.transform, keystream, 0, &mut new_form);
        let delta = codec::delta(&old_form, &new_form)?;
        if !codec::delta_worth(delta.len() as u64, file.object_size) {
            plan.changed_without_delta += 1;
            plan.update_download_bytes += file.object_size;
            continue;
        }
        let path = store.join(layout::delta(&old.sha256, &file.sha256));
        if !path.exists() {
            write_atomic(&path, &delta)?;
        }
        plan.bytes += delta.len() as u64;
        plan.update_download_bytes += delta.len() as u64;
        plan.deltas.push(Delta {
            path: file.path.clone(),
            from_sha256: old.sha256.clone(),
            from_size: old.size,
            to_sha256: file.sha256.clone(),
            delta_size: delta.len() as u64,
        });
    }
    Ok(plan)
}

struct Stored {
    entry: FileEntry,
    written: Option<i32>,
}

/// Object of `data` (already the final file): transformed, compressed and written unless the store has it.
fn store_object(
    store: &Path,
    sha: &str,
    mut data: Vec<u8>,
    transform: Transform,
    keystream: &[u8; codec::KEYSTREAM_SIZE],
) -> Result<(u64, Option<i32>), StoreError> {
    let path = store.join(layout::object(sha));
    if let Ok(meta) = fs::metadata(&path)
        && meta.len() > 0
    {
        return Ok((meta.len(), None));
    }
    codec::apply(transform, keystream, 0, &mut data);
    let (object, level) = codec::compress(&data)?;
    write_atomic(&path, &object)?;
    Ok((object.len() as u64, Some(level)))
}

/// Publishes the tree as the next version: objects for the content the store lacks, then the signed
/// manifest, then the signed pointer (the last write is what makes the version visible).
pub fn publish(request: &PublishRequest<'_>) -> Result<PublishReport, StoreError> {
    let started = Instant::now();
    let key = load_key(request.keys)?;
    let public = key.verifying_key();
    let keystream_bytes = fs::read(request.keystream)?;
    let keystream = codec::keystream(&keystream_bytes)?;
    let previous = read_pointer(request.store, &public)?;
    let version = previous.as_ref().map_or(1, |p| p.current + 1);
    let minimum = request
        .minimum
        .or(previous.as_ref().map(|p| p.minimum))
        .unwrap_or(1);
    if minimum == 0 || minimum > version {
        return Err(refused(
            "the minimum version must be between 1 and the new version",
        ));
    }
    if request.store.join(layout::manifest(version)).exists() {
        return Err(refused(format!(
            "manifests/{version}.json already exists without a pointer to it"
        )));
    }
    let files = tree_files(request.tree)?;
    if files.is_empty() || files.len() > vindictus_client_dist::manifest::MAX_FILES {
        return Err(refused("the tree holds no files or too many"));
    }

    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<Stored>>> = Mutex::new((0..files.len()).map(|_| None).collect());
    let failure: Mutex<Option<StoreError>> = Mutex::new(None);
    std::thread::scope(|scope| {
        for _ in 0..request.threads.clamp(1, 4) {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    if index >= files.len() || failure.lock().map(|f| f.is_some()).unwrap_or(true) {
                        break;
                    }
                    let path = &files[index];
                    let outcome = (|| -> Result<Stored, StoreError> {
                        let data = fs::read(request.tree.join(path))?;
                        let size = data.len() as u64;
                        if size > vindictus_client_dist::manifest::MAX_FILE_SIZE {
                            return Err(refused(format!("{path} is too large")));
                        }
                        let sha = sha256_hex(&data);
                        let transform = rules::transform_of(path);
                        let (object_size, written) =
                            store_object(request.store, &sha, data, transform, &keystream)?;
                        Ok(Stored {
                            entry: FileEntry {
                                path: path.clone(),
                                sha256: sha,
                                size,
                                transform,
                                object_size,
                                preserve: rules::preserved(path),
                            },
                            written,
                        })
                    })();
                    match outcome {
                        Ok(stored) => {
                            if let Ok(mut all) = results.lock() {
                                all[index] = Some(stored);
                            }
                            if index.is_multiple_of(250) {
                                tracing::info!(
                                    done = index,
                                    total = files.len(),
                                    "client-dist publish progress"
                                );
                            }
                        }
                        Err(error) => {
                            if let Ok(mut slot) = failure.lock() {
                                slot.get_or_insert(error);
                            }
                            break;
                        }
                    }
                }
            });
        }
    });
    if let Some(error) = failure
        .into_inner()
        .map_err(|_| refused("a publish worker failed"))?
    {
        return Err(error);
    }
    let stored: Vec<Stored> = results
        .into_inner()
        .map_err(|_| refused("a publish worker failed"))?
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| refused("a file was not stored"))?;

    let keystream_sha = sha256_hex(&keystream_bytes);
    let (keystream_object, _) = store_object(
        request.store,
        &keystream_sha,
        keystream_bytes.clone(),
        Transform::None,
        &keystream,
    )?;
    let created_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut report = PublishReport {
        version,
        minimum,
        files: stored.len(),
        bytes: 0,
        objects_written: 0,
        objects_written_bytes: 0,
        objects_reused: 0,
        install_download_bytes: keystream_object,
        levels: BTreeMap::new(),
        by_transform: BTreeMap::new(),
        seconds: 0,
        deltas: 0,
        delta_bytes: 0,
        changed_without_delta: 0,
        update_download_bytes: None,
    };
    let mut unique: HashMap<&str, u64> = HashMap::new();
    for s in &stored {
        report.bytes += s.entry.size;
        match s.written {
            Some(level) => {
                report.objects_written += 1;
                report.objects_written_bytes += s.entry.object_size;
                *report.levels.entry(level).or_default() += 1;
            }
            None => report.objects_reused += 1,
        }
        let name = match s.entry.transform {
            Transform::None => "none",
            Transform::HfsXor => "hfs-xor",
        };
        let row = report.by_transform.entry(name.into()).or_default();
        row.0 += 1;
        row.1 += s.entry.size;
        row.2 += s.entry.object_size;
        if !s.entry.preserve {
            unique.insert(s.entry.sha256.as_str(), s.entry.object_size);
        }
    }
    report.install_download_bytes += unique.values().sum::<u64>();
    let files: Vec<FileEntry> = stored.into_iter().map(|s| s.entry).collect();
    let deltas = match &previous {
        Some(pointer) => {
            let before = read_manifest(request.store, pointer.current, &public)?;
            let plan = plan_deltas(request.store, request.tree, &before, &files, &keystream)?;
            report.deltas = plan.deltas.len();
            report.delta_bytes = plan.bytes;
            report.changed_without_delta = plan.changed_without_delta;
            report.update_download_bytes = Some(plan.update_download_bytes);
            plan.deltas
        }
        None => Vec::new(),
    };
    let manifest = Manifest {
        format: FORMAT,
        version,
        title: request.title.into(),
        notes: request.notes.into(),
        created_unix,
        keystream: ObjectRef {
            sha256: keystream_sha,
            size: codec::KEYSTREAM_SIZE as u64,
            object_size: keystream_object,
        },
        files,
        deltas,
    };
    manifest.check()?;
    let manifest_bytes = manifest.to_bytes()?;
    let manifest_name = layout::manifest(version);
    write_atomic(&request.store.join(&manifest_name), &manifest_bytes)?;
    write_atomic(
        &request.store.join(layout::signature(&manifest_name)),
        &sign::sign(&key, &manifest_bytes),
    )?;
    write_atomic(
        &request.store.join(layout::PUBLIC_KEY),
        sign::public_hex(&key).as_bytes(),
    )?;
    let pointer = VersionPointer {
        format: FORMAT,
        current: version,
        minimum,
        manifest_sha256: sha256_hex(&manifest_bytes),
    };
    let pointer_bytes = pointer.to_bytes()?;
    write_atomic(
        &request.store.join(layout::signature(layout::POINTER)),
        &sign::sign(&key, &pointer_bytes),
    )?;
    write_atomic(&request.store.join(layout::POINTER), &pointer_bytes)?;
    report.seconds = started.elapsed().as_secs();
    Ok(report)
}

/// What a verification found.
#[derive(Debug, Serialize)]
pub struct VerifyReport {
    pub current: u32,
    pub manifests: usize,
    pub objects_listed: usize,
    pub objects_checked: usize,
    pub deltas_listed: usize,
    pub deltas_checked: usize,
    pub problems: Vec<String>,
}

/// Checks the pointer and every manifest against the public key, that each listed object exists and,
/// with `deep`, that each one decompresses to its SHA-256 (reads the whole store).
pub fn verify(store: &Path, public_hex: &str, deep: bool) -> Result<VerifyReport, StoreError> {
    let public = sign::verifying_key(public_hex)?;
    let pointer =
        read_pointer(store, &public)?.ok_or_else(|| refused("the store has no versao.json"))?;
    let mut report = VerifyReport {
        current: pointer.current,
        manifests: 0,
        objects_listed: 0,
        objects_checked: 0,
        deltas_listed: 0,
        deltas_checked: 0,
        problems: Vec::new(),
    };
    let mut seen = HashMap::new();
    for version in 1..=pointer.current {
        let name = layout::manifest(version);
        let bytes = fs::read(store.join(&name))?;
        sign::verify(
            &public,
            &bytes,
            &fs::read(store.join(layout::signature(&name)))?,
            "manifest",
        )?;
        if version == pointer.current && sha256_hex(&bytes) != pointer.manifest_sha256 {
            report
                .problems
                .push(format!("{name} differs from versao.json"));
        }
        let manifest = Manifest::parse(&bytes)?;
        report.manifests += 1;
        let keystream_bytes = fs::read(store.join(layout::object(&manifest.keystream.sha256)))
            .map_err(StoreError::from)
            .and_then(|object| Ok(codec::decompress(&object, manifest.keystream.size)?))?;
        let keystream = codec::keystream(&keystream_bytes)?;
        for file in &manifest.files {
            if seen.insert(file.sha256.clone(), ()).is_some() {
                continue;
            }
            report.objects_listed += 1;
            let path = store.join(layout::object(&file.sha256));
            match fs::metadata(&path) {
                Ok(meta) if meta.len() == file.object_size => {}
                _ => {
                    report.problems.push(format!(
                        "{} ({}) missing or of another size",
                        layout::object(&file.sha256),
                        file.path
                    ));
                    continue;
                }
            }
            if deep {
                let object = fs::read(&path)?;
                if codec::restore(&object, file.transform, &keystream, &file.sha256, file.size)
                    .is_err()
                {
                    report.problems.push(format!(
                        "{} does not restore {}",
                        layout::object(&file.sha256),
                        file.path
                    ));
                }
                report.objects_checked += 1;
            }
        }
        let transforms: HashMap<&str, Transform> = manifest
            .files
            .iter()
            .map(|f| (f.path.as_str(), f.transform))
            .collect();
        let sizes: HashMap<&str, u64> = manifest
            .files
            .iter()
            .map(|f| (f.path.as_str(), f.size))
            .collect();
        for delta in &manifest.deltas {
            report.deltas_listed += 1;
            let name = layout::delta(&delta.from_sha256, &delta.to_sha256);
            match fs::metadata(store.join(&name)) {
                Ok(meta) if meta.len() == delta.delta_size => {}
                _ => {
                    report.problems.push(format!(
                        "{name} ({}) missing or of another size",
                        delta.path
                    ));
                    continue;
                }
            }
            if deep {
                let size = sizes.get(delta.path.as_str()).copied().unwrap_or(0);
                let transform = transforms
                    .get(delta.path.as_str())
                    .copied()
                    .unwrap_or(Transform::None);
                let restored = fs::read(store.join(layout::object(&delta.from_sha256)))
                    .map_err(StoreError::from)
                    .and_then(|old| Ok(codec::decompress(&old, delta.from_size)?))
                    .and_then(|old| {
                        Ok(codec::restore_from_delta(
                            &old,
                            &fs::read(store.join(&name))?,
                            transform,
                            &keystream,
                            &delta.to_sha256,
                            size,
                        )?)
                    });
                if restored.is_err() {
                    report
                        .problems
                        .push(format!("{name} does not rebuild {}", delta.path));
                }
                report.deltas_checked += 1;
            }
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(root: &Path) {
        let ks: Vec<u8> = (0..4096u32).map(|i| (i * 31 + 5) as u8).collect();
        fs::create_dir_all(root.join("client/hfs")).ok();
        fs::create_dir_all(root.join("client/cfg")).ok();
        fs::create_dir_all(root.join("client/hfs_extracted")).ok();
        fs::write(root.join("ks.bin"), &ks).ok();
        fs::write(root.join("client/Vindictus.exe"), b"MZ exe").ok();
        fs::write(
            root.join("client/hfs/0FABE22A68C0451A7EF97F6E8E682A448BB8122A.hfs"),
            b"HF\x01\x02 pack".repeat(500),
        )
        .ok();
        fs::write(root.join("client/cfg/config_env.cfg"), b"player").ok();
        fs::write(root.join("client/start.bat"), b"excluded").ok();
        fs::write(root.join("client/hfs_extracted/a.txt"), b"excluded").ok();
    }

    fn request<'a>(
        root: &'a Path,
        store: &'a Path,
        keys: &'a Path,
        client: &'a Path,
        ks: &'a Path,
    ) -> PublishRequest<'a> {
        let _ = root;
        PublishRequest {
            store,
            keys,
            tree: client,
            keystream: ks,
            title: "v",
            notes: "",
            minimum: None,
            threads: 2,
        }
    }

    #[test]
    fn publish_twice_reuses_objects_and_verifies() {
        let temp = std::env::temp_dir().join(format!("client-dist-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        tree(&temp);
        let (store, keys, client, ks) = (
            temp.join("store"),
            temp.join("keys"),
            temp.join("client"),
            temp.join("ks.bin"),
        );
        let public = keygen(&keys).unwrap_or_default();
        assert!(keygen(&keys).is_err(), "a second key must be refused");
        let first = publish(&request(&temp, &store, &keys, &client, &ks)).ok();
        assert_eq!(
            first
                .as_ref()
                .map(|r| (r.version, r.files, r.objects_written)),
            Some((1, 3, 3))
        );
        let second = publish(&request(&temp, &store, &keys, &client, &ks)).ok();
        assert_eq!(
            second
                .as_ref()
                .map(|r| (r.version, r.objects_written, r.objects_reused)),
            Some((2, 0, 3))
        );
        let report = verify(&store, &public, true).ok();
        assert_eq!(
            report
                .as_ref()
                .map(|r| (r.current, r.manifests, r.objects_checked, r.problems.len())),
            Some((2, 2, 3, 0))
        );
        let manifest = fs::read(store.join("manifests/2.json"))
            .ok()
            .and_then(|b| Manifest::parse(&b).ok());
        let files = manifest.map(|m| {
            m.files
                .iter()
                .map(|f| (f.path.clone(), f.transform, f.preserve))
                .collect::<Vec<_>>()
        });
        assert_eq!(
            files,
            Some(vec![
                ("Vindictus.exe".into(), Transform::None, false),
                ("cfg/config_env.cfg".into(), Transform::None, true),
                (
                    "hfs/0FABE22A68C0451A7EF97F6E8E682A448BB8122A.hfs".into(),
                    Transform::HfsXor,
                    false
                ),
            ])
        );
        // A tampered pointer no longer verifies.
        fs::write(store.join("versao.json"), b"{}").ok();
        assert!(verify(&store, &public, false).is_err());
        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn a_changed_file_gets_a_delta_but_the_index_does_not() {
        let temp = std::env::temp_dir().join(format!("client-dist-delta-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        tree(&temp);
        let (store, keys, client, ks) = (
            temp.join("store"),
            temp.join("keys"),
            temp.join("client"),
            temp.join("ks.bin"),
        );
        let mut state = 0x9E37_79B9u32;
        let mut noise: Vec<u8> = (0..200_000)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                state as u8
            })
            .collect();
        fs::create_dir_all(client.join("bin")).ok();
        fs::write(client.join("bin/client.dll"), &noise).ok();
        fs::write(client.join(rules::HFS_INDEX), &noise[..50_000]).ok();
        let public = keygen(&keys).unwrap_or_default();
        assert!(publish(&request(&temp, &store, &keys, &client, &ks)).is_ok());
        noise[1_000..1_016].fill(0);
        fs::write(client.join("bin/client.dll"), &noise).ok();
        fs::write(client.join(rules::HFS_INDEX), &noise[1_000..51_000]).ok();
        let second = publish(&request(&temp, &store, &keys, &client, &ks)).ok();
        let summary = second.as_ref().map(|r| {
            (
                r.deltas,
                r.changed_without_delta,
                r.update_download_bytes.unwrap_or(0) < 60_000,
            )
        });
        assert_eq!(summary, Some((1, 1, true)));
        let report = verify(&store, &public, true).ok();
        assert_eq!(
            report
                .as_ref()
                .map(|r| (r.deltas_listed, r.deltas_checked, r.problems.len())),
            Some((1, 1, 0))
        );
        let _ = fs::remove_dir_all(&temp);
    }
}
