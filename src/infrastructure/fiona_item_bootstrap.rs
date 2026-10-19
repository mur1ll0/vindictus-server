//! Unpublished initial ItemOwner prefix,ending before Active/mail timeline.
//! No complete ItemOwner,LoadPlayer,Undo,Release or Create success is returned.
use super::{
    character_lobby::UserKey, fiona_appearance, fiona_character_state as state,
    fiona_initializer_profile::Profile, fiona_title_preset::TitlesJoined, item_catalog::Package,
    item_journals, item_loader_state,
};
use crate::{
    application::empty_item_save::{self, Effect, Flow},
    domain::{
        initial_gear::Projection,
        item_logs::Manager,
        legacy_user_data::{SqlDateTime, SqlDateTime2},
    },
};
use sqlx::PgPool;
use std::{
    collections::{BTreeMap, VecDeque},
    convert::Infallible,
    sync::{Arc, Mutex, OnceLock, Weak},
    time::Duration,
};
#[derive(Debug, thiserror::Error)]
pub enum BootstrapError {
    #[error(transparent)]
    Owner(#[from] state::StateError),
    #[error(transparent)]
    Loader(#[from] item_loader_state::LoaderError),
    #[error("initial item bootstrap database operation failed")]
    Database(#[from] sqlx::Error),
    #[error("initial item bootstrap profile,bindings or fresh state conflict")]
    Conflict,
    #[error("initial item bootstrap clock failed")]
    Clock,
    #[error("empty Save failed;full Undo/reload is required and not completed by this prefix")]
    ReloadRequired,
    #[error("initial item bootstrap timeout;reconcile independent commits before retry")]
    Timeout,
}
struct Service {
    queue: Mutex<VecDeque<Arc<Item>>>,
}
struct Entity {
    cid: i64,
    tag: OnceLock<Weak<Player>>,
}
struct Player {
    uid: i32,
    entity: Arc<Entity>,
    service: Arc<Service>,
    character: OnceLock<Arc<Character>>,
    item: OnceLock<Arc<Item>>,
    frontend: Option<Infallible>,
}
struct CharacterImpl {
    entity: Weak<Entity>,
    player: Weak<Player>,
    service: Weak<Service>,
}
struct Character {
    joined: Mutex<TitlesJoined>,
    login_time: SqlDateTime2,
    implementation: CharacterImpl,
    entity: Weak<Entity>,
}
struct Shared {
    owner: Weak<Item>,
    uid: i32,
    inventory: BTreeMap<u8, Vec<Infallible>>,
    tabs: BTreeMap<u8, Infallible>,
    items: BTreeMap<i64, Infallible>,
    diff: Vec<Infallible>,
    changed_items: BTreeMap<(u8, i32), Infallible>,
    changed_tabs: BTreeMap<u8, Infallible>,
    disabled: bool,
    updated: usize,
}
struct ItemImpl {
    entity: Weak<Entity>,
    service: Weak<Service>,
}
struct Item {
    entity: Arc<Entity>,
    service: Weak<Service>,
    player: Weak<Player>,
    latest_sort: SqlDateTime,
    implementation: ItemImpl,
    shared: OnceLock<Shared>,
    active: bool,
    transaction_scope: Mutex<Option<&'static str>>,
    manager: Mutex<Manager>,
}
/// Empty bags have allocated slot arrays;other four stores have their original empty maps.
pub struct Storage {
    bags: BTreeMap<u8, [Option<i64>; 48]>,
    restore: BTreeMap<i32, Infallible>,
    account: BTreeMap<Vec<u8>, Infallible>,
    cash: BTreeMap<i32, Infallible>,
    story: BTreeMap<i32, Infallible>,
}
impl Storage {
    fn initial() -> Self {
        Self {
            bags: [0, 1, 6].into_iter().map(|i| (i, [None; 48])).collect(),
            restore: BTreeMap::new(),
            account: BTreeMap::new(),
            cash: BTreeMap::new(),
            story: BTreeMap::new(),
        }
    }
    pub fn bag_slots(&self) -> usize {
        self.bags.values().map(|v| v.len()).sum()
    }
    pub fn other_stores(&self) -> [usize; 4] {
        [
            self.restore.len(),
            self.account.len(),
            self.cash.len(),
            self.story.len(),
        ]
    }
}
struct Manufacture {
    owner: Arc<Item>,
    experience: BTreeMap<Vec<u8>, Infallible>,
    recipes: Vec<Vec<u8>>,
}
#[must_use = "ItemOwner remains unpublished;mail/shared/pets and the rest of LoadPlayer are still required"]
pub struct PendingMail {
    service: Arc<Service>,
    entity: Arc<Entity>,
    player: Arc<Player>,
    character: Arc<Character>,
    item: Arc<Item>,
    gear: Projection,
    storage: Storage,
    manufacture: Manufacture,
    new_recipes: BTreeMap<Vec<u8>, Infallible>,
    operation_journal: i64,
    manufacture_journal: i64,
    deleted: Option<Vec<i64>>,
    log_write_calls: usize,
    sync_drained: usize,
}
impl PendingMail {
    pub fn cid(&self) -> i64 {
        self.entity.cid
    }
    pub fn uid(&self) -> i32 {
        self.player.uid
    }
    pub fn login_time(&self) -> SqlDateTime2 {
        self.character.login_time
    }
    pub fn latest_sort(&self) -> SqlDateTime {
        self.item.latest_sort
    }
    pub fn operation_journal(&self) -> i64 {
        self.operation_journal
    }
    pub fn manufacture_journal(&self) -> i64 {
        self.manufacture_journal
    }
    pub fn gear(&self) -> &Projection {
        &self.gear
    }
    pub fn storage(&self) -> &Storage {
        &self.storage
    }
    pub fn shared_cache_counts(&self) -> Option<[usize; 7]> {
        self.item.shared.get().map(|v| {
            [
                v.inventory.len(),
                v.tabs.len(),
                v.items.len(),
                v.diff.len(),
                v.changed_items.len(),
                v.changed_tabs.len(),
                v.updated,
            ]
        })
    }
    pub fn shared_initialized(&self) -> bool {
        self.item.shared.get().is_some()
    }
    pub fn item_published(&self) -> bool {
        self.player.item.get().is_some()
    }
    pub fn active(&self) -> bool {
        self.item.active
    }
    pub fn manufacture_counts(&self) -> [usize; 3] {
        [
            self.manufacture.experience.len(),
            self.manufacture.recipes.len(),
            self.new_recipes.len(),
        ]
    }
    pub fn deleted_count(&self) -> Option<usize> {
        self.deleted.as_ref().map(Vec::len)
    }
    pub fn transaction_scope(&self) -> Result<Option<&'static str>, BootstrapError> {
        Ok(*self
            .item
            .transaction_scope
            .lock()
            .map_err(|_| BootstrapError::Conflict)?)
    }
    pub fn log_write_calls(&self) -> usize {
        self.log_write_calls
    }
    pub fn sync_drained(&self) -> usize {
        self.sync_drained
    }
    pub fn categories(&self) -> Result<Vec<(i64, &'static str)>, BootstrapError> {
        let joined = self
            .character
            .joined
            .lock()
            .map_err(|_| BootstrapError::Conflict)?;
        let obs = joined.prior().prior().observables();
        obs.categories()
            .iter()
            .map(|c| {
                obs.entity_for_category(c)
                    .map(|id| (id, *c))
                    .ok_or(BootstrapError::Conflict)
            })
            .collect()
    }
    pub fn pending_sync(&self) -> Result<usize, BootstrapError> {
        Ok(self
            .service
            .queue
            .lock()
            .map_err(|_| BootstrapError::Conflict)?
            .len())
    }
    /// Fresh creation has no frontend for this prefix's entire lifetime. Draining the
    /// actual queued owners reaches Sync's outer false guard and performs no game work.
    pub fn drain_detached_sync(&mut self) -> Result<usize, BootstrapError> {
        if !self.bindings_valid() || self.player.frontend.is_some() {
            return Err(BootstrapError::Conflict);
        }
        let jobs: Vec<_> = self
            .service
            .queue
            .lock()
            .map_err(|_| BootstrapError::Conflict)?
            .drain(..)
            .collect();
        for job in &jobs {
            if !Arc::ptr_eq(job, &self.item) {
                return Err(BootstrapError::Conflict);
            }
        }
        self.sync_drained += jobs.len();
        Ok(jobs.len())
    }
    pub fn bindings_valid(&self) -> bool {
        self.entity
            .tag
            .get()
            .and_then(Weak::upgrade)
            .is_some_and(|p| Arc::ptr_eq(&p, &self.player))
            && Arc::ptr_eq(&self.player.entity, &self.entity)
            && Arc::ptr_eq(&self.player.service, &self.service)
            && self
                .player
                .character
                .get()
                .is_some_and(|c| Arc::ptr_eq(c, &self.character))
            && self
                .character
                .entity
                .upgrade()
                .is_some_and(|e| Arc::ptr_eq(&e, &self.entity))
            && self
                .character
                .implementation
                .entity
                .upgrade()
                .is_some_and(|e| Arc::ptr_eq(&e, &self.entity))
            && self
                .character
                .implementation
                .player
                .upgrade()
                .is_some_and(|p| Arc::ptr_eq(&p, &self.player))
            && self
                .character
                .implementation
                .service
                .upgrade()
                .is_some_and(|s| Arc::ptr_eq(&s, &self.service))
            && Arc::ptr_eq(&self.item.entity, &self.entity)
            && self
                .item
                .service
                .upgrade()
                .is_some_and(|s| Arc::ptr_eq(&s, &self.service))
            && self
                .item
                .player
                .upgrade()
                .is_some_and(|p| Arc::ptr_eq(&p, &self.player))
            && Arc::ptr_eq(&self.manufacture.owner, &self.item)
            && self
                .item
                .implementation
                .entity
                .upgrade()
                .is_some_and(|e| Arc::ptr_eq(&e, &self.entity))
            && self
                .item
                .implementation
                .service
                .upgrade()
                .is_some_and(|s| Arc::ptr_eq(&s, &self.service))
            && self.item.shared.get().is_none_or(|v| {
                v.uid == self.uid()
                    && !v.disabled
                    && v.owner
                        .upgrade()
                        .is_some_and(|o| Arc::ptr_eq(&o, &self.item))
            })
    }
}
/// Original queued Service.Thread branch. This is a closed unpublished creation
/// context;it is not a connected entity service or a reusable replay entry point.
pub async fn load_prefix<E>(
    pool: &PgPool,
    owner: &UserKey<'_>,
    joined: TitlesJoined,
    profile: &Profile,
    catalog: &Package,
    appearance: &fiona_appearance::Profile,
    mut utc: impl FnMut() -> Result<SqlDateTime2, E>,
) -> Result<PendingMail, BootstrapError> {
    tokio::time::timeout(Duration::from_secs(40), async {
        profile
            .bind(catalog)
            .map_err(|_| BootstrapError::Conflict)?;
        let root = joined.prior().prior().root();
        let cid = root.cid();
        if !empty_item_save::is_character_entity(cid)
            || root.uid() != owner.uid
            || root.profile_fingerprint() != profile.fingerprint()
            || joined.prior().prior().observables().categories()
                != ["Stat", "Skill", "Title", "MicroPlay", "Fishing"]
        {
            return Err(BootstrapError::Conflict);
        }
        state::verify_source_binding(pool, profile.source_state()).await?;
        let tx = state::owned_first_fiona(pool, owner, cid).await?;
        tx.commit().await?;
        let enabled = |k: &str| -> Result<bool, BootstrapError> {
            if !profile.policy().entries().any(|(n, _)| n == k) {
                return Err(BootstrapError::Conflict);
            }
            profile
                .policy()
                .is_enabled(Some(k))
                .map_err(|_| BootstrapError::Conflict)
        };
        let disable_operation = enabled("DisableOperationLogging")?;
        let disabled = enabled("DisableLogging")?;
        let shared = enabled("SharedInventory")?;
        let auto = enabled("AutoItemLogging")?;
        let delete = enabled("ProcessAfterDeleteItem")?;
        let login_time = utc().map_err(|_| BootstrapError::Clock)?;
        let service = Arc::new(Service {
            queue: Mutex::new(VecDeque::new()),
        });
        let entity = Arc::new(Entity {
            cid,
            tag: OnceLock::new(),
        });
        let player = Arc::new(Player {
            uid: owner.uid,
            entity: entity.clone(),
            service: service.clone(),
            character: OnceLock::new(),
            frontend: None,
            item: OnceLock::new(),
        });
        entity
            .tag
            .set(Arc::downgrade(&player))
            .map_err(|_| BootstrapError::Conflict)?;
        let character = Arc::new(Character {
            joined: Mutex::new(joined),
            login_time,
            implementation: CharacterImpl {
                entity: Arc::downgrade(&entity),
                player: Arc::downgrade(&player),
                service: Arc::downgrade(&service),
            },
            entity: Arc::downgrade(&entity),
        });
        player
            .character
            .set(character.clone())
            .map_err(|_| BootstrapError::Conflict)?;
        let operation_journal = item_journals::get_item_journal_id(pool, disable_operation).await;
        let mut manager = Manager::default();
        manager.set_journal(operation_journal, 124);
        let item = Arc::new(Item {
            entity: entity.clone(),
            service: Arc::downgrade(&service),
            player: Arc::downgrade(&player),
            latest_sort: SqlDateTime::from_units300(SqlDateTime::MIN)
                .map_err(|_| BootstrapError::Conflict)?,
            manager: Mutex::new(manager),
            implementation: ItemImpl {
                entity: Arc::downgrade(&entity),
                service: Arc::downgrade(&service),
            },
            shared: OnceLock::new(),
            active: false,
            transaction_scope: Mutex::new(None),
        });
        if !character
            .joined
            .lock()
            .map_err(|_| BootstrapError::Conflict)?
            .register_items(owner.uid, shared)
        {
            return Err(BootstrapError::Conflict);
        }
        if shared {
            item.shared
                .set(Shared {
                    owner: Arc::downgrade(&item),
                    uid: owner.uid,
                    inventory: BTreeMap::new(),
                    tabs: BTreeMap::new(),
                    items: BTreeMap::new(),
                    diff: Vec::new(),
                    changed_items: BTreeMap::new(),
                    changed_tabs: BTreeMap::new(),
                    disabled: false,
                    updated: 1,
                })
                .map_err(|_| BootstrapError::Conflict)?;
        }
        // SharedItemManager ctor only initializes maps/observable;its provider load follows mail.
        let mut tx = state::owned_first_fiona(pool, owner, cid).await?;
        let collection: Option<i32> = sqlx::query_scalar(
            "SELECT permission FROM game_item_collections WHERE owner_id=$1 FOR SHARE",
        )
        .bind(cid)
        .fetch_optional(&mut *tx)
        .await?;
        let mailbox: Option<i32> =
            sqlx::query_scalar("SELECT permission FROM game_mailboxes WHERE owner_id=$1 FOR SHARE")
                .bind(cid)
                .fetch_optional(&mut *tx)
                .await?;
        if collection != Some(3) || mailbox != Some(3) {
            return Err(BootstrapError::Conflict);
        }
        item_loader_state::require_slots_absent(&mut tx, cid).await?;
        let gear = appearance
            .initial_gear(catalog)
            .map_err(|_| BootstrapError::Conflict)?;
        sqlx::query(
            "LOCK TABLE game_items,game_item_storage_status,game_item_identity IN SHARE MODE",
        )
        .execute(&mut *tx)
        .await?;
        let ready: bool =
            sqlx::query_scalar("SELECT initialized FROM game_item_identity WHERE singleton")
                .fetch_one(&mut *tx)
                .await?;
        if !ready {
            return Err(BootstrapError::Conflict);
        }
        let storage = Storage::initial();
        *item
            .transaction_scope
            .lock()
            .map_err(|_| BootstrapError::Conflict)? = Some("ItemsLoad");
        for q in [
            "SELECT EXISTS(SELECT 1 FROM game_item_storage_status WHERE owner_id=$1)",
            "SELECT EXISTS(SELECT 1 FROM game_items WHERE owner_id=$1)",
        ] {
            if sqlx::query_scalar::<_, bool>(q)
                .bind(cid)
                .fetch_one(&mut *tx)
                .await?
            {
                return Err(BootstrapError::Conflict);
            }
        }
        // Empty Submit is unconditional. Failure cannot be reported as a completed Undo.
        let accepted = match tx.commit().await {
            Ok(()) => true,
            Err(_) => {
                tracing::warn!("empty item Submit failed;full reload required");
                false
            }
        };
        let mut deleted = None;
        let mut log_write_calls = 0;
        let mut effect_error = false;
        let flow = empty_item_save::after_submit(accepted, true, |effect| match effect {
            Effect::QueueSync => match service.queue.lock() {
                Ok(mut q) => {
                    if q.is_empty() {
                        q.push_back(item.clone())
                    } else {
                        effect_error = true
                    }
                }
                Err(_) => effect_error = true,
            },
            Effect::Sync => effect_error = true,
            Effect::ProcessAfterDelete => {
                if delete {
                    deleted = Some(Vec::new())
                }
            }
            Effect::WriteLogs => {
                log_write_calls += 1;
                if auto {
                    match item.manager.lock() {
                        Ok(mut m) => m.clear(),
                        Err(_) => effect_error = true,
                    }
                }
            }
        });
        *item
            .transaction_scope
            .lock()
            .map_err(|_| BootstrapError::Conflict)? = None;
        if flow == Flow::ReloadRequired {
            return Err(BootstrapError::ReloadRequired);
        }
        if effect_error {
            return Err(BootstrapError::Conflict);
        }
        let manufacture_journal = item_journals::add_manufacture_journal(pool, disabled).await;
        let mut tx = state::owned_first_fiona(pool, owner, cid).await?;
        item_loader_state::lock_initialized(&mut tx).await?;
        // Actual association providers follow source order after the independent Save.
        for q in [
            "SELECT EXISTS(SELECT 1 FROM game_manufacture WHERE cid=$1)",
            "SELECT EXISTS(SELECT 1 FROM game_manufacture_recipes WHERE cid=$1)",
            "SELECT EXISTS(SELECT 1 FROM game_new_recipes WHERE cid=$1)",
        ] {
            if sqlx::query_scalar::<_, bool>(q)
                .bind(cid)
                .fetch_one(&mut *tx)
                .await?
            {
                return Err(BootstrapError::Conflict);
            }
        }
        tx.commit().await?;
        let manufacture = Manufacture {
            owner: item.clone(),
            experience: BTreeMap::new(),
            recipes: Vec::new(),
        };
        Ok(PendingMail {
            service,
            entity,
            player,
            character,
            item,
            gear,
            storage,
            manufacture,
            new_recipes: BTreeMap::new(),
            operation_journal,
            manufacture_journal,
            deleted,
            log_write_calls,
            sync_drained: 0,
        })
    })
    .await
    .map_err(|_| BootstrapError::Timeout)?
}
