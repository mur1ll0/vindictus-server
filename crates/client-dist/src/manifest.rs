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
        paths::check_all(self.files.iter().map(|f| f.path.as_str()))
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
        };
        assert!(VersionPointer::parse(&p.to_bytes().unwrap_or_default()).is_ok());
        let p = VersionPointer { minimum: 4, ..p };
        assert!(VersionPointer::parse(&p.to_bytes().unwrap_or_default()).is_err());
    }

    #[test]
    fn transforms_have_kebab_names() {
        assert_eq!(
            serde_json::to_string(&Transform::HfsXor).unwrap_or_default(),
            "\"hfs-xor\""
        );
    }
}
