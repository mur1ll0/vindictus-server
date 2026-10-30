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
    map_partition_dir: Option<PathBuf>,
    channel_bind: Option<std::net::SocketAddr>,
    /// Loopback RelayServer endpoint for mission entry (also the advertised address).
    #[serde(default)]
    relay_bind: Option<std::net::SocketAddr>,
    equipment: Option<PathBuf>,
    skills: Option<PathBuf>,
    economy: Option<PathBuf>,
    npc_scripts: Option<PathBuf>,
    #[serde(default)]
    enhance: Option<PathBuf>,
    #[serde(default)]
    random_item: Option<PathBuf>,
    /// DyeAmpleSession sidecar (DyeCashCursor and dye.displace).
    #[serde(default)]
    dye_ample: Option<PathBuf>,
    #[serde(default)]
    cash_shop: Option<PathBuf>,
    /// Per-class DefaultItemInfo, INNERARMOR and CharacterClassInfo (other classes).
    #[serde(default)]
    class_initial: Option<PathBuf>,
    /// Initializer profiles of the other classes.
    #[serde(default)]
    class_initializers: Vec<ClassFiles>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ClassFiles {
    character_class: u8,
    initializer: PathBuf,
    initializer_reference: PathBuf,
}
/// The loaded assets as one swappable value: a reader takes a snapshot, a live tuning
/// (B4.5b) replaces it whole.
pub struct LiveAssets(std::sync::RwLock<std::sync::Arc<Assets>>);

impl LiveAssets {
    pub fn new(assets: Assets) -> Self {
        Self(std::sync::RwLock::new(std::sync::Arc::new(assets)))
    }
    pub fn current(&self) -> std::sync::Arc<Assets> {
        match self.0.read() {
            Ok(guard) => std::sync::Arc::clone(&guard),
            Err(poisoned) => std::sync::Arc::clone(&poisoned.into_inner()),
        }
    }
    pub fn replace(&self, assets: Assets) {
        let next = std::sync::Arc::new(assets);
        match self.0.write() {
            Ok(mut guard) => *guard = next,
            Err(poisoned) => *poisoned.into_inner() = next,
        }
    }
}

