//! The client on the server (docs/future/game-launcher.md 4.10, operator request of 08/11/2026): a version
//! is the previous one plus files added, replaced or removed at a game path, published without rebuilding
//! the client. The store already holds every file of every version as objects; this module
//!
//! - stages new file bytes under their SHA-256 (`staging/<sha256>`),
//! - publishes N+1 = manifest N + changes (objects only for the new content, deltas from N),
//! - keeps the server's client tree (`work/`, where the client builder applies the panel content) equal to
//!   the published version, and lists how that tree differs from it.
//!
//! Hashing the ~16 GB tree is the slow part, so a cache keeps each file's SHA-256 by size and mtime.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use vindictus_client_dist::manifest::{FileEntry, Manifest, Transform, VersionPointer};
use vindictus_client_dist::{codec, layout, paths, rules, sha256_hex, sign};

use super::client_dist_store::{
    self as store, PublishReport, StoreError, StorePaths, current_pointer, load_key, plan_deltas,
    read_manifest, read_pointer, refused, store_object, write_atomic, write_pointer,
};

/// Largest file a draft accepts (the largest client file is ~206 MB, a shader).
pub const MAX_STAGED_FILE: u64 = 256 << 20;

/// One change of a draft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// The file at `path` becomes the staged bytes `staging/<sha256>`.
    Put {
        path: String,
        sha256: String,
        size: u64,
    },
    /// The file at `path` leaves the version (the launcher deletes it).
    Remove { path: String },
}

impl Change {
    pub fn path(&self) -> &str {
        match self {
            Change::Put { path, .. } | Change::Remove { path } => path,
        }
    }
}

/// A game path a draft may carry: a valid relative path, never an excluded one (logs, backups, the
/// launcher's own files), so a version never ships what the launcher would refuse or regenerate.
pub fn check_path(path: &str) -> Result<String, StoreError> {
    let path = path.trim().replace('\\', "/");
    if paths::check(&path).is_err() {
        return Err(refused(format!(
            "caminho inválido: {path} (relativo à pasta do jogo, sem .., sem nomes reservados)"
        )));
    }
    if rules::excluded(&path) {
        return Err(refused(format!(
            "{path} nunca é distribuído (arquivo de log, backup, do launcher ou gerado pelo jogo)"
        )));
    }
    Ok(path)
}

/// The published pointer and manifest, checked against the store's own public key file.
pub fn published(store_dir: &Path) -> Result<(VersionPointer, Manifest), StoreError> {
    let public =
        sign::verifying_key(fs::read_to_string(store_dir.join(layout::PUBLIC_KEY))?.trim())?;
    let pointer = read_pointer(store_dir, &public)?
        .ok_or_else(|| refused("o armazém não tem versão publicada"))?;
    let manifest = read_manifest(store_dir, pointer.current, &public)?;
    Ok((pointer, manifest))
}

/// The HFS keystream of a manifest (an object of the store, checked by its SHA-256).
pub fn keystream_of(
    store_dir: &Path,
    manifest: &Manifest,
) -> Result<[u8; codec::KEYSTREAM_SIZE], StoreError> {
    let k = &manifest.keystream;
    let plain = codec::restore(
        &fs::read(store_dir.join(layout::object(&k.sha256)))?,
        Transform::None,
        &[0; codec::KEYSTREAM_SIZE],
        &k.sha256,
        k.size,
    )?;
    Ok(codec::keystream(&plain)?)
}

/// Keeps `data` as `staging/<sha256>` (once per content). Returns the SHA-256 and size.
pub fn stage(staging: &Path, data: &[u8]) -> Result<(String, u64), StoreError> {
    if data.len() as u64 > MAX_STAGED_FILE {
        return Err(refused("o arquivo passa de 256 MB"));
    }
    let sha = sha256_hex(data);
    let path = staging.join(&sha);
    if !fs::metadata(&path).is_ok_and(|m| m.len() == data.len() as u64) {
        write_atomic(&path, data)?;
    }
    Ok((sha, data.len() as u64))
}

fn staged(staging: &Path, sha: &str, size: u64) -> Result<Vec<u8>, StoreError> {
    if sha.len() != 64 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(refused("arquivo do rascunho inválido"));
    }
    let data = fs::read(staging.join(sha))?;
    if data.len() as u64 != size || sha256_hex(&data) != sha {
        return Err(refused(format!(
            "o arquivo {} do rascunho mudou ou sumiu: envie-o de novo",
            &sha[..16]
        )));
    }
    Ok(data)
}

