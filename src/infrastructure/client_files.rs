//! Read-only access to the 1.69 EN client's files through its HFS index (the same format as
//! scripts/client/hfs.py; docs/future/game-launcher.md, sections 2.2 and 2.4). The admin panel uses
//! it to hand out model kits for the Blender add-on.
//!
//! Index `hfs/E608DF10....hfs`: 14 plain bytes, then a zlib stream XORed with `78 BD 02 47` by
//! position & 3. Packs: stored zips whose names and data are XORed with a 4096-byte keystream by
//! absolute file position; `x.comp` files are `b"comp"`, u32 size, zlib. A file is looked up by the
//! hash of its name and must sit in a pack of its folder (`pack_name`).
use sha1::{Digest, Sha1};
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use thiserror::Error;

pub const INDEX: &str = "hfs/E608DF10F284E19AB75FD7D0D8AE9CCF87831041.hfs";
const INDEX_KEY: [u8; 4] = [0x78, 0xBD, 0x02, 0x47];
const INDEX_HEAD: usize = 14;
const KEYSTREAM_SIZE: usize = 4096;
/// Largest plain index (the 1.69 EN one is 2.2 MB).
const MAX_INDEX: u64 = 32 * 1024 * 1024;
/// Largest file handed out (the biggest model and texture files are a few MB).
const MAX_FILE: u32 = 64 * 1024 * 1024;
const MAX_PARTS: u32 = 64;

