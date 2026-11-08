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

pub(crate) fn refused(message: impl Into<String>) -> StoreError {
    StoreError::Refused(message.into())
}

/// Writes through a temporary name in the same folder and renames, so a reader never sees half a file.
pub(crate) fn write_atomic(path: &Path, data: &[u8]) -> Result<(), StoreError> {
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

pub(crate) fn load_key(keys: &Path) -> Result<ed25519_dalek::SigningKey, StoreError> {
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

pub(crate) fn read_pointer(
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
pub(crate) fn read_manifest(
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

pub(crate) struct DeltaPlan {
    pub(crate) deltas: Vec<Delta>,
    pub(crate) bytes: u64,
    pub(crate) changed_without_delta: usize,
    pub(crate) update_download_bytes: u64,
}

/// Deltas from `previous` for the files that changed at the same path (docs/future/game-launcher.md 4.3
/// item 4): old and new up to codec::DELTA_MAX_FILE, not the HFS index (its zlib changes everywhere), kept
/// only below codec::DELTA_KEEP_PERCENT of the object. The old form comes from the store's object.
pub(crate) fn plan_deltas(
    store: &Path,
    previous: &Manifest,
    files: &[FileEntry],
    new_form: &dyn Fn(&FileEntry) -> Result<Vec<u8>, StoreError>,
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
        let delta = codec::delta(&old_form, &new_form(file)?)?;
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
pub(crate) fn store_object(
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
            let from_tree = |file: &FileEntry| -> Result<Vec<u8>, StoreError> {
                let mut form = fs::read(request.tree.join(&file.path))?;
                codec::apply(file.transform, &keystream, 0, &mut form);
                Ok(form)
            };
            let plan = plan_deltas(request.store, &before, &files, &from_tree)?;
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
        // A published launcher stays published.
        launcher: previous.as_ref().and_then(|p| p.launcher.clone()),
    };
    write_pointer(request.store, &key, &pointer)?;
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
    fn the_panel_operations_sign_what_they_change() {
        let temp = std::env::temp_dir().join(format!("client-dist-panel-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        tree(&temp);
        let (store, keys, client, ks) = (
            temp.join("store"),
            temp.join("keys"),
            temp.join("client"),
            temp.join("ks.bin"),
        );
        let public = keygen(&keys).unwrap_or_default();
        assert!(publish(&request(&temp, &store, &keys, &client, &ks)).is_ok());
        fs::write(client.join("Vindictus.exe"), b"MZ exe v2").ok();
        assert!(publish(&request(&temp, &store, &keys, &client, &ks)).is_ok());
        let paths = StorePaths {
            store: store.clone(),
            keys: keys.clone(),
        };
        assert_eq!(
            set_minimum(&paths, 2)
                .ok()
                .map(|(b, a)| (b.minimum, a.minimum)),
            Some((1, 2))
        );
        assert!(set_minimum(&paths, 3).is_err());
        let launcher = temp.join("launcher.exe");
        fs::write(&launcher, b"MZ launcher").ok();
        assert!(publish_launcher(&paths, &launcher, "0.2.0").is_ok());
        assert!(publish_launcher(&paths, &launcher, "latest").is_err());
        let withdrawn = withdraw(&paths, "Volta").ok();
        assert_eq!(
            withdrawn.as_ref().map(|r| (r.version, r.minimum)),
            Some((3, 2))
        );
        let v1 = fs::read(store.join("manifests/1.json"))
            .ok()
            .and_then(|b| Manifest::parse(&b).ok());
        let v3 = fs::read(store.join("manifests/3.json"))
            .ok()
            .and_then(|b| Manifest::parse(&b).ok());
        assert_eq!(v1.map(|m| m.files), v3.map(|m| m.files));
        assert!(
            write_news(
                &paths,
                vec![vindictus_client_dist::manifest::NewsItem {
                    id: 1,
                    title: "Olá".into(),
                    body: "Primeira notícia.".into(),
                    published_unix: 1
                }]
            )
            .is_ok()
        );
        let view = list_versions(&store, &public).ok();
        assert_eq!(
            view.as_ref().map(|v| (
                v.current,
                v.minimum,
                v.versions.len(),
                v.launcher_version.clone()
            )),
            Some((3, 2, 3, Some("0.2.0".into())))
        );
        // The launcher survives a later publish.
        assert!(publish(&request(&temp, &store, &keys, &client, &ks)).is_ok());
        assert_eq!(
            list_versions(&store, &public)
                .ok()
                .and_then(|v| v.launcher_version),
            Some("0.2.0".into())
        );
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

/// Signs and writes `versao.json` (the signature first: a reader never pairs the new pointer with an
/// old signature that verifies).
pub(crate) fn write_pointer(
    store: &Path,
    key: &ed25519_dalek::SigningKey,
    pointer: &VersionPointer,
) -> Result<(), StoreError> {
    let bytes = pointer.to_bytes()?;
    VersionPointer::parse(&bytes)?;
    write_atomic(
        &store.join(layout::signature(layout::POINTER)),
        &sign::sign(key, &bytes),
    )?;
    write_atomic(&store.join(layout::POINTER), &bytes)?;
    Ok(())
}

pub(crate) fn current_pointer(
    store: &Path,
    key: &ed25519_dalek::SigningKey,
) -> Result<VersionPointer, StoreError> {
    read_pointer(store, &key.verifying_key())?
        .ok_or_else(|| refused("the store has no published version"))
}

/// The store and the key folder the panel works on (L5).
#[derive(Debug, Clone)]
pub struct StorePaths {
    pub store: PathBuf,
    pub keys: PathBuf,
}

/// Changes the oldest version a client may play with (re-signs `versao.json`).
pub fn set_minimum(
    paths: &StorePaths,
    minimum: u32,
) -> Result<(VersionPointer, VersionPointer), StoreError> {
    let key = load_key(&paths.keys)?;
    let before = current_pointer(&paths.store, &key)?;
    if minimum == 0 || minimum > before.current {
        return Err(refused(
            "the minimum version must be between 1 and the current one",
        ));
    }
    let after = VersionPointer {
        minimum,
        ..before.clone()
    };
    write_pointer(&paths.store, &key, &after)?;
    Ok((before, after))
}

/// Withdraws the current version N: publishes N+1 with the files of N-1 (objects already in the store),
/// with deltas from N; the minimum stays (it never exceeds the new current). Nothing is deleted.
pub fn withdraw(paths: &StorePaths, title: &str) -> Result<PublishReport, StoreError> {
    let started = Instant::now();
    let key = load_key(&paths.keys)?;
    let public = key.verifying_key();
    let pointer = current_pointer(&paths.store, &key)?;
    if pointer.current < 2 {
        return Err(refused("the first version cannot be withdrawn"));
    }
    let current = read_manifest(&paths.store, pointer.current, &public)?;
    let restored = read_manifest(&paths.store, pointer.current - 1, &public)?;
    let version = pointer.current + 1;
    let from_store = |file: &FileEntry| -> Result<Vec<u8>, StoreError> {
        Ok(codec::decompress(
            &fs::read(paths.store.join(layout::object(&file.sha256)))?,
            file.size,
        )?)
    };
    let plan = plan_deltas(&paths.store, &current, &restored.files, &from_store)?;
    let created_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let manifest = Manifest {
        version,
        title: title.chars().take(200).collect(),
        notes: format!(
            "Retirada da versão {}: volta ao conteúdo da versão {}.",
            pointer.current,
            pointer.current - 1
        ),
        created_unix,
        deltas: plan.deltas,
        ..restored.clone()
    };
    manifest.check()?;
    let bytes = manifest.to_bytes()?;
    let name = layout::manifest(version);
    write_atomic(&paths.store.join(&name), &bytes)?;
    write_atomic(
        &paths.store.join(layout::signature(&name)),
        &sign::sign(&key, &bytes),
    )?;
    let after = VersionPointer {
        current: version,
        minimum: pointer.minimum.min(version),
        manifest_sha256: sha256_hex(&bytes),
        ..pointer
    };
    write_pointer(&paths.store, &key, &after)?;
    let unique: HashMap<&str, u64> = manifest
        .files
        .iter()
        .filter(|f| !f.preserve)
        .map(|f| (f.sha256.as_str(), f.object_size))
        .collect();
    Ok(PublishReport {
        version,
        minimum: after.minimum,
        files: manifest.files.len(),
        bytes: manifest.files.iter().map(|f| f.size).sum(),
        objects_written: 0,
        objects_written_bytes: 0,
        objects_reused: manifest.files.len(),
        install_download_bytes: manifest.keystream.object_size + unique.values().sum::<u64>(),
        levels: BTreeMap::new(),
        by_transform: BTreeMap::new(),
        seconds: started.elapsed().as_secs(),
        deltas: manifest.deltas.len(),
        delta_bytes: plan.bytes,
        changed_without_delta: plan.changed_without_delta,
        update_download_bytes: Some(plan.update_download_bytes),
    })
}

/// Publishes a launcher executable for self-update: its object and the pointer's `launcher`.
pub fn publish_launcher(
    paths: &StorePaths,
    exe: &Path,
    version: &str,
) -> Result<VersionPointer, StoreError> {
    let key = load_key(&paths.keys)?;
    let pointer = current_pointer(&paths.store, &key)?;
    if vindictus_client_dist::manifest::version_parts(version).is_none() {
        return Err(refused("the launcher version must look like 1.2.3"));
    }
    let data = fs::read(exe)?;
    if data.len() < 2
        || &data[..2] != b"MZ"
        || data.len() as u64 > vindictus_client_dist::manifest::MAX_LAUNCHER_SIZE
    {
        return Err(refused(
            "the launcher must be a Windows executable up to 64 MB",
        ));
    }
    let sha = sha256_hex(&data);
    let size = data.len() as u64;
    let keystream = [0u8; codec::KEYSTREAM_SIZE];
    let (object_size, _) = store_object(&paths.store, &sha, data, Transform::None, &keystream)?;
    let after = VersionPointer {
        launcher: Some(vindictus_client_dist::manifest::LauncherRef {
            version: version.into(),
            sha256: sha,
            size,
            object_size,
        }),
        ..pointer
    };
    write_pointer(&paths.store, &key, &after)?;
    Ok(after)
}

/// Signs and writes `news.json` (newest first, at most MAX_NEWS items).
pub fn write_news(
    paths: &StorePaths,
    items: Vec<vindictus_client_dist::manifest::NewsItem>,
) -> Result<(), StoreError> {
    let key = load_key(&paths.keys)?;
    let feed = vindictus_client_dist::manifest::NewsFeed {
        format: FORMAT,
        items: items
            .into_iter()
            .take(vindictus_client_dist::manifest::MAX_NEWS)
            .collect(),
    };
    let bytes = feed.to_bytes()?;
    vindictus_client_dist::manifest::NewsFeed::parse(&bytes)?;
    write_atomic(
        &paths.store.join(layout::signature(layout::NEWS)),
        &sign::sign(&key, &bytes),
    )?;
    write_atomic(&paths.store.join(layout::NEWS), &bytes)?;
    Ok(())
}

/// One published version, for the panel.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct VersionSummary {
    pub version: u32,
    pub title: String,
    pub notes: String,
    pub created_unix: u64,
    pub files: usize,
    pub bytes: u64,
    /// Download to install it from nothing.
    pub install_bytes: u64,
    /// Download to update from the previous version (deltas where there are).
    pub update_bytes: Option<u64>,
    pub deltas: usize,
    /// Files of the previous version it no longer has.
    pub removed: usize,
}

/// The store as the panel shows it.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct StoreView {
    pub current: u32,
    pub minimum: u32,
    pub launcher_version: Option<String>,
    pub launcher_sha256: Option<String>,
    pub versions: Vec<VersionSummary>,
    /// Bytes of objects/ and deltas/ on disk.
    pub store_bytes: u64,
}

fn folder_bytes(folder: &Path) -> u64 {
    let mut total = 0;
    let mut pending = vec![folder.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            match entry.file_type() {
                Ok(kind) if kind.is_dir() => pending.push(entry.path()),
                Ok(_) => total += entry.metadata().map(|m| m.len()).unwrap_or(0),
                Err(_) => {}
            }
        }
    }
    total
}

/// Every published version (signatures checked with the store's own key file).
pub fn list_versions(store: &Path, public_hex: &str) -> Result<StoreView, StoreError> {
    let public = sign::verifying_key(public_hex)?;
    let pointer = read_pointer(store, &public)?
        .ok_or_else(|| refused("the store has no published version"))?;
    let mut versions = Vec::new();
    let mut previous: Option<Manifest> = None;
    for version in 1..=pointer.current {
        let manifest = read_manifest(store, version, &public)?;
        let unique: HashMap<&str, u64> = manifest
            .files
            .iter()
            .filter(|f| !f.preserve)
            .map(|f| (f.sha256.as_str(), f.object_size))
            .collect();
        let update_bytes = previous.as_ref().map(|before| {
            let had: std::collections::HashSet<&str> =
                before.files.iter().map(|f| f.sha256.as_str()).collect();
            let deltas: HashMap<&str, u64> = manifest
                .deltas
                .iter()
                .map(|d| (d.to_sha256.as_str(), d.delta_size))
                .collect();
            let mut counted = std::collections::HashSet::new();
            manifest
                .files
                .iter()
                .filter(|f| {
                    !f.preserve
                        && !had.contains(f.sha256.as_str())
                        && counted.insert(f.sha256.as_str())
                })
                .map(|f| {
                    deltas
                        .get(f.sha256.as_str())
                        .copied()
                        .unwrap_or(f.object_size)
                })
                .sum()
        });
        let removed = previous.as_ref().map_or(0, |before| {
            let kept: std::collections::HashSet<String> = manifest
                .files
                .iter()
                .map(|f| f.path.to_ascii_lowercase())
                .collect();
            before
                .files
                .iter()
                .filter(|f| !f.preserve && !kept.contains(&f.path.to_ascii_lowercase()))
                .count()
        });
        versions.push(VersionSummary {
            version,
            title: manifest.title.clone(),
            notes: manifest.notes.clone(),
            created_unix: manifest.created_unix,
            files: manifest.files.len(),
            bytes: manifest.files.iter().map(|f| f.size).sum(),
            install_bytes: manifest.keystream.object_size + unique.values().sum::<u64>(),
            update_bytes,
            deltas: manifest.deltas.len(),
            removed,
        });
        previous = Some(manifest);
    }
    Ok(StoreView {
        current: pointer.current,
        minimum: pointer.minimum,
        launcher_version: pointer.launcher.as_ref().map(|l| l.version.clone()),
        launcher_sha256: pointer.launcher.map(|l| l.sha256),
        versions,
        store_bytes: folder_bytes(&store.join("objects")) + folder_bytes(&store.join("deltas")),
    })
}
