//! Panel edit of one equipment item's ENHANCE level and PREFIX/SUFFIX enchants
//! (docs/future/painel-personagem-e-busca.md, section 2): the content's facts about the class,
//! the options the item accepts, a before/after tooltip preview and the offline write with
//! its audit row. The rules are `domain::item_upgrade`; the stats are the game's Item.Stat
//! (`minimum_item_stat::equipment_with`, through `admin_tooltip::build`). Item stats are
//! recomputed from the items when the character loads, so an offline change needs no Gear
//! refresh; the costumes do not depend on these attributes.
use super::{
    admin_characters::{EnglishNames, ItemAttribute},
    admin_store::{self, AuditRecord},
    admin_tooltip::{self, ItemTooltip},
    minimum_assets::Assets,
    minimum_character::MinimumError,
};
use crate::domain::{
    item_attribute::Attributes,
    item_upgrade::{self, ENHANCE, Facts, PREFIX, Refusal, SUFFIX},
};
use serde::Serialize;
use sqlx::{PgPool, Postgres, Transaction};

fn fail() -> MinimumError {
    MinimumError::Conflict
}

fn utf16_key(class: &str) -> Vec<u8> {
    class.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

/// The content's facts of an equipment class; None when the class is not equipment.
pub(super) struct ClassFacts {
    class: String,
    category: String,
    equip_class: Option<String>,
    enchantable: bool,
    max_enhance: i32,
}

impl ClassFacts {
    pub(super) fn of(assets: &Assets, class: &str) -> Option<Self> {
        let key = utf16_key(class);
        let contents = assets.enhance.as_ref()?;
        let category = assets
            .equipment
            .as_ref()?
            .category(&assets.catalog, &key)
            .ok()?;
        Some(Self {
            class: class.to_owned(),
            category,
            equip_class: assets.equip_class.get(&key).cloned().flatten(),
            enchantable: contents.enchantable(&key).is_some_and(|e| e != 0),
            max_enhance: contents.max_level(&key),
        })
    }
    pub(super) fn facts(&self) -> Facts<'_> {
        Facts {
            class: &self.class,
            category: &self.category,
            equip_class: self.equip_class.as_deref(),
            enchantable: self.enchantable,
            max_enhance: self.max_enhance,
        }
    }
}

/// The panel's "give item" with ENHANCE/PREFIX/SUFFIX attributes: the values the game could
/// produce for the class (B4.3b only checked that the stats were computable).
pub(super) fn check_give(
    assets: &Assets,
    class: &str,
    attributes: &Attributes,
) -> Result<(), Refusal> {
    if [ENHANCE, PREFIX, SUFFIX]
        .iter()
        .all(|n| attributes.get(n).is_none())
    {
        return Ok(());
    }
    let facts = ClassFacts::of(assets, class).ok_or(Refusal::NotEnchantable)?;
    let contents = assets.enhance.as_ref().ok_or(Refusal::UnknownEnchant)?;
    item_upgrade::check_attributes(&facts.facts(), attributes, |c| contents.enchant(c))
}

/// One enchant the item accepts, with the client's texts.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct EnchantOption {
    /// EnchantClass.
    pub class: String,
    pub name: String,
    /// EnchantLevel (1 = rank F … 6 = A, 7–9 = ranks 9–7).
    pub rank: i32,
    pub min_arg: i32,
    pub max_arg: i32,
    /// The stat lines (with their conditions) as the tooltip shows them.
    pub lines: Vec<String>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct UpgradeOptions {
    pub item_class: String,
    /// Largest Enhance level of the item's type; 0 without refining.
    pub enhance_max: i32,
    pub enhance: i32,
    pub enchantable: bool,
    pub prefix: Option<String>,
    pub prefix_arg: i32,
    pub suffix: Option<String>,
    pub suffix_arg: i32,
    pub prefixes: Vec<EnchantOption>,
    pub suffixes: Vec<EnchantOption>,
    pub tooltip: ItemTooltip,
}

