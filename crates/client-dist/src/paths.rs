//! Game paths of a manifest (docs/future/game-launcher.md 4.8): relative, `/`-separated, inside the game
//! folder, never a Windows device name, and unique without regard to case (the player's disk is NTFS).

use crate::DistError;
use std::collections::HashSet;

/// Longest path accepted (well below MAX_PATH with the game folder in front).
pub const MAX_PATH_LEN: usize = 200;
const RESERVED: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// Checks one game path: relative, `/`-separated, no `.`/`..`/empty parts, no drive or UNC prefix,
/// printable ASCII without `\ : * ? " < > |`, no part ending in a dot or a space, no reserved device name.
pub fn check(path: &str) -> Result<(), DistError> {
    let bad = || Err(DistError::Path(path.chars().take(80).collect()));
    if path.is_empty() || path.len() > MAX_PATH_LEN || path.starts_with('/') {
        return bad();
    }
    if !path
        .bytes()
        .all(|b| (0x20..0x7f).contains(&b) && !b"\\:*?\"<>|".contains(&b))
    {
        return bad();
    }
    for part in path.split('/') {
        if part.is_empty()
            || part == "."
            || part == ".."
            || part.ends_with('.')
            || part.ends_with(' ')
        {
            return bad();
        }
        let stem = part.split('.').next().unwrap_or(part).to_ascii_lowercase();
        if RESERVED.contains(&stem.trim_end()) {
            return bad();
        }
    }
    Ok(())
}

/// Checks every path and that no two differ only by case.
pub fn check_all<'a>(paths: impl IntoIterator<Item = &'a str>) -> Result<(), DistError> {
    let mut seen = HashSet::new();
    for path in paths {
        check(path)?;
        if !seen.insert(path.to_ascii_lowercase()) {
            return Err(DistError::Path(path.chars().take(80).collect()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_paths_are_accepted() {
        for path in [
            "Vindictus.exe",
            "bin/client.dll",
            "hfs/E608DF10F284E19AB75FD7D0D8AE9CCF87831041.hfs",
            "cfg/config_env.cfg",
            "maps/t01.bsp",
            "hfs/RENOMEADo_9FB527B373DCBD159552ADD1710487A724237AF9.hfs",
        ] {
            assert!(check(path).is_ok(), "{path}");
        }
    }

    #[test]
    fn escapes_and_devices_are_refused() {
        for path in [
            "",
            "/etc/passwd",
            "../x",
            "a/../b",
            "a//b",
            "./a",
            "C:/x",
            "a\\b",
            "con",
            "bin/NUL.txt",
            "com1.dll",
            "a/b.",
            "a/b ",
            "a\u{0}b",
            "a?b",
            "\u{e9}.txt",
        ] {
            assert!(check(path).is_err(), "{path:?}");
        }
        assert!(check(&"a".repeat(MAX_PATH_LEN + 1)).is_err());
    }

    #[test]
    fn duplicates_by_case_are_refused() {
        assert!(check_all(["bin/a.dll", "bin/b.dll"]).is_ok());
        assert!(check_all(["bin/a.dll", "BIN/A.dll"]).is_err());
    }
}
