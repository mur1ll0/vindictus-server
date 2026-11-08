//! The client builder service (docs/future/game-launcher.md 4.10): the server's own client tree, built once
//! from the imported 1.69 original by the Python recipe (`seed`), then kept equal to the published version
//! and refreshed with the published panel content on request (`content` jobs). What the content changed
//! goes into the open draft; the panel publishes it as N+1. The original never needs to be on the
//! operator's machine again, and no version rebuilds the whole client.
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use sqlx::PgPool;

use crate::infrastructure::client_dist_store::StoreError;
use crate::infrastructure::client_drafts::{self as drafts, ItemRow};
use crate::infrastructure::client_tree::{self as tree, HashCache, ServerDirs};

/// Where the builder works.
#[derive(Debug, Clone)]
pub struct BuilderPaths {
    /// The update store (read only here).
    pub store: PathBuf,
    /// `${CLIENT_SERVER_DIR}`: work/, staging/, build-state/, inputs/.
    pub server: ServerDirs,
    /// The recipe checkout inside the image (`scripts/client`, `scripts/content`).
    pub recipe: PathBuf,
    /// The published panel content (the game writes it at start: content-state/content-edits.json).
    pub edits: PathBuf,
    /// Python interpreter.
    pub python: PathBuf,
}

impl BuilderPaths {
    fn script(&self) -> PathBuf {
        self.recipe.join("scripts/client/build_client_version.py")
    }
    fn keystream(&self) -> PathBuf {
        self.server.state().join("hfs-keystream.bin")
    }
    fn hashes(&self) -> PathBuf {
        self.server.state().join("work-hashes.json")
    }
    fn original(&self) -> PathBuf {
        self.server
            .workspace()
            .join("Vindictus1.69EN/Vindictus EU/en-EU")
    }
}

/// A failure, as the panel shows it (Portuguese, no secret).
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("{0}")]
    Store(#[from] StoreError),
    #[error("banco de dados: {0}")]
    Database(#[from] sqlx::Error),
    #[error("{0}")]
    Refused(String),
}

fn refused(text: impl Into<String>) -> BuildError {
    BuildError::Refused(text.into())
}

/// What a content job or a seed left in the draft.
#[derive(Debug, Default, serde::Serialize)]
pub struct Outcome {
    pub base_version: u32,
    pub synced_files: usize,
    pub put: usize,
    pub removed: usize,
    /// Paths an admin changed by hand in the draft, kept over the content's change.
    pub kept_manual: Vec<String>,
}

impl Outcome {
    pub fn message(&self) -> String {
        let mut text = format!(
            "versão {}: {} arquivo(s) novos ou alterados e {} removido(s) no rascunho",
            self.base_version, self.put, self.removed
        );
        if self.synced_files > 0 {
            text += &format!(
                "; {} arquivo(s) da árvore do servidor voltaram à versão publicada",
                self.synced_files
            );
        }
        if !self.kept_manual.is_empty() {
            text += &format!(
                "; mantidas as mudanças manuais de {}",
                self.kept_manual.join(", ")
            );
        }
        text
    }
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, BuildError> + Send + 'static,
) -> Result<T, BuildError> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|_| refused("a tarefa do construtor parou"))?
}

/// The keystream the recipe needs, taken from the published manifest (never imported by hand).
fn write_keystream(
    paths: &BuilderPaths,
    manifest: &vindictus_client_dist::manifest::Manifest,
) -> Result<(), BuildError> {
    let keystream = tree::keystream_of(&paths.store, manifest)?;
    std::fs::create_dir_all(paths.server.state()).map_err(StoreError::from)?;
    std::fs::write(paths.keystream(), keystream).map_err(StoreError::from)?;
    Ok(())
}

