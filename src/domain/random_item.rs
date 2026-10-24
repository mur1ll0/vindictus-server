//! RandomItemContents and RandomItemManager.Open/OpenBundle: the recipe class draw, the
//! per-class item draw (SelectItems with the variable probabilities) and the additional
//! classes, consuming RandomItemManager's static Random in the source order.

/// A RandomItem row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RandomBox {
    pub id: i32,
    pub server_code: Option<i32>,
    pub key: Option<String>,
    pub visual: Option<i32>,
}

/// A RandomItemRecipeClass row.
#[derive(Clone, Debug, PartialEq)]
pub struct RecipeClass {
    pub name: String,
    pub expected: Option<i32>,
    pub probability: f64,
    pub number: i32,
    pub rule: Option<String>,
}

/// A RandomItemRecipe row whose Feature is enabled.
#[derive(Clone, Debug, PartialEq)]
pub struct Recipe {
    pub character: Option<i32>,
    pub item_class: String,
    pub number: i32,
    pub probability: f64,
    pub colors: [Option<i32>; 3],
    pub notify: Option<String>,
    pub variable: i32,
    pub have: f64,
    pub dont_have: f64,
}

/// Steps of the source this port cannot reproduce (they need the appearance tables, throw
/// or are not migrated); nothing is sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unsupported {
    /// SelectFromRareItemList reads RandomItemAppearance counts.
    RareClass,
    /// A '$' bundle class reads and writes RandomItemOpen.
    BundleClass,
    /// A recipe with a null ItemClass, no item drawn, or a Convert.ToInt32 overflow.
    Draw,
}

/// One drawn reward: GiveItemRequest(ItemClassEx, Number, colors) and its NotifyMessage
/// text (None when it is an integer: the telepathy key goes to the broadcast list).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reward {
    pub item_class: String,
    pub number: i32,
    pub colors: [i32; 3],
    pub notify: Option<String>,
    /// NotifyMessage "1": the server-wide NotifyRandomItem key (not migrated).
    pub broadcast: Option<&'static str>,
}

/// RandomItemManager's static Random.
pub trait Random {
    fn next_double(&mut self) -> f64;
    /// Next(maxValue); None for a negative bound.
    fn next(&mut self, maximum: i32) -> Option<i32>;
}

/// What SelectItems reads from the owner (None: Open without an owner).
pub struct Owner<'a> {
    /// ItemOwner.GetItems() or BuybackList holds an item of the class.
    pub holds: &'a (dyn Fn(&str) -> bool + Sync),
    /// ItemContents.GetItemClassInfo(class).Storage (None: unknown class).
    pub storage: &'a (dyn Fn(&str) -> Option<u8> + Sync),
    /// FeatureMatrix VariableItem_ConstraintCheck / VariableProbability_RandomItem.
    pub constraint_check: bool,
    pub variable_probability: bool,
}

/// What SelectItems decided besides the recipes.
#[derive(Default, Debug, PartialEq, Eq)]
pub struct Notes {
    /// SendOperationFailedDialog(UseItemFail_RandomItem): a variable recipe outside the
    /// cash/account storages (the draw still runs).
    pub variable_dialog: bool,
}

/// Convert.ToInt32(double): round half to even, OverflowException outside Int32.
fn to_int32(v: f64) -> Result<i32, Unsupported> {
    let r = v.round_ties_even();
    if !(-2_147_483_648.5..2_147_483_647.5).contains(&r) || r.is_nan() {
        return Err(Unsupported::Draw);
    }
    Ok(r as i32)
}

const RAND_SCALE: f64 = 10_000_000.0;

#[allow(clippy::too_many_arguments)]
fn probability(
    r: &Recipe,
    index: usize,
    having: &[usize],
    variable: bool,
    total_variable: f64,
    total_normal: f64,
) -> Result<i32, Unsupported> {
    if variable {
        if r.variable == 1 {
            return to_int32(
                if having.contains(&index) {
                    r.have
                } else {
                    r.dont_have
                } * RAND_SCALE,
            );
        }
        return to_int32(r.probability * (1.0 - total_variable) / total_normal * RAND_SCALE);
    }
    to_int32(r.probability * RAND_SCALE)
}