/// The values the panel asks for: Enhance level (0 removes) and each enchant with its Arg.
#[derive(Clone, Debug, Default)]
pub struct Upgrade {
    pub enhance: i32,
    pub prefix: Option<(String, i32)>,
    pub suffix: Option<(String, i32)>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct UpgradePreview {
    pub before: ItemTooltip,
    pub after: ItemTooltip,
}

/// An item of the character, as the edit reads it.
struct Held {
    class: String,
    attributes: Vec<ItemAttribute>,
    tradable: bool,
    equippable: Option<(i32, i32, [i32; 3])>,
}

type HeldRow = (Vec<u8>, bool, Option<i32>, Option<i32>, Option<[i32; 3]>);

async fn held(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
    item: i64,
) -> Result<Option<Held>, MinimumError> {
    let row: Option<HeldRow> = sqlx::query_as(
        "SELECT i.item_class_utf16, i.tradable, e.reduced_durability, e.max_durability_bonus,
                ARRAY[e.color1, e.color2, e.color3]
         FROM game_items i JOIN game_equippables e ON e.id = i.id
         WHERE i.id = $1 AND i.owner_id = $2",
    )
    .bind(item)
    .bind(cid)
    .fetch_optional(&mut **tx)
    .await?;
    let Some((class, tradable, lost, bonus, colors)) = row else {
        return Ok(None);
    };
    let attributes: Vec<(Vec<u8>, Vec<u8>, i32, i32)> = sqlx::query_as(
        "SELECT attribute_bytes, value_bytes, arg, arg2 FROM game_item_attributes
         WHERE item_id = $1 ORDER BY id LIMIT 64",
    )
    .bind(item)
    .fetch_all(&mut **tx)
    .await?;
    let units: Vec<u16> = class
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u16::from_le_bytes(*p))
        .collect();
    let ascii = |b: &[u8]| String::from_utf8_lossy(b).into_owned();
    Ok(Some(Held {
        class: String::from_utf16(&units).map_err(|_| fail())?,
        attributes: attributes
            .into_iter()
            .map(|(name, value, arg, arg2)| ItemAttribute {
                name: ascii(&name),
                value: ascii(&value),
                arg,
                arg2,
            })
            .collect(),
        tradable,
        equippable: Some((
            lost.unwrap_or(0),
            bonus.unwrap_or(0),
            colors.unwrap_or([0; 3]),
        )),
    }))
}

fn attribute<'a>(list: &'a [ItemAttribute], name: &str) -> Option<&'a ItemAttribute> {
    list.iter().find(|a| a.name == name)
}

/// The item's attributes with ENHANCE/PREFIX/SUFFIX replaced (others kept, in order).
fn replaced(list: &[ItemAttribute], upgrade: &Upgrade) -> Vec<ItemAttribute> {
    let mut out: Vec<ItemAttribute> = list
        .iter()
        .filter(|a| ![ENHANCE, PREFIX, SUFFIX].contains(&a.name.as_str()))
        .cloned()
        .collect();
    if upgrade.enhance != 0 {
        out.push(ItemAttribute {
            name: ENHANCE.into(),
            value: upgrade.enhance.to_string(),
            arg: 0,
            arg2: 0,
        });
    }
    for (name, value) in [(PREFIX, &upgrade.prefix), (SUFFIX, &upgrade.suffix)] {
        if let Some((class, arg)) = value {
            out.push(ItemAttribute {
                name: name.into(),
                value: class.clone(),
                arg: *arg,
                arg2: 0,
            });
        }
    }
    out
}

fn judge(assets: &Assets, facts: &ClassFacts, upgrade: &Upgrade) -> Result<(), Refusal> {
    let contents = assets.enhance.as_ref().ok_or(Refusal::UnknownEnchant)?;
    let facts = facts.facts();
    item_upgrade::check_enhance(&facts, upgrade.enhance)?;
    for (value, prefix) in [(&upgrade.prefix, true), (&upgrade.suffix, false)] {
        if let Some((class, arg)) = value {
            item_upgrade::check_enchant(&facts, contents.enchant(class), prefix, *arg)?;
        }
    }
    Ok(())
}

fn options_of(
    names: &EnglishNames,
    assets: &Assets,
    facts: &ClassFacts,
    prefix: bool,
) -> Vec<EnchantOption> {
    let Some(contents) = assets.enhance.as_ref() else {
        return Vec::new();
    };
    item_upgrade::options(&facts.facts(), contents.enchants(), prefix)
        .into_iter()
        .map(|info| {
            let (name, lines) = admin_tooltip::enchant_text(names, assets, &info.class, prefix);
            EnchantOption {
                class: info.class.clone(),
                name,
                rank: info.level,
                min_arg: info.min_arg,
                max_arg: info.max_arg,
                lines,
            }
        })
        .collect()
}

