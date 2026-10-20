//! Explicit private configuration for the operator-authorized minimum laboratory path.
use super::{
    character_lobby::{QueryConfig, UserKey},
    fiona_appearance, fiona_initializer_profile, fiona_inventory_profile, item_catalog,
    item_colors, item_stats,
    minimum_character::{self, MinimumError},
};
use crate::{
    application::character_creation::EntityAuthority,
    domain::{character_costume::BodyArithmetic, item_colors::ColorRandom},
    error::Error,
    protocol::{
        DEFAULT_MAX_BODY,
        character_creation::{self, Kind, Message},
    },
};
use serde::Deserialize;
use std::{
    io::Read,
    path::{Path, PathBuf},
};
use tokio::sync::Mutex;
use zeroize::Zeroizing;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Files {
    format_version: u32,
    catalog: PathBuf,
    stats: PathBuf,
    colors: PathBuf,
    inventory: PathBuf,
    appearance: PathBuf,
    initializer: PathBuf,
    initializer_reference: PathBuf,
    entity_service_id: i32,
    color_seed: i32,
    body_arithmetic: String,
}
pub struct Assets {
    pub initializer: fiona_initializer_profile::Profile,
    catalog: item_catalog::Package,
    stats: item_stats::Package,
    colors: item_colors::Package,
    inventory: fiona_inventory_profile::Profile,
    appearance: fiona_appearance::Profile,
    pub entity_service_id: i32,
    arithmetic: Option<BodyArithmetic>,
    random: Mutex<ColorRandom>,
}
/// Already parsed, source-bound packages; construction still validates all bindings.
pub struct Parts {
    pub initializer: fiona_initializer_profile::Profile,
    pub catalog: item_catalog::Package,
    pub stats: item_stats::Package,
    pub colors: item_colors::Package,
    pub inventory: fiona_inventory_profile::Profile,
    pub appearance: fiona_appearance::Profile,
    pub entity_service_id: i32,
    pub arithmetic: Option<BodyArithmetic>,
    pub color_seed: i32,
}
pub struct Created {
    pub cid: i64,
    pub body: Zeroizing<Vec<u8>>,
}
pub struct Attempt<'a> {
    pub cid: i64,
    pub body: &'a [u8],
    pub offset: i32,
    pub config: QueryConfig,
    pub delay: i32,
}
fn open(p: &Path) -> Result<std::fs::File, MinimumError> {
    std::fs::File::open(p)
        .map_err(Error::from)
        .map_err(MinimumError::from)
}
impl Assets {
    pub fn read(path: &Path) -> Result<Self, MinimumError> {
        let mut bytes = Zeroizing::new(Vec::new());
        open(path)?
            .take(16385)
            .read_to_end(&mut bytes)
            .map_err(Error::from)?;
        if bytes.len() > 16384 {
            return Err(MinimumError::Conflict);
        }
        let f: Files = serde_json::from_slice(&bytes).map_err(|_| MinimumError::Conflict)?;
        if f.format_version != 1 || !(0..=65534).contains(&f.entity_service_id) {
            return Err(MinimumError::Conflict);
        }
        let arithmetic = match f.body_arithmetic.as_str() {
            "measured-x86" => Some(BodyArithmetic::FrameworkX86),
            "measured-x64" => Some(BodyArithmetic::FrameworkX64),
            "unambiguous-only" => None,
            _ => return Err(MinimumError::Conflict),
        };
        Self::from_parts(Parts {
            initializer: fiona_initializer_profile::Profile::read(
                open(&f.initializer)?,
                open(&f.initializer_reference)?,
            )?,
            catalog: item_catalog::read_package(open(&f.catalog)?)?,
            stats: item_stats::Package::read(open(&f.stats)?)?,
            colors: item_colors::Package::read(open(&f.colors)?)?,
            inventory: fiona_inventory_profile::Profile::read(open(&f.inventory)?)?,
            appearance: fiona_appearance::Profile::read(open(&f.appearance)?)?,
            entity_service_id: f.entity_service_id,
            arithmetic,
            color_seed: f.color_seed,
        })
    }
    pub fn from_parts(parts: Parts) -> Result<Self, MinimumError> {
        if !(0..=65534).contains(&parts.entity_service_id)
            || !parts
                .initializer
                .policy()
                .entries()
                .any(|(key, _)| key == "DeleteCharacterDelay")
        {
            return Err(MinimumError::Conflict);
        }
        let out = Self {
            initializer: parts.initializer,
            catalog: parts.catalog,
            stats: parts.stats,
            colors: parts.colors,
            inventory: parts.inventory,
            appearance: parts.appearance,
            entity_service_id: parts.entity_service_id,
            arithmetic: parts.arithmetic,
            random: Mutex::new(ColorRandom::from_seed(parts.color_seed)),
        };
        out.initializer.bind(&out.catalog)?;
        out.inventory.bind(&out.catalog)?;
        out.appearance.bind(&out.catalog)?;
        out.stats.bind(&out.catalog)?;
        out.colors.bind(&out.catalog)?;
        Ok(out)
    }
    pub async fn initialize_and_list(
        &self,
        pool: &sqlx::PgPool,
        owner: &UserKey<'_>,
        authority: &mut EntityAuthority,
        attempt: Attempt<'_>,
    ) -> Result<Created, MinimumError> {
        let Attempt {
            cid,
            body,
            offset,
            config,
            delay,
        } = attempt;
        let Some(Message::Create {
            template: Some(template),
            preset: false,
            premium: false,
            ..
        }) = character_creation::decode(Kind::Create, body, DEFAULT_MAX_BODY)
            .map_err(|_| MinimumError::Conflict)?
        else {
            return Err(MinimumError::Conflict);
        };
        let mut random = self.random.lock().await;
        let mut utc = || super::lab_clock::utc_now().map_err(|_| Error::LegacyClockOverflow);
        let mut civil =
            || super::lab_clock::civil_now(offset).map_err(|_| Error::LegacyClockOverflow);
        let done = minimum_character::initialize(
            pool,
            owner,
            cid,
            minimum_character::Inputs {
                initializer: &self.initializer,
                appearance: super::fiona_customization::Appearance {
                    profile: &self.appearance,
                    inventory: &self.inventory,
                    catalog: &self.catalog,
                    colors: &self.colors,
                    template: &template,
                    arithmetic: self.arithmetic,
                },
                stats: &self.stats,
            },
            authority,
            &mut random,
            minimum_character::Clock {
                utc: &mut utc,
                civil: &mut civil,
            },
        )
        .await?;
        if done.cid() != cid {
            return Err(MinimumError::Conflict);
        }
        let body = super::minimum_character_list::query(
            pool,
            owner,
            &self.initializer,
            config,
            delay,
            || super::lab_clock::utc_now().map_err(|_| MinimumError::Conflict),
        )
        .await?;
        Ok(Created { cid, body })
    }
}
