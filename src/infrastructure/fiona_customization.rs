//! Internal normal-Fiona inventory then appearance boundary, before skills/lifecycle/TCP.
use super::{
    character_costumes::{self, CostumeError},
    character_lobby::UserKey,
    fiona_appearance, fiona_initializer_profile, fiona_inventory_delivery, fiona_inventory_profile,
    item_catalog, item_colors,
};
use crate::{
    application::inventory_delivery::{Journals, Prepared},
    domain::character_costume::{BodyArithmetic, Costume},
    error::Error,
    protocol::character_creation::Template,
};
use sqlx::PgPool;
use thiserror::Error as ThisError;

pub struct Appearance<'a> {
    pub profile: &'a fiona_appearance::Profile,
    pub inventory: &'a fiona_inventory_profile::Profile,
    pub catalog: &'a item_catalog::Package,
    pub colors: &'a item_colors::Package,
    pub template: &'a Template<'a>,
    pub arithmetic: Option<BodyArithmetic>,
}
#[derive(Debug, ThisError)]
pub enum CustomizationError {
    #[error(transparent)]
    Inventory(#[from] fiona_inventory_delivery::DeliveryError),
    #[error("appearance preparation failed; reconcile the committed inventory before retrying")]
    Appearance(#[source] Error),
    #[error("appearance storage failed; reconcile the committed inventory before retrying")]
    Storage(#[from] CostumeError),
    #[error("customization content binding is invalid")]
    Binding(#[source] Error),
}
pub struct Outcome {
    pub gear: crate::domain::initial_gear::Projection,
    pub inventory: fiona_inventory_delivery::Outcome,
    pub appearance: Vec<Costume>,
    pub costumes_changed: usize,
}
/// No successful character-creation response is produced here. Later skills/callbacks,
/// Login/Logout and Summary must still complete. Errors after inventory commit require
/// reconciliation; this entry point never resumes or replays an interrupted attempt.
pub async fn customize(
    pool: &PgPool,
    key: &UserKey<'_>,
    cid: i64,
    initializer: &fiona_initializer_profile::Profile,
    appearance: Appearance<'_>,
    prepare: impl FnOnce(Journals) -> Result<Prepared, fiona_inventory_delivery::DeliveryError>,
) -> Result<Outcome, CustomizationError> {
    initializer
        .bind(appearance.catalog)
        .map_err(CustomizationError::Binding)?;
    let gear = appearance
        .profile
        .initial_gear(appearance.catalog)
        .map_err(CustomizationError::Binding)?;
    let inventory = fiona_inventory_delivery::deliver(pool, key, cid, initializer, prepare).await?;
    gear.admit_new_inventory(inventory.prepared.inventory())
        .map_err(|_| CustomizationError::Appearance(Error::FionaAppearanceProfile))?;
    // The admitted normal empty Gear has no item-derived costumes. Validation and
    // loading of base/skin/extras/body follow the first SaveWithoutSync and ledgers.
    let rows = appearance
        .profile
        .prepare(
            appearance.template,
            appearance.inventory,
            appearance.catalog,
            appearance.colors,
            appearance.arithmetic,
        )
        .map_err(CustomizationError::Appearance)?;
    let changed = character_costumes::save_first_fiona(pool, key, cid, &rows).await?;
    Ok(Outcome {
        gear,
        inventory,
        appearance: rows,
        costumes_changed: changed,
    })
}
