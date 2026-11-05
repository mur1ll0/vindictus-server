//! Private ItemClassInfo economy projection (Unique, ExpireIn, prices, requirements),
//! bound by row position to the effective item catalog. Never opens the source SQLite.
use super::{
    item_catalog::{self, Package},
    minimum_character::MinimumError,
    user_export::{PrivateText, nullable},
};
use crate::domain::legacy_user_data::LegacyBytes;
use serde::Deserialize;
use std::io::Read;
use zeroize::Zeroizing;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    database_sha256: String,
    config_sha256: String,
    class_rows: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawClass {
    #[serde(deserialize_with = "nullable")]
    item_utf16: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    category_utf16: Option<PrivateText>,
    unique: i32,
    #[serde(deserialize_with = "nullable")]
    expire_in: Option<i32>,
    price: i32,
    sell_price: i32,
    required_level: i32,
    class_restriction: i32,
    use_in_town: i32,
    indestructible: i32,
    level_upper_bound: i32,
    cafe_only: i32,
    #[serde(deserialize_with = "nullable")]
    required_skill_utf16: Option<PrivateText>,
    required_skill_rank: i32,
    required_enhance: i32,
    required_enchant: i32,
    quick_slot_max_stack: i32,
    draft: i32,
    #[serde(default)]
    sort_order: Option<String>,
    #[serde(default)]
    level: i32,
    #[serde(default)]
    inventory_filter: Option<String>,
    #[serde(default)]
    set_id: Option<String>,
    #[serde(default)]
    trade_restriction: Option<String>,
    #[serde(default)]
    trade_category: Option<String>,
    #[serde(default)]
    trade_category_sub: Option<String>,
}
/// The TradeSystem's FeatureMatrix values and ItemTradeLimitInfo rows.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MarketplaceExport {
    features: Vec<FeatureRow>,
    trade_limits: Vec<TradeLimit>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FeatureRow {
    key: String,
    value: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TradeLimit {
    item_class: String,
    max_limit_value: Option<i32>,
}
/// ItemContents trade dictionaries and the TradeSystem FeatureMatrix values.
pub struct Marketplace {
    pub features: crate::domain::marketplace::Features,
    /// ItemContents.itemTradeLimitInfo: MaxLimitValue by ItemClass (ordinal keys).
    limits: std::collections::BTreeMap<String, Option<i32>>,
}
impl Marketplace {
    /// ItemContents.GetItemTradeLimitInfo(class)?.MaxLimitValue.
    pub fn max_limit_value(&self, class: &str) -> Option<i32> {
        self.limits.get(class).copied().flatten()
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Export {
    format_version: u32,
    source: String,
    manifest: Manifest,
    classes: Vec<RawClass>,
    alchemist_boxes: Vec<PrivateText>,
    #[serde(default)]
    hot_spring: Vec<HotSpringPotion>,
    /// ItemContents pet dictionaries (absent from exports without PetInfo).
    #[serde(default)]
    pets: Option<crate::domain::pet::Export>,
    /// MailTypeInfo rows (absent from older exports: every mail type is then unknown).
    #[serde(default)]
    mail_types: Vec<crate::domain::mail::MailTypeInfo>,
    /// The marketplace section (absent from older exports: the marketplace is refused).
    #[serde(default)]
    marketplace: Option<MarketplaceExport>,
    /// FeatureMatrix event switches of the region with the content rows per table.
    #[serde(default)]
    events: Vec<EventSwitch>,
    /// EventTemplate rows (the event schedule's templates).
    #[serde(default)]
    event_templates: Vec<EventTemplate>,
    /// The panel's collab packs (scripts/content/collabs.py): keys switched on and off together.
    #[serde(default)]
    event_packs: Vec<EventPack>,
    /// GuildLevelUpInfo rows (absent from older exports: no guild levels).
    #[serde(default)]
    guild_levels: Vec<GuildLevelRow>,
    /// GuildLevelBonusInfo rows (absent from older exports: no guild bonuses).
    #[serde(default)]
    guild_level_bonuses: Vec<GuildBonusRow>,
    /// RankDescription rows (absent from older exports: no rankings).
    #[serde(default)]
    rank_descriptions: Vec<RankDescription>,
    /// RankTitle rows (absent from older exports: no rank titles).
    #[serde(default)]
    rank_titles: Vec<RankTitleRow>,
}
/// A collab pack of the panel: every key the whole event needs, and the other regions' copies
/// of the same content to switch off (laboratory extension, scripts/content/collabs.py).
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EventPack {
    pub id: String,
    pub name: String,
    pub collab: String,
    pub enable: Vec<String>,
    pub disable: Vec<String>,
}
/// Packs name only exported event keys, never the same key on and off, within bounds.
pub fn packs_valid(packs: &[EventPack], events: &[EventSwitch]) -> bool {
    packs.len() <= 16
        && packs.iter().all(|p| {
            p.enable.len() + p.disable.len() <= 64
                && p.enable
                    .iter()
                    .chain(&p.disable)
                    .all(|k| events.iter().any(|e| &e.key == k))
                && !p.enable.iter().any(|k| p.disable.contains(k))
        })
}
/// AdminContents.EventTemplate: Name, Feature, StartScript, EndScript.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, serde::Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct EventTemplate {
    pub name: String,
    pub feature: Option<String>,
    pub start_script: Option<String>,
    pub end_script: Option<String>,
}
/// An event switch of the FeatureMatrix: its value in the region and its uses.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EventSwitch {
    pub key: String,
    pub value: Option<String>,
    pub uses: std::collections::BTreeMap<String, i32>,
    /// What the event brings (the panel's description), absent from older exports.
    #[serde(default)]
    pub detail: EventDetail,
    /// The collab the key belongs to (SAO, Fairy Tail), laboratory extension.
    #[serde(default)]
    pub collab: Option<String>,
    /// The other service a collab key comes from (JP, KR, CN...), adapted to the EU laboratory.
    #[serde(default)]
    pub adapted: Option<String>,
}
/// The content rows naming an event switch, at most 60 per table, in provider order.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EventDetail {
    /// ShopInfo: ShopID, ItemClass, Count, PriceItemClass, PriceCount.
    #[serde(default)]
    #[allow(clippy::type_complexity)]
    pub shops: Vec<(
        Option<String>,
        Option<String>,
        Option<i64>,
        Option<String>,
        Option<i64>,
    )>,
    /// CouponShopItemInfo (its Feature or its version's): ShopVersion, RewardItemClass,
    /// RewardItemCount, the version's PriceItemClass, PriceItemCount.
    #[serde(default)]
    #[allow(clippy::type_complexity)]
    pub coupons: Vec<(
        Option<i64>,
        Option<String>,
        Option<i64>,
        Option<String>,
        Option<i64>,
    )>,
    /// EventDropItemInfo: the monster (Class Variation, None for every boss), ItemClass,
    /// Probability, Condition.
    #[serde(default)]
    #[allow(clippy::type_complexity)]
    pub drops: Vec<(Option<String>, Option<String>, Option<i64>, Option<String>)>,
    /// QuestInfo IDs.
    #[serde(default)]
    pub quests: Vec<Option<String>>,
    /// StoryLineInfo: StoryLineID, Npc, RewardExp, RewardGold, RewardAp, RewardItem,
    /// RewardItemCount, RewardTitle.
    #[serde(default)]
    #[allow(clippy::type_complexity)]
    pub stories: Vec<(
        Option<String>,
        Option<String>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<String>,
        Option<i64>,
        Option<i64>,
    )>,
    /// TitleInfo IDs.
    #[serde(default)]
    pub titles: Vec<Option<i64>>,
    /// PeriodicEventInfo: RewardType, RewardArg, Amount, Possibility.
    #[serde(default)]
    #[allow(clippy::type_complexity)]
    pub periodic: Vec<(Option<String>, Option<String>, Option<i64>, Option<i64>)>,
    /// AllUserGoalInfo: GoalName, Goal, GoalCount, RewardItemClass, RewardNum, StartTime,
    /// EndTime.
    #[serde(default)]
    #[allow(clippy::type_complexity)]
    pub goals: Vec<(
        Option<String>,
        Option<String>,
        Option<i64>,
        Option<String>,
        Option<i64>,
        Option<String>,
        Option<String>,
    )>,
    /// NpcTalkInfo defaults the key enables: the NPC id and its town location.
    #[serde(default)]
    pub npcs: Vec<(Option<String>, Option<String>)>,
}
/// HotSpringPotionInfo row (UntilEffectSec and ShowName are not read by the server).
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HotSpringPotion {
    pub item_class: String,
    pub status_effect: String,
    pub level: i32,
    pub bath_time_sec: i32,
    pub overlap_min_sec: i32,
    pub all_announce: i32,
}
/// One ItemClassInfo row; the effective row is chosen by the catalog's source_index.
pub struct EconomyClass {
    item_class: Option<LegacyBytes<1024, true>>,
    pub category: Option<LegacyBytes<1024, true>>,
    pub unique: i32,
    pub expire_in: Option<i32>,
    pub price: i32,
    pub sell_price: i32,
    pub required_level: i32,
    pub class_restriction: i32,
    pub use_in_town: bool,
    /// ItemClassInfo.Indestructible (1 cannot be sold).
    pub indestructible: i32,
    /// ItemClassInfoConstraintTrait (EntityCommunicator.IsConstraintSatisfied).
    pub constraint: crate::domain::item_constraint::Constraint,
    /// ItemClassInfo.QuickSlotMaxStack and Draft (MakeConsumablesInfo).
    pub quick_slot_max_stack: i32,
    pub draft: i32,
    /// ItemClassInfo.SortOrder, Level and InventoryFilterName, and the first SetInfo SetID
    /// (ItemOwner.ItemComparer of SortInventory).
    pub sort_order: Option<String>,
    pub level: i32,
    pub inventory_filter: Option<String>,
    pub set_id: Option<String>,
    /// ItemClassInfo.TradeRestirction (Item.IsMailableTo; null is tradable).
    pub trade_restriction: Option<String>,
    /// ItemClassInfo.TradeCategory and TradeCategorySub (AddTradeItem).
    pub trade_category: Option<String>,
    pub trade_category_sub: Option<String>,
}
pub struct Profile {
    manifest: Manifest,
    classes: Vec<EconomyClass>,
    /// AlchemistBoxInfo.BoxItemClass (UTF-16LE).
    alchemist_boxes: Vec<Vec<u8>>,
    hot_spring: Vec<HotSpringPotion>,
    pets: Option<crate::domain::pet::Export>,
    mail_types: Vec<crate::domain::mail::MailTypeInfo>,
    marketplace: Option<Marketplace>,
    events: Vec<EventSwitch>,
    event_templates: Vec<EventTemplate>,
    event_packs: Vec<EventPack>,
    guild_levels: Vec<GuildLevelRow>,
    guild_level_bonuses: Vec<GuildBonusRow>,
    rank_descriptions: Vec<RankDescription>,
    rank_titles: Vec<RankTitleRow>,
}
/// A RankTitle row: the title of a rank in a ranking event.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RankTitleRow {
    pub event_id: i32,
    pub rank: i32,
    pub title_id: i32,
}
/// A RankDescription row (RankContents of the RankService).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RankDescription {
    pub rank_id: Option<String>,
    pub event_id: i32,
    pub score_type: Option<String>,
    pub sort_type: Option<String>,
    pub score_result_type: Option<String>,
    pub feature: Option<String>,
    pub score_limit: Option<i64>,
    pub is_guild: Option<i32>,
}
/// A GuildLevelUpInfo row (RequiredExp as stored; GuildContents multiplies it by 10).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuildLevelRow {
    pub level: i32,
    pub required_exp: i64,
    pub reward: Option<String>,
    pub feature: Option<String>,
}
/// A GuildLevelBonusInfo row.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuildBonusRow {
    pub level: i32,
    pub bonus: Option<String>,
    pub arg: i32,
    pub feature: Option<String>,
}
impl Profile {
    pub fn read(reader: impl Read) -> Result<Self, MinimumError> {
        let mut bytes = Zeroizing::new(Vec::new());
        reader
            .take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| MinimumError::Conflict)?;
        if bytes.len() > 16 * 1024 * 1024 {
            return Err(MinimumError::Conflict);
        }
        let export: Export = serde_json::from_slice(&bytes).map_err(|_| MinimumError::Conflict)?;
        if export.format_version != 4
            || export.source != "heroesContents.item-economy"
            || export.classes.len() != export.manifest.class_rows
            || export.classes.len() > 50_000
        {
            return Err(MinimumError::Conflict);
        }
        let classes = export
            .classes
            .into_iter()
            .map(|r| {
                let item_class = item_catalog::opaque(r.item_utf16)?;
                let skill = item_catalog::opaque(r.required_skill_utf16)?
                    .map(|s| {
                        let raw = s.as_bytes();
                        if !raw.len().is_multiple_of(2) {
                            return Err(crate::error::Error::Configuration(
                                "economy required skill".into(),
                            ));
                        }
                        let units: Vec<u16> = raw
                            .as_chunks::<2>()
                            .0
                            .iter()
                            .map(|a| u16::from_le_bytes(*a))
                            .collect();
                        String::from_utf16(&units).map_err(|_| {
                            crate::error::Error::Configuration("economy required skill".into())
                        })
                    })
                    .transpose()?;
                let campfire: Vec<u8> = "guild_campfire_kit_lvl1"
                    .encode_utf16()
                    .flat_map(u16::to_le_bytes)
                    .collect();
                let constraint = crate::domain::item_constraint::Constraint {
                    cafe_only: r.cafe_only,
                    required_level: r.required_level,
                    level_upper_bound: r.level_upper_bound,
                    class_restriction: r.class_restriction,
                    required_guild_level: if item_class
                        .as_ref()
                        .is_some_and(|c| c.as_bytes() == campfire.as_slice())
                    {
                        10
                    } else {
                        0
                    },
                    required_skill: skill.map(|s| (s, r.required_skill_rank)),
                    required_enhance: r.required_enhance,
                    required_enchant: r.required_enchant,
                };
                Ok(EconomyClass {
                    constraint,
                    item_class,
                    category: item_catalog::opaque(r.category_utf16)?,
                    unique: r.unique,
                    expire_in: r.expire_in,
                    price: r.price,
                    sell_price: r.sell_price,
                    required_level: r.required_level,
                    class_restriction: r.class_restriction,
                    use_in_town: r.use_in_town != 0,
                    indestructible: r.indestructible,
                    quick_slot_max_stack: r.quick_slot_max_stack,
                    draft: r.draft,
                    sort_order: r.sort_order,
                    level: r.level,
                    inventory_filter: r.inventory_filter,
                    set_id: r.set_id,
                    trade_restriction: r.trade_restriction,
                    trade_category: r.trade_category,
                    trade_category_sub: r.trade_category_sub,
                })
            })
            .collect::<Result<Vec<_>, crate::error::Error>>()?;
        let alchemist_boxes = export
            .alchemist_boxes
            .into_iter()
            .map(|b| {
                item_catalog::opaque(Some(b))
                    .map(|b| b.map(|b| b.as_bytes().to_vec()).unwrap_or_default())
            })
            .collect::<Result<Vec<_>, crate::error::Error>>()?;
        if export.hot_spring.len() > 1000
            || export.hot_spring.iter().enumerate().any(|(i, h)| {
                h.item_class.len() > 50
                    || h.status_effect.len() > 50
                    || export.hot_spring[..i]
                        .iter()
                        .any(|o| o.item_class == h.item_class)
            })
        {
            return Err(MinimumError::Conflict);
        }
        if export.mail_types.len() > 256
            || export.mail_types.iter().enumerate().any(|(i, m)| {
                !(0..=255).contains(&m.mail_type)
                    || export.mail_types[..i]
                        .iter()
                        .any(|o| o.mail_type == m.mail_type)
            })
        {
            return Err(MinimumError::Conflict);
        }
        if export.events.len() > 1000
            || export.events.iter().any(|e| {
                e.key.len() > 256
                    || !e.key.is_ascii()
                    || !crate::domain::content_overlay::Overlay::is_event_switch(&e.key)
                    || e.uses.len() > 64
            })
        {
            return Err(MinimumError::Conflict);
        }
        if !packs_valid(&export.event_packs, &export.events) {
            return Err(MinimumError::Conflict);
        }
        let marketplace = match export.marketplace {
            None => None,
            Some(m) => {
                if m.features.len() > 64 || m.trade_limits.len() > 10_000 {
                    return Err(MinimumError::Conflict);
                }
                let policy = crate::domain::feature_policy::FeaturePolicy::new(
                    m.features.into_iter().map(|f| (f.key, f.value)),
                )?;
                let mut limits = std::collections::BTreeMap::new();
                for l in m.trade_limits {
                    if l.item_class.len() > 50
                        || limits.insert(l.item_class, l.max_limit_value).is_some()
                    {
                        return Err(MinimumError::Conflict);
                    }
                }
                Some(Marketplace {
                    features: crate::domain::marketplace::Features::new(&policy)?,
                    limits,
                })
            }
        };
        Ok(Self {
            manifest: export.manifest,
            classes,
            alchemist_boxes,
            hot_spring: export.hot_spring,
            pets: export.pets,
            mail_types: export.mail_types,
            marketplace,
            events: export.events,
            event_templates: export.event_templates,
            event_packs: export.event_packs,
            guild_levels: export.guild_levels,
            guild_level_bonuses: export.guild_level_bonuses,
            rank_descriptions: export.rank_descriptions,
            rank_titles: export.rank_titles,
        })
    }
    /// RankDescription rows (features unevaluated).
    pub fn rank_descriptions(&self) -> &[RankDescription] {
        &self.rank_descriptions
    }
    /// RankTitle rows.
    pub fn rank_titles(&self) -> &[RankTitleRow] {
        &self.rank_titles
    }
    /// GuildLevelUpInfo rows (features unevaluated).
    pub fn guild_levels(&self) -> &[GuildLevelRow] {
        &self.guild_levels
    }
    /// GuildLevelBonusInfo rows (features unevaluated).
    pub fn guild_level_bonuses(&self) -> &[GuildBonusRow] {
        &self.guild_level_bonuses
    }
    /// The event schedule's templates (EventTemplate).
    pub fn event_templates(&self) -> &[EventTemplate] {
        &self.event_templates
    }
    /// The region's FeatureMatrix event switches (the panel's Eventos tab).
    pub fn events(&self) -> &[EventSwitch] {
        &self.events
    }
    /// The panel's collab packs.
    pub fn event_packs(&self) -> &[EventPack] {
        &self.event_packs
    }
    /// The marketplace contents, when the export has them.
    pub fn marketplace(&self) -> Option<&Marketplace> {
        self.marketplace.as_ref()
    }
    /// ItemContents.GetMailTypeInfo.
    pub fn mail_type(&self, mail_type: u8) -> Option<&crate::domain::mail::MailTypeInfo> {
        self.mail_types
            .iter()
            .find(|m| m.mail_type == i32::from(mail_type))
    }
    /// The exported pet tables, evaluated against the feature policy by the assets.
    pub fn pets(&self) -> Option<&crate::domain::pet::Export> {
        self.pets.as_ref()
    }
    /// ItemContents.GetHotSpringPotionInfo.
    pub fn hot_spring(&self, class: &str) -> Option<&HotSpringPotion> {
        self.hot_spring.iter().find(|h| h.item_class == class)
    }
    /// ItemContents.GetHotSpringStatusEffectList: the distinct StatusEffect values in order.
    pub fn hot_spring_effects(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for h in &self.hot_spring {
            if !out.contains(&h.status_effect.as_str()) {
                out.push(&h.status_effect);
            }
        }
        out
    }
    /// Same source files and the same row sequence as the bound catalog.
    pub fn bind(&self, catalog: &Package) -> Result<(), MinimumError> {
        let same_rows = self.classes.len() == catalog.classes.len()
            && self.classes.iter().zip(&catalog.classes).all(|(e, c)| {
                e.item_class.as_ref().map(LegacyBytes::as_bytes)
                    == c.item_class.as_ref().map(LegacyBytes::as_bytes)
            });
        if !same_rows
            || self.manifest.database_sha256 != catalog.manifest.database_sha256
            || self.manifest.config_sha256 != catalog.manifest.config_sha256
        {
            return Err(MinimumError::Conflict);
        }
        Ok(())
    }
    /// Whether the UTF-16LE item class is an AlchemistBoxInfo box.
    pub fn is_alchemist_box(&self, class_utf16: &[u8]) -> bool {
        self.alchemist_boxes.iter().any(|b| b == class_utf16)
    }
    pub fn get(&self, source_index: usize) -> Option<&EconomyClass> {
        self.classes.get(source_index)
    }
}

