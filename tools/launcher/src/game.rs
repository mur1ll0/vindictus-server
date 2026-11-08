//! Pasta do jogo e o processo do cliente. O cliente recebe o ticket por `-launcher_ticket`; o patch
//! `scripts/client/patch_client_launcher_login.py` faz o diálogo de login entrar sozinho com ele.
use std::path::Path;
use std::process::{Child, Command};

pub const EXECUTABLE: &str = "Vindictus.exe";
/// Argumentos do `start.bat` do 1.69 (`docs/client-compatibility.md`), mais o ticket.
const ARGUMENTS: [&str; 7] = [
    "-stage",
    "-dev",
    "-lang",
    "en-eu-x-gm",
    "-noupdate",
    "-threads",
    "4",
];

pub fn is_game_folder(folder: &Path) -> bool {
    folder.join(EXECUTABLE).is_file() && folder.join("bin").join("client.dll").is_file()
}

pub fn check_folder(folder: &Path) -> Result<(), String> {
    if is_game_folder(folder) {
        Ok(())
    } else {
        Err(format!(
            "A pasta do jogo não tem o {EXECUTABLE} e o bin\\client.dll. Escolha a pasta do jogo."
        ))
    }
}

/// Linha de argumentos do cliente com o ticket (para o modo administrador, que passa uma string).
pub fn command_line(ticket: &str) -> String {
    let mut line = ARGUMENTS.join(" ");
    line.push_str(" -launcher_ticket ");
    line.push_str(ticket);
    line
}

/// Porta do messenger do servidor (B5 MS5, `docs/messenger-design.md`).
pub const MESSENGER_PORT: u16 = 47611;

/// O `nmcogame.ini` do messenger para o servidor do launcher: mesmo host, porta do messenger.
pub fn messenger_ini(server_base: &str) -> Option<String> {
    let rest = server_base
        .strip_prefix("http://")
        .or_else(|| server_base.strip_prefix("https://"))?;
    let authority = rest.split('/').next()?;
    let authority = authority.rsplit('@').next()?;
    let host = match authority.strip_prefix('[') {
        Some(v6) => format!("[{}]", v6.split(']').next()?),
        None => authority.split(':').next()?.to_owned(),
    };
    if host.is_empty() || host == "[]" {
        return None;
    }
    Some(format!(
        "[messenger]
address={host}:{MESSENGER_PORT}
"
    ))
}

/// Grava o `nmcogame.ini` na pasta do jogo antes de abrir; sem ele (ou se falhar) o jogo abre
/// com o messenger desligado ou no endereço padrão.
pub fn write_messenger_ini(folder: &Path, server_base: &str) {
    if let Some(text) = messenger_ini(server_base) {
        let _ = std::fs::write(folder.join("nmcogame.ini"), text);
    }
}

/// Abre o jogo sem elevação. O `Vindictus.exe` do 1.69 pede administrador no manifesto
/// (`requireAdministrator`), o que faria o `CreateProcess` falhar (erro 740) num launcher sem
/// elevação. `__COMPAT_LAYER=RunAsInvoker` é a camada de compatibilidade do próprio Windows (desde
/// o Vista) que ignora esse pedido e roda o jogo com as permissões do usuário, sem UAC e sem mudar
/// o exe. Se o jogo precisar mesmo de administrador, o launcher tem o modo elevado (`ui.rs`).
pub fn start(folder: &Path, ticket: &str) -> Result<Child, String> {
    check_folder(folder)?;
    Command::new(folder.join(EXECUTABLE))
        .current_dir(folder)
        .env("__COMPAT_LAYER", "RunAsInvoker")
        .args(ARGUMENTS)
        .args(["-launcher_ticket", ticket])
        .spawn()
        .map_err(|e| format!("Não foi possível abrir o jogo: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_messenger_ini_uses_the_server_host() {
        assert_eq!(
            messenger_ini("http://192.168.0.10:27090").as_deref(),
            Some(
                "[messenger]
address=192.168.0.10:47611
"
            )
        );
        assert_eq!(
            messenger_ini("https://jogo.example/base").as_deref(),
            Some(
                "[messenger]
address=jogo.example:47611
"
            )
        );
        assert_eq!(
            messenger_ini("http://[::1]:27090").as_deref(),
            Some(
                "[messenger]
address=[::1]:47611
"
            )
        );
        assert_eq!(messenger_ini("ftp://x"), None);
    }

    #[test]
    fn a_folder_without_the_game_is_refused() {
        let folder = std::env::temp_dir().join("vindictus-launcher-test-no-game");
        assert!(!is_game_folder(&folder));
        assert!(start(&folder, "lt0000000000000000000000").is_err());
    }
}