/// Applies `changes` to manifest `manifest` (case-insensitive paths; the published spelling wins). Errors
/// on removing a path the version lacks, on a change that leaves the file as it is, or on no change at all.
pub fn apply_changes(
    manifest: &Manifest,
    changes: &[(Change, Option<FileEntry>)],
) -> Result<Vec<FileEntry>, StoreError> {
    let mut files: BTreeMap<String, FileEntry> = manifest
        .files
        .iter()
        .map(|f| (f.path.to_ascii_lowercase(), f.clone()))
        .collect();
    for (change, entry) in changes {
        let key = change.path().to_ascii_lowercase();
        match (change, entry) {
            (Change::Remove { path }, _) => {
                if files.remove(&key).is_none() {
                    return Err(refused(format!("{path} não está na versão publicada")));
                }
            }
            (Change::Put { path, .. }, Some(entry)) => {
                let spelled = files
                    .get(&key)
                    .map_or_else(|| path.clone(), |f| f.path.clone());
                if files.get(&key).is_some_and(|f| f.sha256 == entry.sha256) {
                    return Err(refused(format!(
                        "{path} já está publicado com este conteúdo"
                    )));
                }
                files.insert(
                    key,
                    FileEntry {
                        path: spelled,
                        ..entry.clone()
                    },
                );
            }
            (Change::Put { path, .. }, None) => {
                return Err(refused(format!("{path} ficou sem objeto no armazém")));
            }
        }
    }
    let mut out: Vec<FileEntry> = files.into_values().collect();
    out.sort_by(|a, b| a.path.cmp(&b.path));
    if out.is_empty() || out.len() > vindictus_client_dist::manifest::MAX_FILES {
        return Err(refused(
            "a versão ficaria sem arquivos ou com arquivos demais",
        ));
    }
    Ok(out)
}

/// Publishes version N+1 = the current version N (`base_version`) plus `changes`: objects for the new
/// content only, deltas from N, the signed manifest and then the signed pointer. Refuses when the store
/// moved past `base_version` (another publish or a withdraw since the draft started).
pub fn publish_changes(
    paths: &StorePaths,
    staging: &Path,
    base_version: u32,
    changes: &[Change],
    title: &str,
    notes: &str,
) -> Result<PublishReport, StoreError> {
    let started = Instant::now();
    let key = load_key(&paths.keys)?;
    let public = key.verifying_key();
    let pointer = current_pointer(&paths.store, &key)?;
    if pointer.current != base_version {
        return Err(refused(format!(
            "a versão publicada agora é a {}, e o rascunho começou na {base_version}: descarte-o e refaça",
            pointer.current
        )));
    }
    if changes.is_empty() {
        return Err(refused("o rascunho não tem mudanças"));
    }
    let previous = read_manifest(&paths.store, pointer.current, &public)?;
    let keystream = keystream_of(&paths.store, &previous)?;
    let mut entries = Vec::with_capacity(changes.len());
    let mut forms: HashMap<String, Vec<u8>> = HashMap::new();
    let mut written = 0usize;
    let mut written_bytes = 0u64;
    for change in changes {
        let path = check_path(change.path())?;
        let entry = match change {
            Change::Remove { .. } => None,
            Change::Put { sha256, size, .. } => {
                let data = staged(staging, sha256, *size)?;
                let transform = rules::transform_of(&path);
                let mut form = data.clone();
                codec::apply(transform, &keystream, 0, &mut form);
                let (object_size, level) =
                    store_object(&paths.store, sha256, data, transform, &keystream)?;
                if level.is_some() {
                    written += 1;
                    written_bytes += object_size;
                }
                forms.insert(sha256.clone(), form);
                Some(FileEntry {
                    path: path.clone(),
                    sha256: sha256.clone(),
                    size: *size,
                    transform,
                    object_size,
                    preserve: rules::preserved(&path),
                })
            }
        };
        let change = match change {
            Change::Put { sha256, size, .. } => Change::Put {
                path,
                sha256: sha256.clone(),
                size: *size,
            },
            Change::Remove { .. } => Change::Remove { path },
        };
        entries.push((change, entry));
    }
    let files = apply_changes(&previous, &entries)?;
    let new_form = |file: &FileEntry| -> Result<Vec<u8>, StoreError> {
        forms
            .get(&file.sha256)
            .cloned()
            .ok_or_else(|| refused("um arquivo alterado não está no rascunho"))
    };
    let plan = plan_deltas(&paths.store, &previous, &files, &new_form)?;
    let version = pointer.current + 1;
    let created_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let manifest = Manifest {
        version,
        title: title.trim().chars().take(200).collect(),
        notes: notes.trim().chars().take(4000).collect(),
        created_unix,
        files,
        deltas: plan.deltas,
        ..previous.clone()
    };
    manifest.check()?;
    let bytes = manifest.to_bytes()?;
    let name = layout::manifest(version);
    if paths.store.join(&name).exists() {
        return Err(refused(format!(
            "{name} já existe sem estar publicado: confira o armazém"
        )));
    }
    write_atomic(&paths.store.join(&name), &bytes)?;
    write_atomic(
        &paths.store.join(layout::signature(&name)),
        &sign::sign(&key, &bytes),
    )?;
    let after = VersionPointer {
        current: version,
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
        objects_written: written,
        objects_written_bytes: written_bytes,
        objects_reused: manifest.files.len().saturating_sub(written),
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

/// Published files whose path contains `search` (case-insensitive), at most `limit`.
pub fn search(manifest: &Manifest, search: &str, limit: usize) -> Vec<FileEntry> {
    let needle = search.trim().to_ascii_lowercase();
    manifest
        .files
        .iter()
        .filter(|f| needle.is_empty() || f.path.to_ascii_lowercase().contains(&needle))
        .take(limit)
        .cloned()
        .collect()
}

// ------------------------------------------------------------------ the server's client tree

/// SHA-256 of the tree's files by path, size and mtime (`build-state/work-hashes.json`).
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct HashCache {
    files: HashMap<String, (u64, u128, String)>,
}

fn mtime(meta: &fs::Metadata) -> u128 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos())
}