#[derive(Debug)]
pub enum Outcome<T> {
    Done(T),
    /// No equipment item of this character with that ID.
    NotFound,
    /// The item is not refinable/enchantable that way.
    Refused(Refusal),
    /// The item already has these values.
    Unchanged,
    Online,
    Repeated,
}

async fn read_only(pool: &PgPool) -> Result<Transaction<'_, Postgres>, MinimumError> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    Ok(tx)
}

async fn holder(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
) -> Result<(super::minimum_item_stat::ItemContext, i32, i32), MinimumError> {
    let context = super::minimum_item_stat::context(tx, cid).await?;
    let (level, class): (i32, i16) =
        sqlx::query_as("SELECT level, class FROM game_character_info WHERE id = $1")
            .bind(cid)
            .fetch_one(&mut **tx)
            .await?;
    Ok((context, level, i32::from(class)))
}

fn tooltip(
    names: &EnglishNames,
    assets: &Assets,
    owner: &(super::minimum_item_stat::ItemContext, i32, i32),
    item: &Held,
    attributes: &[ItemAttribute],
) -> ItemTooltip {
    admin_tooltip::build(
        names,
        Some(assets),
        Some((&owner.0, owner.1, owner.2)),
        &admin_tooltip::Input {
            class: &item.class,
            attributes,
            tradable: item.tradable,
            equippable: item.equippable,
        },
    )
}

/// The Enhance levels and enchants one item of the character accepts.
pub async fn options(
    pool: &PgPool,
    names: &EnglishNames,
    assets: &Assets,
    cid: i64,
    item: i64,
) -> Result<Outcome<UpgradeOptions>, MinimumError> {
    let mut tx = read_only(pool).await?;
    let Some(held) = held(&mut tx, cid, item).await? else {
        return Ok(Outcome::NotFound);
    };
    let owner = holder(&mut tx, cid).await?;
    tx.commit().await?;
    let Some(facts) = ClassFacts::of(assets, &held.class) else {
        return Ok(Outcome::Refused(Refusal::NotEnchantable));
    };
    let enchant = |name| attribute(&held.attributes, name).map(|a| (a.value.clone(), a.arg));
    let (prefix, prefix_arg) = enchant(PREFIX).unzip();
    let (suffix, suffix_arg) = enchant(SUFFIX).unzip();
    Ok(Outcome::Done(UpgradeOptions {
        enhance_max: facts.max_enhance.max(0),
        enhance: attribute(&held.attributes, ENHANCE)
            .and_then(|a| a.value.parse().ok())
            .unwrap_or(0),
        enchantable: facts.enchantable,
        prefix,
        prefix_arg: prefix_arg.unwrap_or(0),
        suffix,
        suffix_arg: suffix_arg.unwrap_or(0),
        prefixes: options_of(names, assets, &facts, true),
        suffixes: options_of(names, assets, &facts, false),
        tooltip: tooltip(names, assets, &owner, &held, &held.attributes),
        item_class: held.class,
    }))
}

/// Before/after tooltips for the requested values, without writing.
pub async fn preview(
    pool: &PgPool,
    names: &EnglishNames,
    assets: &Assets,
    cid: i64,
    item: i64,
    upgrade: &Upgrade,
) -> Result<Outcome<UpgradePreview>, MinimumError> {
    let mut tx = read_only(pool).await?;
    let Some(held) = held(&mut tx, cid, item).await? else {
        return Ok(Outcome::NotFound);
    };
    let owner = holder(&mut tx, cid).await?;
    tx.commit().await?;
    let Some(facts) = ClassFacts::of(assets, &held.class) else {
        return Ok(Outcome::Refused(Refusal::NotEnchantable));
    };
    if let Err(refusal) = judge(assets, &facts, upgrade) {
        return Ok(Outcome::Refused(refusal));
    }
    let after = replaced(&held.attributes, upgrade);
    Ok(Outcome::Done(UpgradePreview {
        before: tooltip(names, assets, &owner, &held, &held.attributes),
        after: tooltip(names, assets, &owner, &held, &after),
    }))
}

