//! The signed documents: a manifest per version (`manifests/<n>.json`) and the version pointer
//! (`versao.json`). Each is signed as the exact bytes stored (`<file>.sig`, 64 raw bytes, Ed25519).

use crate::{DistError, paths};
use serde::{Deserialize, Serialize};

/// Format of both documents.
pub const FORMAT: u32 = 1;
/// Bounds of a manifest: files and bytes of a file (the largest client file is ~115 MB).
pub const MAX_FILES: usize = 20_000;
/// Largest file a manifest may list.
pub const MAX_FILE_SIZE: u64 = 1 << 30;

/// Reversible change made to a file before compression; the object holds the changed form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Transform {
    /// Stored as on disk.
    None,
    /// The pack keystream XOR undone over the whole file, by absolute position (codec::hfs_xor).
    HfsXor,
}

/// One file of a version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    /// Game path (paths::check).
    pub path: String,
    /// SHA-256 of the file as the player's disk holds it; also the object's name.
    pub sha256: String,
    /// Size on disk.
    pub size: u64,
    /// Change applied before compression.
    pub transform: Transform,
    /// Size of `objects/<aa>/<sha256>.zst`.
    pub object_size: u64,
    /// The player's file: installed when missing, never checked nor overwritten.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub preserve: bool,
}

/// An object outside the game tree (the pack keystream).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectRef {
    /// SHA-256 of the plain bytes.
    pub sha256: String,
    /// Plain size.
    pub size: u64,
    /// Compressed object size.
    pub object_size: u64,
}

/// The complete state of version `version`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// [`FORMAT`].
    pub format: u32,
    /// Version number, from 1.
    pub version: u32,
    /// Short title shown by the launcher.
    pub title: String,
    /// Release notes.
    #[serde(default)]
    pub notes: String,
    /// Seconds since the Unix epoch.
    pub created_unix: u64,
    /// The 4096-byte keystream the `hfs-xor` transform needs.
    pub keystream: ObjectRef,
    /// Files, sorted by path.
    pub files: Vec<FileEntry>,
    /// Deltas from version `version - 1` (`deltas/<from>-<to>.zst`), for files that changed at the same path.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deltas: Vec<Delta>,
}

/// A binary delta from a file of the previous version to this one's (codec::delta: zstd with the old
/// file's transformed form as prefix). The launcher uses it only when its file has `from_sha256`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Delta {
    /// Game path (a file of this manifest).
    pub path: String,
    /// SHA-256 of the previous version's file.
    pub from_sha256: String,
    /// Size of the previous version's file.
    pub from_size: u64,
    /// SHA-256 of this version's file (the entry of `path`).
    pub to_sha256: String,
    /// Size of `deltas/<from>-<to>.zst`.
    pub delta_size: u64,
}

/// `versao.json`: the current version, the oldest one the server still accepts, and the manifest's hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionPointer {
    /// [`FORMAT`].
    pub format: u32,
    /// The version the launcher installs or updates to.
    pub current: u32,
    /// Older launchers' clients below it must update before playing.
    pub minimum: u32,
    /// SHA-256 of `manifests/<current>.json`.
    pub manifest_sha256: String,
    /// The launcher published for self-update (`client-dist launcher`), if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launcher: Option<LauncherRef>,
}

/// The launcher executable a launcher updates itself to: an object of the store (no transform).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LauncherRef {
    /// Its version (`1.2.3`, compared part by part).
    pub version: String,
    /// SHA-256 of the executable.
    pub sha256: String,
    /// Size of the executable.
    pub size: u64,
    /// Size of its object.
    pub object_size: u64,
}

/// Largest launcher executable accepted.
pub const MAX_LAUNCHER_SIZE: u64 = 64 << 20;

/// `1.2.3` as numbers (one to four parts, each up to 9 digits).
pub fn version_parts(text: &str) -> Option<Vec<u32>> {
    let parts: Vec<&str> = text.split('.').collect();
    if parts.is_empty() || parts.len() > 4 {
        return None;
    }
    parts
        .iter()
        .map(|p| {
            (!p.is_empty() && p.len() <= 9 && p.bytes().all(|b| b.is_ascii_digit()))
                .then(|| p.parse().ok())
                .flatten()
        })
        .collect()
}

/// True when `candidate` is a later version than `current` (missing parts count as 0).
pub fn is_newer(candidate: &str, current: &str) -> bool {
    match (version_parts(candidate), version_parts(current)) {
        (Some(mut a), Some(mut b)) => {
            let len = a.len().max(b.len());
            a.resize(len, 0);
            b.resize(len, 0);
            a > b
        }
        _ => false,
    }
}