pub struct Assets {
    pub(super) equipment: Option<super::equipment_profile::Profile>,
    /// Skill contents with features evaluated against `catalog`.
    pub(super) skills:
        Option<std::collections::BTreeMap<String, crate::domain::skill_list::SkillContent>>,
    /// The Fiona initializer; its FeatureMatrix also serves class-independent policy.
    pub initializer: fiona_initializer_profile::Profile,
    /// Initializer profiles of the other classes that can be created.
    pub(super) class_initializers:
        std::collections::BTreeMap<u8, fiona_initializer_profile::Profile>,
    pub(super) catalog: item_catalog::Package,
    pub(super) stats: item_stats::Package,
    /// `stats` bound to `catalog` once; lookups must not rebind per request.
    pub(super) stats_index: item_stats::StatsIndex,
    colors: item_colors::Package,
    /// Effective equipment material names, bound once with `catalog`.
    pub(super) color_index: item_colors::ColorIndex,
    /// ItemClassInfo economy rows bound to `catalog` (Unique, ExpireIn, prices).
    pub(super) economy: Option<super::item_economy::Profile>,
    /// ItemContents pet dictionaries, built on first use: the PetActiveTimePeriodInfo
    /// features need the NPC FeatureMatrix (region atoms).
    pets: std::sync::OnceLock<Option<crate::domain::pet::Contents>>,
    /// NpcTalkInfo defaults and storylines bound to `catalog`.
    pub(super) npc: Option<super::npc_scripts::Profile>,
    /// Enhance dictionaries and EnhanceType bound to `catalog`.
    pub(super) enhance: Option<super::item_enhance::Contents>,
    /// RandomItemContents bound to `catalog`.
    pub(super) random_item: Option<super::random_item::Contents>,
    /// DyeAmpleSession contents bound to `catalog`.
    pub(super) dye: Option<super::minimum_dye::Contents>,
    /// RandomItemManager's static Random.
    pub(super) random_item_random: Mutex<ColorRandom>,
    /// Laboratory cash shop catalog bound to `catalog`.
    pub(super) cash_shop: Option<super::cash_shop::Contents>,
    /// Framework arithmetic of item stats (the runtime's identity profile); `None`
    /// evaluates both measured profiles and refuses a disagreement.
    pub(super) item_arithmetic: Option<crate::domain::character_stats::Arithmetic>,
    /// The processor's static EnhanceRandom/random (new Random(): time-seeded).
    pub(super) enhance_random: Mutex<super::minimum_enhance::EnhanceRandom>,
    /// The enchant session/dice/arg static Random instances.
    pub(super) enchant_random: Mutex<super::minimum_enchant::EnchantRandom>,
    /// SkillEnhanceSession.RandomObj (static, time-seeded).
    pub(super) skill_enhance_random: Mutex<ColorRandom>,
    /// Seed of ProcessProduct's `new Random()` (None: time-seeded per craft).
    pub(super) craft_seed: Option<i32>,
    /// shopInfoDic with PriceCalc prices for the configured arithmetic profile.
    pub(super) shops: Option<crate::domain::shop::Shops>,
    /// GetEquipItemInfo().EquipClass of each effective equipment class.
    pub(super) equip_class: std::collections::BTreeMap<Vec<u8>, Option<String>>,
    pub(super) inventory: fiona_inventory_profile::Profile,
    pub(super) appearance: fiona_appearance::Profile,
    pub entity_service_id: i32,
    pub(super) arithmetic: Option<BodyArithmetic>,
    pub(super) random: Mutex<ColorRandom>,
    pub channel: Option<(std::net::SocketAddr, crate::domain::map_partition::Topology)>,
    /// RelayServer endpoint (loopback laboratory).
    pub relay: Option<std::net::SocketAddr>,
    /// The published content version applied over these assets (0: none).
    pub content_version: i32,
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
/// Environment.TickCount-like seed of `new Random()`.
fn time_seed() -> i32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i32)
}
fn enhance_random(seed: i32) -> super::minimum_enhance::EnhanceRandom {
    super::minimum_enhance::EnhanceRandom {
        roll: ColorRandom::from_seed(seed),
        other: ColorRandom::from_seed(seed.wrapping_add(1)),
    }
}
fn enchant_random(seed: i32) -> super::minimum_enchant::EnchantRandom {
    super::minimum_enchant::EnchantRandom {
        session: ColorRandom::from_seed(seed.wrapping_add(2)),
        dice: ColorRandom::from_seed(seed.wrapping_add(3)),
        arg: ColorRandom::from_seed(seed.wrapping_add(4)),
    }
}
fn open(p: &Path) -> Result<std::fs::File, MinimumError> {
    std::fs::File::open(p)
        .map_err(Error::from)
        .map_err(MinimumError::from)
}
impl Assets {
    /// The initializer profiles by BaseCharacter.
    pub fn initializers(&self) -> fiona_initializer_profile::Initializers<'_> {
        fiona_initializer_profile::Initializers {
            fiona: &self.initializer,
            others: &self.class_initializers,
        }
    }
    /// The initializer profile of a BaseCharacter.
    pub fn initializer_for(
        &self,
        class: u8,
    ) -> Result<&fiona_initializer_profile::Profile, MinimumError> {
        self.initializers().get(class).ok_or(MinimumError::Conflict)
    }
    /// Whether CreateCharacter has every initial content of this BaseCharacter.
    pub fn supports_class(&self, class: u8) -> bool {
        self.initializers().get(class).is_some()
            && self.inventory.has_class(class)
            && self.appearance.has_class(class)
    }
    /// The effective NPC shops (ShopInfo after PriceCalc), when the NPC scripts are loaded.
    pub fn shops(&self) -> Option<&crate::domain::shop::Shops> {
        self.shops.as_ref()
    }
    /// The cash shop catalog, when loaded.
    pub fn cash_shop(&self) -> Option<&super::cash_shop::Contents> {
        self.cash_shop.as_ref()
    }
    /// Whether a FeatureMatrix expression (ShopInfo.Feature and the like) is enabled in the
    /// loaded region; no NPC scripts or an unreadable expression count as disabled.
    pub fn feature_enabled(&self, feature: Option<&str>) -> bool {
        self.npc
            .as_ref()
            .is_some_and(|n| n.policy.is_enabled(feature).unwrap_or(false))
    }
    /// Whether the item catalog has this ItemClass.
    pub fn has_item_class(&self, class: &str) -> bool {
        let key: Vec<u8> = class.encode_utf16().flat_map(u16::to_le_bytes).collect();
        self.stats_index.class(Some(&key)).is_some()
    }
    /// CharacterContents.StatusEffectGroupInfo of the cash-shop profile, when loaded.
    pub fn status_effects(&self) -> Option<&crate::domain::status_effect::Contents> {
        self.cash_shop
            .as_ref()
            .map(|c| &c.customize().status_effects)
    }
    /// FeatureMatrix values of the NPC script profile.
    pub fn npc_policy(&self) -> Option<&crate::domain::feature_policy::FeaturePolicy> {
        self.npc.as_ref().map(|n| &n.policy)
    }
    /// ItemContents pet dictionaries (economy export) with the PetActiveTimePeriodInfo rows
    /// enabled by the NPC FeatureMatrix; None without either or when the rows conflict.
    pub fn pets(&self) -> Option<&crate::domain::pet::Contents> {
        self.pets
            .get_or_init(|| {
                let export = self.economy.as_ref()?.pets()?;
                let policy = self.npc_policy()?;
                let built = crate::domain::pet::Contents::new(export.clone(), |f| {
                    policy.is_enabled(f).ok()
                });
                if built.is_err() {
                    tracing::warn!("pet content refused; values omitted");
                }
                built.ok()
            })
            .as_ref()
    }
    /// ShopList.aspx of the laboratory cash shop, when exported.
    pub fn cash_shop_list(&self, per_category: usize) -> Option<String> {
        self.cash_shop.as_ref().map(|c| c.shop_list(per_category))
    }
    /// Mission entry contents of the NPC script profile, when exported.
    /// Enhance/enchant contents, when exported.
    pub fn enhance_contents(&self) -> Option<&super::item_enhance::Contents> {
        self.enhance.as_ref()
    }
    pub fn mission(&self) -> Option<&crate::domain::mission::MissionContents> {
        self.npc.as_ref().and_then(|n| n.mission.as_ref())
    }
    pub fn read(path: &Path) -> Result<Self, MinimumError> {
        Self::read_with_overlay(path, &crate::domain::content_overlay::Overlay::default())
    }
    /// The assets with the published panel content edits over the exported shops and cash
    /// shop prices (B4.4c: a published version takes effect when the server starts).
    pub fn read_with_overlay(
        path: &Path,
        overlay: &crate::domain::content_overlay::Overlay,
    ) -> Result<Self, MinimumError> {
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
        let mut assets = Self::from_parts(Parts {
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
        })?;
        let mut class_stats = Vec::new();
        if let Some(path) = &f.class_initial {
            let package = super::class_initial::Package::read(open(path)?)?;
            package.bind(&assets.catalog)?;
            for (class, initial) in package.classes {
                if class == 1 {
                    continue;
                }
                assets.inventory.add_class(class, initial.requests);
                assets
                    .appearance
                    .add_class(class, initial.inner, initial.inner_colors);
                class_stats.push((class, initial.class_stats));
            }
        }
        for c in &f.class_initializers {
            let profile = fiona_initializer_profile::Profile::read(
                open(&c.initializer)?,
                open(&c.initializer_reference)?,
            )?;
            profile.bind(&assets.catalog)?;
            if c.character_class == 1
                || profile.character_class() != c.character_class
                || assets
                    .class_initializers
                    .insert(c.character_class, profile)
                    .is_some()
            {
                return Err(MinimumError::Conflict);
            }
        }
        match (f.map_partition_dir, f.channel_bind) {
            (Some(dir), Some(bind)) if bind.ip().is_loopback() => {
                let mut paths = std::fs::read_dir(dir)
                    .map_err(Error::from)?
                    .map(|p| p.map(|p| p.path()))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(Error::from)?;
                paths.retain(|p| p.extension().is_some_and(|e| e == "dat"));
                paths.sort();
                if paths.len() != 8 {
                    return Err(MinimumError::Conflict);
                }
                let mut maps = Vec::new();
                for path in paths {
                    let mut bytes = Vec::new();
                    open(&path)?
                        .take((crate::domain::map_partition::MAX_MAP_FILE_BYTES + 1) as u64)
                        .read_to_end(&mut bytes)
                        .map_err(Error::from)?;
                    let bsp = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .ok_or(MinimumError::Conflict)?;
                    maps.push(
                        crate::domain::map_partition::MapFile::parse(bsp, &bytes)
                            .map_err(|_| MinimumError::Conflict)?,
                    );
                }
                assets.channel = Some((
                    bind,
                    crate::domain::map_partition::Topology::from_maps(&maps),
                ));
            }
            (None, None) => {}
            _ => return Err(MinimumError::Conflict),
        }
        assets.content_version = overlay.version;
        match f.relay_bind {
            Some(bind) if bind.ip().is_loopback() && bind.port() != 0 => assets.relay = Some(bind),
            None => {}
            Some(_) => return Err(MinimumError::Conflict),
        }
        if let Some(path) = f.equipment {
            assets =
                assets.with_equipment(super::equipment_profile::Profile::read(open(&path)?)?)?;
        }
        if let Some(path) = f.skills {
            assets = assets.with_skills(super::skill_profile::Profile::read(open(&path)?)?)?;
        }
        if let Some(path) = f.economy {
            assets = assets.with_economy(super::item_economy::Profile::read(open(&path)?)?)?;
        }
        if let Some(path) = f.npc_scripts {
            let mut profile = super::npc_scripts::Profile::read(open(&path)?)?;
            profile.shop_rows = overlay.apply_shop_rows(std::mem::take(&mut profile.shop_rows));
            // B4.5: FeatureMatrix rates and the quests' drop counts.
            for (key, value) in &overlay.features {
                profile.policy.override_value(key, value.to_string());
            }
            if let Some(mission) = profile.mission.as_mut() {
                for (quest, [min, max]) in &overlay.quest_drops {
                    mission.set_drop_counts(quest, *min, *max);
                }
            }
            if let Some(progression) = profile.progression.as_mut() {
                for (class, rows) in std::mem::take(&mut class_stats) {
                    progression.set_class_stats(class, rows);
                }
            }
            assets = assets.with_npc_scripts(profile)?;
        }
        if let Some(path) = f.enhance {
            assets = assets.with_enhance(super::item_enhance::Profile::read(open(&path)?)?)?;
            assets.enhance = assets.enhance.take().map(|e| e.with_overlay(overlay));
        }
        if let Some(path) = f.random_item {
            assets = assets.with_random_item(super::random_item::Profile::read(open(&path)?)?)?;
        }
        if let Some(path) = f.dye_ample {
            assets = assets.with_dye(super::minimum_dye::Profile::read(open(&path)?)?)?;
        }
        if let Some(path) = f.cash_shop {
            assets = assets.with_cash_shop(
                super::cash_shop::Profile::read(open(&path)?)?.with_overlay(overlay),
            )?;
        }
        Ok(assets)
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
        let mut out = Self {
            equipment: None,
            skills: None,
            stats_index: item_stats::StatsIndex::default(),
            color_index: item_colors::ColorIndex::default(),
            economy: None,
            pets: std::sync::OnceLock::new(),
            npc: None,
            enhance: None,
            random_item: None,
            dye: None,
            random_item_random: Mutex::new(ColorRandom::from_seed(time_seed())),
            cash_shop: None,
            item_arithmetic: None,
            enhance_random: Mutex::new(enhance_random(time_seed())),
            enchant_random: Mutex::new(enchant_random(time_seed())),
            craft_seed: None,
            skill_enhance_random: Mutex::new(ColorRandom::from_seed(time_seed())),
            shops: None,
            equip_class: Default::default(),
            initializer: parts.initializer,
            class_initializers: std::collections::BTreeMap::new(),
            catalog: parts.catalog,
            stats: parts.stats,
            colors: parts.colors,
            inventory: parts.inventory,
            appearance: parts.appearance,
            entity_service_id: parts.entity_service_id,
            arithmetic: parts.arithmetic,
            random: Mutex::new(ColorRandom::from_seed(parts.color_seed)),
            channel: None,
            relay: None,
            content_version: 0,
        };
        out.initializer.bind(&out.catalog)?;
        out.inventory.bind(&out.catalog)?;
        out.appearance.bind(&out.catalog)?;
        out.stats_index = out.stats.bind(&out.catalog)?.to_index();
        out.color_index = out.colors.bind(&out.catalog)?.to_index();
        Ok(out)
    }
    pub fn with_equipment(
        mut self,
        profile: super::equipment_profile::Profile,
    ) -> Result<Self, MinimumError> {
        profile.resolve(&self.catalog)?;
        self.equipment = Some(profile);
        Ok(self)
    }
    pub(super) fn colors_materials(&self) -> &crate::domain::item_colors::Materials {
        self.colors.materials()
    }
    pub fn with_npc_scripts(
        mut self,
        profile: super::npc_scripts::Profile,
    ) -> Result<Self, MinimumError> {
        profile.bind(&self.catalog)?;
        // FeatureMatrix.GetFloat("ShopPriceRatio"); without an explicit arithmetic
        // profile both Framework architectures must agree on every price.
        use crate::domain::{shop::Shops, shop_price};
        let ratio = shop_price::ratio(profile.policy.get_string("ShopPriceRatio"))
            .ok_or(MinimumError::Conflict)?;
        let rows = profile.shop_rows.clone();
        let shops = match self.arithmetic {
            Some(BodyArithmetic::FrameworkX86) => Shops::load(rows, ratio, shop_price::Arch::X86),
            Some(BodyArithmetic::FrameworkX64) => Shops::load(rows, ratio, shop_price::Arch::X64),
            None => {
                let x64 = Shops::load(rows.clone(), ratio, shop_price::Arch::X64);
                let x86 = Shops::load(rows, ratio, shop_price::Arch::X86);
                match (x64, x86) {
                    (Some(a), Some(b)) if a == b => Some(a),
                    _ => None,
                }
            }
        }
        .ok_or(MinimumError::Conflict)?;
        self.shops = Some(shops);
        self.equip_class = crate::application::item_catalog_inspection::catalog(&self.catalog)?
            .equipment()
            .map(|(key, row)| {
                Ok((
                    key.to_vec(),
                    profile
                        .equip_item_class
                        .get(row)
                        .ok_or(MinimumError::Conflict)?
                        .clone(),
                ))
            })
            .collect::<Result<_, MinimumError>>()?;
        self.npc = Some(profile);
        Ok(self)
    }
    pub fn with_economy(
        mut self,
        profile: super::item_economy::Profile,
    ) -> Result<Self, MinimumError> {
        profile.bind(&self.catalog)?;
        self.pets = std::sync::OnceLock::new();
        self.economy = Some(profile);
        Ok(self)
    }
    /// Deterministic enhance rolls (tests and laboratory reproduction).
    pub fn with_enhance_seed(mut self, seed: i32) -> Self {
        self.enhance_random = Mutex::new(enhance_random(seed));
        self.enchant_random = Mutex::new(enchant_random(seed));
        self.craft_seed = Some(seed.wrapping_add(5));
        self.skill_enhance_random = Mutex::new(ColorRandom::from_seed(seed.wrapping_add(6)));
        self.random_item_random = Mutex::new(ColorRandom::from_seed(seed.wrapping_add(7)));
        self
    }
    /// The seed of a craft's fresh Random.
    pub(super) fn craft_random_seed(&self) -> i32 {
        self.craft_seed.unwrap_or_else(time_seed)
    }
    pub fn with_item_arithmetic(
        mut self,
        arithmetic: crate::domain::character_stats::Arithmetic,
    ) -> Self {
        self.item_arithmetic = Some(arithmetic);
        self
    }
    pub fn with_enhance(
        mut self,
        profile: super::item_enhance::Profile,
    ) -> Result<Self, MinimumError> {
        self.enhance = Some(profile.bind(&self.catalog)?);
        Ok(self)
    }
    pub fn with_random_item(
        mut self,
        profile: super::random_item::Profile,
    ) -> Result<Self, MinimumError> {
        self.random_item = Some(profile.bind(&self.catalog)?);
        Ok(self)
    }
    pub fn with_dye(mut self, profile: super::minimum_dye::Profile) -> Result<Self, MinimumError> {
        self.dye = Some(profile.bind(&self.catalog)?);
        Ok(self)
    }
    pub fn with_cash_shop(
        mut self,
        profile: super::cash_shop::Profile,
    ) -> Result<Self, MinimumError> {
        self.cash_shop = Some(profile.bind(&self.catalog)?);
        Ok(self)
    }
    pub fn with_skills(
        mut self,
        profile: super::skill_profile::Profile,
    ) -> Result<Self, MinimumError> {
        self.skills = Some(profile.contents(&self.catalog)?);
        Ok(self)
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
        let initializer = self.initializer_for(template.character_class)?;
        let mut random = self.random.lock().await;
        let mut utc = || super::lab_clock::utc_now().map_err(|_| Error::LegacyClockOverflow);
        let mut civil =
            || super::lab_clock::civil_now(offset).map_err(|_| Error::LegacyClockOverflow);
        let done = minimum_character::initialize(
            pool,
            owner,
            cid,
            minimum_character::Inputs {
                initializer,
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
            self.initializers(),
            self.npc.as_ref().and_then(|n| n.progression.as_ref()),
            config,
            delay,
            || super::lab_clock::utc_now().map_err(|_| MinimumError::Conflict),
        )
        .await?;
        Ok(Created { cid, body })
    }
}
