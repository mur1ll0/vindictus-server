//! What a version holds and how each file is stored (docs/future/game-launcher.md 4.3). The same lists as
//! scripts/client/build_client_version.py (EXCLUDED_DIRS, EXCLUDED_GLOBS, PRESERVE_GLOBS): a change goes
//! to both, with the operator's decision (D-L3 and the additions of 07/11/2026).

use crate::manifest::Transform;

/// Folders left out anywhere in the tree.
pub const EXCLUDED_DIRS: &[&str] = &[
    "hfs_extracted",
    "lab-backup",
    "passport",
    // Not BlackCipher: Vindictus.exe imports NGClient.aes, which refuses to start the game without
    // BlackCipher/*.aes ("add an exemption for Nexon Game Security", LV of 08/11/2026). Its logs
    // already fall under *.log.
    "downloads",
    "DownloadLists",
    "__pycache__",
];
/// Files left out: globs over the relative path, or over the file name when the glob has no `/`.
pub const EXCLUDED_GLOBS: &[&str] = &[
    "l0-*",
    "*.bak",
    "*.pre-*.bak",
    "*.log",
    "npp_*_std*.log",
    "fmod.log",
    "*.nexon",
    "*.partial",
    "NMService.exe",
    "_eu_AdBalloonExt.exe",
    "VindictusLauncher.exe",
    "bcdedit.exe",
    "start*.bat",
    "maps/graphs/*.ain",
    "demoheader.tmp",
    "hfs-keystream.bin",
    "cfg/config_pendingkey_concmd.cfg",
    "bin/class-routing.json",
    "sql/heroes.db3.pre-*",
    "sql/heroes.db3.lab-items.json",
    "nmcogame.ini",
    "vindictus-launcher.*",
    "bin/config_material.txt",
];
/// The player's files: installed when missing, never checked nor overwritten.
pub const PRESERVE_GLOBS: &[&str] = &[
    "SAVE/*",
    "cfg/config.cfg",
    "cfg/config_env.cfg",
    "cfg/config_ex.cfg",
    "cfg/language.cfg",
    "cfg/config_*_ex.cfg",
];
/// The HFS index: its own zlib stream with a 4-byte XOR, not the pack keystream; stored as it is.
pub const HFS_INDEX: &str = "hfs/E608DF10F284E19AB75FD7D0D8AE9CCF87831041.hfs";

/// `*` and `?` wildcards, ASCII case-insensitive (fnmatch without character classes).
pub fn glob(pattern: &str, text: &str) -> bool {
    let (p, t) = (pattern.as_bytes(), text.as_bytes());
    let (mut pi, mut ti, mut star, mut mark) = (0, 0, None, 0);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == b'?' || p[pi].eq_ignore_ascii_case(&t[ti])) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == b'*' {
            star = Some(pi);
            pi += 1;
            mark = ti;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|&c| c == b'*')
}

fn matches(path: &str, globs: &[&str]) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    globs
        .iter()
        .any(|g| glob(g, path) || (!g.contains('/') && glob(g, name)))
}

/// True when the path is not part of a version (EXCLUDED_DIRS anywhere above it, or EXCLUDED_GLOBS).
pub fn excluded(path: &str) -> bool {
    let mut parts: Vec<&str> = path.split('/').collect();
    parts.pop();
    parts
        .iter()
        .any(|p| EXCLUDED_DIRS.iter().any(|d| d.eq_ignore_ascii_case(p)))
        || matches(path, EXCLUDED_GLOBS)
}

/// True for the player's files (manifest `preserve`).
pub fn preserved(path: &str) -> bool {
    matches(path, PRESERVE_GLOBS)
}

/// `hfs-xor` for every pack (the server keeps them with the keystream XOR undone), `none` for the rest.
pub fn transform_of(path: &str) -> Transform {
    if path.to_ascii_lowercase().ends_with(".hfs")
        && !path.eq_ignore_ascii_case(HFS_INDEX)
        && path.starts_with("hfs/")
    {
        Transform::HfsXor
    } else {
        Transform::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn globs_follow_fnmatch() {
        assert!(glob("*.bak", "client.dll.b284.pre-routing.bak"));
        assert!(glob(
            "sql/heroes.db3.pre-*",
            "sql/heroes.db3.pre-lab-additions"
        ));
        assert!(glob("start*.bat", "START-lab.bat"));
        assert!(!glob("*.log", "log.txt"));
        assert!(glob("a?c", "abc") && !glob("a?c", "ac"));
    }

    #[test]
    fn the_recipe_exclusions_and_player_files() {
        for path in [
            "hfs_extracted/a.txt",
            "x/lab-backup/b.hfs",
            "bin/client.dll.b284ba719047556b.bak",
            "fmod.log",
            "nmcogame.dll.nexon",
            "start.bat",
            "maps/graphs/t01.ain",
            "bin/config_material.txt",
            "vindictus-launcher.exe",
            "sql/heroes.db3.pre-story-goals",
            "BlackCipher/BlackCipher.log",
        ] {
            assert!(excluded(path), "{path}");
        }
        for path in [
            "BlackCipher/BlackCipher.aes",
            "BlackCipher/config.bc",
            "NGClient.aes",
            "Vindictus.exe",
            "bin/client.dll",
            "sql/heroes.db3",
            "maps/t01.bsp",
            "cfg/config_env.cfg",
        ] {
            assert!(!excluded(path), "{path}");
        }
        assert!(
            preserved("cfg/config_miri_ex.cfg")
                && preserved("SAVE/hidedate.hdf")
                && !preserved("cfg/autoexec.cfg")
        );
    }

    #[test]
    fn packs_take_hfs_xor_but_not_the_index() {
        assert_eq!(
            transform_of("hfs/0FABE22A68C0451A7EF97F6E8E682A448BB8122A.hfs"),
            Transform::HfsXor
        );
        assert_eq!(transform_of(HFS_INDEX), Transform::None);
        assert_eq!(transform_of("Base_hl2.hfs"), Transform::None);
        assert_eq!(transform_of("bin/client.dll"), Transform::None);
    }
}
