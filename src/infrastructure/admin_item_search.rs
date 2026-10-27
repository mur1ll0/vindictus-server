//! Item picker of the admin panel: the panel catalog's items indexed by the game's marketplace
//! categories (ItemClassInfo.TradeCategory/TradeCategorySub, the tree of MainCategoryInfo and
//! SubCategoryInfo with the client's HEROES_ITEM_TRADECATEGORY_NAME_* texts, exported by
//! scripts/content/export_panel_catalog.py), searched page by page in memory.
use serde::{Deserialize, Serialize};

pub const MAX_PER_PAGE: usize = 100;
pub const MAX_TERM_CHARS: usize = 64;

/// One marketplace category and its subcategories, in the content's order.
#[derive(Clone, Debug, Default, Deserialize, Serialize, utoipa::ToSchema)]
pub struct TradeCategory {
    /// TradeCategory (WEAPON, PLATEARMOR...).
    pub code: String,
    /// The client's English name.
    pub name: String,
    pub subs: Vec<TradeSubCategory>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, utoipa::ToSchema)]
pub struct TradeSubCategory {
    /// TradeCategorySub as the items carry it (LONGSWORD, PLATEARMOR_HELM...).
    pub code: String,
    pub name: String,
    /// Items of the export in this subcategory (hidden ones included).
    pub count: u32,
}

/// One item as the picker lists it.
#[derive(Clone, Debug, Serialize, utoipa::ToSchema)]
pub struct PickerItem {
    /// Item class as the game writes it (lower case).
    pub class: String,
    pub name: Option<String>,
    pub icon: Option<String>,
    pub category: Option<String>,
    pub sub: Option<String>,
    /// ItemClassInfo.RequiredLevel (missing = none).
    pub required_level: Option<i32>,
    pub rarity: Option<i32>,
    /// ItemClassInfo.ClassRestriction bit mask (bit = character class; missing = none).
    pub class_restriction: Option<i32>,
}

#[derive(Debug)]
struct Entry {
    item: PickerItem,
    /// Lower-case class and name, matched by the search term.
    keys: String,
    hidden: bool,
}

/// The catalog's items sorted by name (items without a name last, by class).
#[derive(Debug, Default)]
pub struct ItemIndex {
    entries: Vec<Entry>,
}

/// Picker query; every filter is optional.
#[derive(Debug, Default)]
pub struct Filter<'a> {
    pub term: &'a str,
    pub category: Option<&'a str>,
    pub sub: Option<&'a str>,
    /// Character class (0–31): items whose ClassRestriction admits it.
    pub class: Option<u8>,
    pub min_level: Option<i32>,
    pub max_level: Option<i32>,
    pub rarity: Option<i32>,
    pub include_hidden: bool,
    /// 1-based.
    pub page: usize,
    pub per_page: usize,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct Page {
    pub items: Vec<PickerItem>,
    /// Items matching the filters (all pages).
    pub total: usize,
    pub page: usize,
    pub per_page: usize,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("invalid item search")]
pub struct InvalidFilter;