/// RandomItemManager.SelectItems(items, selectNum, owner).
fn select_items(
    items: Vec<&Recipe>,
    count: i32,
    owner: Option<&Owner<'_>>,
    random: &mut dyn Random,
    notes: &mut Notes,
) -> Result<Vec<Recipe>, Unsupported> {
    if usize::try_from(count).ok() == Some(items.len()) {
        return Ok(items.into_iter().cloned().collect());
    }
    let mut having = Vec::new();
    let (mut total_variable, mut total_normal) = (0.0, 0.0);
    if let Some(owner) = owner {
        for (i, r) in items.iter().enumerate() {
            if r.variable == 1 {
                let class = crate::domain::item_attribute::parse_class_ex(&r.item_class)
                    .map(|(c, _)| c)
                    .unwrap_or_else(|_| r.item_class.clone());
                if (owner.holds)(&class) {
                    having.push(i);
                    total_variable += r.have;
                } else {
                    total_variable += r.dont_have;
                }
                if owner.constraint_check && !matches!((owner.storage)(&class), Some(2 | 3)) {
                    notes.variable_dialog = true;
                }
            } else {
                total_normal += r.probability;
            }
        }
    }
    let mut variable = owner.is_some_and(|o| o.variable_probability) && !having.is_empty();
    let sum = |variable: bool| -> Result<i32, Unsupported> {
        items.iter().enumerate().try_fold(0i32, |n, (i, r)| {
            Ok(n.wrapping_add(probability(
                r,
                i,
                &having,
                variable,
                total_variable,
                total_normal,
            )?))
        })
    };
    let mut total = sum(variable)?;
    if total <= 0 && variable {
        variable = false;
        total = sum(variable)?;
    }
    // Indexes into `items` still available.
    let mut left: Vec<usize> = (0..items.len()).collect();
    let mut out = Vec::new();
    for _ in 0..count.max(0) {
        let mut roll = random.next(total).ok_or(Unsupported::Draw)?;
        let mut picked = None;
        for (pos, &i) in left.iter().enumerate() {
            let p = probability(items[i], i, &having, variable, total_variable, total_normal)?;
            if roll < p {
                picked = Some(pos);
                break;
            }
            roll = roll.wrapping_sub(p);
        }
        // A null pick throws in getProbability(null) after list.Remove(null).
        let pos = picked.ok_or(Unsupported::Draw)?;
        let i = left.remove(pos);
        total = total.wrapping_sub(probability(
            items[i],
            i,
            &having,
            variable,
            total_variable,
            total_normal,
        )?);
        out.push(items[i].clone());
    }
    Ok(out)
}

/// The recipes of a class for the character (CharacterType null or the character code).
fn class_items(recipes: &[Recipe], character: i32) -> Vec<&Recipe> {
    recipes
        .iter()
        .filter(|r| r.character.is_none_or(|c| c == character))
        .collect()
}

/// `additional{x}` of an AdditionalRule (case-insensitive; the greedy `.*` prefix makes
/// the last occurrence win).
fn additional_ratio(rule: &str) -> Option<f64> {
    let lower = rule.to_ascii_lowercase();
    let start = lower.rfind("additional{")? + "additional{".len();
    let end = start + lower[start..].find('}')?;
    let number = &rule[start..end];
    let valid = !number.is_empty()
        && number.chars().all(|c| c.is_ascii_digit() || c == '.')
        && number.matches('.').count() <= 1
        && !number.ends_with('.');
    valid.then(|| number.parse().ok()).flatten()
}

