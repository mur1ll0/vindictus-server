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
//! - `update`: instalar, atualizar, verificar e reparar o cliente pelo serviço `updates` (L3).
//! - `selfupdate`: o próprio launcher, quando o servidor publica uma versão nova (L5).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![deny(unsafe_code)]

mod api;
mod config;
mod game;
mod selfupdate;
#[allow(unsafe_code)]
mod ui;
mod update;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--update") {
        std::process::exit(headless(&args[1..]));
    }
    if args.first().map(String::as_str) == Some("--self-update") {
        std::process::exit(headless_self_update(&args[1..]));
    }
    ui::run();
}

/// Sem janela: `vindictus-launcher.exe --self-update --log <arquivo> [--updates <url>]` baixa o launcher
/// publicado (se for mais novo) e faz a troca, sem reabrir; o resultado vai para o log.
fn headless_self_update(args: &[String]) -> i32 {
    use std::io::Write;
    let value = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let Some(log) = value("--log") else { return 2 };
    let Ok(mut out) = std::fs::File::create(&log) else {
        return 2;
    };
    let config = config::Config::load();
    let updates =
        value("--updates").map_or_else(|| config.updates_base(), |u| config::normalize_server(&u));
    let Some(exe) = selfupdate::current_exe() else {
        return 2;
    };
    selfupdate::cleanup(&exe);
    let result = updates
        .map_err(update::UpdateError::Offline)
        .and_then(|base| {
            let fetch = update::HttpFetch::new(&base);
            let key = update::pinned_key()?;
            let pointer = update::remote(&fetch, &key)?.pointer;
            match selfupdate::wanted(&pointer, &exe) {
                None => Ok(None),
                Some(launcher) => {
                    selfupdate::download(
                        &fetch,
                        &launcher,
                        &exe,
                        &std::sync::atomic::AtomicBool::new(false),
                    )?;
                    selfupdate::swap(&exe)?;
                    Ok(Some(launcher.version))
                }
            }
        });
    match result {
        Ok(Some(version)) => {
            let _ = writeln!(out, "ok trocado {} -> {version}", selfupdate::VERSION);
            0
        }
        Ok(None) => {
            let _ = writeln!(out, "ok atual {}", selfupdate::VERSION);
            0
        }
        Err(e) => {
            let _ = writeln!(out, "erro: {} ({e:?})", e.message());
            1
        }
    }
}

/// Sem janela, para diagnóstico e testes do exe real (o mesmo código do botão):
/// `vindictus-launcher.exe --update <pasta> --log <arquivo> [--updates <url>] [--version N] [--verify]`.
/// Instala ou atualiza a pasta e escreve o progresso e o resultado no log (o exe não tem console).
fn headless(args: &[String]) -> i32 {
    use std::io::Write;
    use std::sync::atomic::AtomicBool;
    let value = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let (Some(folder), Some(log)) = (args.first().cloned(), value("--log")) else {
        return 2;
    };
    let Ok(mut out) = std::fs::File::create(&log) else {
        return 2;
    };
    let config = config::Config::load();
    let updates =
        value("--updates").map_or_else(|| config.updates_base(), |u| config::normalize_server(&u));
    let version = value("--version").and_then(|v| v.parse().ok());
    let mode = if args.iter().any(|a| a == "--verify") {
        update::Mode::Full
    } else {
        update::Mode::Changes
    };
    let result = updates
        .map_err(update::UpdateError::Offline)
        .and_then(|base| {
            let fetch = update::HttpFetch::new(&base);
            let key = update::pinned_key()?;
            let bootstrap = update::bootstrap_url(&base);
            let last = std::cell::Cell::new((String::new(), 0u64));
            let report = |p: update::Progress| {
                let step = (p.done * 20).checked_div(p.total).unwrap_or(0);
                let (phase, shown) = last.take();
                if phase != p.phase || step != shown {
                    let _ = writeln!(&out, "{} {}/{} {}", p.phase, p.done, p.total, p.file);
                }
                last.set((p.phase.to_owned(), step));
            };
            update::run(
                std::path::Path::new(&folder),
                &fetch,
                &key,
                mode,
                version,
                &AtomicBool::new(false),
                &|| false,
                &report,
            )
            .and_then(|outcome| {
                // O cliente procura o jogo no host do servidor de atualização.
                if let Some(url) = bootstrap.as_deref() {
                    let changed = update::apply_endpoint(std::path::Path::new(&folder), url)?;
                    let _ = writeln!(
                        &out,
                        "endereco {url} {}",
                        if changed { "ajustado" } else { "igual" }
                    );
                }
                Ok(outcome)
            })
        });
    match result {
        Ok(o) => {
            let _ = writeln!(
                out,
                "ok version={} checked={} downloaded={} bytes={} deltas={} fallbacks={} preserved={}",
                o.version,
                o.checked,
                o.downloaded,
                o.downloaded_bytes,
                o.deltas,
                o.delta_fallbacks,
                o.preserved_kept
            );
            0
        }
        Err(e) => {
            let _ = writeln!(out, "erro: {} ({e:?})", e.message());
            1
        }
    }
}