impl HashCache {
    pub fn load(path: &Path) -> Self {
        fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<(), StoreError> {
        let bytes = serde_json::to_vec(self).map_err(|e| refused(e.to_string()))?;
        write_atomic(path, &bytes)
    }

    /// The file's SHA-256 and size (hashed only when its size or mtime changed).
    pub fn sha(&mut self, root: &Path, path: &str) -> Result<(String, u64), StoreError> {
        let full = root.join(path);
        let meta = fs::metadata(&full)?;
        let stamp = (meta.len(), mtime(&meta));
        if let Some((size, time, sha)) = self.files.get(path)
            && (*size, *time) == stamp
        {
            return Ok((sha.clone(), *size));
        }
        let sha = sha256_hex(&fs::read(&full)?);
        self.files
            .insert(path.into(), (stamp.0, stamp.1, sha.clone()));
        Ok((sha, stamp.0))
    }

    fn forget(&mut self, path: &str) {
        self.files.remove(path);
    }
}

/// How the tree differs from a version.
#[derive(Debug, Default, Serialize)]
pub struct TreeDiff {
    /// New or changed files: path, SHA-256, size and the published SHA-256 it replaces.
    pub put: Vec<(String, String, u64, Option<String>)>,
    /// Published files the tree no longer has (never the player's files).
    pub removed: Vec<(String, String)>,
    pub unchanged: usize,
}

/// Compares the tree (without the excluded files: backups, lab-backup/, logs) with the manifest.
pub fn diff_tree(
    work: &Path,
    manifest: &Manifest,
    cache: &mut HashCache,
) -> Result<TreeDiff, StoreError> {
    let published: HashMap<String, &FileEntry> = manifest
        .files
        .iter()
        .map(|f| (f.path.to_ascii_lowercase(), f))
        .collect();
    let mut seen = std::collections::HashSet::new();
    let mut diff = TreeDiff::default();
    for path in store::tree_files(work)? {
        let (sha, size) = cache.sha(work, &path)?;
        let key = path.to_ascii_lowercase();
        seen.insert(key.clone());
        match published.get(&key) {
            Some(f) if f.sha256 == sha => diff.unchanged += 1,
            // The player's files ship once; the tree's own copy never replaces them.
            Some(f) if f.preserve => diff.unchanged += 1,
            Some(f) => diff
                .put
                .push((f.path.clone(), sha, size, Some(f.sha256.clone()))),
            None => diff.put.push((path, sha, size, None)),
        }
    }
    for f in &manifest.files {
        if !f.preserve && !seen.contains(&f.path.to_ascii_lowercase()) {
            diff.removed.push((f.path.clone(), f.sha256.clone()));
        }
    }
    Ok(diff)
}

/// What a sync did.
#[derive(Debug, Default, Serialize)]
pub struct SyncReport {
    pub written: usize,
    pub written_bytes: u64,
    pub deleted: usize,
    pub unchanged: usize,
}

/// Makes the tree equal to the published version: files that differ come back from their objects, files
/// the version lacks are deleted (the excluded ones, such as the build's backups, are never touched).
pub fn sync_tree(
    store_dir: &Path,
    manifest: &Manifest,
    work: &Path,
    cache: &mut HashCache,
) -> Result<SyncReport, StoreError> {
    let keystream = keystream_of(store_dir, manifest)?;
    let diff = diff_tree(work, manifest, cache)?;
    let mut report = SyncReport {
        unchanged: diff.unchanged,
        ..SyncReport::default()
    };
    let wanted: HashMap<String, &FileEntry> = manifest
        .files
        .iter()
        .map(|f| (f.path.to_ascii_lowercase(), f))
        .collect();
    for (path, _, _, before) in &diff.put {
        match (before, wanted.get(&path.to_ascii_lowercase())) {
            (Some(_), Some(entry)) => {
                restore_into(store_dir, entry, &keystream, work, path)?;
                cache.forget(path);
                report.written += 1;
                report.written_bytes += entry.size;
            }
            _ => {
                fs::remove_file(work.join(path))?;
                cache.forget(path);
                report.deleted += 1;
            }
        }
    }
    let missing_player_files = manifest
        .files
        .iter()
        .filter(|f| f.preserve && !work.join(&f.path).exists());
    let restore: Vec<&FileEntry> = diff
        .removed
        .iter()
        .filter_map(|(path, _)| wanted.get(&path.to_ascii_lowercase()).copied())
        .chain(missing_player_files)
        .collect();
    for entry in restore {
        restore_into(store_dir, entry, &keystream, work, &entry.path)?;
        report.written += 1;
        report.written_bytes += entry.size;
    }
    Ok(report)
}

fn restore_into(
    store_dir: &Path,
    entry: &FileEntry,
    keystream: &[u8; codec::KEYSTREAM_SIZE],
    work: &Path,
    path: &str,
) -> Result<(), StoreError> {
    paths::check(path)?;
    let data = codec::restore(
        &fs::read(store_dir.join(layout::object(&entry.sha256)))?,
        entry.transform,
        keystream,
        &entry.sha256,
        entry.size,
    )?;
    let target = work.join(path);
    // A read-only hard link (an old build's) is replaced, never written through.
    if target.exists() {
        fs::remove_file(&target)?;
    }
    write_atomic(&target, &data)
}

/// Copies a tree file into the staging folder (the draft keeps it even if the tree changes later).
pub fn stage_from_tree(
    staging: &Path,
    work: &Path,
    path: &str,
    sha: &str,
) -> Result<(), StoreError> {
    let target = staging.join(sha);
    if target.exists() {
        return Ok(());
    }
    let data = fs::read(work.join(path))?;
    if sha256_hex(&data) != sha {
        return Err(refused(format!("{path} mudou enquanto era copiado")));
    }
    write_atomic(&target, &data)
}

/// Folders of the server's client data (`${CLIENT_SERVER_DIR}`).
#[derive(Debug, Clone)]
pub struct ServerDirs {
    pub root: PathBuf,
}

impl ServerDirs {
    /// The server's client tree (the published version plus the build's backups).
    pub fn work(&self) -> PathBuf {
        self.root.join("work")
    }
    /// Files waiting in a draft, by SHA-256.
    pub fn staging(&self) -> PathBuf {
        self.root.join("staging")
    }
    /// The builder's own state (hash cache, keystream copy for the recipe).
    pub fn state(&self) -> PathBuf {
        self.root.join("build-state")
    }
    /// The imported read-only evidence (the 1.69 original and the recipe's inputs).
    pub fn workspace(&self) -> PathBuf {
        self.root.join("inputs/workspace")
    }
}

#[cfg(test)]
mod tests;
