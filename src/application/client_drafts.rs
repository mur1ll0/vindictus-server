//! Drafts of the next client version (docs/future/game-launcher.md 4.10, operator request of 08/11/2026):
//! the panel adds, replaces or removes files at a game path, or asks the client builder to apply the
//! published panel content to the server's client; publishing makes N+1 from N plus those changes,
//! without rebuilding the client. Every change is audited; only admins change a draft.
use crate::application::admin_session::AdminError;
use crate::application::content_admin::{Actor, audit, require};
use crate::domain::admin::Permission;
use crate::infrastructure::admin_store::{SessionAccount, insert_audit};
use crate::infrastructure::client_dist_store::{StoreError, StorePaths};
use crate::infrastructure::client_drafts::{self as drafts, DraftRow, ItemRow, JobRow};
use crate::infrastructure::client_tree::{self as tree, Change};
use serde::Serialize;
use sqlx::PgPool;
use std::path::Path;

/// The open draft of the next version, its changes and the client builder's recent jobs.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct DraftView {
    pub draft: Option<DraftRow>,
    pub items: Vec<ItemRow>,
    pub jobs: Vec<JobRow>,
    /// The panel can stage files (the server's client folder is mounted).
    pub staging: bool,
}

/// One published file, for choosing what to replace or remove.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct PublishedFile {
    pub path: String,
    pub sha256: String,
    pub size: u64,
}

/// A store refusal keeps its message (Portuguese, no host path); other failures stay generic.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, StoreError> + Send + 'static,
) -> Result<T, AdminError> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|_| AdminError::ContentUnavailable)?
        .map_err(|error| match error {
            StoreError::Refused(text) => AdminError::ClientRefused(text),
            other => {
                tracing::warn!(error = %other, "client version store failed");
                AdminError::ContentUnavailable
            }
        })
}

async fn insert_once(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    record: &crate::infrastructure::admin_store::AuditRecord<'_>,
) -> Result<(), AdminError> {
    match insert_audit(tx, record).await {
        Ok(()) => Ok(()),
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            Err(AdminError::RepeatedOperation)
        }
        Err(e) => Err(e.into()),
    }
}

pub async fn view(
    pool: &PgPool,
    session: &SessionAccount,
    staging: Option<&Path>,
) -> Result<DraftView, AdminError> {
    require(session, Permission::Read)?;
    let draft = drafts::open_draft(pool).await?;
    let items = match &draft {
        Some(d) => drafts::items(pool, d.id).await?,
        None => Vec::new(),
    };
    Ok(DraftView {
        draft,
        items,
        jobs: drafts::recent_jobs(pool, 5).await?,
        staging: staging.is_some(),
    })
}

/// Published files whose path contains `search` (at most 200).
pub async fn published_files(
    session: &SessionAccount,
    paths: Option<&StorePaths>,
    search: String,
) -> Result<Vec<PublishedFile>, AdminError> {
    require(session, Permission::Read)?;
    let paths = paths.cloned().ok_or(AdminError::ContentUnavailable)?;
    blocking(move || {
        let (_, manifest) = tree::published(&paths.store)?;
        Ok(tree::search(&manifest, &search, 200)
            .into_iter()
            .map(|f| PublishedFile {
                path: f.path,
                sha256: f.sha256,
                size: f.size,
            })
            .collect())
    })
    .await
}

/// The published version, the path as published (any case) and its published SHA-256.
async fn published_entry(
    paths: &StorePaths,
    path: String,
) -> Result<(u32, String, Option<String>), AdminError> {
    let store_dir = paths.store.clone();
    blocking(move || {
        let path = tree::check_path(&path)?;
        let (pointer, manifest) = tree::published(&store_dir)?;
        let found = manifest
            .files
            .iter()
            .find(|f| f.path.eq_ignore_ascii_case(&path));
        Ok((
            pointer.current,
            found.map_or(path, |f| f.path.clone()),
            found.map(|f| f.sha256.clone()),
        ))
    })
    .await
}