/// RandomItemManager.Open (bundle false) or OpenBundle (bundle true) for a character code
/// (BaseCharacter: Lethita 0, Fiona 1, ...). `recipes(class)` is
/// RandomItemContents.GetRandomItemRecipe.
pub fn open(
    classes: &[RecipeClass],
    recipes: &dyn Fn(&str) -> Vec<Recipe>,
    character: i32,
    owner: Option<&Owner<'_>>,
    bundle: bool,
    random: &mut dyn Random,
) -> Result<(Vec<Reward>, Notes), Unsupported> {
    let mut notes = Notes::default();
    if classes.iter().any(|c| c.expected.is_some()) {
        return Err(Unsupported::RareClass);
    }
    let normal: Vec<&RecipeClass> = classes.iter().filter(|c| c.expected.is_none()).collect();
    // SelectFromNormalItemList (OpenBundle skips it without normal classes).
    let selected = if normal.is_empty() && bundle {
        None
    } else {
        let mut roll = random.next_double();
        let mut chosen = None;
        for c in &normal {
            if roll < c.probability {
                chosen = Some(*c);
                break;
            }
            roll -= c.probability;
        }
        // SelectMaxFromNormalItemList: the first class with the largest Probability.
        chosen.or_else(|| {
            normal
                .iter()
                .copied()
                .fold(None, |best: Option<&RecipeClass>, c| match best {
                    Some(b) if b.probability >= c.probability => Some(b),
                    _ => Some(c),
                })
        })
    };
    if bundle && selected.is_some_and(|c| c.name.starts_with('$')) {
        return Err(Unsupported::BundleClass);
    }
    let owner = if bundle { owner } else { None };
    let mut drawn = match selected {
        Some(c) => select_items(
            class_items(&recipes(&c.name), character),
            c.number,
            owner,
            random,
            &mut notes,
        )?,
        None if bundle => Vec::new(),
        None => return Err(Unsupported::Draw),
    };
    if bundle {
        let extra: Vec<&RecipeClass> = classes
            .iter()
            .filter(|c| c.rule.as_deref().is_some_and(|r| !r.is_empty()))
            .filter(|c| {
                let roll = random.next_double();
                additional_ratio(c.rule.as_deref().unwrap_or("")).is_some_and(|x| roll < x)
            })
            .collect();
        for c in extra {
            drawn.extend(select_items(
                class_items(&recipes(&c.name), character),
                c.number,
                owner,
                random,
                &mut notes,
            )?);
        }
    }
    let rewards = drawn
        .into_iter()
        .map(|r| {
            let (notify, broadcast) = match r.notify.as_deref().map(|n| n.trim().parse::<i32>()) {
                Some(Ok(1)) if r.number == 1 => {
                    (None, Some("GameUI_Heroes_RandomItem_GetItem_Telepathy"))
                }
                Some(Ok(1)) if r.number > 1 => (
                    None,
                    Some("GameUI_Heroes_RandomItem_GetItem_Plural_Telepathy"),
                ),
                Some(Ok(_)) => (None, None),
                // OpenBundle passes a non-integer NotifyMessage to the request.
                _ if bundle => (r.notify.clone(), None),
                _ => (None, None),
            };
            Reward {
                item_class: r.item_class,
                number: r.number,
                colors: r.colors.map(|c| c.unwrap_or(-1)),
                notify,
                broadcast,
            }
        })
        .collect();
    Ok((rewards, notes))
}

/// A RandomItemV2Recipe row (RandomItemContentsV2), with its Cond_Feature evaluated.
#[derive(Clone, Debug, PartialEq)]
pub struct V2Recipe {
    pub item_class: Option<String>,
    pub count: i32,
    pub probability: f64,
    pub rare: Option<i32>,
    pub character: Option<String>,
    pub item: Option<String>,
    pub feature_enabled: bool,
    pub colors: [i32; 3],
    pub bind: bool,
    pub expire: Option<i32>,
    pub announce: Option<String>,
    pub message: Option<String>,
}

/// RandomItemData: the recipes of one RandomItemID split by RecipeType (0, 1, 2).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct V2Data {
    pub package: Vec<V2Recipe>,
    pub gachapon: Vec<V2Recipe>,
    pub sequence: Vec<V2Recipe>,
}

/// A GiveItemRequest of the session (ItemClassEx as written in the recipe).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct V2Grant {
    pub item_class: String,
    pub count: i32,
    pub colors: [i32; 3],
    pub bind: bool,
}

/// What RandomItemSession.OpenRandomItem collected: the give requests (count 0 entries
/// included, Apply drops them) and the SystemMessages (format, ItemClass ?? "", Count).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct V2Opened {
    pub grants: Vec<V2Grant>,
    pub messages: Vec<(String, String, i32)>,
}

/// RandomItemSession.Random draws (Random.NextDouble).
pub trait DoubleRandom {
    fn next_double(&mut self) -> f64;
}
impl<T: Random> DoubleRandom for T {
    fn next_double(&mut self) -> f64 {
        Random::next_double(self)
    }
}