/// The attributes as audit text: ENHANCE, PREFIX and SUFFIX with their Arg.
fn audit_view(list: &[ItemAttribute], tradable: bool) -> String {
    let pick = |name| attribute(list, name).map(|a| format!("{}({})", a.value, a.arg));
    serde_json::json!({
        "enhance": pick(ENHANCE), "prefix": pick(PREFIX), "suffix": pick(SUFFIX), "tradable": tradable
    })
    .to_string()
}

/// Write the values on an offline character's item, with the audit row in the same
/// transaction. Values the game binds at (Enhance 9, EnchantLevel 7) make it untradable.
pub async fn apply(
    pool: &PgPool,
    assets: &Assets,
    cid: i64,
    item: i64,
    upgrade: &Upgrade,
    mut audit: AuditRecord<'_>,
) -> Result<Outcome<(String, String)>, MinimumError> {
    let user: Option<(i32, String, i64)> = sqlx::query_as(
        "SELECT u.id,u.name,u.local_revision FROM game_character_info c JOIN game_users u ON u.id=c.uid WHERE c.id=$1",
    )
    .bind(cid)
    .fetch_optional(pool)
    .await?;
    let (uid, name, revision) = user.ok_or_else(fail)?;
    let owner = super::character_lobby::UserKey {
        uid,
        name: &name,
        revision,
    };
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, &owner, cid).await?;
    let connected: bool =
        sqlx::query_scalar("SELECT is_connected FROM game_character_info WHERE id=$1")
            .bind(cid)
            .fetch_one(&mut *tx)
            .await?;
    if connected {
        return Ok(Outcome::Online);
    }
    let Some(held) = held(&mut tx, cid, item).await? else {
        return Ok(Outcome::NotFound);
    };
    let Some(facts) = ClassFacts::of(assets, &held.class) else {
        return Ok(Outcome::Refused(Refusal::NotEnchantable));
    };
    if let Err(refusal) = judge(assets, &facts, upgrade) {
        return Ok(Outcome::Refused(refusal));
    }
    // The stats must stay computable, as for every item the game loads.
    let after = replaced(&held.attributes, upgrade);
    let stored = Attributes::new(
        after
            .iter()
            .map(|a| {
                crate::domain::item_attribute::Attribute::from_stored(
                    a.name.as_bytes(),
                    a.value.as_bytes(),
                    a.arg,
                    a.arg2,
                )
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| fail())?,
    )
    .map_err(|_| fail())?;
    let context = super::minimum_item_stat::context(&mut tx, cid).await?;
    super::minimum_item_stat::equipment_with(
        assets,
        &utf16_key(&held.class),
        &stored,
        Some(&context),
    )?;
    let current = super::minimum_inventory::load_attributes(&mut tx, &[item])
        .await?
        .remove(&item)
        .unwrap_or_default();
    let enhance = (upgrade.enhance != 0).then(|| upgrade.enhance.to_string());
    super::minimum_enhance::set_attribute_args(
        &mut tx,
        item,
        &current,
        ENHANCE,
        enhance.as_deref(),
        0,
        0,
    )
    .await?;
    let contents = assets.enhance.as_ref().ok_or_else(fail)?;
    let mut levels = [0; 2];
    for (n, (name, value)) in [(PREFIX, &upgrade.prefix), (SUFFIX, &upgrade.suffix)]
        .into_iter()
        .enumerate()
    {
        let (class, arg) = value
            .as_ref()
            .map_or((None, 0), |(c, a)| (Some(c.as_str()), *a));
        levels[n] = class
            .and_then(|c| contents.enchant(c))
            .map_or(0, |i| i.level);
        super::minimum_enhance::set_attribute_args(&mut tx, item, &current, name, class, arg, 0)
            .await?;
    }
    let tradable = held.tradable && !item_upgrade::binds(upgrade.enhance, levels);
    if tradable != held.tradable {
        sqlx::query("UPDATE game_items SET tradable=false WHERE id=$1")
            .bind(item)
            .execute(&mut *tx)
            .await?;
    }
    let before = audit_view(&held.attributes, held.tradable);
    let after = audit_view(&after, tradable);
    if before == after {
        return Ok(Outcome::Unchanged);
    }
    audit.before = Some(before.clone());
    audit.after = Some(after.clone());
    match admin_store::insert_audit(&mut tx, &audit).await {
        Ok(()) => {}
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => return Ok(Outcome::Repeated),
        Err(e) => return Err(e.into()),
    }
    tx.commit().await?;
    Ok(Outcome::Done((before, after)))
}