/// `news.json`: the launcher's news, newest first, signed like the pointer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewsFeed {
    /// [`FORMAT`].
    pub format: u32,
    /// At most [`MAX_NEWS`] items.
    pub items: Vec<NewsItem>,
}

/// One news item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewsItem {
    /// Id in the panel's registry.
    pub id: i64,
    /// Up to [`MAX_NEWS_TITLE`] characters.
    pub title: String,
    /// Up to [`MAX_NEWS_BODY`] characters.
    pub body: String,
    /// Seconds since the Unix epoch.
    pub published_unix: u64,
}

/// News items kept in the feed.
pub const MAX_NEWS: usize = 20;
/// Longest title.
pub const MAX_NEWS_TITLE: usize = 120;
/// Longest body.
pub const MAX_NEWS_BODY: usize = 2000;

impl NewsItem {
    /// Bounds of an item: non-empty title and body within the limits, no control character but line breaks.
    pub fn valid(&self) -> bool {
        let text_ok = |t: &str, max: usize, lines: bool| {
            !t.trim().is_empty()
                && t.chars().count() <= max
                && t.chars()
                    .all(|c| !c.is_control() || (lines && (c == '\n' || c == '\r')))
        };
        text_ok(&self.title, MAX_NEWS_TITLE, false) && text_ok(&self.body, MAX_NEWS_BODY, true)
    }
}

impl NewsFeed {
    /// Parses and checks the feed.
    pub fn parse(bytes: &[u8]) -> Result<Self, DistError> {
        let feed: Self =
            serde_json::from_slice(bytes).map_err(|_| DistError::Format("news.json"))?;
        if feed.format != FORMAT
            || feed.items.len() > MAX_NEWS
            || !feed.items.iter().all(NewsItem::valid)
        {
            return Err(DistError::Format("news.json"));
        }
        Ok(feed)
    }

    /// Canonical bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>, DistError> {
        serde_json::to_vec_pretty(self).map_err(|_| DistError::Format("news.json"))
    }
}

