//! Item tooltip of the admin panel (B4.4e): the sections the client's ItemToolTip.gfx fills
//! (setBaseInfo, setStats, addEnchant, addConstraint, setDesc, setCondition, setSetInfo,
//! setItemTrade), with the client's own English texts from the panel catalog. The stats are
//! Item.Stat as the game builds it (`minimum_item_stat::equipment_with`: the class stat plus
//! ENHANCE, QUALITY and the PREFIX/SUFFIX enchants), the durability EquipmentDurability.
use super::{
    admin_characters::{EnglishNames, ItemAttribute},
    minimum_assets::Assets,
    minimum_item_stat::ItemContext,
};
use serde::Serialize;

/// ItemStatInfo order (the client lists the stats in it).
const STAT_ORDER: [&str; 22] = [
    "ATK",
    "PVP_ATK",
    "MATK",
    "PVP_MATK",
    "Balance",
    "Critical",
    "ATK_Speed",
    "ATK_Range",
    "ATK_Absolute",
    "DEF",
    "PVP_DEF",
    "DEF_Absolute",
    "DEF_Destroyed",
    "STR",
    "DEX",
    "INT",
    "WILL",
    "LUCK",
    "HP",
    "STAMINA",
    "Res_Critical",
    "TOWN_SPEED",
];
/// BaseCharacter in value order, with the English class names.
const CLASSES: [&str; 10] = [
    "Lann", "Fiona", "Evie", "Karok", "Kai", "Vella", "Hurk", "Lynn", "Arisha", "Hagie",
];

#[derive(Clone, Debug, Default, Serialize, utoipa::ToSchema)]
pub struct ItemTooltip {
    /// ItemClassInfo.Rarity and its client name (Regular, Beginners, ..., Rare).
    pub rarity: Option<i32>,
    pub rarity_name: Option<String>,
    /// The category text (Weapon, Pants, Consumable, ...) or the raw Category.
    pub category: Option<String>,
    pub unique: bool,
    /// GameUI_Heroes_Item_Tooltip_Weight: Item.Stat Weight / 1000 and the base weight.
    pub weight: Option<String>,
    pub sell_price: Option<i64>,
    /// Item.Stat in ItemStatInfo order; empty for items without stats.
    pub stats: Vec<TooltipStat>,
    pub enhance_level: i32,
    /// QUALITY attribute (craft quality, 0–5 stars).
    pub quality: Option<i32>,
    pub enchants: Vec<TooltipEnchant>,
    /// Current and maximum (EquipmentDurability, game units).
    pub durability: Option<[i32; 2]>,
    /// Level, class and skill requirements; `ok` when this character meets them.
    pub constraints: Vec<TooltipLine>,
    /// Trade/bind texts of setItemTrade.
    pub trade: Vec<String>,
    pub set_name: Option<String>,
    /// Set bonuses by pieces (`ok` once that many pieces are equipped).
    pub set_bonuses: Vec<TooltipLine>,
    /// The item's three colors (game_equippables), 0xRRGGBB.
    pub colors: Option<[i32; 3]>,
    /// The icon is a dye mask (EquipItemInfo.ColorOverride).
    pub dye_mask: bool,
    pub dyeable: bool,
    pub enchantable: bool,
}

#[derive(Clone, Debug, Serialize, utoipa::ToSchema)]
pub struct TooltipStat {
    pub key: String,
    pub label: String,
    pub value: i32,
}

#[derive(Clone, Debug, Serialize, utoipa::ToSchema)]
pub struct TooltipLine {
    pub text: String,
    pub ok: bool,
}

#[derive(Clone, Debug, Serialize, utoipa::ToSchema)]
pub struct TooltipEnchant {
    pub prefix: bool,
    pub name: String,
    pub rank: Option<i32>,
    pub lines: Vec<TooltipLine>,
}

pub(super) struct Input<'a> {
    pub class: &'a str,
    pub attributes: &'a [ItemAttribute],
    pub tradable: bool,
    /// game_equippables reduced_durability, max_durability_bonus and colors.
    pub equippable: Option<(i32, i32, [i32; 3])>,
}