/// Runs the recipe (one JSON line on stdout; stderr kept in build-state/<log>).
fn run_recipe(
    paths: &BuilderPaths,
    args: &[&std::ffi::OsStr],
    log: &str,
) -> Result<serde_json::Value, BuildError> {
    let output = Command::new(&paths.python)
        .arg("-s")
        .arg(paths.script())
        .args(args)
        .current_dir(&paths.recipe)
        .env("VINDICTUS_WORKSPACE", paths.server.workspace())
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .output()
        .map_err(|e| refused(format!("não consegui rodar o Python da receita: {e}")))?;
    let _ = std::fs::write(paths.server.state().join(log), &output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let last = stdout.lines().last().unwrap_or_default();
    let value: serde_json::Value = serde_json::from_str(last).unwrap_or(serde_json::Value::Null);
    if !output.status.success() || value.get("built") != Some(&serde_json::Value::Bool(true)) {
        let why = value
            .get("error")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("veja build-state/");
        return Err(refused(format!(
            "a receita falhou: {}",
            why.chars().take(600).collect::<String>()
        )));
    }
    Ok(value)
}

/// Compares the tree with the published version and stages what differs (blocking).
fn diff_and_stage(paths: &BuilderPaths) -> Result<(u32, Vec<ItemRow>), BuildError> {
    let (pointer, manifest) = tree::published(&paths.store)?;
    let mut cache = HashCache::load(&paths.hashes());
    let diff = tree::diff_tree(&paths.server.work(), &manifest, &mut cache)?;
    let mut items = Vec::new();
    for (path, sha, size, before) in diff.put {
        tree::stage_from_tree(&paths.server.staging(), &paths.server.work(), &path, &sha)?;
        items.push(ItemRow {
            path,
            action: "put".into(),
            sha256: Some(sha),
            size: Some(i64::try_from(size).map_err(|_| refused("arquivo grande demais"))?),
            before_sha256: before,
            source: "content".into(),
        });
    }
    for (path, before) in diff.removed {
        items.push(ItemRow {
            path,
            action: "remove".into(),
            sha256: None,
            size: None,
            before_sha256: Some(before),
            source: "content".into(),
        });
    }
    cache.save(&paths.hashes())?;
    Ok((pointer.current, items))
}

/// Puts the tree's differences in the open draft (created on the published version if none).
async fn fill_draft(
    pool: &PgPool,
    base: u32,
    items: Vec<ItemRow>,
    synced: usize,
) -> Result<Outcome, BuildError> {
    let mut tx = pool.begin().await?;
    let draft = drafts::lock_or_create(
        &mut tx,
        i32::try_from(base).map_err(|_| refused("versão inválida"))?,
        None,
    )
    .await?;
    if u32::try_from(draft.base_version).ok() != Some(base) {
        return Err(refused(format!(
            "o rascunho aberto começou na versão {} e a publicada é a {base}: descarte-o e gere de novo",
            draft.base_version
        )));
    }
    let kept_manual = drafts::replace_content_items(&mut tx, draft.id, &items).await?;
    tx.commit().await?;
    let kept: std::collections::HashSet<String> =
        kept_manual.iter().map(|p| p.to_ascii_lowercase()).collect();
    let active: Vec<&ItemRow> = items
        .iter()
        .filter(|i| !kept.contains(&i.path.to_ascii_lowercase()))
        .collect();
    let removed = active.iter().filter(|i| i.action == "remove").count();
    Ok(Outcome {
        base_version: base,
        synced_files: synced,
        put: active.len() - removed,
        removed,
        kept_manual,
    })
}

/// A content job: the tree back to the published version, the recipe's content step again, and what
/// changed into the draft.
pub async fn content_job(pool: &PgPool, paths: &BuilderPaths) -> Result<Outcome, BuildError> {
    let work = paths.server.work();
    if !work.join("lab-backup").join("client-build.json").is_file() {
        return Err(refused(
            "a árvore do cliente do servidor não existe: importe o cliente base (scripts/dev/import-client-base.ps1)",
        ));
    }
    let p = paths.clone();
    let synced = blocking(move || {
        let (_, manifest) = tree::published(&p.store)?;
        write_keystream(&p, &manifest)?;
        let mut cache = HashCache::load(&p.hashes());
        let report = tree::sync_tree(&p.store, &manifest, &p.server.work(), &mut cache)?;
        cache.save(&p.hashes())?;
        tracing::info!(
            written = report.written,
            deleted = report.deleted,
            unchanged = report.unchanged,
            "client builder: server tree synced to the published version"
        );
        Ok(report.written + report.deleted)
    })
    .await?;
    let p = paths.clone();
    blocking(move || {
        let (work, ks, edits, original) = (
            p.server.work(),
            p.keystream(),
            p.edits.clone(),
            p.original(),
        );
        run_recipe(
            &p,
            &[
                "--refresh".as_ref(),
                "--out".as_ref(),
                work.as_os_str(),
                "--keystream".as_ref(),
                ks.as_os_str(),
                "--edits".as_ref(),
                edits.as_os_str(),
                "--original".as_ref(),
                original.as_os_str(),
            ],
            "refresh.log",
        )
    })
    .await?;
    let p = paths.clone();
    let (base, items) = blocking(move || diff_and_stage(&p)).await?;
    fill_draft(pool, base, items, synced).await
}

/// The one-time build of the server's client tree from the imported original (`client-builder seed`):
/// the recipe's layer `layer`, keeping its backups; what differs from the published version goes into
/// the draft (ideally nothing but the content published since).
pub async fn seed(
    pool: &PgPool,
    paths: &BuilderPaths,
    layer: u8,
    nmcogame: &Path,
) -> Result<Outcome, BuildError> {
    let work = paths.server.work();
    if work.exists()
        && std::fs::read_dir(&work)
            .map_err(StoreError::from)?
            .next()
            .is_some()
    {
        return Err(refused(format!(
            "{} já existe: apague-a para montar de novo",
            work.display()
        )));
    }
    let next = paths.server.root.join("work.new");
    if next.exists() {
        return Err(refused(format!(
            "{} sobrou de uma montagem interrompida: apague-a",
            next.display()
        )));
    }
    let p = paths.clone();
    let nmcogame = nmcogame.to_path_buf();
    blocking(move || {
        let (_, manifest) = tree::published(&p.store)?;
        write_keystream(&p, &manifest)?;
        let (ks, edits, original, layer) = (
            p.keystream(),
            p.edits.clone(),
            p.original(),
            layer.to_string(),
        );
        run_recipe(
            &p,
            &[
                "--version".as_ref(),
                layer.as_ref(),
                "--keep-state".as_ref(),
                "--out".as_ref(),
                next.as_os_str(),
                "--keystream".as_ref(),
                ks.as_os_str(),
                "--edits".as_ref(),
                edits.as_os_str(),
                "--original".as_ref(),
                original.as_os_str(),
                "--nmcogame".as_ref(),
                nmcogame.as_os_str(),
            ],
            "seed.log",
        )?;
        if work.exists() {
            std::fs::remove_dir(&work).map_err(StoreError::from)?;
        }
        std::fs::rename(&next, &work).map_err(StoreError::from)?;
        Ok(())
    })
    .await?;
    let p = paths.clone();
    let (base, items) = blocking(move || diff_and_stage(&p)).await?;
    fill_draft(pool, base, items, 0).await
}

/// The service: takes queued jobs one at a time until `stop` resolves.
pub async fn worker(
    pool: PgPool,
    paths: BuilderPaths,
    stop: impl std::future::Future<Output = ()>,
) -> Result<(), BuildError> {
    let interrupted = drafts::fail_interrupted(&pool).await?;
    if interrupted > 0 {
        tracing::warn!(
            interrupted,
            "client builder: jobs left running were marked failed"
        );
    }
    tokio::pin!(stop);
    loop {
        let job = match drafts::claim(&pool).await {
            Ok(job) => job,
            Err(error) => {
                tracing::warn!(%error, "client builder: cannot read jobs");
                None
            }
        };
        let Some(job) = job else {
            tokio::select! {
                () = &mut stop => return Ok(()),
                () = tokio::time::sleep(Duration::from_secs(3)) => continue,
            }
        };
        tracing::info!(job = job.id, kind = %job.kind, "client builder: job started");
        let result = match job.kind.as_str() {
            "content" => content_job(&pool, &paths).await,
            other => Err(refused(format!("tarefa desconhecida: {other}"))),
        };
        let (ok, message) = match &result {
            Ok(outcome) => (true, outcome.message()),
            Err(error) => (false, error.to_string()),
        };
        tracing::info!(job = job.id, ok, "client builder: job finished");
        drafts::finish(&pool, job.id, ok, &message).await?;
    }
}
