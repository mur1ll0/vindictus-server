//! Names inside the store (`${CLIENT_DIST_DIR}`, docs/future/client-distribution-storage.md 5.3), the same
//! as the HTTP paths the launcher asks for.

/// The version pointer.
pub const POINTER: &str = "versao.json";
/// Dynamic service status (not signed).
pub const STATUS: &str = "status.json";
/// Public key file served for pinning (hex).
pub const PUBLIC_KEY: &str = "updates.ed25519.pub";
/// Suffix of a signature next to its document.
pub const SIGNATURE_SUFFIX: &str = ".sig";

/// `objects/<aa>/<sha256>.zst`.
pub fn object(sha256: &str) -> String {
    format!("objects/{}/{sha256}.zst", sha256.get(..2).unwrap_or("00"))
}

/// `deltas/<from>-<to>.zst`.
pub fn delta(from_sha256: &str, to_sha256: &str) -> String {
    format!("deltas/{from_sha256}-{to_sha256}.zst")
}

/// `manifests/<n>.json`.
pub fn manifest(version: u32) -> String {
    format!("manifests/{version}.json")
}

/// `<document>.sig`.
pub fn signature(document: &str) -> String {
    format!("{document}{SIGNATURE_SUFFIX}")
}

#[cfg(test)]
mod tests {
    #[test]
    fn objects_fan_out_by_the_first_two_digits() {
        assert_eq!(super::object("ab12"), "objects/ab/ab12.zst");
        assert_eq!(super::manifest(3), "manifests/3.json");
        assert_eq!(super::signature("versao.json"), "versao.json.sig");
    }
}