impl ItemIndex {
    pub fn new(items: impl IntoIterator<Item = (PickerItem, bool)>) -> Self {
        let mut entries: Vec<Entry> = items
            .into_iter()
            .map(|(item, hidden)| Entry {
                keys: format!(
                    "{}\n{}",
                    item.class,
                    item.name.as_deref().unwrap_or("").to_lowercase()
                ),
                item,
                hidden,
            })
            .collect();
        entries.sort_by(|a, b| {
            (a.item.name.is_none(), &a.item.name, &a.item.class).cmp(&(
                b.item.name.is_none(),
                &b.item.name,
                &b.item.class,
            ))
        });
        Self { entries }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The page of the items matching `filter`. A search without a category needs 2+
    /// characters, so a bare request never lists the whole catalog page by page by accident.
    pub fn search(&self, filter: &Filter<'_>) -> Result<Page, InvalidFilter> {
        let term = filter.term.trim().to_lowercase();
        let chars = term.chars().count();
        if chars > MAX_TERM_CHARS
            || (filter.category.is_none() && chars < 2)
            || filter.page == 0
            || !(1..=MAX_PER_PAGE).contains(&filter.per_page)
            || filter.class.is_some_and(|c| c > 31)
        {
            return Err(InvalidFilter);
        }
        let matches = |e: &&Entry| {
            let i = &e.item;
            (filter.include_hidden || !e.hidden)
                && (term.is_empty() || e.keys.contains(&term))
                && filter
                    .category
                    .is_none_or(|c| i.category.as_deref() == Some(c))
                && filter.sub.is_none_or(|s| i.sub.as_deref() == Some(s))
                && filter.class.is_none_or(|c| {
                    i.class_restriction
                        .is_none_or(|mask| mask & (1_i32 << c) != 0)
                })
                && filter
                    .min_level
                    .is_none_or(|l| i.required_level.unwrap_or(0) >= l)
                && filter
                    .max_level
                    .is_none_or(|l| i.required_level.unwrap_or(0) <= l)
                && filter.rarity.is_none_or(|r| i.rarity.unwrap_or(0) == r)
        };
        let skip = (filter.page - 1).saturating_mul(filter.per_page);
        let mut total = 0;
        let mut items = Vec::new();
        for e in self.entries.iter().filter(matches) {
            if total >= skip && items.len() < filter.per_page {
                items.push(e.item.clone());
            }
            total += 1;
        }
        Ok(Page {
            items,
            total,
            page: filter.page,
            per_page: filter.per_page,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(
        class: &str,
        name: Option<&str>,
        sub: &str,
        level: i32,
        mask: Option<i32>,
    ) -> PickerItem {
        PickerItem {
            class: class.into(),
            name: name.map(str::to_owned),
            icon: None,
            category: Some(
                if sub == "HELM" {
                    "PLATEARMOR"
                } else {
                    "WEAPON"
                }
                .into(),
            ),
            sub: Some(sub.into()),
            required_level: Some(level),
            rarity: Some(2),
            class_restriction: mask,
        }
    }

    fn index() -> ItemIndex {
        ItemIndex::new([
            (
                item("longsword_b", Some("Broadsword"), "LONGSWORD", 10, Some(2)),
                false,
            ),
            (
                item(
                    "longsword_a",
                    Some("Aidan Longsword"),
                    "LONGSWORD",
                    8,
                    Some(2),
                ),
                false,
            ),
            (item("longsword_x", None, "LONGSWORD", 1, Some(2)), false),
            (
                item("longsword_h", Some("Hidden Sword"), "LONGSWORD", 1, Some(2)),
                true,
            ),
            (
                item("pillar_a", Some("Battle Pillar"), "PILLAR", 30, Some(8)),
                false,
            ),
            (
                item("plate_helm", Some("Plate Helm"), "HELM", 40, None),
                false,
            ),
        ])
    }

    fn filter<'a>() -> Filter<'a> {
        Filter {
            page: 1,
            per_page: 2,
            ..Filter::default()
        }
    }

    #[test]
    fn pages_follow_the_name_order_and_count_every_match() {
        let index = index();
        let first = index
            .search(&Filter {
                category: Some("WEAPON"),
                ..filter()
            })
            .expect("valid");
        assert_eq!(first.total, 4);
        let classes: Vec<_> = first.items.iter().map(|i| i.class.as_str()).collect();
        assert_eq!(classes, ["longsword_a", "pillar_a"]);
        let last = index
            .search(&Filter {
                category: Some("WEAPON"),
                page: 2,
                ..filter()
            })
            .expect("valid");
        // Unnamed items come last; the hidden one is left out.
        let classes: Vec<_> = last.items.iter().map(|i| i.class.as_str()).collect();
        assert_eq!(classes, ["longsword_b", "longsword_x"]);
        let past = index
            .search(&Filter {
                category: Some("WEAPON"),
                page: 9,
                ..filter()
            })
            .expect("valid");
        assert!(past.items.is_empty());
        assert_eq!(past.total, 4);
    }

    #[test]
    fn filters_combine() {
        let index = index();
        let search = |f: Filter<'_>| {
            index
                .search(&Filter { per_page: 100, ..f })
                .expect("valid")
                .items
                .into_iter()
                .map(|i| i.class)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            search(Filter {
                term: "SWORD",
                ..filter()
            }),
            ["longsword_a", "longsword_b", "longsword_x"]
        );
        assert_eq!(
            search(Filter {
                category: Some("WEAPON"),
                sub: Some("PILLAR"),
                ..filter()
            }),
            ["pillar_a"]
        );
        // Kalok (class 3): the pillar and the unrestricted helm.
        assert_eq!(
            search(Filter {
                term: "la",
                class: Some(3),
                ..filter()
            }),
            ["pillar_a", "plate_helm"]
        );
        assert_eq!(
            search(Filter {
                term: "sword",
                min_level: Some(8),
                max_level: Some(9),
                ..filter()
            }),
            ["longsword_a"]
        );
        assert_eq!(
            search(Filter {
                term: "hidden",
                include_hidden: true,
                ..filter()
            }),
            ["longsword_h"]
        );
    }

    #[test]
    fn bounds_are_refused() {
        let index = index();
        for bad in [
            Filter {
                term: "a",
                ..filter()
            },
            Filter {
                term: &"x".repeat(65),
                category: Some("WEAPON"),
                ..filter()
            },
            Filter {
                category: Some("WEAPON"),
                page: 0,
                ..filter()
            },
            Filter {
                category: Some("WEAPON"),
                per_page: 101,
                ..filter()
            },
            Filter {
                category: Some("WEAPON"),
                class: Some(32),
                ..filter()
            },
        ] {
            assert_eq!(index.search(&bad).err(), Some(InvalidFilter));
        }
    }
}
