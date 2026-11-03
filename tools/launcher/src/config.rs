//! `vindictus-launcher.json`, ao lado do exe. Guarda o endereço do servidor, a pasta do jogo e o
//! último login digitado; nunca a senha, a sessão ou o ticket.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Endereço do listener do painel no laboratório. O operador distribui o exe com o
/// `vindictus-launcher.json` do servidor público ao lado.
pub const DEFAULT_SERVER: &str = "http://127.0.0.1:27090";
const FILE: &str = "vindictus-launcher.json";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub server: String,
    #[serde(default)]
    pub game_folder: String,
    #[serde(default)]
    pub last_login: String,
    /// Abrir o jogo como administrador (UAC a cada Jogar). Padrão: não; o jogo roda com as
    /// permissões do usuário (`game::start`).
    #[serde(default)]
    pub game_as_admin: bool,
}

fn exe_folder() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf))
        .unwrap_or_default()
}

fn path() -> PathBuf {
    exe_folder().join(FILE)
}

impl Config {
    /// Lê o arquivo; um arquivo ausente ou inválido dá os padrões. Sem pasta configurada, usa a
    /// pasta do exe se ela tiver o jogo.
    pub fn load() -> Self {
        let mut config: Config = std::fs::read(path())
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        if config.server.trim().is_empty() {
            config.server = DEFAULT_SERVER.into();
        }
        if config.game_folder.is_empty() {
            let here = exe_folder();
            if crate::game::is_game_folder(&here) {
                config.game_folder = here.to_string_lossy().into_owned();
            }
        }
        config
    }

    pub fn save(&self) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path(), bytes)
            .map_err(|e| format!("Não foi possível gravar {}: {e}", path().display()))
    }

    /// Servidor sem barra final; só http(s).
    pub fn server_base(&self) -> Result<String, String> {
        normalize_server(&self.server)
    }
}

pub fn normalize_server(text: &str) -> Result<String, String> {
    let base = text.trim().trim_end_matches('/');
    let rest = base
        .strip_prefix("http://")
        .or_else(|| base.strip_prefix("https://"))
        .ok_or("O endereço do servidor deve começar com http:// ou https://.")?;
    if rest.is_empty() || rest.contains(char::is_whitespace) {
        return Err("Endereço do servidor inválido.".into());
    }
    Ok(base.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_addresses_are_normalized_and_checked() {
        assert_eq!(
            normalize_server(" http://127.0.0.1:27090/ "),
            Ok("http://127.0.0.1:27090".into())
        );
        assert_eq!(
            normalize_server("https://jogo.example"),
            Ok("https://jogo.example".into())
        );
        assert!(normalize_server("127.0.0.1:27090").is_err());
        assert!(normalize_server("http://").is_err());
        assert!(normalize_server("http://a b").is_err());
    }
}
