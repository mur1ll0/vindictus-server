//! Model kits uploaded through the item editor (docs/future/item-editor.md, cut 2): the zip the
//! Blender add-on exports (kit.json plus models/ and materials/ game files), checked and kept in
//! the private assets folder (.cache/private-assets/<assets>, never versioned), from where
//! scripts/client/apply_lab_content_additions.py packs them into the client copy.
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    path::{Path, PathBuf},
};

/// Largest kit upload (the panel's body limit for this route).
pub const MAX_KIT_BYTES: usize = 64 * 1024 * 1024;
const MAX_ENTRIES: usize = 512;
const MAX_FILE: u64 = 64 * 1024 * 1024;
const MAX_TOTAL: u64 = 256 * 1024 * 1024;
const MAX_PATH: usize = 200;
const EXTENSIONS: [&str; 6] = ["mdl", "vvd", "vtx", "phy", "vtf", "vmt"];
const MANIFEST: &str = "kit.json";

#[derive(Debug, thiserror::Error)]
pub enum KitError {
    #[error("not a zip, or a malformed one")]
    Zip,
    #[error("kit larger than its bounds")]
    TooLarge,
    #[error("kit path outside models/ and materials/ game files")]
    Path,
    #[error("kit.json missing or its model is not in the kit")]
    Manifest,
    #[error("unknown kit")]
    Missing,
    #[error("kit storage: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, utoipa::ToSchema)]
pub struct StoredKit {
    /// Asset folder id (`kit_<16 hex of the zip's SHA-256>`).
    pub assets: String,
    /// Game path of the studio model.
    pub model: String,
    /// Game paths of every file.
    pub files: Vec<String>,
}

/// A game path of a kit file: lower case, under models/ or materials/, known extension.
pub fn game_path(name: &str) -> Result<String, KitError> {
    let path = name.to_ascii_lowercase();
    let valid = !path.is_empty()
        && path.len() <= MAX_PATH
        && (path.starts_with("models/") || path.starts_with("materials/"))
        && path
            .split('/')
            .all(|s| !s.is_empty() && s != "." && s != "..")
        && path
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_./-".contains(&b))
        && path
            .rsplit_once('.')
            .is_some_and(|(_, ext)| EXTENSIONS.contains(&ext));
    valid.then_some(path).ok_or(KitError::Path)
}

fn u16_at(b: &[u8], at: usize) -> Result<usize, KitError> {
    b.get(at..at + 2)
        .map(|s| usize::from(u16::from_le_bytes([s[0], s[1]])))
        .ok_or(KitError::Zip)
}

fn u32_at(b: &[u8], at: usize) -> Result<u32, KitError> {
    b.get(at..at + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or(KitError::Zip)
}

fn usize_at(b: &[u8], at: usize) -> Result<usize, KitError> {
    usize::try_from(u32_at(b, at)?).map_err(|_| KitError::Zip)
}

/// A game path and its content.
pub type KitFile = (String, Vec<u8>);

/// Files of a zip (stored or deflated), bounded, with their CRC checked.
pub fn unzip(bytes: &[u8]) -> Result<Vec<KitFile>, KitError> {
    if bytes.len() > MAX_KIT_BYTES {
        return Err(KitError::TooLarge);
    }
    // End of central directory: last 22 bytes plus a comment of up to 64 KiB.
    let floor = bytes.len().saturating_sub(22 + 65_535);
    let end = (floor..=bytes.len().saturating_sub(22))
        .rev()
        .find(|&i| u32_at(bytes, i).ok() == Some(0x0605_4b50))
        .ok_or(KitError::Zip)?;
    let count = u16_at(bytes, end + 10)?;
    if count > MAX_ENTRIES {
        return Err(KitError::TooLarge);
    }
    let mut at = usize_at(bytes, end + 16)?;
    let mut files = Vec::with_capacity(count);
    let mut total = 0u64;
    for _ in 0..count {
        if u32_at(bytes, at)? != 0x0201_4b50 {
            return Err(KitError::Zip);
        }
        let flags = u16_at(bytes, at + 8)?;
        let method = u16_at(bytes, at + 10)?;
        let crc = u32_at(bytes, at + 16)?;
        let packed = usize_at(bytes, at + 20)?;
        let size = u64::from(u32_at(bytes, at + 24)?);
        let name_len = u16_at(bytes, at + 28)?;
        let extra = u16_at(bytes, at + 30)? + u16_at(bytes, at + 32)?;
        let local = usize_at(bytes, at + 42)?;
        let name = bytes
            .get(at + 46..at + 46 + name_len)
            .ok_or(KitError::Zip)?;
        let name = std::str::from_utf8(name)
            .map_err(|_| KitError::Path)?
            .to_owned();
        at += 46 + name_len + extra;
        if name.ends_with('/') {
            continue;
        }
        // Encrypted entries are refused; sizes must fit before anything is inflated.
        total += size;
        if flags & 1 != 0 || size > MAX_FILE || total > MAX_TOTAL {
            return Err(KitError::TooLarge);
        }
        if u32_at(bytes, local)? != 0x0403_4b50 {
            return Err(KitError::Zip);
        }
        let start = local + 30 + u16_at(bytes, local + 26)? + u16_at(bytes, local + 28)?;
        let data = bytes.get(start..start + packed).ok_or(KitError::Zip)?;
        let content = match method {
            0 => data.to_vec(),
            8 => {
                let mut out = Vec::new();
                flate2::read::DeflateDecoder::new(data)
                    .take(size + 1)
                    .read_to_end(&mut out)
                    .map_err(|_| KitError::Zip)?;
                out
            }
            _ => return Err(KitError::Zip),
        };
        if content.len() as u64 != size || crc32fast::hash(&content) != crc {
            return Err(KitError::Zip);
        }
        files.push((name, content));
    }
    Ok(files)
}

/// The kit's game files and its studio model, checked.
pub fn read_kit(bytes: &[u8]) -> Result<(String, Vec<KitFile>), KitError> {
    let mut manifest = None;
    let mut files: Vec<KitFile> = Vec::new();
    for (name, content) in unzip(bytes)? {
        if name.eq_ignore_ascii_case(MANIFEST) {
            manifest = Some(content);
            continue;
        }
        let path = game_path(&name)?;
        if files.iter().any(|(p, _)| *p == path) {
            return Err(KitError::Path);
        }
        files.push((path, content));
    }
    let manifest: serde_json::Value = serde_json::from_slice(&manifest.ok_or(KitError::Manifest)?)
        .map_err(|_| KitError::Manifest)?;
    let model = manifest
        .get("model")
        .and_then(serde_json::Value::as_str)
        .map(|m| m.replace('\\', "/"))
        .ok_or(KitError::Manifest)
        .and_then(|m| game_path(m.trim_start_matches('/')))?;
    // The player_costume.txt entry takes the model path as content_additions.MODEL checks it.
    let stem_ok = model.strip_suffix(".mdl").is_some_and(|stem| {
        stem.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'/')
    });
    if !stem_ok || !files.iter().any(|(p, _)| *p == model) {
        return Err(KitError::Manifest);
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    Ok((model, files))
}

/// The asset id of a kit: its content decides it, so the same zip lands in the same folder.
pub fn assets_id(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let hex: String = digest.iter().take(8).map(|b| format!("{b:02x}")).collect();
    format!("kit_{hex}")
}

pub fn valid_assets(assets: &str) -> bool {
    (1..=64).contains(&assets.len())
        && assets
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// Keep a checked kit in `root/<assets>` (game files and kit.json); an existing folder of the
/// same content is kept as it is.
pub fn store(root: &Path, bytes: &[u8]) -> Result<StoredKit, KitError> {
    let (model, files) = read_kit(bytes)?;
    let assets = assets_id(bytes);
    let stored = StoredKit {
        files: files.iter().map(|(p, _)| p.clone()).collect(),
        assets: assets.clone(),
        model: model.clone(),
    };
    let target = root.join(&assets);
    if target.is_dir() {
        return Ok(stored);
    }
    let partial = root.join(format!(
        ".partial-{assets}-{}",
        uuid::Uuid::new_v4().simple()
    ));
    let written = (|| -> Result<(), KitError> {
        for (path, content) in &files {
            let file = partial.join(path);
            if let Some(parent) = file.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(file, content)?;
        }
        std::fs::write(
            partial.join(MANIFEST),
            serde_json::json!({ "model": model }).to_string(),
        )?;
        std::fs::rename(&partial, &target)?;
        Ok(())
    })();
    if written.is_err() {
        let _ = std::fs::remove_dir_all(&partial);
        // Another upload of the same zip may have finished first.
        if target.is_dir() {
            return Ok(stored);
        }
    }
    written.map(|()| stored)
}

/// A stored kit: its model and game files.
pub fn stored(root: &Path, assets: &str) -> Result<StoredKit, KitError> {
    if !valid_assets(assets) {
        return Err(KitError::Missing);
    }
    let folder = root.join(assets);
    let manifest = std::fs::read(folder.join(MANIFEST)).map_err(|_| KitError::Missing)?;
    let model = serde_json::from_slice::<serde_json::Value>(&manifest)
        .ok()
        .and_then(|v| v.get("model")?.as_str().map(str::to_owned))
        .ok_or(KitError::Manifest)?;
    let mut files = Vec::new();
    let mut pending: Vec<PathBuf> = vec![folder.clone()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                pending.push(path);
            } else if dir != folder {
                let relative = path
                    .strip_prefix(&folder)
                    .map_err(|_| KitError::Path)?
                    .to_string_lossy()
                    .replace('\\', "/");
                files.push(game_path(&relative)?);
                if files.len() > MAX_ENTRIES {
                    return Err(KitError::TooLarge);
                }
            }
        }
    }
    files.sort();
    Ok(StoredKit {
        assets: assets.to_owned(),
        model,
        files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stored (method 0) zip of the files.
    fn zip(files: &[(&str, &[u8])]) -> Vec<u8> {
        let owned: Vec<(String, Vec<u8>)> = files
            .iter()
            .map(|(n, c)| ((*n).to_owned(), c.to_vec()))
            .collect();
        super::super::model_kit::zip(&owned)
    }

    #[test]
    fn a_kit_keeps_its_game_files_and_model() {
        let bytes = zip(&[
            ("kit.json", br#"{"model": "models/player/fiona/new.mdl"}"#),
            ("models/player/fiona/new.mdl", b"IDST"),
            ("materials/models/player/fiona/new/a.vtf", b"VTF"),
        ]);
        let (model, files) = read_kit(&bytes).unwrap();
        assert_eq!(model, "models/player/fiona/new.mdl");
        assert_eq!(files.len(), 2);
        let root = std::env::temp_dir().join(format!("kit-test-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&root).unwrap();
        let kept = store(&root, &bytes).unwrap();
        assert_eq!(store(&root, &bytes).unwrap(), kept);
        assert_eq!(stored(&root, &kept.assets).unwrap(), kept);
        assert!(kept.assets.starts_with("kit_") && kept.assets.len() == 20);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn unsafe_or_foreign_kits_are_refused() {
        let model = ("models/a.mdl", &b"IDST"[..]);
        let manifest = ("kit.json", &br#"{"model":"models/a.mdl"}"#[..]);
        for files in [
            vec![manifest, model, ("models/../x.mdl", &b"x"[..])],
            vec![manifest, model, ("scripts/player_costume.txt", &b"x"[..])],
            vec![manifest, model, ("models/a.exe", &b"x"[..])],
            vec![manifest, model, ("/models/b.mdl", &b"x"[..])],
        ] {
            assert!(matches!(read_kit(&zip(&files)), Err(KitError::Path)));
        }
        assert!(matches!(read_kit(&zip(&[model])), Err(KitError::Manifest)));
        assert!(matches!(
            read_kit(&zip(&[("kit.json", br#"{"model":"models/b.mdl"}"#), model])),
            Err(KitError::Manifest)
        ));
        assert!(matches!(
            read_kit(b"not a zip at all, padding padding"),
            Err(KitError::Zip)
        ));
        assert!(!valid_assets("../x"));
    }
}