#[derive(Debug, Error)]
pub enum ClientFilesError {
    #[error("client file I/O failed")]
    Io(#[from] std::io::Error),
    #[error("the HFS index or keystream is malformed")]
    Malformed,
    #[error("a client file exceeds its bound")]
    Bound,
}

/// The client copy and its index, loaded once.
pub struct ClientFiles {
    root: PathBuf,
    keystream: Vec<u8>,
    packs: Vec<String>,
    entries: HashMap<[u8; 6], Vec<(u16, u32, u32)>>,
}

pub fn name_key(name: &str) -> [u8; 6] {
    let lower = name.to_ascii_lowercase();
    let mut h: u32 = 0xAAAA_AAAA;
    for b in lower.bytes() {
        h = h.wrapping_mul(33).wrapping_add(u32::from(b));
    }
    let bytes = lower.as_bytes();
    let first = bytes.first().copied().unwrap_or(0);
    let last = bytes.last().copied().unwrap_or(0);
    let mut key = [0u8; 6];
    key[..4].copy_from_slice(&h.to_le_bytes());
    key[4] = first;
    key[5] = last;
    key
}

/// FileSystem_Stdio 0x1000b050: separators become '@', `.<part>` is appended, case alternates
/// (upper at even positions), then SHA-1 printed as five %08X words.
pub fn pack_name(folder: &str, part: u32) -> String {
    let joined = folder
        .split(['/', '\\'])
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("@");
    let text: String = format!("{joined}.{part}")
        .chars()
        .enumerate()
        .map(|(i, c)| {
            if i % 2 == 0 {
                c.to_ascii_uppercase()
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect();
    Sha1::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect()
}

fn xor_at(keystream: &[u8], pos: u32, data: &mut [u8]) {
    for (i, b) in data.iter_mut().enumerate() {
        *b ^= keystream[(pos as usize + i) & (KEYSTREAM_SIZE - 1)];
    }
}

fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

fn inflate(data: &[u8], limit: u64) -> Result<Vec<u8>, ClientFilesError> {
    let mut out = Vec::new();
    flate2::read::ZlibDecoder::new(data)
        .take(limit + 1)
        .read_to_end(&mut out)
        .map_err(|_| ClientFilesError::Malformed)?;
    if out.len() as u64 > limit {
        return Err(ClientFilesError::Bound);
    }
    Ok(out)
}

impl ClientFiles {
    pub fn open(root: &Path, keystream: &Path) -> Result<Self, ClientFilesError> {
        let keystream = std::fs::read(keystream)?;
        if keystream.len() != KEYSTREAM_SIZE {
            return Err(ClientFilesError::Malformed);
        }
        let raw = std::fs::read(root.join(INDEX))?;
        let mut stream = raw.get(INDEX_HEAD..).ok_or(ClientFilesError::Malformed)?.to_vec();
        for (i, b) in stream.iter_mut().enumerate() {
            *b ^= INDEX_KEY[i & 3];
        }
        let plain = inflate(&stream, MAX_INDEX)?;
        if plain.get(..4) != Some(b"HFI\x01") {
            return Err(ClientFilesError::Malformed);
        }
        let count = usize::from(u16_at(&plain, 4).ok_or(ClientFilesError::Malformed)?);
        let files = u32_at(&plain, 0x10).ok_or(ClientFilesError::Malformed)? as usize;
        let table = 0x14 + 0x14 * count;
        if plain.len() != table + 16 * files {
            return Err(ClientFilesError::Malformed);
        }
        let packs = (0..count)
            .map(|i| {
                (0..5)
                    .map(|w| u32_at(&plain, 0x14 + 0x14 * i + 4 * w).map(|v| format!("{v:08X}")))
                    .collect::<Option<String>>()
            })
            .collect::<Option<Vec<_>>>()
            .ok_or(ClientFilesError::Malformed)?;
        let mut entries: HashMap<[u8; 6], Vec<(u16, u32, u32)>> = HashMap::new();
        for e in plain[table..].as_chunks::<16>().0 {
            let key: [u8; 6] = e[..6].try_into().map_err(|_| ClientFilesError::Malformed)?;
            let pack = u16_at(e, 6).ok_or(ClientFilesError::Malformed)?;
            let at = u32_at(e, 8).ok_or(ClientFilesError::Malformed)?;
            let size = u32_at(e, 12).ok_or(ClientFilesError::Malformed)?;
            entries.entry(key).or_default().push((pack, at, size));
        }
        Ok(Self {
            root: root.to_path_buf(),
            keystream,
            packs,
            entries,
        })
    }

    /// A plain file by game path (`models/player/fiona/x.mdl`), from its folder's packs, or loose.
    pub fn read(&self, path: &str) -> Result<Option<Vec<u8>>, ClientFilesError> {
        let path = path.replace('\\', "/").to_ascii_lowercase();
        let path = path.trim_matches('/');
        if path.is_empty() || path.split('/').any(|p| p.is_empty() || p == "..") {
            return Ok(None);
        }
        let (folder, name) = path.rsplit_once('/').unwrap_or(("", path));
        let candidates = self.entries.get(&name_key(name)).map(Vec::as_slice).unwrap_or(&[]);
        if !candidates.is_empty() {
            let folder_packs: Vec<String> = (0..MAX_PARTS).map(|p| pack_name(folder, p)).collect();
            for &(pack, at, size) in candidates {
                let Some(pack_id) = self.packs.get(usize::from(pack)) else {
                    continue;
                };
                if !folder_packs.contains(pack_id) {
                    continue;
                }
                if size > MAX_FILE {
                    return Err(ClientFilesError::Bound);
                }
                let mut file = std::fs::File::open(self.root.join("hfs").join(format!("{pack_id}.hfs")))?;
                file.seek(SeekFrom::Start(u64::from(at)))?;
                let mut data = vec![0u8; size as usize];
                file.read_exact(&mut data)?;
                xor_at(&self.keystream, at, &mut data);
                if data.get(..4) == Some(b"comp") {
                    let expected = u32_at(&data, 4).ok_or(ClientFilesError::Malformed)?;
                    if expected > MAX_FILE {
                        return Err(ClientFilesError::Bound);
                    }
                    let plain = inflate(data.get(8..).ok_or(ClientFilesError::Malformed)?, u64::from(expected))?;
                    if plain.len() != expected as usize {
                        return Err(ClientFilesError::Malformed);
                    }
                    return Ok(Some(plain));
                }
                return Ok(Some(data));
            }
        }
        let loose = self.root.join(path);
        if loose.is_file() {
            let meta = std::fs::metadata(&loose)?;
            if meta.len() > u64::from(MAX_FILE) {
                return Err(ClientFilesError::Bound);
            }
            return Ok(Some(std::fs::read(loose)?));
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_names_match_the_client() {
        assert_eq!(pack_name("models/armor/kingdom", 0), "0000AA13EFACC8399BA8F63B23986D12A7697817");
        assert_eq!(pack_name("scripts", 0), "763FD8F12D3D013194261D98B4AEF0FB0F6FBA1B");
    }

    #[test]
    fn name_key_hashes_the_lower_case_file_name() {
        let key = name_key("Player_Costume.txt");
        assert_eq!(&key[4..], b"pt");
        assert_eq!(key, name_key("player_costume.txt"));
    }
}
