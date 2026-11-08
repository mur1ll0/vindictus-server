//! "Versões do cliente" of the panel (L5, docs/future/game-launcher.md 4.7): the published client
//! versions of the update store, the minimum version, withdrawing the current one, the launcher
//! published for self-update and the launcher news. The store (signed manifests and pointer) is the
//! source of the versions; PostgreSQL keeps the news; every change is audited.
use crate::application::admin_session::AdminError;
use crate::application::content_admin::{Actor, audit, require};
use crate::domain::admin::Permission;
use crate::infrastructure::admin_store::{SessionAccount, insert_audit};
use crate::infrastructure::client_dist_store::{self as store, StorePaths, StoreView};
use crate::infrastructure::launcher_news::{self, NewsRow};
use serde::Serialize;
use sqlx::PgPool;
use vindictus_client_dist::manifest::{MAX_NEWS, NewsItem};

/// The page: the store's versions and the news.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ClientVersionsView {
    /// None when the store is not configured or has no published version yet.
    pub store: Option<StoreView>,
    /// Why the store could not be read (no secret in it).
    pub store_error: Option<String>,
    pub news: Vec<NewsRow>,
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, store::StoreError> + Send + 'static,
) -> Result<T, AdminError> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|_| AdminError::ContentUnavailable)?
        .map_err(|error| match error {
            store::StoreError::Refused(_) => AdminError::InvalidInput,
            _ => AdminError::ContentUnavailable,
        })
}

fn public_key(paths: &StorePaths) -> Result<String, store::StoreError> {
    Ok(std::fs::read_to_string(
        paths.store.join(vindictus_client_dist::layout::PUBLIC_KEY),
    )?)
}

pub async fn view(
    pool: &PgPool,
    session: &SessionAccount,
    paths: Option<&StorePaths>,
) -> Result<ClientVersionsView, AdminError> {
    require(session, Permission::Read)?;
    let (store, store_error) = match paths.cloned() {
        None => (
            None,
            Some("armazém de atualização não configurado no painel".into()),
        ),
        Some(paths) => {
            match blocking(move || store::list_versions(&paths.store, &public_key(&paths)?)).await {
                Ok(view) => (Some(view), None),
                Err(_) => (
                    None,
                    Some("armazém sem versão publicada ou ilegível".into()),
                ),
            }
        }
    };
    Ok(ClientVersionsView {
        store,
        store_error,
        news: launcher_news::list(pool, 100).await?,
    })
}

/// Changes the oldest version allowed to play.
pub async fn set_minimum(
    pool: &PgPool,
    actor: &Actor<'_>,
    paths: Option<&StorePaths>,
    minimum: u32,
) -> Result<(), AdminError> {
    require(actor.session, Permission::Administer)?;
    let paths = paths.cloned().ok_or(AdminError::ContentUnavailable)?;
    let mut tx = pool.begin().await?;
    let target = minimum.to_string();
    let mut record = audit(actor, "client.minimum", "client_version", &target);
    // The audit row first: a repeated operation never re-signs.
    record.after = Some(serde_json::json!({ "minimum": minimum }).to_string());
    insert_once(&mut tx, &record).await?;
    let (before, _) = blocking(move || store::set_minimum(&paths, minimum)).await?;
    tracing::info!(
        minimum,
        before = before.minimum,
        "client minimum version changed"
    );
    tx.commit().await?;
    Ok(())
}

/// Withdraws the current version (publishes N+1 = N-1).
pub async fn withdraw(
    pool: &PgPool,
    actor: &Actor<'_>,
    paths: Option<&StorePaths>,
) -> Result<u32, AdminError> {
    require(actor.session, Permission::Administer)?;
    let paths = paths.cloned().ok_or(AdminError::ContentUnavailable)?;
    let mut tx = pool.begin().await?;
    let record = audit(actor, "client.withdraw", "client_version", "current");
    insert_once(&mut tx, &record).await?;
    let report = blocking(move || store::withdraw(&paths, "Versão retirada")).await?;
    tx.commit().await?;
    Ok(report.version)
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

/// Re-signs news.json from the visible items, inside the change's transaction (a failure rolls it back).
async fn publish_news(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    paths: Option<&StorePaths>,
) -> Result<(), AdminError> {
    let Some(paths) = paths.cloned() else {
        return Ok(());
    };
    let items: Vec<NewsItem> = launcher_news::visible(tx, MAX_NEWS as i64)
        .await?
        .into_iter()
        .map(|row| NewsItem {
            id: row.id,
            title: row.title,
            body: row.body,
            published_unix: u64::try_from(row.published_unix).unwrap_or(0),
        })
        .collect();
    blocking(move || store::write_news(&paths, items)).await
}

/// A new news item (trimmed; title up to 120 and body up to 2000 characters).
pub async fn add_news(
    pool: &PgPool,
    actor: &Actor<'_>,
    paths: Option<&StorePaths>,
    title: &str,
    body: &str,
) -> Result<i64, AdminError> {
    require(actor.session, Permission::Manage)?;
    let (title, body) = (title.trim(), body.trim());
    let probe = NewsItem {
        id: 0,
        title: title.into(),
        body: body.into(),
        published_unix: 0,
    };
    if !probe.valid() {
        return Err(AdminError::InvalidInput);
    }
    let mut tx = pool.begin().await?;
    let id = launcher_news::insert(&mut tx, title, body, actor.session.account_id).await?;
    let target = id.to_string();
    let mut record = audit(actor, "client.news", "launcher_news", &target);
    record.after = Some(serde_json::json!({ "title": title, "body": body }).to_string());
    insert_once(&mut tx, &record).await?;
    publish_news(&mut tx, paths).await?;
    tx.commit().await?;
    Ok(id)
}

/// Hides (or shows again) a news item.
pub async fn set_news_hidden(
    pool: &PgPool,
    actor: &Actor<'_>,
    paths: Option<&StorePaths>,
    id: i64,
    hidden: bool,
) -> Result<(), AdminError> {
    require(actor.session, Permission::Manage)?;
    let mut tx = pool.begin().await?;
    if !launcher_news::set_hidden(&mut tx, id, hidden).await? {
        return Err(AdminError::NotFound);
    }
    let target = id.to_string();
    let mut record = audit(actor, "client.news_hidden", "launcher_news", &target);
    record.after = Some(serde_json::json!({ "hidden": hidden }).to_string());
    insert_once(&mut tx, &record).await?;
    publish_news(&mut tx, paths).await?;
    tx.commit().await?;
    Ok(())
}
