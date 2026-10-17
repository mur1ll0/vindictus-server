//! Recovered normal Fiona appearance preparation. No database/TCP success is implied.
use crate::{
    application::item_catalog_inspection,
    domain::{
        character_costume::{BodyArithmetic, Costume, CostumeSet},
        feature_policy::FeaturePolicy,
        legacy_user_data::LegacyBytes,
    },
    error::Error,
    infrastructure::{
        fiona_inventory_profile::Profile as Inventory,
        item_catalog::{self, Package as Catalog},
        item_colors::Package as Colors,
        user_export::{PrivateText, nullable},
    },
    protocol::character_creation::Template,
};
use serde::Deserialize;
use std::{collections::BTreeMap, io::Read};
use zeroize::Zeroizing;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Feature {
    key: String,
    #[serde(deserialize_with = "nullable")]
    value: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEquip {
    source_index: usize,
    #[serde(deserialize_with = "nullable")]
    item_utf16: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    feature: Option<String>,
    costume_type: i32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCustom {
    #[serde(deserialize_with = "nullable")]
    item_utf16: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    feature: Option<String>,
    #[serde(deserialize_with = "nullable")]
    cash_type: Option<String>,
    is_cash: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawColor {
    #[serde(deserialize_with = "nullable")]
    category_utf16: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    feature: Option<String>,
    #[serde(deserialize_with = "nullable")]
    cash_type: Option<String>,
    is_cash: bool,
    order: i32,
    color: i32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPart {
    consumable: bool,
    #[serde(deserialize_with = "nullable")]
    category_utf16: Option<PrivateText>,
    equip_part: i32,
    #[serde(deserialize_with = "nullable")]
    costume_part: Option<i32>,
    decoration: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Export {
    format_version: u32,
    source: String,
    database_sha256: String,
    config_sha256: String,
    implementation_sha256: String,
    gear_sources: BTreeMap<String, String>,
    features: Vec<Feature>,
    equips: Vec<RawEquip>,
    custom: Vec<RawCustom>,
    colors: Vec<RawColor>,
    parts: Vec<RawPart>,
    inner_item_utf16: PrivateText,
    inner_colors: [i32; 3],
}
struct Equip {
    key: Option<LegacyBytes<1024, true>>,
    feature: Option<String>,
    serial: i32,
}
struct Custom {
    key: Option<LegacyBytes<1024, true>>,
    feature: Option<String>,
    tag: Option<String>,
    cash: bool,
}
struct Color {
    key: Option<LegacyBytes<1024, true>>,
    feature: Option<String>,
    tag: Option<String>,
    cash: bool,
    order: i32,
    value: i32,
}
struct Part {
    consumable: bool,
    key: Option<LegacyBytes<1024, true>>,
    number: i32,
    costume: Option<i32>,
    decoration: bool,
}
pub struct Profile {
    db: String,
    config: String,
    policy: FeaturePolicy,
    equips: Vec<Equip>,
    custom: Vec<Custom>,
    colors: Vec<Color>,
    parts: Vec<Part>,
    inner: LegacyBytes<1024, true>,
    inner_colors: [i32; 3],
}
fn fail() -> Error {
    Error::FionaAppearanceProfile
}
fn expression(s: &Option<String>) -> Result<(), Error> {
    if s.as_ref().is_some_and(|v| v.len() > 512) {
        Err(fail())
    } else {
        Ok(())
    }
}
fn tag(s: &Option<String>) -> Result<(), Error> {
    if s.as_ref().is_some_and(|v| v.len() > 256) {
        Err(fail())
    } else {
        Ok(())
    }
}
fn text(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
impl Profile {
    pub fn read(reader: impl Read) -> Result<Self, Error> {
        let mut bytes = Zeroizing::new(Vec::new());
        reader
            .take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| fail())?;
        if bytes.len() > 16 * 1024 * 1024 {
            return Err(fail());
        }
        let r: Export = serde_json::from_slice(&bytes).map_err(|_| fail())?;
        #[derive(Deserialize)]
        struct Source {
            sha256: String,
        }
        let known: BTreeMap<String, Source> =
            serde_json::from_str(include_str!("../domain/fiona_gear_sources.json"))
                .map_err(|_| fail())?;
        if r.gear_sources.len() != known.len()
            || known
                .iter()
                .any(|(k, v)| r.gear_sources.get(k) != Some(&v.sha256))
        {
            return Err(fail());
        }
        let hash = |s: &str| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit());
        if r.format_version != 1
            || r.source != "heroesContents.fiona-appearance"
            || !hash(&r.database_sha256)
            || !hash(&r.config_sha256)
            || !hash(&r.implementation_sha256)
            || r.equips.len() > 50000
            || r.custom.len() > 50000
            || r.colors.len() > 50000
            || r.parts.len() > 4096
        {
            return Err(fail());
        }
        let policy =
            FeaturePolicy::extended_source(r.features.into_iter().map(|f| (f.key, f.value)))?;
        if !policy.entries().any(|(k, _)| k == "LanguageTag") {
            return Err(fail());
        }
        let equips = r
            .equips
            .into_iter()
            .enumerate()
            .map(|(i, r)| {
                if i != r.source_index {
                    return Err(fail());
                }
                expression(&r.feature)?;
                Ok(Equip {
                    key: item_catalog::opaque(r.item_utf16)?,
                    feature: r.feature,
                    serial: r.costume_type,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let custom = r
            .custom
            .into_iter()
            .map(|r| {
                expression(&r.feature)?;
                tag(&r.cash_type)?;
                Ok(Custom {
                    key: item_catalog::opaque(r.item_utf16)?,
                    feature: r.feature,
                    tag: r.cash_type,
                    cash: r.is_cash,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let colors = r
            .colors
            .into_iter()
            .map(|r| {
                expression(&r.feature)?;
                tag(&r.cash_type)?;
                Ok(Color {
                    key: item_catalog::opaque(r.category_utf16)?,
                    feature: r.feature,
                    tag: r.cash_type,
                    cash: r.is_cash,
                    order: r.order,
                    value: r.color,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let parts = r
            .parts
            .into_iter()
            .map(|r| {
                Ok(Part {
                    consumable: r.consumable,
                    key: item_catalog::opaque(r.category_utf16)?,
                    number: r.equip_part,
                    costume: r.costume_part,
                    decoration: r.decoration,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        Ok(Self {
            db: r.database_sha256,
            config: r.config_sha256,
            policy,
            equips,
            custom,
            colors,
            parts,
            inner: item_catalog::opaque(Some(r.inner_item_utf16))?.ok_or_else(fail)?,
            inner_colors: r.inner_colors,
        })
    }
    pub fn bind(&self, catalog: &Catalog) -> Result<(), Error> {
        if !self
            .db
            .eq_ignore_ascii_case(&catalog.manifest.database_sha256)
            || !self
                .config
                .eq_ignore_ascii_case(&catalog.manifest.config_sha256)
            || self.policy.current_version() != catalog.policy.current_version()
            || self.equips.len() != catalog.equips.len()
        {
            return Err(fail());
        }
        let own = self.policy.entries().collect::<BTreeMap<_, _>>();
        if catalog
            .policy
            .entries()
            .any(|(k, v)| own.get(k).is_some_and(|actual| *actual != v))
        {
            return Err(fail());
        }
        for (a, b) in self.equips.iter().zip(&catalog.equips) {
            if a.key.as_ref().map(LegacyBytes::as_bytes)
                != b.item_class.as_ref().map(LegacyBytes::as_bytes)
                || a.feature != b.feature
            {
                return Err(fail());
            }
        }
        Ok(())
    }
    /// Exact empty-slot projection for the fresh creation path, not a stored Gear loader.
    pub fn initial_gear(
        &self,
        catalog: &Catalog,
    ) -> Result<crate::domain::initial_gear::Projection, Error> {
        self.bind(catalog)?;
        // ToSetBonusInfo checks this feature even with no equipped items; num=0
        // prevents the avatar-set lookup for either measured boolean outcome.
        if !self.policy.entries().any(|(k, _)| k == "AvatarSlotBonus") {
            return Err(fail());
        }
        self.policy.is_enabled(Some("AvatarSlotBonus"))?;
        let mut keys = std::collections::BTreeSet::new();
        let mut rows = Vec::new();
        for r in &self.parts {
            let key = r.key.as_ref().ok_or_else(fail)?.as_bytes();
            if !keys.insert((r.number, key)) {
                return Err(fail());
            }
            rows.push(crate::domain::initial_gear::SlotPart {
                slot: u8::try_from(r.number).map_err(|_| fail())?,
                costume_part: r.costume,
                decoration: r.decoration,
                consumable: r.consumable,
            });
        }
        let projection =
            crate::domain::initial_gear::Projection::from_empty_slots(&rows).map_err(|_| fail())?;
        if projection.gear_slots() > 0 {
            if !self.policy.entries().any(|(k, _)| k == "Avatar") {
                return Err(fail());
            }
            self.policy.is_enabled(Some("Avatar"))?;
        }
        Ok(projection)
    }
    /// General appearance preparation for the admitted class and empty equipped-item set.
    /// None arithmetic admits only results equal in both measured Framework profiles.
    pub fn prepare(
        &self,
        template: &Template<'_>,
        inventory: &Inventory,
        catalog: &Catalog,
        colors: &Colors,
        arithmetic: Option<BodyArithmetic>,
    ) -> Result<Vec<Costume>, Error> {
        self.bind(catalog)?;
        inventory.bind(catalog)?;
        if template.character_class != 1 {
            return Err(fail());
        }
        let inputs = template.items.as_ref().ok_or_else(fail)?;
        if inputs.len() > 256 || template.body_shape.as_ref().is_some_and(|v| v.len() > 256) {
            return Err(fail());
        }
        let items = item_catalog_inspection::catalog(catalog)?;
        let equipment = items.equipment().collect::<BTreeMap<_, _>>();
        let materials = colors.bind(catalog)?;
        let language = self.policy.get_string("LanguageTag");
        let active = |feature: &Option<String>, tag: &Option<String>| -> Result<bool, Error> {
            Ok(self.policy.is_enabled(feature.as_deref())?
                && tag.as_deref().unwrap_or(language) == language)
        };
        let mut custom = BTreeMap::new();
        for r in &self.custom {
            if active(&r.feature, &r.tag)? {
                let key = r.key.as_ref().ok_or_else(fail)?.as_bytes();
                if custom.insert(key, r.cash).is_some() {
                    return Err(fail());
                }
            }
        }
        let mut color_map = BTreeMap::new();
        let mut skin_order = BTreeMap::new();
        let mut sss = BTreeMap::new();
        let skin = text("skin");
        let skin_sss = text("skin_sss");
        let eye = text("eye");
        for r in &self.colors {
            if !active(&r.feature, &r.tag)? {
                continue;
            }
            let category = r.key.as_ref().ok_or_else(fail)?.as_bytes();
            // Duplicate colors keep the first entry; skin Order uses Dictionary.Add.
            if category != skin_sss {
                color_map.entry((category, r.value)).or_insert(r.cash);
            }
            if category == skin && skin_order.insert(r.order, r.value).is_some() {
                return Err(fail());
            }
        }
        for r in &self.colors {
            if active(&r.feature, &r.tag)?
                && r.key.as_ref().is_some_and(|k| k.as_bytes() == skin_sss)
            {
                // Original DictionaryExtension.TryGetValue<int,int> returns default(int)=0.
                let color = skin_order.get(&r.order).copied().unwrap_or_default();
                sss.insert(color, r.value);
            }
        }
        let mut set = CostumeSet::default();
        let selected = inputs
            .iter()
            .map(|i| {
                let i = i.as_ref().ok_or_else(fail)?;
                let name = i.item_class.ok_or_else(fail)?;
                if name.encode_utf16().count() > 50 {
                    return Err(fail());
                }
                Ok((text(name), i.colors))
            })
            .collect::<Result<Vec<_>, Error>>()?;
        for (key, chosen) in selected
            .iter()
            .map(|(k, c)| (k.as_slice(), *c))
            .chain(std::iter::once((self.inner.as_bytes(), self.inner_colors)))
        {
            let item = items.get(Some(key)).ok_or_else(fail)?;
            let cash = *custom.get(key).ok_or_else(fail)?;
            if cash || !item.binding.equippable {
                return Err(fail());
            }
            let category = inventory.category(item.source_index).ok_or_else(fail)?;
            let matching = self
                .parts
                .iter()
                .filter(|p| p.key.as_ref().is_some_and(|k| k.as_bytes() == category))
                .collect::<Vec<_>>();
            if matching.len() != 1 {
                return Err(fail());
            }
            let part = matching[0];
            let costume_part = part.costume.ok_or_else(fail)?;
            let names = materials.get(Some(key)).ok_or_else(fail)?;
            for (name, color) in names.into_iter().zip(chosen) {
                if let Some(name) = name {
                    let key = name
                        .iter()
                        .flat_map(|u| u.to_le_bytes())
                        .collect::<Vec<_>>();
                    // The source computes an invalid-color flag but never uses it to reject.
                    if color_map
                        .get(&(key.as_slice(), color))
                        .copied()
                        .unwrap_or(false)
                    {
                        return Err(fail());
                    }
                }
            }
            let serial = self
                .equips
                .get(*equipment.get(key).ok_or_else(fail)?)
                .ok_or_else(fail)?
                .serial;
            if costume_part != -1 {
                // ToBaseCostumeInfo sets Durability=-1 and IsAvatar=false: ToCostume Gloss=255.
                set.set(Costume {
                    is_base: true,
                    kind: u8::from(part.decoration),
                    key: costume_part as u8,
                    serial: Some(serial),
                    colors: chosen.map(Some),
                    gloss: Some(255),
                    effect: Some(0),
                })
                .map_err(|_| fail())?;
            }
            let _ = part.number; // Source EquipPartNum is projected, not a persisted costume field.
        }
        if !color_map.contains_key(&(skin.as_slice(), template.skin_color))
            || !color_map.contains_key(&(eye.as_slice(), template.eye_color))
        {
            return Err(fail());
        }
        set.set(Costume {
            is_base: true,
            kind: 0,
            key: 2,
            serial: None,
            colors: [
                Some(template.skin_color),
                Some(sss.get(&template.skin_color).copied().unwrap_or_default()),
                Some(template.eye_color),
            ],
            gloss: None,
            effect: None,
        })
        .map_err(|_| fail())?;
        // New inventory initialization leaves all Equippable.Part NULL: ItemCostume is empty.
        for (key, value) in [
            (2, template.bust),
            (1, template.height),
            (0, template.shineness),
            (3, template.painting_x),
            (4, template.painting_y),
            (5, template.painting_rotation),
            (6, template.painting_size),
        ] {
            set.set(Costume::extra(key, value)).map_err(|_| fail())?;
        }
        if let Some(values) = &template.body_shape {
            for (i, &(key, bits)) in values.iter().enumerate() {
                if values[..i].iter().any(|(k, _)| *k == key) {
                    return Err(fail());
                }
                let value = f32::from_bits(bits);
                let costume = if let Some(profile) = arithmetic {
                    Costume::body(key, value, profile)
                } else {
                    let x = Costume::body(key, value, BodyArithmetic::FrameworkX86);
                    let y = Costume::body(key, value, BodyArithmetic::FrameworkX64);
                    if x != y {
                        return Err(Error::AppearanceArithmetic);
                    }
                    x
                };
                set.set(costume).map_err(|_| fail())?;
            }
        }
        Ok(set.rows().copied().collect())
    }
}