/// Plain text of a client HTML/format string: %s/%d/{n} replaced in order (the color
/// argument of a `<font color='%s'>` included), then the tags removed.
fn format(template: &str, args: &[&str]) -> String {
    let mut args = args.iter();
    let mut text = String::new();
    let mut rest = template;
    while let Some(i) = rest.find(['%', '{']) {
        text.push_str(&rest[..i]);
        let tail = &rest[i..];
        let len = if tail.starts_with("%s") || tail.starts_with("%d") {
            2
        } else if let Some(end) = tail.starts_with('{').then(|| tail.find('}')).flatten() {
            end + 1
        } else {
            text.push_str(&tail[..1]);
            rest = &tail[1..];
            continue;
        };
        text.push_str(args.next().copied().unwrap_or(""));
        rest = &tail[len..];
    }
    text.push_str(rest);
    let mut out = String::new();
    let mut tag = false;
    for c in text.chars() {
        match c {
            '<' => tag = true,
            '>' if tag => tag = false,
            _ if !tag => out.push(c),
            _ => {}
        }
    }
    out.trim().to_owned()
}

fn utf16_name(name: &[u16]) -> String {
    String::from_utf16_lossy(name)
}

pub(super) fn build(
    names: &EnglishNames,
    assets: Option<&Assets>,
    owner: Option<(&ItemContext, i32, i32)>,
    input: &Input<'_>,
) -> ItemTooltip {
    let info = names.item_info(input.class);
    let int = |key: &str| {
        info.and_then(|i| i.get(key))
            .and_then(serde_json::Value::as_i64)
    };
    let string = |key: &str| {
        info.and_then(|i| i.get(key))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    };
    let mut tip = ItemTooltip {
        rarity: int("r").and_then(|r| i32::try_from(r).ok()),
        unique: int("u").is_some_and(|u| u > 0),
        sell_price: int("sp").filter(|p| *p > 0),
        dye_mask: int("co") == Some(1),
        dyeable: int("dy") == Some(1),
        enchantable: int("en") == Some(1),
        colors: input.equippable.map(|e| e.2),
        ..ItemTooltip::default()
    };
    // ItemStat.from_equipment: trunc(EquipItemInfo.Weight) * 1000.
    let base_weight = info
        .and_then(|i| i.get("w"))
        .and_then(serde_json::Value::as_f64)
        .filter(|w| w.is_finite() && *w >= 1.0 && *w < 1e6)
        .map(|w| w.trunc() as i32);
    let weight_text = |current: i32, base: i32| {
        let template = names
            .text("GAMEUI_HEROES_ITEM_TOOLTIP_WEIGHT")
            .unwrap_or_else(|| "%d stones (base weight: %d)".to_owned());
        format(&template, &[&current.to_string(), &base.to_string()])
    };
    tip.weight = base_weight.map(|b| weight_text(b, b));
    tip.rarity_name = tip
        .rarity
        .and_then(|r| names.text(&format!("GAMEUI_HEROES_ITEMTOOLTIP_RARITY_{r}")))
        .filter(|t| t != "0");
    tip.category = string("c").map(|c| {
        names
            .text(&format!(
                "GAMEUI_HEROES_ITEM_TOOLTIP_CATEGORY_{}",
                c.to_ascii_uppercase()
            ))
            .unwrap_or_else(|| match c.to_ascii_uppercase().as_str() {
                "INNERARMOR" => "Inner Armor".to_owned(),
                _ => c,
            })
    });
    let attribute = |name: &str| input.attributes.iter().find(|a| a.name == name);
    tip.enhance_level = attribute("ENHANCE").map_or(0, |a| a.arg.max(a.value.parse().unwrap_or(0)));
    tip.quality = attribute("QUALITY").map(|a| a.arg);

    // Item.Stat and durability, as the game computes them.
    let mut status: [Vec<(i32, bool)>; 2] = [Vec::new(), Vec::new()];
    if let (Some(assets), Some((reduced, bonus, _))) = (assets, input.equippable) {
        let class: Vec<u8> = input
            .class
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        let attributes = crate::domain::item_attribute::Attributes::new(
            input
                .attributes
                .iter()
                .map(|a| crate::domain::item_attribute::Attribute {
                    name: a.name.clone(),
                    value: a.value.clone(),
                    arg: a.arg,
                    arg2: a.arg2,
                })
                .collect(),
        );
        if let (Ok(attributes), Some(base)) = (attributes, assets.stats_index.get(Some(&class)))
            && let Ok(item) = super::minimum_item_stat::equipment_with(
                assets,
                &class,
                &attributes,
                owner.map(|o| o.0),
            )
        {
            let entries: Vec<(String, i32)> = item
                .stat
                .character_entries()
                .map(|e| (utf16_name(e.name), e.value))
                .collect();
            for key in STAT_ORDER {
                if let Some((_, value)) = entries.iter().find(|(n, v)| n == key && *v != 0) {
                    tip.stats.push(TooltipStat {
                        key: key.to_owned(),
                        label: names
                            .text(&format!(
                                "GAMEUI_HEROES_ITEMSTAT_{}",
                                key.to_ascii_uppercase()
                            ))
                            .unwrap_or_else(|| key.to_owned()),
                        value: *value,
                    });
                }
            }
            let fields = item.stat.fields();
            if fields[1] > 0 {
                tip.weight = Some(weight_text(
                    fields[1] / 1000,
                    base_weight.unwrap_or(fields[1] / 1000),
                ));
            }
            let d = crate::domain::item_durability::EquipmentDurability::load(
                base, &item.stat, reduced, bonus,
            );
            if d.has_durability() {
                tip.durability = Some([d.current(), d.maximum()]);
            }
            tip.enhance_level = item.enhance_level;
            status = [item.prefix_status, item.suffix_status];
        }
    }

    for (n, name) in ["PREFIX", "SUFFIX"].into_iter().enumerate() {
        let Some(a) = attribute(name) else { continue };
        let class = a.value.to_ascii_uppercase();
        let rank = assets
            .and_then(|s| s.enhance.as_ref())
            .and_then(|c| c.enchant(&a.value))
            .map(|e| e.level);
        let mut orders: Vec<(i32, bool)> = status[n].clone();
        if orders.is_empty() {
            orders = assets
                .and_then(|s| s.enhance.as_ref())
                .and_then(|c| c.enchant_stats(&a.value))
                .map(|rows| rows.iter().map(|r| (r.order, true)).collect())
                .unwrap_or_default();
        }
        let lines = orders
            .into_iter()
            .filter_map(|(order, ok)| {
                let stat = names.text(&format!("GAMEUI_HEROES_ENCHANTSTAT_{class}_{order}"))?;
                let condition =
                    names.text(&format!("GAMEUI_HEROES_ENCHANTCONDITION_{class}_{order}"));
                Some(TooltipLine {
                    text: match condition.filter(|c| !c.is_empty()) {
                        Some(c) => format!("{} ({})", format(&stat, &[]), format(&c, &[])),
                        None => format(&stat, &[]),
                    },
                    ok,
                })
            })
            .collect();
        tip.enchants.push(TooltipEnchant {
            prefix: n == 0,
            name: names
                .text(&format!("GAMEUI_HEROES_ATTRIBUTE_{name}_{class}"))
                .unwrap_or_else(|| a.value.clone()),
            rank,
            lines,
        });
    }

    // addConstraint: level, class and skill.
    let (level, class) = owner.map_or((None, None), |o| (Some(o.1), Some(o.2)));
    if let Some(required) = int("rl").filter(|l| *l > 1) {
        let template = names
            .text("GAMEUI_HEROES_ITEM_TOOLTIP_REQUIRED_LEVEL")
            .unwrap_or_else(|| "Level %d and Above".to_owned());
        tip.constraints.push(TooltipLine {
            text: format(&template, &["", &required.to_string()]),
            ok: level.is_none_or(|l| i64::from(l) >= required),
        });
    }
    if let Some(mask) = int("cr") {
        let allowed: Vec<usize> = (0..CLASSES.len())
            .filter(|i| mask & (1 << i) != 0)
            .collect();
        if !allowed.is_empty() && allowed.len() < CLASSES.len() {
            let text = if allowed.len() == 1 {
                let key = match allowed[0] {
                    0 => "LETHITA",
                    1 => "FIONA",
                    _ => "",
                };
                names
                    .text(&format!(
                        "GAMEUI_HEROES_ITEM_TOOLTIP_REQUIRED_CHARACTER_CLASS_{key}"
                    ))
                    .map(|t| format(&t, &[""]))
                    .unwrap_or_else(|| format!("For {}", CLASSES[allowed[0]]))
            } else {
                format!(
                    "For {}",
                    allowed
                        .iter()
                        .map(|i| CLASSES[*i])
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            tip.constraints.push(TooltipLine {
                text,
                ok: class.is_none_or(|c| usize::try_from(c).is_ok_and(|c| allowed.contains(&c))),
            });
        }
    }
    if let Some(skill) = string("rs") {
        let rank = int("rr").unwrap_or(0);
        let template = names
            .text("GAMEUI_HEROES_ITEM_TOOLTIP_REQUIRED_SKILL")
            .unwrap_or_else(|| "%s %s and Above".to_owned());
        let skill_name = names.skill_name(&skill).unwrap_or_else(|| skill.clone());
        tip.constraints.push(TooltipLine {
            text: format(&template, &["", &skill_name, &format!("Rank {rank}")]),
            ok: owner.is_none_or(|o| {
                i64::from(crate::domain::enchant::Context::skill_rank(
                    o.0,
                    Some(&skill),
                )) >= rank
            }),
        });
    }

    // setItemTrade.
    if let Some(restriction) = string("tr")
        && let Some(t) = names.text(&format!(
            "GAMEUI_HEROES_ITEMTOOLTIP_NOT_TRADABLE_{}",
            restriction.to_ascii_uppercase()
        ))
    {
        tip.trade.push(t);
    }
    if !input.tradable {
        let max = int("ab").unwrap_or(0);
        let used = i64::from(attribute("ANTIBIND").map_or(0, |a| a.arg));
        match (max - used).max(0) {
            0 => tip
                .trade
                .extend(names.text("GAMEUI_HEROES_ITEMTOOLTIP_BINDED_NOANTIBIND")),
            left => {
                tip.trade
                    .extend(names.text("GAMEUI_HEROES_ITEMTOOLTIP_BINDED_NOANTIBIND"));
                if let Some(t) = names.text("GAMEUI_HEROES_ITEMTOOLTIP_ANTIBIND") {
                    tip.trade.push(format(&t, &[&left.to_string()]));
                }
            }
        }
    }

    // setSetInfo (the equipped count is filled by the sheet).
    if let Some(set) = string("set") {
        let key = set.to_ascii_uppercase();
        tip.set_name = names.text(&format!("HEROES_ITEM_SETITEM_NAME_{key}"));
        for pieces in 2..=8 {
            if let Some(t) = names.text(&format!("HEROES_SET_ITEM_DESC_{key}_{pieces}")) {
                tip.set_bonuses.push(TooltipLine {
                    text: format!("[{pieces}] {}", t.trim().replace('\n', ", ")),
                    ok: false,
                });
            }
        }
    }
    tip
}

/// The set of a class, for the equipped-piece count.
/// The client's name of an enchant and its stat lines (each with its condition), for the
/// panel's enchant choice; the lines are not judged against an owner.
pub(super) fn enchant_text(
    names: &EnglishNames,
    assets: &Assets,
    enchant: &str,
    prefix: bool,
) -> (String, Vec<String>) {
    let class = enchant.to_ascii_uppercase();
    let slot = if prefix { "PREFIX" } else { "SUFFIX" };
    let lines = assets
        .enhance
        .as_ref()
        .and_then(|c| c.enchant_stats(enchant))
        .map(|rows| {
            rows.iter()
                .filter_map(|r| {
                    let stat =
                        names.text(&format!("GAMEUI_HEROES_ENCHANTSTAT_{class}_{}", r.order))?;
                    let condition = names
                        .text(&format!(
                            "GAMEUI_HEROES_ENCHANTCONDITION_{class}_{}",
                            r.order
                        ))
                        .filter(|c| !c.is_empty());
                    Some(match condition {
                        Some(c) => format!("{} ({})", format(&stat, &[]), format(&c, &[])),
                        None => format(&stat, &[]),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let name = names
        .text(&format!("GAMEUI_HEROES_ATTRIBUTE_{slot}_{class}"))
        .unwrap_or_else(|| enchant.to_owned());
    (name, lines)
}

pub(super) fn set_of(names: &EnglishNames, class: &str) -> Option<String> {
    names
        .item_info(class)
        .and_then(|i| i.get("set"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_ascii_uppercase)
}

/// Mark the set bonuses reached by `equipped` pieces.
pub(super) fn mark_set(tip: &mut ItemTooltip, equipped: usize) {
    for line in &mut tip.set_bonuses {
        let pieces: usize = line
            .text
            .trim_start_matches('[')
            .split(']')
            .next()
            .and_then(|n| n.parse().ok())
            .unwrap_or(usize::MAX);
        line.ok = equipped >= pieces;
    }
}

#[cfg(test)]
mod tests {
    use super::format;

    #[test]
    fn client_format_strings_become_plain_text() {
        assert_eq!(
            format("<font color='%s'>Level %d and Above</font>", &["", "16"]),
            "Level 16 and Above"
        );
        assert_eq!(
            format("Remaining Unbind Count : {0}", &["2"]),
            "Remaining Unbind Count : 2"
        );
        assert_eq!(format("100% sure", &[]), "100% sure");
    }
}
