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

pub fn start(folder: &Path, ticket: &str) -> Result<Child, String> {
    if !is_game_folder(folder) {
        return Err(format!(
            "A pasta do jogo não tem o {EXECUTABLE} e o bin\\client.dll. Escolha a pasta do jogo."
        ));
    }
    Command::new(folder.join(EXECUTABLE))
        .current_dir(folder)
        .args(ARGUMENTS)
        .args(["-launcher_ticket", ticket])
        .spawn()
        .map_err(|e| format!("Não foi possível abrir o jogo: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_without_the_game_is_refused() {
        let folder = std::env::temp_dir().join("vindictus-launcher-test-no-game");
        assert!(!is_game_folder(&folder));
        assert!(start(&folder, "lt0000000000000000000000").is_err());
    }
}
