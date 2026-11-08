//! Autoatualização do launcher (L5, `docs/future/game-launcher.md` 4.6): o `versao.json` assinado
//! aponta o exe publicado (`client-dist launcher`). Se a versão dele for maior que a deste exe e o
//! conteúdo for outro, o objeto é baixado, descomprimido e conferido pelo SHA-256 em
//! `vindictus-launcher.exe.new`; a troca renomeia o exe em uso para `.old` (o Windows deixa renomear um
//! exe aberto), põe o novo no lugar e abre o novo com os mesmos argumentos. O `.old` sai na próxima
//! abertura. Sem nada publicado, ou com o mesmo conteúdo, nada acontece.
use crate::update::{Fetch, UpdateError};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use vindictus_client_dist::manifest::{LauncherRef, VersionPointer, is_newer};
use vindictus_client_dist::{codec, layout, sha256_hex};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

fn with_suffix(exe: &Path, suffix: &str) -> PathBuf {
    let mut name = exe.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

pub fn current_exe() -> Option<PathBuf> {
    std::env::current_exe().ok()
}

/// O exe preparado para a troca.
pub fn staged(exe: &Path) -> PathBuf {
    with_suffix(exe, ".new")
}

/// Apaga o exe antigo de uma troca anterior (falha em silêncio se ainda estiver aberto).
pub fn cleanup(exe: &Path) {
    let _ = fs::remove_file(with_suffix(exe, ".old"));
    let _ = fs::remove_file(with_suffix(exe, ".new.part"));
}

/// O launcher publicado, se ele for uma versão maior que esta e outro conteúdo.
pub fn wanted(pointer: &VersionPointer, exe: &Path) -> Option<LauncherRef> {
    let published = pointer.launcher.as_ref()?;
    if !is_newer(&published.version, VERSION) {
        return None;
    }
    let own = fs::read(exe).ok().map(|b| sha256_hex(&b));
    (own.as_deref() != Some(published.sha256.as_str())).then(|| published.clone())
}

/// Baixa e confere o exe publicado em `<exe>.new`.
pub fn download(
    fetch: &dyn Fetch,
    launcher: &LauncherRef,
    exe: &Path,
    cancel: &AtomicBool,
) -> Result<(), UpdateError> {
    let part = with_suffix(exe, ".new.part");
    fetch.download(
        &layout::object(&launcher.sha256),
        &part,
        launcher.object_size,
        cancel,
        &|_| {},
    )?;
    let object = fs::read(&part)?;
    let data = codec::decompress(&object, launcher.size)?;
    if data.len() as u64 != launcher.size
        || sha256_hex(&data) != launcher.sha256
        || data.get(..2) != Some(b"MZ")
    {
        let _ = fs::remove_file(&part);
        return Err(UpdateError::Digest("vindictus-launcher.exe".into()));
    }
    fs::write(staged(exe), data)?;
    fs::remove_file(&part)?;
    Ok(())
}

/// Põe o exe preparado no lugar do atual; `true` quando trocou.
pub fn swap(exe: &Path) -> Result<bool, UpdateError> {
    let new = staged(exe);
    if !new.is_file() {
        return Ok(false);
    }
    let old = with_suffix(exe, ".old");
    let _ = fs::remove_file(&old);
    fs::rename(exe, &old)?;
    if let Err(error) = fs::rename(&new, exe) {
        // Sem o novo no lugar, o antigo volta.
        let _ = fs::rename(&old, exe);
        return Err(error.into());
    }
    Ok(true)
}

/// Abre o exe (o novo, depois da troca) com os argumentos desta execução.
pub fn restart(exe: &Path) -> bool {
    std::process::Command::new(exe)
        .args(std::env::args_os().skip(1))
        .spawn()
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use vindictus_client_dist::manifest::FORMAT;

    fn pointer(version: &str, sha: &str) -> VersionPointer {
        VersionPointer {
            format: FORMAT,
            current: 1,
            minimum: 1,
            manifest_sha256: "a".repeat(64),
            launcher: Some(LauncherRef {
                version: version.into(),
                sha256: sha.into(),
                size: 4,
                object_size: 4,
            }),
        }
    }

    #[test]
    fn only_a_newer_different_launcher_is_wanted() {
        let dir = std::env::temp_dir().join(format!("launcher-self-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let exe = dir.join("vindictus-launcher.exe");
        fs::write(&exe, b"MZ old").ok();
        let other = "b".repeat(64);
        assert!(wanted(&pointer("99.0.0", &other), &exe).is_some());
        assert!(wanted(&pointer("0.0.1", &other), &exe).is_none());
        assert!(wanted(&pointer("99.0.0", &sha256_hex(b"MZ old")), &exe).is_none());
        let mut none = pointer("99.0.0", &other);
        none.launcher = None;
        assert!(wanted(&none, &exe).is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn swap_keeps_the_old_one_aside() {
        let dir = std::env::temp_dir().join(format!("launcher-swap-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let exe = dir.join("vindictus-launcher.exe");
        fs::write(&exe, b"MZ old").ok();
        assert_eq!(swap(&exe).ok(), Some(false));
        fs::write(staged(&exe), b"MZ new").ok();
        assert_eq!(swap(&exe).ok(), Some(true));
        assert_eq!(fs::read(&exe).ok(), Some(b"MZ new".to_vec()));
        assert!(with_suffix(&exe, ".old").is_file());
        cleanup(&exe);
        assert!(!with_suffix(&exe, ".old").exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