async fn draft_change(
    pool: &PgPool,
    actor: &Actor<'_>,
    current: u32,
    item: ItemRow,
    action: &'static str,
) -> Result<ItemRow, AdminError> {
    let mut tx = pool.begin().await?;
    let draft = drafts::lock_or_create(
        &mut tx,
        i32::try_from(current).map_err(|_| AdminError::InvalidInput)?,
        Some(actor.session.account_id),
    )
    .await?;
    if u32::try_from(draft.base_version).ok() != Some(current) {
        return Err(AdminError::ClientRefused(format!(
            "o rascunho começou na versão {} e a publicada agora é a {current}: descarte-o e refaça",
            draft.base_version
        )));
    }
    let mut record = audit(actor, action, "client_file", &item.path);
    record.after = Some(
        serde_json::json!({ "action": item.action, "sha256": item.sha256, "size": item.size,
                            "before_sha256": item.before_sha256, "draft": draft.id })
        .to_string(),
    );
    insert_once(&mut tx, &record).await?;
    drafts::upsert_item(&mut tx, draft.id, &item).await?;
    tx.commit().await?;
    Ok(item)
}

/// Adds or replaces the file at `path` in the draft (admin). The bytes wait in the staging folder.
pub async fn upload_file(
    pool: &PgPool,
    actor: &Actor<'_>,
    paths: Option<&StorePaths>,
    staging: Option<&Path>,
    path: String,
    data: axum::body::Bytes,
) -> Result<ItemRow, AdminError> {
    require(actor.session, Permission::Administer)?;
    let paths = paths.cloned().ok_or(AdminError::ContentUnavailable)?;
    let staging = staging
        .map(Path::to_path_buf)
        .ok_or(AdminError::ContentUnavailable)?;
    if data.is_empty() {
        return Err(AdminError::InvalidInput);
    }
    let (current, path, before) = published_entry(&paths, path).await?;
    let (sha256, size) = blocking(move || tree::stage(&staging, &data)).await?;
    if before.as_deref() == Some(sha256.as_str()) {
        return Err(AdminError::ClientRefused(format!(
            "{path} já está publicado com este conteúdo"
        )));
    }
    let item = ItemRow {
        path,
        action: "put".into(),
        sha256: Some(sha256),
        size: Some(i64::try_from(size).map_err(|_| AdminError::InvalidInput)?),
        before_sha256: before,
        source: "upload".into(),
    };
    draft_change(pool, actor, current, item, "client.draft_file").await
}

/// Marks a published file for removal in the draft (admin).
pub async fn remove_file(
    pool: &PgPool,
    actor: &Actor<'_>,
    paths: Option<&StorePaths>,
    path: String,
) -> Result<ItemRow, AdminError> {
    require(actor.session, Permission::Administer)?;
    let paths = paths.cloned().ok_or(AdminError::ContentUnavailable)?;
    let (current, path, before) = published_entry(&paths, path).await?;
    let Some(before) = before else {
        return Err(AdminError::ClientRefused(format!(
            "{path} não está na versão publicada"
        )));
    };
    let item = ItemRow {
        path,
        action: "remove".into(),
        sha256: None,
        size: None,
        before_sha256: Some(before),
        source: "upload".into(),
    };
    draft_change(pool, actor, current, item, "client.draft_remove").await
}