fn is_sha(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl Manifest {
    /// Parses and checks the bounds: format, version, hashes, sizes, sorted unique valid paths.
    pub fn parse(bytes: &[u8]) -> Result<Self, DistError> {
        let manifest: Self =
            serde_json::from_slice(bytes).map_err(|_| DistError::Format("manifest"))?;
        manifest.check()?;
        Ok(manifest)
    }

    /// The checks of [`Manifest::parse`].
    pub fn check(&self) -> Result<(), DistError> {
        let bad = Err(DistError::Format("manifest"));
        if self.format != FORMAT
            || self.version == 0
            || self.files.is_empty()
            || self.files.len() > MAX_FILES
            || self.title.len() > 200
            || self.notes.len() > 20_000
            || !is_sha(&self.keystream.sha256)
            || self.keystream.size != crate::codec::KEYSTREAM_SIZE as u64
        {
            return bad;
        }
        if self.files.windows(2).any(|w| w[0].path >= w[1].path) {
            return bad;
        }
        if self
            .files
            .iter()
            .any(|f| !is_sha(&f.sha256) || f.size > MAX_FILE_SIZE || f.object_size == 0)
        {
            return bad;
        }
        paths::check_all(self.files.iter().map(|f| f.path.as_str()))?;
        // Each delta ends at its file of this version, from another content no larger than a delta source.
        let files: std::collections::HashMap<&str, &FileEntry> =
            self.files.iter().map(|f| (f.path.as_str(), f)).collect();
        for delta in &self.deltas {
            let to = files.get(delta.path.as_str());
            if to.is_none_or(|f| {
                f.sha256 != delta.to_sha256 || f.size > crate::codec::DELTA_MAX_FILE
            }) || !is_sha(&delta.from_sha256)
                || delta.from_sha256 == delta.to_sha256
                || delta.from_size > crate::codec::DELTA_MAX_FILE
                || delta.delta_size == 0
            {
                return bad;
            }
        }
        Ok(())
    }

    /// Canonical bytes (what is stored and signed).
    pub fn to_bytes(&self) -> Result<Vec<u8>, DistError> {
        serde_json::to_vec_pretty(self).map_err(|_| DistError::Format("manifest"))
    }
}

impl VersionPointer {
    /// Parses and checks the pointer.
    pub fn parse(bytes: &[u8]) -> Result<Self, DistError> {
        let pointer: Self =
            serde_json::from_slice(bytes).map_err(|_| DistError::Format("versao.json"))?;
        if pointer.format != FORMAT
            || pointer.current == 0
            || pointer.minimum == 0
            || pointer.minimum > pointer.current
            || !is_sha(&pointer.manifest_sha256)
            || pointer.launcher.as_ref().is_some_and(|l| {
                version_parts(&l.version).is_none()
                    || !is_sha(&l.sha256)
                    || l.size == 0
                    || l.size > MAX_LAUNCHER_SIZE
                    || l.object_size == 0
            })
        {
            return Err(DistError::Format("versao.json"));
        }
        Ok(pointer)
    }

    /// Canonical bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>, DistError> {
        serde_json::to_vec_pretty(self).map_err(|_| DistError::Format("versao.json"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> Manifest {
        let entry = |path: &str| FileEntry {
            path: path.into(),
            sha256: "a".repeat(64),
            size: 3,
            transform: Transform::None,
            object_size: 12,
            preserve: false,
        };
        Manifest {
            format: FORMAT,
            version: 1,
            title: "v1".into(),
            notes: String::new(),
            created_unix: 0,
            keystream: ObjectRef {
                sha256: "b".repeat(64),
                size: 4096,
                object_size: 10,
            },
            files: vec![entry("Vindictus.exe"), entry("bin/client.dll")],
            deltas: Vec::new(),
        }
    }

    #[test]
    fn a_manifest_round_trips() {
        let m = manifest();
        assert_eq!(
            Manifest::parse(&m.to_bytes().unwrap_or_default()).ok(),
            Some(m)
        );
    }

    #[test]
    fn deltas_must_end_at_their_file() {
        let mut m = manifest();
        m.deltas.push(Delta {
            path: "bin/client.dll".into(),
            from_sha256: "d".repeat(64),
            from_size: 3,
            to_sha256: "a".repeat(64),
            delta_size: 5,
        });
        assert!(m.check().is_ok());
        assert_eq!(
            Manifest::parse(&m.to_bytes().unwrap_or_default()).ok(),
            Some(m.clone())
        );
        m.deltas[0].to_sha256 = "e".repeat(64);
        assert!(m.check().is_err());
        m.deltas[0].to_sha256 = "a".repeat(64);
        m.deltas[0].path = "bin/other.dll".into();
        assert!(m.check().is_err());
    }

    #[test]
    fn unsorted_or_escaping_paths_are_refused() {
        let mut m = manifest();
        m.files.reverse();
        assert!(m.check().is_err());
        let mut m = manifest();
        m.files[1].path = "../bin/client.dll".into();
        assert!(m.check().is_err());
    }

    #[test]
    fn the_pointer_needs_minimum_up_to_current() {
        let p = VersionPointer {
            format: FORMAT,
            current: 3,
            minimum: 1,
            manifest_sha256: "c".repeat(64),
            launcher: None,
        };
        assert!(VersionPointer::parse(&p.to_bytes().unwrap_or_default()).is_ok());
        let p = VersionPointer { minimum: 4, ..p };
        assert!(VersionPointer::parse(&p.to_bytes().unwrap_or_default()).is_err());
    }

    #[test]
    fn launcher_versions_compare_part_by_part() {
        assert!(is_newer("0.2.0", "0.1.9"));
        assert!(is_newer("1.10", "1.9.5"));
        assert!(!is_newer("1.0", "1.0.0"));
        assert!(!is_newer("x", "0.1.0"));
        assert_eq!(version_parts("1.2.3.4.5"), None);
    }

    #[test]
    fn the_pointer_carries_a_valid_launcher() {
        let mut p = VersionPointer {
            format: FORMAT,
            current: 1,
            minimum: 1,
            manifest_sha256: "c".repeat(64),
            launcher: Some(LauncherRef {
                version: "0.2.0".into(),
                sha256: "d".repeat(64),
                size: 10,
                object_size: 9,
            }),
        };
        assert!(VersionPointer::parse(&p.to_bytes().unwrap_or_default()).is_ok());
        p.launcher = Some(LauncherRef {
            version: "beta".into(),
            sha256: "d".repeat(64),
            size: 10,
            object_size: 9,
        });
        assert!(VersionPointer::parse(&p.to_bytes().unwrap_or_default()).is_err());
    }

    #[test]
    fn news_items_are_bounded() {
        let item = NewsItem {
            id: 1,
            title: "Manutenção".into(),
            body: "Volta às 20h.\nObrigado.".into(),
            published_unix: 0,
        };
        let feed = NewsFeed {
            format: FORMAT,
            items: vec![item.clone()],
        };
        assert!(NewsFeed::parse(&feed.to_bytes().unwrap_or_default()).is_ok());
        assert!(
            !NewsItem {
                title: "a\tb".into(),
                ..item.clone()
            }
            .valid()
        );
        assert!(
            !NewsItem {
                body: "x".repeat(MAX_NEWS_BODY + 1),
                ..item
            }
            .valid()
        );
    }

    #[test]
    fn transforms_have_kebab_names() {
        assert_eq!(
            serde_json::to_string(&Transform::HfsXor).unwrap_or_default(),
            "\"hfs-xor\""
        );
    }
}
