//! Objects (docs/future/game-launcher.md 4.3 items 2-3): the file, changed by its transform, compressed
//! with zstd 3, or with zstd 19 when level 3 left it below 97 % of the original (the `.comp` packs of the
//! game stay at 3). Frame checksum and content size on, the level's default window, no long mode: the
//! launcher decompresses any level with at most ~9 MB of window on a 32-bit Windows 7.

use crate::manifest::Transform;
use crate::{DistError, sha256_hex};
use std::io::{Read, Write};

/// Size of the pack keystream (scripts/client/hfs.py).
pub const KEYSTREAM_SIZE: usize = 4096;
/// First level tried.
pub const FAST_LEVEL: i32 = 3;
/// Level used when the fast one already compresses.
pub const STRONG_LEVEL: i32 = 19;
/// Fast result at or above this share of the original keeps the fast level (percent).
pub const KEEP_FAST_PERCENT: u64 = 97;
/// Largest window a decoder accepts (log2): the default of level 19 is 2^23; 2^24 leaves room.
pub const MAX_WINDOW_LOG: u32 = 24;

/// XOR with the pack keystream by absolute file position (`offset` = position of `data[0]`). An
/// involution: the same call undoes it.
pub fn hfs_xor(keystream: &[u8; KEYSTREAM_SIZE], offset: u64, data: &mut [u8]) {
    for (i, byte) in data.iter_mut().enumerate() {
        let position = (offset as usize).wrapping_add(i) & (KEYSTREAM_SIZE - 1);
        *byte ^= keystream[position];
    }
}

/// Applies a transform in place (both directions: every transform is its own inverse).
pub fn apply(transform: Transform, keystream: &[u8; KEYSTREAM_SIZE], offset: u64, data: &mut [u8]) {
    match transform {
        Transform::None => {}
        Transform::HfsXor => hfs_xor(keystream, offset, data),
    }
}

fn compress_level(plain: &[u8], level: i32) -> Result<Vec<u8>, DistError> {
    let mut encoder = zstd::stream::Encoder::new(Vec::with_capacity(plain.len() / 2 + 64), level)?;
    encoder.include_checksum(true)?;
    encoder.include_contentsize(true)?;
    encoder.set_pledged_src_size(Some(plain.len() as u64))?;
    encoder.write_all(plain)?;
    Ok(encoder.finish()?)
}

/// The object of already transformed bytes and the level chosen.
pub fn compress(plain: &[u8]) -> Result<(Vec<u8>, i32), DistError> {
    let fast = compress_level(plain, FAST_LEVEL)?;
    if (fast.len() as u64) * 100 >= (plain.len() as u64) * KEEP_FAST_PERCENT {
        return Ok((fast, FAST_LEVEL));
    }
    let strong = compress_level(plain, STRONG_LEVEL)?;
    Ok(if strong.len() < fast.len() {
        (strong, STRONG_LEVEL)
    } else {
        (fast, FAST_LEVEL)
    })
}

/// Decompresses an object (at most `size` bytes plus one, to detect a longer one).
pub fn decompress(object: &[u8], size: u64) -> Result<Vec<u8>, DistError> {
    let mut decoder = zstd::stream::Decoder::new(object)?;
    decoder.window_log_max(MAX_WINDOW_LOG)?;
    let mut out = Vec::with_capacity(usize::try_from(size).unwrap_or(0));
    decoder.take(size.saturating_add(1)).read_to_end(&mut out)?;
    Ok(out)
}

/// The file on disk from its object: decompress, undo the transform, check size and SHA-256.
pub fn restore(
    object: &[u8],
    transform: Transform,
    keystream: &[u8; KEYSTREAM_SIZE],
    sha256: &str,
    size: u64,
) -> Result<Vec<u8>, DistError> {
    let mut data = decompress(object, size)?;
    apply(transform, keystream, 0, &mut data);
    if data.len() as u64 != size || sha256_hex(&data) != sha256 {
        return Err(DistError::Digest(sha256.chars().take(16).collect()));
    }
    Ok(data)
}

/// Reads a 4096-byte keystream.
pub fn keystream(bytes: &[u8]) -> Result<[u8; KEYSTREAM_SIZE], DistError> {
    bytes.try_into().map_err(|_| DistError::Format("keystream"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ks() -> [u8; KEYSTREAM_SIZE] {
        std::array::from_fn(|i| (i * 7 + 3) as u8)
    }

    #[test]
    fn xor_is_by_absolute_position_and_undoes_itself() {
        let original: Vec<u8> = (0..10_000u32).map(|i| (i % 251) as u8).collect();
        let mut whole = original.clone();
        hfs_xor(&ks(), 0, &mut whole);
        let mut split = original.clone();
        let (a, b) = split.split_at_mut(5000);
        hfs_xor(&ks(), 0, a);
        hfs_xor(&ks(), 5000, b);
        assert_eq!(whole, split);
        assert_eq!(whole[4096], original[4096] ^ ks()[0]);
        hfs_xor(&ks(), 0, &mut whole);
        assert_eq!(whole, original);
    }

    #[test]
    fn compressible_data_takes_the_strong_level() {
        let text: Vec<u8> = (0..20_000u32)
            .flat_map(|i| format!("\"key_{i}\"\t\"{}\"\n", i * 7919 % 104_729).into_bytes())
            .collect();
        let (object, level) = compress(&text).unwrap_or_default();
        assert_eq!(level, STRONG_LEVEL);
        assert_eq!(decompress(&object, text.len() as u64).ok(), Some(text));
    }

    #[test]
    fn incompressible_data_keeps_the_fast_level() {
        let mut state = 0x1234_5678u32;
        let noise: Vec<u8> = (0..65_536)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                state as u8
            })
            .collect();
        assert_eq!(
            compress(&noise).map(|(_, level)| level).ok(),
            Some(FAST_LEVEL)
        );
    }

    #[test]
    fn restore_checks_the_digest() {
        let mut data = b"HF\x01\x02 pack bytes".repeat(100);
        let sha = sha256_hex(&data);
        let size = data.len() as u64;
        apply(Transform::HfsXor, &ks(), 0, &mut data);
        let (object, _) = compress(&data).unwrap_or_default();
        assert!(restore(&object, Transform::HfsXor, &ks(), &sha, size).is_ok());
        assert!(restore(&object, Transform::None, &ks(), &sha, size).is_err());
        assert!(restore(&object, Transform::HfsXor, &ks(), &sha, size - 1).is_err());
    }
}
