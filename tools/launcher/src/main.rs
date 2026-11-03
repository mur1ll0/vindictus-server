//! Launcher do Vindictus (trilha L, `docs/future/game-launcher.md`): login com login e senha da
//! conta (os mesmos do painel), Jogar direto para a seleção de personagem, botões do painel e do
//! cadastro. Janela Win32 nativa compilada para o Windows 7 (`build.ps1`), sem dependências para o
//! jogador.
//!
//! - `config`: `vindictus-launcher.json` ao lado do exe (servidor, pasta do jogo, último login).
//! - `api`: sessão do launcher no servidor (`/api/launcher/login`, `/ticket`, `/logout`).
//! - `game`: pasta do jogo e o processo do cliente com `-launcher_ticket`.
//! - `ui`: a janela. É o único módulo com `unsafe` (chamadas Win32); o resto é código seguro.
//!
//! Atualização e instalação do cliente (L1–L3, serviço `updates`) entram depois, sobre a mesma
//! janela.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![deny(unsafe_code)]

mod api;
mod config;
mod game;
#[allow(unsafe_code)]
mod ui;

fn main() {
    ui::run();
}