/// Steps of RandomItemSession this port cannot reproduce; nothing is sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum V2Unsupported {
    /// RareItemID counts are read from and written to RandomItemV2RareCount.
    Rare,
    /// A sequence list reads and writes the per-character RandomItemV2Sequence row.
    Sequence,
    /// '@' AP rewards (GiveNonItemRequest).
    NonItem,
    /// ExpireBySec give requests.
    Expire,
}

/// RandomItemContentsExtension.IsConstraintPassed (player part; the rare part is refused).
fn passes(
    r: &V2Recipe,
    character: i32,
    holds: &dyn Fn(&str) -> bool,
) -> Result<bool, V2Unsupported> {
    if r.character
        .as_ref()
        .is_some_and(|c| *c != character.to_string())
    {
        return Ok(false);
    }
    if let Some(item) = r.item.as_deref().filter(|i| !i.is_empty()) {
        let (negated, class) = match item.strip_prefix('!') {
            Some(class) => (true, class),
            None => (false, item),
        };
        if negated == holds(class) {
            return Ok(false);
        }
    }
    if !r.feature_enabled {
        return Ok(false);
    }
    if r.rare.is_some() {
        return Err(V2Unsupported::Rare);
    }
    Ok(true)
}

/// RandomItemSession.OpenRandomItem(randomItemID): PickRecipe (one gachapon recipe by the
/// summed probabilities, then each package recipe by its own roll) and the '#' recursion up
/// to RecursiveRandomItemMaxDepth.
pub fn open_v2<'a>(
    data: &dyn Fn(&str) -> Option<&'a V2Data>,
    id: &str,
    character: i32,
    holds: &dyn Fn(&str) -> bool,
    random: &mut dyn DoubleRandom,
) -> Result<V2Opened, V2Unsupported> {
    let mut out = V2Opened::default();
    open_v2_at(data, id, 0, character, holds, random, &mut out)?;
    Ok(out)
}

fn open_v2_at<'a>(
    data: &dyn Fn(&str) -> Option<&'a V2Data>,
    id: &str,
    depth: usize,
    character: i32,
    holds: &dyn Fn(&str) -> bool,
    random: &mut dyn DoubleRandom,
    out: &mut V2Opened,
) -> Result<(), V2Unsupported> {
    if depth >= 10 {
        return Ok(());
    }
    let Some(d) = data(id) else {
        return Ok(());
    };
    let mut picked = Vec::new();
    let mut gachapon = Vec::new();
    for r in &d.gachapon {
        if passes(r, character, holds)? {
            gachapon.push(r);
        }
    }
    if !gachapon.is_empty() {
        let total: f64 = gachapon.iter().map(|r| r.probability).sum();
        let mut roll = random.next_double() * total;
        for r in gachapon {
            if roll < r.probability {
                picked.push(r);
                break;
            }
            roll -= r.probability;
        }
    }
    for r in &d.package {
        if passes(r, character, holds)? && random.next_double() < r.probability {
            picked.push(r);
        }
    }
    if !d.sequence.is_empty() {
        return Err(V2Unsupported::Sequence);
    }
    for r in picked {
        match r.item_class.as_deref() {
            None => out.grants.push(V2Grant {
                item_class: "metal_clamp".into(),
                count: 0,
                colors: [-1; 3],
                bind: false,
            }),
            Some(_) if r.count <= 0 => out.grants.push(V2Grant {
                item_class: "metal_clamp".into(),
                count: 0,
                colors: [-1; 3],
                bind: false,
            }),
            Some(c) if c.starts_with('#') => {
                open_v2_at(data, &c[1..], depth + 1, character, holds, random, out)?;
                continue;
            }
            Some(c) if c.starts_with('@') => return Err(V2Unsupported::NonItem),
            Some(c) => {
                if r.expire.is_some() {
                    return Err(V2Unsupported::Expire);
                }
                out.grants.push(V2Grant {
                    item_class: c.to_owned(),
                    count: r.count,
                    colors: r.colors,
                    bind: r.bind,
                });
            }
        }
        // Announce goes to every frontend through NotifyRandomItem (server-wide broadcast,
        // not migrated); '$*' burn shares are not reached outside BurnItems.
        if let Some(m) = &r.message
            && !r.item_class.as_deref().is_some_and(|c| c.starts_with("$*"))
        {
            out.messages
                .push((m.clone(), r.item_class.clone().unwrap_or_default(), r.count));
        }
    }
    Ok(())
}
