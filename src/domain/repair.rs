//! Item.RepairPrice, Item.GetItemValue and RepairableItemSelector for items without
//! QUALITY/ENCHANT attributes (QualityLevel 2, enchant 0); EnhanceLevel is the item's.
use std::collections::BTreeMap;

/// ItemContents repair dictionaries (ToDictionary: a duplicate key refuses the load).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct RepairTables {
    pub enhance: BTreeMap<i32, i32>,
    pub quality: BTreeMap<i32, i32>,
    pub enchant: BTreeMap<i32, (i32, i32)>,
    /// RepairEquipClassInfo by EquipClass.ToLower().
    pub equip_class: BTreeMap<String, i32>,
    /// FeatureMatrix.GetInteger("RepairPriceRate").
    pub rate: i32,
}
impl RepairTables {
    /// GetItemValue for an item without QUALITY (QualityLevel 2).
    pub fn item_value(&self, enhance_level: i32, prefix_level: i32, suffix_level: i32) -> i32 {
        let enhance = self.enhance.get(&enhance_level).copied().unwrap_or(0);
        let quality = self.quality.get(&2).copied().unwrap_or(0);
        let (prefix, _) = self.enchant.get(&prefix_level).copied().unwrap_or((0, 0));
        let (_, suffix) = self.enchant.get(&suffix_level).copied().unwrap_or((0, 0));
        enhance
            .wrapping_add(quality)
            .wrapping_add(prefix)
            .wrapping_add(suffix)
            .wrapping_add(44)
    }
}

/// (int)double conversion of the JIT: out-of-range and NaN give int.MinValue.
fn to_int(value: f64) -> i32 {
    if value.is_nan() || value < i32::MIN as f64 || value >= 2_147_483_648.0 {
        i32::MIN
    } else {
        value as i32
    }
}

/// Item.RepairPrice for a character without a guild repair bonus.
pub fn repair_price(
    tables: &RepairTables,
    durability: i32,
    maximum: i32,
    price: i32,
    equip_class: Option<&str>,
    levels: (i32, i32, i32),
) -> i32 {
    if maximum.wrapping_sub(durability) <= 0 {
        return 0;
    }
    let current = durability.wrapping_add(99) / 100;
    let max = maximum.wrapping_add(99) / 100;
    if max.wrapping_sub(current) <= 0 {
        return 0;
    }
    let mut base = f64::from(maximum.wrapping_sub(durability)) * f64::from(price);
    if max <= 10 {
        base = f64::from(1000_i32.wrapping_mul(price));
    }
    let point = equip_class
        .and_then(|c| tables.equip_class.get(&c.to_lowercase()).copied())
        .unwrap_or(1);
    let value = base
        * f64::from(tables.item_value(levels.0, levels.1, levels.2))
        * f64::from(point)
        * 2.999999970665357E-10;
    let rated = to_int(value * f64::from(tables.rate) / 100.0);
    // GuildLevelUpBonus "RepairPrice" is 0 without a guild: num7 * 100 / 100.
    rated.wrapping_mul(100) / 100
}