/// Takes a path out of the draft (admin).
pub async fn drop_change(pool: &PgPool, actor: &Actor<'_>, path: &str) -> Result<(), AdminError> {
    require(actor.session, Permission::Administer)?;
    let mut tx = pool.begin().await?;
    let draft = drafts::lock_open(&mut tx)
        .await?
        .ok_or(AdminError::NotFound)?;
    if !drafts::delete_item(&mut tx, draft.id, path).await? {
        return Err(AdminError::NotFound);
    }
    insert_once(
        &mut tx,
        &audit(actor, "client.draft_drop", "client_file", path),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Discards the open draft (admin); its staged files stay until a cleanup.
pub async fn discard(pool: &PgPool, actor: &Actor<'_>) -> Result<(), AdminError> {
    require(actor.session, Permission::Administer)?;
    let mut tx = pool.begin().await?;
    let draft = drafts::lock_open(&mut tx)
        .await?
        .ok_or(AdminError::NotFound)?;
    let target = draft.id.to_string();
    insert_once(
        &mut tx,
        &audit(actor, "client.draft_discard", "client_draft", &target),
    )
    .await?;
    drafts::close(&mut tx, draft.id, None).await?;
    tx.commit().await?;
    Ok(())
}

/// Asks the client builder to apply the published panel content to the server's client and put what
/// changed in the draft (admin). Refused while one is queued or running.
pub async fn request_content(pool: &PgPool, actor: &Actor<'_>) -> Result<i64, AdminError> {
    require(actor.session, Permission::Administer)?;
    let mut tx = pool.begin().await?;
    let id = drafts::enqueue(&mut tx, "content", Some(actor.session.account_id))
        .await?
        .ok_or_else(|| {
            AdminError::ClientRefused(
                "o construtor do cliente já está aplicando o conteúdo; espere terminar".into(),
            )
        })?;
    let target = id.to_string();
    insert_once(
        &mut tx,
        &audit(actor, "client.build_content", "client_build_job", &target),
    )
    .await?;
    tx.commit().await?;
    Ok(id)
}

/// The draft's items as store changes.
pub fn changes_of(items: &[ItemRow]) -> Result<Vec<Change>, AdminError> {
    items
        .iter()
        .map(|i| match (i.action.as_str(), &i.sha256, i.size) {
            ("put", Some(sha256), Some(size)) => Ok(Change::Put {
                path: i.path.clone(),
                sha256: sha256.clone(),
                size: u64::try_from(size).map_err(|_| AdminError::InvalidInput)?,
            }),
            ("remove", _, _) => Ok(Change::Remove {
                path: i.path.clone(),
            }),
            _ => Err(AdminError::InvalidInput),
        })
        .collect()
}

/// Publishes the open draft as version N+1 (admin): N plus its changes, deltas from N, signed. A
/// launcher behind it must update before playing (game-launcher.md 4.6).
pub async fn publish(
    pool: &PgPool,
    actor: &Actor<'_>,
    paths: Option<&StorePaths>,
    staging: Option<&Path>,
    title: &str,
    notes: &str,
) -> Result<u32, AdminError> {
    require(actor.session, Permission::Administer)?;
    let paths = paths.cloned().ok_or(AdminError::ContentUnavailable)?;
    let staging = staging
        .map(Path::to_path_buf)
        .ok_or(AdminError::ContentUnavailable)?;
    let (title, notes) = (title.trim().to_owned(), notes.trim().to_owned());
    if title.is_empty() || title.chars().count() > 200 || notes.chars().count() > 4000 {
        return Err(AdminError::InvalidInput);
    }
    let mut tx = pool.begin().await?;
    let draft = drafts::lock_open(&mut tx)
        .await?
        .ok_or(AdminError::NotFound)?;
    let items = drafts::items_in(&mut tx, draft.id).await?;
    let changes = changes_of(&items)?;
    let target = draft.id.to_string();
    let mut record = audit(actor, "client.publish", "client_draft", &target);
    record.after = Some(
        serde_json::json!({ "title": title, "notes": notes, "changes": items.len(),
                            "base_version": draft.base_version })
        .to_string(),
    );
    insert_once(&mut tx, &record).await?;
    let base = u32::try_from(draft.base_version).map_err(|_| AdminError::InvalidInput)?;
    // The draft row stays locked while the store writes: a second publish waits, then finds it closed.
    let report =
        blocking(move || tree::publish_changes(&paths, &staging, base, &changes, &title, &notes))
            .await?;
    drafts::close(
        &mut tx,
        draft.id,
        Some(i32::try_from(report.version).map_err(|_| AdminError::InvalidInput)?),
    )
    .await?;
    tx.commit().await?;
    tracing::info!(
        version = report.version,
        changes = items.len(),
        written = report.objects_written,
        "client version published from a draft"
    );
    Ok(report.version)
}
