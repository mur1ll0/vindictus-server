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

/// Largest old or new file a delta covers (docs/future/game-launcher.md 4.3 item 4): the decoder holds both,
/// at most 128 MB, safe in a 32-bit launcher.
pub const DELTA_MAX_FILE: u64 = 64 << 20;
/// Largest window of a delta (log2): old + new of 64 MB each.
pub const DELTA_MAX_WINDOW_LOG: u32 = 27;
/// A delta is kept only below this share of the whole object (percent).
pub const DELTA_KEEP_PERCENT: u64 = 50;
/// Level of the deltas (the patch-from measurements of the study used 19).
pub const DELTA_LEVEL: i32 = 19;

/// Window (log2) covering the old file as prefix plus the new one.
pub fn delta_window_log(old: usize, new: usize) -> u32 {
    let span = (old + new).max(1 << 10) as u64;
    (64 - (span - 1).leading_zeros()).clamp(10, DELTA_MAX_WINDOW_LOG)
}

/// Delta from `old` to `new`, both in their transformed form (zstd `--patch-from`: the old one is the
/// prefix, long-distance matching on, checksum on).
pub fn delta(old: &[u8], new: &[u8]) -> Result<Vec<u8>, DistError> {
    if old.len() as u64 > DELTA_MAX_FILE || new.len() as u64 > DELTA_MAX_FILE {
        return Err(DistError::Format("delta source"));
    }
    let mut encoder = zstd::stream::Encoder::with_ref_prefix(Vec::new(), DELTA_LEVEL, old)?;
    encoder.window_log(delta_window_log(old.len(), new.len()))?;
    encoder.long_distance_matching(true)?;
    encoder.include_checksum(true)?;
    encoder.include_contentsize(true)?;
    encoder.set_pledged_src_size(Some(new.len() as u64))?;
    encoder.write_all(new)?;
    Ok(encoder.finish()?)
}

/// The new transformed form from the old one and a delta (at most `size` bytes plus one).
pub fn apply_delta(old: &[u8], delta: &[u8], size: u64) -> Result<Vec<u8>, DistError> {
    if old.len() as u64 > DELTA_MAX_FILE || size > DELTA_MAX_FILE {
        return Err(DistError::Format("delta source"));
    }
    let mut decoder = zstd::stream::Decoder::with_ref_prefix(delta, old)?;
    decoder.window_log_max(DELTA_MAX_WINDOW_LOG)?;
    let mut out = Vec::with_capacity(usize::try_from(size).unwrap_or(0));
    decoder.take(size.saturating_add(1)).read_to_end(&mut out)?;
    Ok(out)
}

/// The new file on disk from the old file's transformed form and a delta: apply, redo the transform, check
/// size and SHA-256 (what the launcher does when updating, docs/future/client-distribution-storage.md 5.4).
pub fn restore_from_delta(
    old_transformed: &[u8],
    delta: &[u8],
    transform: Transform,
    keystream: &[u8; KEYSTREAM_SIZE],
    sha256: &str,
    size: u64,
) -> Result<Vec<u8>, DistError> {
    let mut data = apply_delta(old_transformed, delta, size)?;
    apply(transform, keystream, 0, &mut data);
    if data.len() as u64 != size || sha256_hex(&data) != sha256 {
        return Err(DistError::Digest(sha256.chars().take(16).collect()));
    }
    Ok(data)
}

/// True when a delta of `delta_size` is worth keeping next to an object of `object_size`.
pub fn delta_worth(delta_size: u64, object_size: u64) -> bool {
    delta_size * 100 < object_size * DELTA_KEEP_PERCENT
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
    fn a_delta_rebuilds_the_new_file_from_the_old_one() {
        let old: Vec<u8> = (0..300_000u32)
            .map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8)
            .collect();
        let mut new = old.clone();
        new[150_000..150_100].fill(7);
        new.extend_from_slice(b"appended");
        let patch = delta(&old, &new).unwrap_or_default();
        assert!(patch.len() < 2_000, "delta of {} bytes", patch.len());
        assert!(apply_delta(&old, &patch, new.len() as u64).is_ok_and(|out| out == new));
        // Another prefix gives other bytes or a checksum error; the launcher's SHA-256 check is what
        // proves the file (one byte the delta never copies would not change the output).
        let other = vec![0u8; old.len()];
        assert!(
            apply_delta(&other, &patch, new.len() as u64).is_ok_and(|out| out != new)
                || apply_delta(&other, &patch, new.len() as u64).is_err()
        );
    }

    #[test]
    fn delta_bounds() {
        assert_eq!(delta_window_log(64 << 20, 64 << 20), 27);
        assert_eq!(delta_window_log(10, 10), 10);
        assert!(delta_worth(49, 100) && !delta_worth(50, 100));
        assert!(delta(&vec![0u8; (DELTA_MAX_FILE + 1) as usize], b"x").is_err());
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