#[cfg(test)]
mod collab_tests {
    use super::*;

    fn event(key: &str) -> EventSwitch {
        serde_json::from_value(serde_json::json!({
            "key": key, "value": null, "uses": {"NpcTalkInfo": 3},
            "detail": {"npcs": [["SAO_Kirito", "t01_inn"]]}, "collab": "SAO", "adapted": "JP"
        }))
        .expect("event switch")
    }

    fn pack(enable: &[&str], disable: &[&str]) -> EventPack {
        EventPack {
            id: "sao_eu".into(),
            name: "SAO completo (EU)".into(),
            collab: "SAO".into(),
            enable: enable.iter().map(|k| (*k).to_owned()).collect(),
            disable: disable.iter().map(|k| (*k).to_owned()).collect(),
        }
    }

    #[test]
    fn collab_event_fields_and_npcs_deserialize() {
        let e = event("Event_sao_npc_jajp");
        assert_eq!(e.collab.as_deref(), Some("SAO"));
        assert_eq!(e.adapted.as_deref(), Some("JP"));
        assert_eq!(
            e.detail.npcs,
            vec![(Some("SAO_Kirito".to_owned()), Some("t01_inn".to_owned()))]
        );
    }

    #[test]
    fn packs_name_known_keys_once() {
        let events = [
            event("Event_sao_npc_jajp"),
            event("Event_SAO_AllUserGoal_aus"),
        ];
        assert!(packs_valid(
            &[pack(
                &["Event_sao_npc_jajp"],
                &["Event_SAO_AllUserGoal_aus"]
            )],
            &events
        ));
        assert!(!packs_valid(&[pack(&["Event_unknown"], &[])], &events));
        assert!(!packs_valid(
            &[pack(&["Event_sao_npc_jajp"], &["Event_sao_npc_jajp"])],
            &events
        ));
    }
}
