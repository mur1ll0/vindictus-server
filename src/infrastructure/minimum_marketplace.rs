//! Marketplace of the PlayerService TradeSystem over game_trade_* (migration 0055): the
//! processors TradeInsertOrder, TradePurchaseItem, TradeCancelMyItem, TradeCategorySearch,
//! TradeItemClassListSearch, TradeRequestMyItem and TradeGetMyItems, with the
//! heroesMarketPlace procedures they call. The source runs the gold, the procedure and the
//! give in separate steps and compensates by hand; here each request is one transaction
//! of the owned character, so a failure leaves nothing behind. Bought, canceled and
//! expired items that do not fit, and the seller's gold, go to the mailbox as
//! AuctionItemMail (ItemGenerator). TradeLog rows of failures are written after rollback.
use super::{
    character_lobby::UserKey,
    minimum_assets::Assets,
    minimum_character::MinimumError,
    minimum_give::{self, Equipment, Request},
    minimum_mail::{self, Changed},
};
use crate::{
    domain::{
        item_attribute::{self, Attribute, Attributes},
        map_partition::OutgoingPacket,
        marketplace::{self as market, Sort, log, order, result, status},
    },
    protocol::{
        DEFAULT_MAX_BODY,
        marketplace::{self as wire, DetailOption, Kind, Message, TradeItemInfo},
    },
};
use sqlx::{PgPool, Postgres, Transaction};
use std::{
    collections::BTreeMap,
    sync::{Mutex, PoisonError},
    time::Duration,
};

fn fail() -> MinimumError {
    MinimumError::Conflict
}
const TIMEOUT: Duration = Duration::from_secs(15);
/// DateTime.MaxValue ticks (DateTime.UtcNow.AddMinutes beyond it throws).
const MAX_TICKS: i64 = 3_155_378_975_999_999_999;
const MINUTE_TICKS: i64 = 600_000_000;
/// Entity IDs a purchase or a cancellation may draw: the given stacks and the mailed item.
pub const GIVE_ENTITY_IDS: usize = 8;
/// Entity IDs of one TradeGetMyItems: one mailed system item per expired registration and
/// per sale; what does not fit waits for the next call.
pub const COLLECT_ENTITY_IDS: usize = 64;

/// What one marketplace request answers.
#[derive(Default)]
pub struct Outcome {
    pub packets: Vec<OutgoingPacket>,
}

fn packet(kind: Kind, message: Message) -> Result<OutgoingPacket, MinimumError> {
    Ok(OutgoingPacket {
        category: kind.category(),
        body: wire::encode(kind, Some(&message), DEFAULT_MAX_BODY)
            .map_err(|_| fail())?
            .to_vec(),
    })
}
fn now() -> Result<i64, MinimumError> {
    Ok(super::lab_clock::utc_now().map_err(|_| fail())?.ticks())
}
fn utf16(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
fn text(raw: &[u8]) -> String {
    let units: Vec<u16> = raw
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_le_bytes(*c))
        .collect();
    String::from_utf16_lossy(&units)
}
/// A SqlParameter longer than the procedure's nvarchar(n)/varchar(n) parameter is cut.
fn cut(value: &str, units: usize) -> String {
    String::from_utf16_lossy(&crate::domain::mail::truncate_units(value, units))
}
fn console(format: &str, parameters: &[String]) -> Result<OutgoingPacket, MinimumError> {
    super::minimum_shop::system_message(super::minimum_shop::SYSTEM, format, parameters)
}
fn marketplace(assets: &Assets) -> Option<&super::item_economy::Marketplace> {
    assets.economy.as_ref()?.marketplace()
}
async fn bounds(tx: &mut Transaction<'_, Postgres>) -> Result<(), MinimumError> {
    sqlx::query("SET LOCAL lock_timeout='5s'")
        .execute(&mut **tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout='10s'")
        .execute(&mut **tx)
        .await?;
    // The mail paths lock the item tables before game_mail; the marketplace does the same
    // and takes its rows in between, so the lock order never inverts.
    sqlx::query("LOCK TABLE game_items,game_equippables IN SHARE ROW EXCLUSIVE MODE")
        .execute(&mut **tx)
        .await?;
    Ok(())
}
/// TradeLogger.Log (AddTradeLog); failures of the log never undo the operation.
async fn trade_log<'e>(
    db: impl sqlx::PgExecutor<'e>,
    event: i32,
    tid: Option<i64>,
    cid: Option<i64>,
    oid: Option<i64>,
    extra: Option<&str>,
) {
    let logged = now().unwrap_or_default();
    if sqlx::query("INSERT INTO game_trade_logs(logged_ticks,event,tid,cid,oid,extra) VALUES($1,$2,$3,$4,$5,$6)")
        .bind(logged)
        .bind(event)
        .bind(tid)
        .bind(cid)
        .bind(oid)
        .bind(extra.map(|e| cut(e, 1024)))
        .execute(db)
        .await
        .is_err()
    {
        tracing::warn!(event, "trade log not written; values omitted");
    }
}

// --- Registrations. ---

const TRADE_COLUMNS: &str = "t.tid,t.cid,t.character_name,t.item_class,t.item_count,t.item_price,t.close_ticks,t.attribute_ex,t.color1,t.color2,t.color3,t.max_armor_condition_mod,t.trade_type";

/// One TradeItem row as the procedures return it.
#[derive(sqlx::FromRow, Clone, Debug)]
struct TradeRow {
    tid: i64,
    cid: i64,
    character_name: String,
    item_class: String,
    item_count: i32,
    item_price: i32,
    close_ticks: i64,
    attribute_ex: Option<String>,
    color1: Option<i32>,
    color2: Option<i32>,
    color3: Option<i32>,
    max_armor_condition_mod: Option<i32>,
    trade_type: Option<i16>,
}
impl TradeRow {
    fn colors(&self) -> [i32; 3] {
        [self.color1, self.color2, self.color3].map(|c| c.unwrap_or(-1))
    }
    /// TradeItem.ToLogString with the purchased count.
    fn log_string(&self, count: i64) -> String {
        format!(
            "{}/C:{count}/P:{}",
            market::attributed_class_ex(&self.item_class, self.attribute_ex.as_deref()),
            self.item_price
        )
    }
    /// TradeItem(...).ToTradeItemInfo(); `prices` is the TradeManager.Prices lookup of the
    /// constructor with the service (the _Attribute procedures use the one without it).
    fn info(&self, now: i64, prices: Option<&Prices>) -> TradeItemInfo {
        let range = prices
            .and_then(|p| p.get(&lookup_key(&self.item_class, self.attribute_ex.as_deref())?));
        TradeItemInfo {
            tid: self.tid,
            cid: self.cid,
            character_name: Some(self.character_name.clone()),
            item_class: Some(self.item_class.clone()),
            item_count: i64::from(self.item_count),
            item_price: i64::from(self.item_price),
            close_date: market::remain_seconds(self.close_ticks, now),
            attribute_ex: self.attribute_ex.clone(),
            max_armor_condition: self.max_armor_condition_mod.unwrap_or(0),
            color1: self.colors()[0],
            color2: self.colors()[1],
            color3: self.colors()[2],
            min_price: range.map_or(0, |r| r.min),
            max_price: range.map_or(0, |r| r.max),
            avg_price: range.map_or(0, |r| r.price),
            trade_type: u8::try_from(self.trade_type.unwrap_or(0)).unwrap_or(0),
        }
    }
}
/// ItemClassExBuilder.Parse of an AttributeEX ("" and unreadable text are empty).
fn parse_attributes(attribute_ex: Option<&str>) -> Attributes {
    match attribute_ex {
        Some(a) if !a.is_empty() => item_attribute::parse_class_ex(&format!("x[{a}]"))
            .map(|(_, attrs)| attrs)
            .unwrap_or_default(),
        _ => Attributes::default(),
    }
}
/// ItemClassExBuilder.Build(attributes): "" without attributes.
fn build_attributes(attributes: &Attributes) -> Result<String, MinimumError> {
    if attributes.is_empty() {
        return Ok(String::new());
    }
    let built = item_attribute::build_class_ex("x", attributes).map_err(|_| fail())?;
    built
        .strip_prefix("x[")
        .and_then(|b| b.strip_suffix(']'))
        .map(str::to_owned)
        .ok_or_else(fail)
}

// --- TradeManager.Prices (ItemPriceInfoInItemSlot without ItemPriceInfoInService). ---

type Prices = BTreeMap<String, market::PriceRange>;
static PRICES: Mutex<Option<(i64, Prices)>> = Mutex::new(None);

/// TradeItem's lookup key: the AttributeEX with QUALITY:(2) added when absent.
fn lookup_key(class: &str, attribute_ex: Option<&str>) -> Option<String> {
    let mut list = parse_attributes(attribute_ex).as_slice().to_vec();
    if !list.iter().any(|a| a.name == "QUALITY") {
        list.push(Attribute {
            name: "QUALITY".into(),
            value: String::new(),
            arg: 2,
            arg2: 0,
        });
    }
    item_attribute::build_class_ex(class, &Attributes::new(list).ok()?).ok()
}
/// TradeManager.UpdatePrice's key: QUALITY of level 2 removed (Arg, else the Value),
/// the attributes ordered by name.
fn statistics_key(class: &str, attribute_ex: Option<&str>) -> Option<String> {
    let mut list = parse_attributes(attribute_ex).as_slice().to_vec();
    if let Some(q) = list.iter_mut().find(|a| a.name == "QUALITY") {
        let level = if q.arg > 0 {
            q.value = String::new();
            q.arg
        } else {
            q.value.trim().parse().unwrap_or(0)
        };
        if level == 2 {
            list.retain(|a| a.name != "QUALITY");
        }
    }
    list.sort_by(|a, b| a.name.cmp(&b.name));
    item_attribute::build_class_ex(class, &Attributes::new(list).ok()?).ok()
}
/// JobTradeItemAvgPrice: the purchases of the last day (TradeItem_History Status 2)
/// grouped by ItemClass and AttributeEX with at least ten units, merged into
/// TradeItem_Statistics (a NULL AttributeEX never matches, as MERGE's equality).
const AVG_PRICE: &str = "WITH w AS (SELECT item_class,attribute_ex,item_price,COALESCE(buy_count,1)::bigint AS buy_count,item_price::bigint*COALESCE(buy_count,1) AS sales FROM game_trade_item_history WHERE log_ticks>$1 AND status=2), p AS (SELECT item_class,attribute_ex,min(item_price) AS min_price,max(item_price) AS max_price,sum(buy_count)::bigint AS buy_count,sum(sales)::bigint AS sales FROM w GROUP BY item_class,attribute_ex HAVING sum(buy_count)>=$2)";
async fn refresh_prices(pool: &PgPool, at: i64) -> Result<Prices, MinimumError> {
    let mut tx = pool.begin().await?;
    sqlx::query(&format!("{AVG_PRICE} UPDATE game_trade_item_statistics s SET min_price=p.min_price,max_price=p.max_price,buy_count=p.buy_count,sales=p.sales,last_update_ticks=$3 FROM p WHERE s.item_class=p.item_class AND s.attribute_ex=p.attribute_ex"))
        .bind(at - market::DAY_TICKS)
        .bind(market::AVG_PRICE_MIN_BUYS)
        .bind(at)
        .execute(&mut *tx)
        .await?;
    sqlx::query(&format!("{AVG_PRICE} INSERT INTO game_trade_item_statistics(item_class,attribute_ex,min_price,max_price,buy_count,sales,last_update_ticks) SELECT p.item_class,p.attribute_ex,p.min_price,p.max_price,p.buy_count,p.sales,$3 FROM p WHERE NOT EXISTS(SELECT 1 FROM game_trade_item_statistics s WHERE s.item_class=p.item_class AND s.attribute_ex=p.attribute_ex)"))
        .bind(at - market::DAY_TICKS)
        .bind(market::AVG_PRICE_MIN_BUYS)
        .bind(at)
        .execute(&mut *tx)
        .await?;
    let rows: Vec<(String, Option<String>, i32, i32, i64, i64)> = sqlx::query_as(
        "SELECT item_class,attribute_ex,min_price,max_price,buy_count,sales FROM game_trade_item_statistics ORDER BY id LIMIT 100000",
    )
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    let mut groups: BTreeMap<String, Vec<(i32, i32, i64, i64)>> = BTreeMap::new();
    for (class, attribute_ex, min, max, buys, sales) in rows {
        if let Some(key) = statistics_key(&class, attribute_ex.as_deref()) {
            groups.entry(key).or_default().push((min, max, buys, sales));
        }
    }
    Ok(groups
        .into_iter()
        .filter_map(|(k, rows)| market::price_range(&rows).map(|r| (k, r)))
        .collect())
}
/// TradeManager.Prices, refreshed every hour (ScheduleUpdatePrice) on first use.
async fn prices(pool: &PgPool, assets: &Assets) -> Option<Prices> {
    if !marketplace(assets)?.features.price_info_in_trade_item {
        return None;
    }
    let at = now().ok()?;
    {
        let cached = PRICES.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some((refreshed, prices)) = cached.as_ref()
            && at.saturating_sub(*refreshed) < market::PRICE_REFRESH_TICKS
        {
            return Some(prices.clone());
        }
    }
    match refresh_prices(pool, at).await {
        Ok(prices) => {
            *PRICES.lock().unwrap_or_else(PoisonError::into_inner) = Some((at, prices.clone()));
            Some(prices)
        }
        Err(_) => {
            tracing::warn!("trade price refresh refused; values omitted");
            Some(Prices::new())
        }
    }
}

// --- Searches. ---

fn sort_sql(sort: Sort) -> &'static str {
    match sort {
        Sort::LevelAsc => {
            "t.trade_type DESC NULLS LAST,t.required_level,lower(t.item_class) COLLATE \"C\",t.item_price,t.tid"
        }
        Sort::LevelDesc => {
            "t.trade_type DESC NULLS LAST,t.required_level DESC,lower(t.item_class) COLLATE \"C\",t.item_price,t.tid"
        }
        Sort::PriceAsc => {
            "t.trade_type DESC NULLS LAST,t.item_price,t.required_level DESC,lower(t.item_class) COLLATE \"C\",t.tid"
        }
        Sort::PriceDesc => {
            "t.trade_type DESC NULLS LAST,t.item_price DESC,t.required_level DESC,lower(t.item_class) COLLATE \"C\",t.tid"
        }
    }
}
/// The TradeItem rows a search may return.
enum Scope<'a> {
    /// TradeItemListByCategory: open rows, the level range and the categories ('' or
    /// blank leaves a level out; the sub-category needs the category).
    Category {
        category: String,
        sub: String,
        min: i32,
        max: i32,
    },
    /// TradeItemList: ItemClassSeq of the listed classes, closed rows included.
    Classes(&'a [String]),
}
/// The @Attribute rows of a search (Type, Value, OperatorType).
struct Options {
    types: Vec<Option<String>>,
    values: Vec<i32>,
    ops: Vec<i32>,
}
/// TradeItemList_Attribute/TradeItemListByCategory_Attribute: per OperatorType the rows
/// that satisfy every option of that type (1 under the price, 2 equal, 3 at least, 4–6
/// any equal, 10–15 the Color1 bounds with the procedure's own HAVING counts and guards),
/// then the rows found by every requested type.
const ATTRIBUTE_FILTER: &str = "opt AS (SELECT o.type,o.value,o.op FROM unnest($A::text[],$B::int[],$C::int[]) AS o(type,value,op)), n AS (SELECT op,count(*) AS c FROM opt GROUP BY op), base AS (SELECT t.tid,t.item_price FROM game_trade_items t WHERE SCOPE), r AS ( \
 (SELECT b.tid,1 AS op FROM base b JOIN opt o ON o.op=1 AND o.value>=b.item_price GROUP BY b.tid HAVING count(*)=(SELECT c FROM n WHERE op=1) ORDER BY b.tid LIMIT 10000) \
 UNION ALL SELECT a.tid,2 FROM base b JOIN game_trade_item_attributes a ON a.tid=b.tid JOIN opt o ON o.op=2 AND lower(o.type)=lower(a.type) AND o.value=a.value GROUP BY a.tid HAVING count(*)=(SELECT c FROM n WHERE op=2) \
 UNION ALL SELECT a.tid,3 FROM base b JOIN game_trade_item_attributes a ON a.tid=b.tid JOIN opt o ON o.op=3 AND lower(o.type)=lower(a.type) AND o.value<=a.value GROUP BY a.tid HAVING count(*)=(SELECT c FROM n WHERE op=3) \
 UNION ALL SELECT DISTINCT a.tid,o.op FROM base b JOIN game_trade_item_attributes a ON a.tid=b.tid JOIN opt o ON o.op IN (4,5,6) AND lower(o.type)=lower(a.type) AND o.value=a.value \
 UNION ALL SELECT a.tid,10 FROM base b JOIN game_trade_item_attributes a ON a.tid=b.tid JOIN opt o ON o.op=10 AND lower(o.type)=lower(a.type) AND o.value<=a.value WHERE EXISTS(SELECT 1 FROM n WHERE op=10) GROUP BY a.tid HAVING count(*)=(SELECT c FROM n WHERE op=14) \
 UNION ALL SELECT a.tid,11 FROM base b JOIN game_trade_item_attributes a ON a.tid=b.tid JOIN opt o ON o.op=11 AND lower(o.type)=lower(a.type) AND o.value>=a.value WHERE EXISTS(SELECT 1 FROM n WHERE op=10) GROUP BY a.tid HAVING count(*)=(SELECT c FROM n WHERE op=11) \
 UNION ALL SELECT a.tid,12 FROM base b JOIN game_trade_item_attributes a ON a.tid=b.tid JOIN opt o ON o.op=12 AND lower(o.type)=lower(a.type) AND o.value<=a.value WHERE EXISTS(SELECT 1 FROM n WHERE op=12) GROUP BY a.tid HAVING count(*)=(SELECT c FROM n WHERE op=12) \
 UNION ALL SELECT a.tid,13 FROM base b JOIN game_trade_item_attributes a ON a.tid=b.tid JOIN opt o ON o.op=13 AND lower(o.type)=lower(a.type) AND o.value>=a.value WHERE EXISTS(SELECT 1 FROM n WHERE op=12) GROUP BY a.tid HAVING count(*)=(SELECT c FROM n WHERE op=13) \
 UNION ALL SELECT a.tid,14 FROM base b JOIN game_trade_item_attributes a ON a.tid=b.tid JOIN opt o ON o.op=14 AND lower(o.type)=lower(a.type) AND o.value<=a.value WHERE EXISTS(SELECT 1 FROM n WHERE op=14) GROUP BY a.tid HAVING count(*)=(SELECT c FROM n WHERE op=14) \
 UNION ALL SELECT a.tid,15 FROM base b JOIN game_trade_item_attributes a ON a.tid=b.tid JOIN opt o ON o.op=15 AND lower(o.type)=lower(a.type) AND o.value>=a.value WHERE EXISTS(SELECT 1 FROM n WHERE op=14) GROUP BY a.tid HAVING count(*)=(SELECT c FROM n WHERE op=15) \
), hit AS (SELECT tid FROM r GROUP BY tid HAVING sum(1::bigint<<op)=(SELECT sum(DISTINCT 1::bigint<<op) FROM opt))";

/// Runs one search page; Err is a procedure error (ExceptionOccured).
async fn search(
    pool: &PgPool,
    scope: Scope<'_>,
    options: Option<Options>,
    sort: Sort,
    window: (i64, i64),
    now: i64,
) -> Result<Vec<TradeRow>, MinimumError> {
    let (scope_sql, next) = match &scope {
        Scope::Category { .. } => (
            "t.close_ticks>$1 AND t.required_level BETWEEN $2 AND $3 AND ($4='' OR lower(rtrim(t.trade_category,' '))=$4) AND ($4='' OR $5='' OR lower(rtrim(t.trade_category_sub,' '))=$5)",
            6,
        ),
        Scope::Classes(_) => ("lower(rtrim(t.item_class,' '))=ANY($1::text[])", 2),
    };
    let sql = match &options {
        None => format!(
            "SELECT {TRADE_COLUMNS} FROM game_trade_items t WHERE {scope_sql} ORDER BY {} OFFSET ${} LIMIT ${}",
            sort_sql(sort),
            next,
            next + 1
        ),
        Some(_) => {
            let filter = ATTRIBUTE_FILTER
                .replace("SCOPE", scope_sql)
                .replace("$A", &format!("${next}"))
                .replace("$B", &format!("${}", next + 1))
                .replace("$C", &format!("${}", next + 2));
            format!(
                "WITH {filter} SELECT {TRADE_COLUMNS} FROM game_trade_items t JOIN hit h ON h.tid=t.tid ORDER BY {} OFFSET ${} LIMIT ${}",
                sort_sql(sort),
                next + 3,
                next + 4
            )
        }
    };
    let mut q = sqlx::query_as::<_, TradeRow>(&sql);
    q = match scope {
        Scope::Category {
            category,
            sub,
            min,
            max,
        } => q.bind(now).bind(min).bind(max).bind(category).bind(sub),
        Scope::Classes(classes) => q.bind(classes.to_vec()),
    };
    if let Some(o) = options {
        q = q.bind(o.types).bind(o.values).bind(o.ops);
    }
    Ok(q.bind(window.0).bind(window.1).fetch_all(pool).await?)
}
/// The DetailOption rows of a search: a null element or an OperatorType whose POWER(2, n)
/// overflows Int32 throws.
fn options(list: &[Option<DetailOption>]) -> Result<Option<Options>, ()> {
    if list.is_empty() {
        return Ok(None);
    }
    let mut o = Options {
        types: Vec::new(),
        values: Vec::new(),
        ops: Vec::new(),
    };
    for d in list {
        let d = d.as_ref().ok_or(())?;
        if d.search_type > 30 {
            return Err(());
        }
        o.types.push(d.key.clone());
        o.values.push(d.value);
        o.ops.push(i32::from(d.search_type));
    }
    Ok(Some(o))
}
fn search_result(
    unique_number: i32,
    code: i32,
    rows: Option<Vec<TradeItemInfo>>,
) -> Result<Outcome, MinimumError> {
    // OnComplete: IsMoreResult when a whole chunk came back; OnFail: no list.
    let (list, more) = match (code, rows) {
        (result::SUCCESS, Some(rows)) => {
            let more = rows.len() == market::CHUNK_SIZE as usize;
            (Some(rows.into_iter().map(Some).collect()), more)
        }
        _ => (None, false),
    };
    Ok(Outcome {
        packets: vec![packet(
            Kind::TradeSearchResult,
            Message::TradeSearchResult(wire::TradeSearchResult {
                trade_item_list: list,
                unique_number,
                is_more_result: more,
                result: code,
            }),
        )?],
    })
}
async fn page_of(
    pool: &PgPool,
    assets: &Assets,
    scope: Scope<'_>,
    detail: &[Option<DetailOption>],
    order: i32,
    descending: bool,
    page_no: i32,
) -> Result<Result<Vec<TradeItemInfo>, i32>, MinimumError> {
    let Ok(options) = options(detail) else {
        return Ok(Err(result::EXCEPTION_OCCURED));
    };
    let window = match market::page(page_no, market::CHUNK_SIZE) {
        Err(market::ProcedureError) => return Ok(Err(result::EXCEPTION_OCCURED)),
        Ok(None) => return Ok(Err(result::EMPTY_LIST)),
        Ok(Some(w)) => w,
    };
    let at = now()?;
    let attributed = options.is_some();
    let rows = match search(
        pool,
        scope,
        options,
        Sort::new(order, descending),
        window,
        at,
    )
    .await
    {
        Ok(rows) => rows,
        Err(_) => return Ok(Err(result::EXCEPTION_OCCURED)),
    };
    if rows.is_empty() {
        return Ok(Err(result::EMPTY_LIST));
    }
    let prices = if attributed {
        None
    } else {
        prices(pool, assets).await
    };
    Ok(Ok(rows
        .iter()
        .map(|r| {
            let mut info = r.info(at, prices.as_ref());
            // SpCaller._Attribute readers: AttributeEX null becomes "".
            if attributed && info.attribute_ex.is_none() {
                info.attribute_ex = Some(String::new());
            }
            info
        })
        .collect()))
}

/// TradeCategorySearchMessage → TradeCategorySearchProcessor (chunk size 100).
pub async fn category_search(
    pool: &PgPool,
    assets: &Assets,
    overhead: &mut market::SearchOverhead,
    m: &wire::TradeCategorySearch,
) -> Result<Outcome, MinimumError> {
    if !overhead.check(now()?) {
        return search_result(m.unique_number, result::SEARCH_OVERHEAD, None);
    }
    let Some(detail) = m.detail_options.as_deref() else {
        return search_result(m.unique_number, result::EXCEPTION_OCCURED, None);
    };
    let key = |v: &Option<String>| {
        v.as_deref()
            .unwrap_or("")
            .trim_end_matches(' ')
            .to_lowercase()
    };
    let scope = Scope::Category {
        category: key(&m.trade_category),
        sub: key(&m.trade_category_sub),
        min: m.min_level,
        max: m.max_level,
    };
    match page_of(
        pool,
        assets,
        scope,
        detail,
        m.order,
        m.is_descending,
        m.chunk_page_number,
    )
    .await?
    {
        Ok(rows) => search_result(m.unique_number, result::SUCCESS, Some(rows)),
        Err(code) => search_result(m.unique_number, code, None),
    }
}
/// TradeItemClassListSearchMessage → TradeItemClassListSearchProcessor.
pub async fn class_search(
    pool: &PgPool,
    assets: &Assets,
    overhead: &mut market::SearchOverhead,
    cid: i64,
    m: &wire::TradeItemClassListSearch,
) -> Result<Outcome, MinimumError> {
    if !overhead.check(now()?) {
        return search_result(m.unique_number, result::SEARCH_OVERHEAD, None);
    }
    let Some(list) = m.item_class_list.as_deref() else {
        return search_result(m.unique_number, result::EXCEPTION_OCCURED, None);
    };
    // GenVerifyItemclassFunc: ItemContents.GetItemClassInfo(name) != null.
    let classes = match market::class_list(list, |c| assets.has_item_class(c)) {
        Ok(c) => c,
        Err(system) => {
            trade_log(
                pool,
                log::INVALID_ITEM_CLASS_NAME_QUERY,
                Some(-1),
                Some(cid),
                Some(-1),
                Some("InvalidItemClassNameQuery"),
            )
            .await;
            let code = if system {
                result::SYSTEM_ERROR
            } else {
                result::EXCEPTION_OCCURED
            };
            return search_result(m.unique_number, code, None);
        }
    };
    let Some(detail) = m.detail_options.as_deref() else {
        return search_result(m.unique_number, result::EXCEPTION_OCCURED, None);
    };
    let keys: Vec<String> = classes
        .iter()
        .map(|c| c.trim_end_matches(' ').to_lowercase())
        .collect();
    match page_of(
        pool,
        assets,
        Scope::Classes(&keys),
        detail,
        m.order,
        m.is_descending,
        m.chunk_page_number,
    )
    .await?
    {
        Ok(rows) => search_result(m.unique_number, result::SUCCESS, Some(rows)),
        Err(code) => search_result(m.unique_number, code, None),
    }
}
/// TradeRequestMyItemMessage → MyTradeItemList (open rows, by ItemClass and price).
pub async fn my_items(pool: &PgPool, assets: &Assets, cid: i64) -> Result<Outcome, MinimumError> {
    let at = now()?;
    let rows: Vec<TradeRow> = sqlx::query_as(&format!(
        "SELECT {TRADE_COLUMNS} FROM game_trade_items t WHERE t.cid=$1 AND t.close_ticks>$2 ORDER BY lower(t.item_class) COLLATE \"C\",t.item_price,t.tid LIMIT 1000"
    ))
    .bind(cid)
    .bind(at)
    .fetch_all(pool)
    .await?;
    let (list, code) = if rows.is_empty() {
        (None, result::EMPTY_LIST)
    } else {
        let prices = prices(pool, assets).await;
        (
            Some(
                rows.iter()
                    .map(|r| Some(r.info(at, prices.as_ref())))
                    .collect(),
            ),
            result::SUCCESS,
        )
    };
    Ok(Outcome {
        packets: vec![packet(
            Kind::TradeMyItemInfos,
            Message::TradeMyItemInfos(wire::TradeMyItemInfos {
                trade_item_list: list,
                result: code,
            }),
        )?],
    })
}

// --- Registration (TradeInsertOrderProcessor and ItemTradeProxy.InsertTradeOrder). ---

fn order_result(code: i32) -> Result<Outcome, MinimumError> {
    Ok(Outcome {
        packets: vec![packet(
            Kind::InsertTradeOrderResult,
            Message::InsertTradeOrderResult(code),
        )?],
    })
}
#[derive(sqlx::FromRow)]
struct OwnedItem {
    item_class_utf16: Vec<u8>,
    count: i32,
    expire_ticks: Option<i64>,
    tradable: bool,
}
#[derive(sqlx::FromRow)]
struct EquipRow {
    color1: i32,
    color2: i32,
    color3: i32,
    reduced_durability: i32,
    max_durability_bonus: i32,
}
/// ItemContents.GetShopItemPrice(class, "gold"): the lowest PriceCount/Count of the
/// enabled gold ShopInfo rows of the class, -1 without one.
fn shop_gold_price(assets: &Assets, class: &str) -> i32 {
    let Some(shops) = assets.shops() else {
        return -1;
    };
    shops
        .all()
        .filter(|r| {
            r.item_class == class
                && r.price_item_class.to_lowercase() == "gold"
                && r.count >= 1
                && assets.feature_enabled(r.feature.as_deref())
        })
        .map(|r| r.price_count / r.count)
        .min()
        .unwrap_or(-1)
}
/// Item.IsAuctionable and the Item facts the registration reads.
struct Listed {
    class: String,
    equippable: bool,
    auctionable: bool,
    expire_ticks: Option<i64>,
    tradable_class: bool,
    attributes: Attributes,
    equipment: Option<EquipRow>,
    max_durability: i32,
    stat: Option<crate::domain::item_stats::ItemStat>,
    category: String,
    category_sub: String,
    required_level: i32,
}
async fn listed(
    tx: &mut Transaction<'_, Postgres>,
    assets: &Assets,
    cid: i64,
    item_id: i64,
    at: i64,
) -> Result<Option<(Listed, i32)>, MinimumError> {
    let item: Option<OwnedItem> = sqlx::query_as(
        "SELECT item_class_utf16,count,expire_ticks,tradable FROM game_items WHERE id=$1 AND owner_id=$2 FOR UPDATE",
    )
    .bind(item_id)
    .bind(cid)
    .fetch_optional(&mut **tx)
    .await?;
    let Some(item) = item else {
        return Ok(None);
    };
    let indexed = assets
        .stats_index
        .class(Some(&item.item_class_utf16))
        .ok_or_else(fail)?;
    let economy = assets
        .economy
        .as_ref()
        .and_then(|e| e.get(indexed.source_index))
        .ok_or_else(fail)?;
    let attributes = super::minimum_inventory::load_attributes(tx, &[item_id])
        .await?
        .remove(&item_id)
        .unwrap_or_default();
    let equipment: Option<EquipRow> = sqlx::query_as(
        "SELECT color1,color2,color3,reduced_durability,max_durability_bonus FROM game_equippables WHERE id=$1",
    )
    .bind(item_id)
    .fetch_optional(&mut **tx)
    .await?;
    let class = text(&item.item_class_utf16);
    let (stat, max_durability, has_durability) = if indexed.equippable {
        let base = assets
            .stats_index
            .get(Some(&item.item_class_utf16))
            .ok_or_else(fail)?;
        let has_durability = base.fields()[0] > 0;
        let context = super::minimum_item_stat::context(tx, cid).await?;
        let stat = super::minimum_item_stat::equipment_with(
            assets,
            &item.item_class_utf16,
            &attributes,
            Some(&context),
        )?
        .stat;
        let bonus = equipment.as_ref().map_or(0, |e| e.max_durability_bonus);
        let max = if has_durability {
            stat.fields()[0].wrapping_add(bonus)
        } else {
            0
        };
        (Some(stat), max, has_durability)
    } else {
        (None, 0, false)
    };
    // Item.Durability: Max(0, MaxDurability - ReducedDurability); IsRepairable when lower.
    let durability = if max_durability == 0 {
        0
    } else {
        (max_durability - equipment.as_ref().map_or(0, |e| e.reduced_durability)).max(0)
    };
    let restored = attributes
        .get("RESTORED")
        .is_some_and(|a| a.value.trim().parse::<i32>().is_ok_and(|v| v != 0));
    let protected = attributes
        .get("VALUE")
        .is_some_and(|a| a.value.to_uppercase() == "SKILLENHANCE_PROTECTED" && a.arg > 0);
    let tradable_class = economy.trade_restriction.is_none();
    let auctionable = tradable_class
        && !item.expire_ticks.is_some_and(|e| e < at)
        && max_durability - durability <= 0
        && item.tradable
        && !restored
        && class != market::ENCHANT_SCROLL
        && has_durability == (max_durability > 0)
        && !protected;
    Ok(Some((
        Listed {
            class,
            equippable: indexed.equippable,
            auctionable,
            expire_ticks: item.expire_ticks,
            tradable_class,
            attributes,
            equipment,
            max_durability,
            stat,
            category: economy.trade_category.clone().unwrap_or_default(),
            category_sub: economy.trade_category_sub.clone().unwrap_or_default(),
            required_level: economy.required_level,
        },
        item.count,
    )))
}
/// TradeManager.MakeDetailOption(item, price): the item's stats, ENHANCE, QUALITY, the
/// enchant IDs, MAXDURABILITY/100, the Color1 bytes and PRICE. Err refuses the order
/// (the source throws on what it cannot read: combine-craft parts are not migrated).
fn detail_options(assets: &Assets, l: &Listed, price: i32) -> Result<Vec<(String, i32)>, ()> {
    let mut out = Vec::new();
    if l.equippable {
        let stat = l.stat.as_ref().ok_or(())?;
        for e in stat.character_entries() {
            if e.value != 0 {
                out.push((
                    String::from_utf16(e.name).map_err(|_| ())?.to_uppercase(),
                    e.value,
                ));
            }
        }
        let enhance = match l.attributes.get("ENHANCE") {
            Some(a) => a.value.trim().parse::<i32>().map_err(|_| ())?,
            None => 0,
        };
        out.push(("ENHANCE".into(), enhance));
        out.push((
            "QUALITY".into(),
            l.attributes.get("QUALITY").map_or(2, |a| a.arg),
        ));
        for key in ["PREFIX", "SUFFIX"] {
            if let Some(a) = l.attributes.get(key)
                && let Some(id) = assets
                    .enhance_contents()
                    .and_then(|c| c.enchant_id(&a.value))
            {
                out.push((key.into(), id));
            }
        }
        if l.max_durability > 0 {
            out.push(("MAXDURABILITY".into(), l.max_durability / 100));
        }
        if l.attributes
            .as_slice()
            .iter()
            .any(|a| a.name.starts_with("PS_"))
        {
            return Err(());
        }
        let color = l.equipment.as_ref().map_or(-1, |e| e.color1);
        let [r, g, b] = market::color_bytes(color);
        out.push(("COLOR1R".into(), r));
        out.push(("COLOR1G".into(), g));
        out.push(("COLOR1B".into(), b));
    } else if l.attributes.get("VARIABLESTAT").is_some() {
        return Err(());
    }
    out.push(("PRICE".into(), price));
    if out.iter().any(|(k, _)| k.len() > 20) {
        return Err(());
    }
    Ok(out)
}

/// The TradeItemDataInfo of AddTradeItem.
struct Registration<'a> {
    cid: i64,
    name: &'a str,
    item_id: i64,
    category: &'a str,
    sub: &'a str,
    class: &'a str,
    required_level: i32,
    count: i32,
    price: i32,
    open: i64,
    close: i64,
    attribute_ex: &'a str,
    colors: [i32; 3],
    bonus: i32,
    trade_type: u8,
    options: &'a [(String, i32)],
}
/// AddTradeItem: the TradeCategoryInfo MERGE, the TradeItem row, its DetailOption rows
/// and the history row (Status 1). The nvarchar(50)/nvarchar(2048) parameters are cut.
async fn insert_registration(
    tx: &mut Transaction<'_, Postgres>,
    r: &Registration<'_>,
) -> Result<i64, MinimumError> {
    let category = cut(r.category, 50);
    let sub = cut(r.sub, 50);
    let class = cut(r.class, 50);
    sqlx::query("INSERT INTO game_trade_category_info(trade_category,trade_category_sub,item_class,required_level) VALUES($1,$2,$3,$4) ON CONFLICT DO NOTHING")
        .bind(&category)
        .bind(&sub)
        .bind(&class)
        .bind(r.required_level)
        .execute(&mut **tx)
        .await?;
    let seq: i32 = sqlx::query_scalar("SELECT item_class_seq FROM game_trade_category_info WHERE trade_category=$1 AND trade_category_sub=$2 AND item_class=$3 AND required_level=$4")
        .bind(&category)
        .bind(&sub)
        .bind(&class)
        .bind(r.required_level)
        .fetch_one(&mut **tx)
        .await?;
    let tid: i64 = sqlx::query_scalar("INSERT INTO game_trade_items(cid,character_name,item_id,trade_category,trade_category_sub,item_class,required_level,item_count,item_price,open_ticks,close_ticks,item_class_seq,attribute_ex,color1,color2,color3,anti_bind,max_armor_condition_mod,armor_condition,trade_type) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,0,$17,$17,$18) RETURNING tid")
        .bind(r.cid)
        .bind(cut(r.name, 50))
        .bind(r.item_id)
        .bind(&category)
        .bind(&sub)
        .bind(&class)
        .bind(r.required_level)
        .bind(r.count)
        .bind(r.price)
        .bind(r.open)
        .bind(r.close)
        .bind(seq)
        .bind(cut(r.attribute_ex, 2048))
        .bind(r.colors[0])
        .bind(r.colors[1])
        .bind(r.colors[2])
        .bind(r.bonus)
        .bind(i16::from(r.trade_type))
        .fetch_one(&mut **tx)
        .await?;
    for (key, value) in r.options {
        sqlx::query(
            "INSERT INTO game_trade_item_attributes(tid,type,value,log_ticks) VALUES($1,$2,$3,$4)",
        )
        .bind(tid)
        .bind(key)
        .bind(value)
        .bind(r.open)
        .execute(&mut **tx)
        .await?;
    }
    sqlx::query(&format!("INSERT INTO game_trade_item_history(tid,cid,character_name,item_id,trade_category,trade_category_sub,item_class,required_level,item_count,item_price,open_ticks,close_ticks,item_class_seq,attribute_ex,color1,color2,color3,anti_bind,max_armor_condition_mod,armor_condition,trade_type,status,log_ticks) SELECT tid,cid,character_name,item_id,trade_category,trade_category_sub,item_class,required_level,item_count,item_price,open_ticks,close_ticks,item_class_seq,attribute_ex,color1,color2,color3,anti_bind,max_armor_condition_mod,armor_condition,trade_type,{},$2 FROM game_trade_items WHERE tid=$1", status::ADDED))
        .bind(tid)
        .bind(r.open)
        .execute(&mut **tx)
        .await?;
    Ok(tid)
}

/// InsertTradeOrderMessage (after UseSecondPassword) → TradeInsertOrderProcessor.
pub async fn insert_order(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    m: &wire::InsertTradeOrder,
) -> Result<Outcome, MinimumError> {
    let Some(contents) = marketplace(assets) else {
        tracing::warn!("marketplace contents absent; order refused");
        return order_result(order::INTERNAL_ERROR);
    };
    let features = &contents.features;
    // ItemServiceProxy: PremiumTradeTicket disabled refuses a premium order at once.
    if !features.premium_ticket && m.trade_type > 0 {
        return order_result(order::PREMIUM_TICKET_DISABLE);
    }
    let result = tokio::time::timeout(TIMEOUT, async {
        let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
        bounds(&mut tx).await?;
        let at = now()?;
        let refuse = |code| Ok::<_, MinimumError>((code, Vec::new(), None));
        let Some((item, count)) = listed(&mut tx, assets, cid, m.item_id, at).await? else {
            return refuse(order::INTERNAL_ERROR);
        };
        if m.unit_price <= 0 || (item.equippable && m.num > 1) || m.num <= 0 || !item.auctionable
        {
            return refuse(order::INTERNAL_ERROR);
        }
        if item.expire_ticks.is_some() {
            return refuse(order::TIME_LIMIT_ITEM);
        }
        if !item.tradable_class {
            return refuse(order::NOT_TRADABLE);
        }
        let (level, name_utf16): (i32, Vec<u8>) =
            sqlx::query_as("SELECT level,name_utf16 FROM game_character_info WHERE id=$1")
                .bind(cid)
                .fetch_one(&mut *tx)
                .await?;
        if features.level_limit(&item.class) > level {
            return refuse(order::NOT_ENOUGH_LEVEL);
        }
        if features.use_price_limit
            && m.unit_price
                > features.price_limit(
                    contents.max_limit_value(&item.class),
                    shop_gold_price(assets, &item.class),
                )
        {
            return refuse(order::PRICE_OUT_OF_RANGE);
        }
        let ticket = utf16(market::PREMIUM_TICKET);
        if m.trade_type > 0 {
            let tickets: i64 = sqlx::query_scalar(
                "SELECT COALESCE(sum(count),0)::bigint FROM game_items WHERE owner_id=$1 AND item_class_utf16=$2",
            )
            .bind(cid)
            .bind(&ticket)
            .fetch_one(&mut *tx)
            .await?;
            if tickets <= 0 {
                return refuse(order::NO_PREMIUM_TICKET);
            }
        }
        // ItemTradeProxy.InsertTradeOrder.
        if features.over_money_limit(m.unit_price, m.num) {
            return refuse(order::INTERNAL_ERROR);
        }
        let mut changed = Changed::default();
        let destroyed = if m.trade_type == 0 {
            let Some(fee) =
                market::register_fee(features.register_fee, m.unit_price, m.num, m.duration_min, 0)
            else {
                return refuse(order::INTERNAL_ERROR);
            };
            fee == 0
                || minimum_mail::destroy_owned_class(
                    &mut tx,
                    assets,
                    cid,
                    assets.inventory.gold_class(),
                    fee,
                    &mut changed,
                )
                .await?
        } else {
            minimum_mail::destroy_owned_class(&mut tx, assets, cid, &ticket, 1, &mut changed)
                .await?
        };
        if !destroyed
            || m.num > count
            || !minimum_mail::destroy_owned_item(&mut tx, assets, cid, m.item_id, m.num, &mut changed)
                .await?
        {
            return refuse(order::INTERNAL_ERROR);
        }
        let Ok(options) = detail_options(assets, &item, m.unit_price) else {
            return refuse(order::INTERNAL_ERROR);
        };
        let close = i64::from(m.duration_min)
            .checked_mul(MINUTE_TICKS)
            .and_then(|d| at.checked_add(d))
            .filter(|c| (0..=MAX_TICKS).contains(c));
        let Some(close) = close else {
            return refuse(order::INTERNAL_ERROR);
        };
        // AddTradeItem (SERIALIZABLE in the source; the owned character lock serializes
        // one seller's orders): at most 20 open registrations.
        let open: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM game_trade_items WHERE cid=$1 AND close_ticks>$2",
        )
        .bind(cid)
        .bind(at)
        .fetch_one(&mut *tx)
        .await?;
        if open + 1 > market::MAX_OPEN_ORDERS {
            return refuse(order::WEB_ERROR);
        }
        let colors = item
            .equipment
            .as_ref()
            .filter(|_| item.equippable)
            .map_or([-1; 3], |e| [e.color1, e.color2, e.color3]);
        let bonus = item
            .equipment
            .as_ref()
            .filter(|_| item.equippable)
            .map_or(0, |e| e.max_durability_bonus);
        let tid = insert_registration(
            &mut tx,
            &Registration {
                cid,
                name: &text(&name_utf16),
                item_id: m.item_id,
                category: &item.category,
                sub: &item.category_sub,
                class: &item.class,
                required_level: item.required_level,
                count: m.num,
                price: m.unit_price,
                open: at,
                close,
                attribute_ex: &build_attributes(&item.attributes)?,
                colors,
                bonus,
                trade_type: m.trade_type,
                options: &options,
            },
        )
        .await?;
        trade_log(&mut *tx, log::ADD_ITEM_SUCCESS, Some(tid), Some(cid), None, Some("")).await;
        let slots = changed.slots(&mut tx, owner, assets, cid).await?;
        tx.commit().await?;
        Ok((order::OK, slots, Some(tid)))
    })
    .await
    .map_err(|_| fail())??;
    let (code, slots, _) = result;
    let mut packets = Vec::new();
    if !slots.is_empty() {
        packets.push(minimum_mail::inventory_update(slots)?);
    }
    packets.push(packet(
        Kind::InsertTradeOrderResult,
        Message::InsertTradeOrderResult(code),
    )?);
    Ok(Outcome { packets })
}
/// InsertTradeTirOrderMessage → TradeInsertTirOrderProcessor: the laboratory has no
/// TIR_TRADABLE coins (the Tir coin cash system is not migrated), so `numTircoin < Num`
/// answers InternalError_Tir as the source does for an empty balance.
pub fn insert_tir_order() -> Result<Outcome, MinimumError> {
    order_result(order::INTERNAL_ERROR_TIR)
}

// --- ItemGenerator: give to the inventory, else the AuctionItemMail. ---

/// ItemGenerator.CheckCharacterBind(false, info, attributes).
fn check_character_bind(
    assets: &Assets,
    class: &[u8],
    attributes: &Attributes,
) -> Result<bool, MinimumError> {
    let Some(indexed) = assets.stats_index.class(Some(class)) else {
        return Ok(true);
    };
    if !indexed.equippable && !assets.inventory.is_gold(class) {
        return Ok(true);
    }
    let catalog = assets
        .catalog
        .classes
        .get(indexed.source_index)
        .ok_or_else(fail)?;
    let policy = &assets.catalog.policy;
    if policy.is_enabled(catalog.bind.as_deref())?
        || policy.is_enabled(catalog.trade_on_bind.as_deref())?
    {
        return Ok(true);
    }
    let name = text(class);
    if name.starts_with(market::ENCHANT_SCROLL) {
        return Ok(policy.is_enabled(Some("Macro_EnchantTradeRestriction"))?);
    }
    minimum_mail::high_level_equipment(assets, &name, indexed.equippable, attributes)
}
/// One TradeItem to give (GetGiveItemRequest): class, count, colors, MaxArmorCondition as
/// the durability bonus, the AttributeEX attributes.
struct Give<'a> {
    row: &'a TradeRow,
    count: i32,
}
/// ItemGenerator.GiveItem: GiveItem, else MakeSystemItem and the AuctionItemMail, with
/// the console message of each branch. False is the source's failure (SystemError).
#[allow(clippy::too_many_arguments)]
async fn give_item(
    tx: &mut Transaction<'_, Postgres>,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    give: Give<'_>,
    bound: Option<bool>,
    messages: Option<(&str, &str)>,
    mail: (String, String),
    ids: &mut (dyn FnMut() -> Result<i64, MinimumError> + Send),
    packets: &mut Vec<OutgoingPacket>,
) -> Result<bool, MinimumError> {
    let class = utf16(&give.row.item_class);
    let attributes = parse_attributes(give.row.attribute_ex.as_deref());
    let bound = match bound {
        Some(b) => b,
        None => check_character_bind(assets, &class, &attributes)?,
    };
    let Some(info) = minimum_give::class_info(assets, &class)? else {
        return Ok(false);
    };
    // The server's own listings have no item: GiveItem draws the material colors.
    let equipment =
        (info.equippable && give.row.cid != market::auto::SELLER_CID).then(|| Equipment {
            colors: give.row.colors(),
            reduced: 0,
            bonus: give.row.max_armor_condition_mod.unwrap_or(0),
        });
    let request = Request {
        class: &class,
        count: give.count,
        bound,
        equipment,
        attributes: &attributes,
    };
    let shown = || {
        vec![
            market::attributed_class_ex(&give.row.item_class, give.row.attribute_ex.as_deref()),
            give.count.to_string(),
        ]
    };
    match minimum_give::apply(
        tx,
        owner,
        assets,
        cid,
        &[],
        std::slice::from_ref(&request),
        ids,
    )
    .await?
    {
        Ok(applied) => {
            if let Some((console_key, _)) = messages {
                packets.push(console(console_key, &shown())?);
            }
            if !applied.slots.is_empty() {
                packets.push(minimum_mail::inventory_update(applied.slots)?);
            }
            Ok(true)
        }
        Err(_) => {
            // MakeSystemItem requires IsSingle.
            if give.count <= 0
                || give.count > info.max_stack
                || (info.equippable && give.count != 1)
            {
                return Ok(false);
            }
            minimum_mail::send_auction_item(
                tx,
                assets,
                cid,
                ids()?,
                &class,
                give.count,
                bound,
                &attributes,
                equipment,
                &mail.0,
                &mail.1,
            )
            .await?;
            if let Some((_, mail_key)) = messages {
                packets.push(console(mail_key, &shown())?);
            }
            Ok(true)
        }
    }
}

/// TradePurchaseItemMessage (after UseSecondPassword) → TradeManager.BuyTradeItem.
pub async fn purchase(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    m: &wire::TradePurchaseItem,
    entity_ids: Vec<i64>,
) -> Result<Outcome, MinimumError> {
    let mut ids = entity_ids.into_iter();
    let mut next = move || ids.next().ok_or_else(fail);
    let gold = assets.inventory.gold_class().to_vec();
    let result = tokio::time::timeout(TIMEOUT, async {
        let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
        bounds(&mut tx).await?;
        let at = now()?;
        let mut packets = Vec::new();
        // QueryTradeItemDetailedInfo (TradeItemDetail: open rows).
        let row: Option<TradeRow> = sqlx::query_as(&format!(
            "SELECT {TRADE_COLUMNS} FROM game_trade_items t WHERE t.tid=$1 AND t.close_ticks>$2 FOR UPDATE"
        ))
        .bind(m.tid)
        .bind(at)
        .fetch_optional(&mut *tx)
        .await?;
        let row = match row {
            Some(r) if r.item_count >= m.purchase_count => r,
            Some(r) => {
                return Ok::<_, MinimumError>((
                    result::ALREADY_SOLD_OUT,
                    i64::from(r.item_count),
                    packets,
                    None,
                ));
            }
            None => return Ok((result::ALREADY_SOLD_OUT, 0, packets, None)),
        };
        let before = i64::from(row.item_count);
        let total = i64::from(row.item_price) * i64::from(m.purchase_count);
        // DestoryGold((int)num2).
        let mut changed = Changed::default();
        if !minimum_mail::destroy_owned_class(&mut tx, assets, cid, &gold, total as i32, &mut changed)
            .await?
        {
            return Ok((result::NOT_ENOUGH_MONEY, 0, packets, None));
        }
        let deduction = format!("Gold : {total}");
        // BuyTradeItem.
        if m.purchase_count <= 0 {
            return Ok((result::SYSTEM_ERROR, 0, packets, Some((row, deduction))));
        }
        let after = row.item_count - m.purchase_count;
        if after == 0 {
            sqlx::query("DELETE FROM game_trade_items WHERE tid=$1")
                .bind(row.tid)
                .execute(&mut *tx)
                .await?;
        } else {
            sqlx::query("UPDATE game_trade_items SET item_count=$2 WHERE tid=$1")
                .bind(row.tid)
                .bind(after)
                .execute(&mut *tx)
                .await?;
        }
        let oid: i64 = sqlx::query_scalar("INSERT INTO game_trade_orders(tid,item_count,buyer_cid,buy_count,log_ticks) VALUES($1,$2,$3,$4,$5) RETURNING oid")
            .bind(row.tid)
            .bind(row.item_count)
            .bind(cid)
            .bind(m.purchase_count)
            .bind(at)
            .fetch_one(&mut *tx)
            .await?;
        // @ItemPrice*@BuyCount is an Int32 product (an overflow is a procedure error).
        let Ok(seller_gold) = i32::try_from(total) else {
            return Ok((result::SYSTEM_ERROR, 0, Vec::new(), Some((row, deduction))));
        };
        // The server's own listings (laboratory extension) keep no seller gold.
        if row.cid != market::auto::SELLER_CID {
            let gid: i64 = sqlx::query_scalar("INSERT INTO game_trade_gold(cid,tid,oid,gold) VALUES($1,$2,$3,$4) RETURNING gid")
                .bind(row.cid)
                .bind(row.tid)
                .bind(oid)
                .bind(seller_gold)
                .fetch_one(&mut *tx)
                .await?;
            sqlx::query("INSERT INTO game_trade_gold_history(gid,cid,tid,oid,gold,status,log_ticks) VALUES($1,$2,$3,$4,$5,1,$6)")
                .bind(gid)
                .bind(row.cid)
                .bind(row.tid)
                .bind(oid)
                .bind(seller_gold)
                .bind(at)
                .execute(&mut *tx)
                .await?;
        }
        sqlx::query(&format!("INSERT INTO game_trade_item_history(tid,cid,character_name,item_id,trade_category,trade_category_sub,item_class,required_level,item_count,item_price,open_ticks,close_ticks,item_class_seq,attribute_ex,color1,color2,color3,anti_bind,max_armor_condition_mod,armor_condition,trade_type,status,log_ticks,buy_count) SELECT tid,cid,character_name,item_id,trade_category,trade_category_sub,item_class,required_level,$2,item_price,open_ticks,close_ticks,item_class_seq,attribute_ex,color1,color2,color3,anti_bind,max_armor_condition_mod,armor_condition,trade_type,{},$3,$4 FROM game_trade_item_history WHERE tid=$1 AND status={} LIMIT 1", status::BOUGHT, status::ADDED))
            .bind(row.tid)
            .bind(after)
            .bind(at)
            .bind(m.purchase_count)
            .execute(&mut *tx)
            .await?;
        trade_log(&mut *tx, log::PURCHASE_MONEY_DEDUCTION, Some(row.tid), Some(cid), None, Some(&deduction)).await;
        trade_log(&mut *tx, log::PURCHASE_REQUEST, Some(row.tid), Some(cid), Some(oid), Some(&row.log_string(i64::from(m.purchase_count)))).await;
        let slots = changed.slots(&mut tx, owner, assets, cid).await?;
        if !slots.is_empty() {
            packets.push(minimum_mail::inventory_update(slots)?);
        }
        let (title, content) = market::buy_mail(&row.item_class, i64::from(m.purchase_count));
        if !give_item(
            &mut tx,
            owner,
            assets,
            cid,
            Give {
                row: &row,
                count: m.purchase_count,
            },
            None,
            Some((market::BUY_CONSOLE, market::BUY_MAIL_CONSOLE)),
            (title, content),
            &mut next,
            &mut packets,
        )
        .await?
        {
            return Ok((result::SYSTEM_ERROR, 0, Vec::new(), Some((row, deduction))));
        }
        trade_log(&mut *tx, log::PURCHASE_SUCCESS, Some(row.tid), Some(cid), Some(oid), None).await;
        tx.commit().await?;
        Ok((result::SUCCESS, before - i64::from(m.purchase_count), packets, None))
    })
    .await
    .map_err(|_| fail())??;
    let (code, left, mut packets, failed) = result;
    if let Some((row, deduction)) = failed {
        // The source refunds the deducted gold and logs the failure.
        trade_log(
            pool,
            log::PURCHASE_FAIL,
            Some(row.tid),
            Some(cid),
            None,
            Some(&deduction),
        )
        .await;
    }
    packets.push(packet(
        Kind::TradePurchaseItemResult,
        Message::TradePurchaseItemResult(wire::TradePurchaseItemResult {
            unique_number: m.unique_number,
            result: code,
            left_number: left as i32,
        }),
    )?);
    Ok(Outcome { packets })
}

/// TradeCancelMyItemMessage (after UseSecondPassword) → TradeManager.CancelTradeItem.
/// The procedure deletes by TID alone; the laboratory also requires the seller's CID, so a
/// character cannot cancel (and receive) another seller's registration.
pub async fn cancel(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    m: &wire::TradeCancelMyItem,
    entity_ids: Vec<i64>,
) -> Result<Outcome, MinimumError> {
    let mut ids = entity_ids.into_iter();
    let mut next = move || ids.next().ok_or_else(fail);
    let result = tokio::time::timeout(TIMEOUT, async {
        let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
        bounds(&mut tx).await?;
        let at = now()?;
        let mut packets = Vec::new();
        let row: Option<TradeRow> = sqlx::query_as(&format!(
            "SELECT {TRADE_COLUMNS} FROM game_trade_items t WHERE t.tid=$1 AND t.close_ticks>$2 AND t.cid=$3 FOR UPDATE"
        ))
        .bind(m.tid)
        .bind(at)
        .bind(cid)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(row) = row else {
            return Ok::<_, MinimumError>((result::ALREADY_SOLD_OUT, packets, None));
        };
        sqlx::query(&format!("WITH d AS (DELETE FROM game_trade_items WHERE tid=$1 RETURNING *) INSERT INTO game_trade_item_history(tid,cid,character_name,item_id,trade_category,trade_category_sub,item_class,required_level,item_count,item_price,open_ticks,close_ticks,item_class_seq,attribute_ex,color1,color2,color3,anti_bind,max_armor_condition_mod,armor_condition,trade_type,status,log_ticks) SELECT tid,cid,character_name,item_id,trade_category,trade_category_sub,item_class,required_level,item_count,item_price,open_ticks,close_ticks,item_class_seq,attribute_ex,color1,color2,color3,anti_bind,max_armor_condition_mod,armor_condition,trade_type,{},$2 FROM d", status::CANCELED))
            .bind(row.tid)
            .bind(at)
            .execute(&mut *tx)
            .await?;
        trade_log(&mut *tx, log::CANCEL_REQUEST, Some(row.tid), Some(cid), None, Some(&row.log_string(i64::from(row.item_count)))).await;
        let (title, content) = market::canceled_mail(&row.item_class, i64::from(row.item_count));
        if !give_item(
            &mut tx,
            owner,
            assets,
            cid,
            Give {
                row: &row,
                count: row.item_count,
            },
            Some(false),
            Some((market::CANCELED_CONSOLE, market::CANCELED_MAIL_CONSOLE)),
            (title, content),
            &mut next,
            &mut packets,
        )
        .await?
        {
            return Ok((result::SYSTEM_ERROR, Vec::new(), Some(row.tid)));
        }
        trade_log(&mut *tx, log::CANCEL_SUCCESS, Some(row.tid), Some(cid), None, None).await;
        tx.commit().await?;
        Ok((result::SUCCESS, packets, None))
    })
    .await
    .map_err(|_| fail())??;
    let (code, mut packets, failed) = result;
    if let Some(tid) = failed {
        trade_log(
            pool,
            log::CANCEL_FAIL,
            Some(tid),
            Some(cid),
            None,
            Some("Error CancelTradeItem"),
        )
        .await;
    }
    packets.push(packet(
        Kind::TradeCancelMyItemResult,
        Message::TradeCancelMyItemResult(wire::TradeCancelMyItemResult {
            unique_number: m.unique_number,
            result: code,
        }),
    )?);
    Ok(Outcome { packets })
}

// --- TradeManager.GetMyThings (ItemTradeProxy creation and EndTradeSessionMessage). ---

/// JobExpireTradeItem: registrations closed before now move to TradeItem_Expire.
async fn expire_job(tx: &mut Transaction<'_, Postgres>, at: i64) -> Result<(), MinimumError> {
    sqlx::query("WITH d AS (DELETE FROM game_trade_items WHERE close_ticks<$1 RETURNING *) INSERT INTO game_trade_items_expired(tid,cid,character_name,item_id,trade_category,trade_category_sub,item_class,required_level,item_count,item_price,open_ticks,close_ticks,item_class_seq,attribute_ex,color1,color2,color3,anti_bind,max_armor_condition_mod,armor_condition,trade_type) SELECT tid,cid,character_name,item_id,trade_category,trade_category_sub,item_class,required_level,item_count,item_price,open_ticks,close_ticks,item_class_seq,attribute_ex,color1,color2,color3,anti_bind,max_armor_condition_mod,armor_condition,trade_type FROM d")
        .bind(at)
        .execute(&mut **tx)
        .await?;
    Ok(())
}
#[derive(sqlx::FromRow)]
struct GoldRow {
    gid: i64,
    tid: i64,
    oid: i64,
    gold: i32,
    buy_count: Option<i32>,
    item_class: String,
}
/// GetExpiredTradeItem then GetMyGold for the owner: every expired registration and every
/// sale's gold is mailed (AuctionItemMail), with the console count of each.
pub async fn get_my_things(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    entity_ids: Vec<i64>,
) -> Result<Outcome, MinimumError> {
    let budget = entity_ids.len().min(COLLECT_ENTITY_IDS);
    let mut ids = entity_ids.into_iter();
    let gold = assets.inventory.gold_class().to_vec();
    tokio::time::timeout(TIMEOUT, async {
        let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
        bounds(&mut tx).await?;
        let at = now()?;
        expire_job(&mut tx, at).await?;
        let mut packets = Vec::new();
        // ExpiredTradeItem(CID): the rows, history Status 4, then deleted.
        let expired: Vec<TradeRow> = sqlx::query_as(&format!(
            "SELECT {TRADE_COLUMNS} FROM game_trade_items_expired t WHERE t.cid=$1 ORDER BY t.tid LIMIT $2 FOR UPDATE"
        ))
        .bind(cid)
        .bind(i64::try_from(budget / 2).map_err(|_| fail())?)
        .fetch_all(&mut *tx)
        .await?;
        let mut mailed = 0;
        for row in &expired {
            sqlx::query(&format!("WITH d AS (DELETE FROM game_trade_items_expired WHERE tid=$1 RETURNING *) INSERT INTO game_trade_item_history(tid,cid,character_name,item_id,trade_category,trade_category_sub,item_class,required_level,item_count,item_price,open_ticks,close_ticks,item_class_seq,attribute_ex,color1,color2,color3,anti_bind,max_armor_condition_mod,armor_condition,trade_type,status,log_ticks) SELECT tid,cid,character_name,item_id,trade_category,trade_category_sub,item_class,required_level,item_count,item_price,open_ticks,close_ticks,item_class_seq,attribute_ex,color1,color2,color3,anti_bind,max_armor_condition_mod,armor_condition,trade_type,{},$2 FROM d", status::EXPIRED))
                .bind(row.tid)
                .bind(at)
                .execute(&mut *tx)
                .await?;
            // SendExpiredItemMail: MakeSystemItem (not bound) and the mail.
            let class = utf16(&row.item_class);
            let attributes = parse_attributes(row.attribute_ex.as_deref());
            let info = minimum_give::class_info(assets, &class)?.ok_or_else(fail)?;
            let equipment = info.equippable.then(|| Equipment {
                colors: row.colors(),
                reduced: 0,
                bonus: row.max_armor_condition_mod.unwrap_or(0),
            });
            let (title, content) = market::expired_mail(&row.item_class, i64::from(row.item_count));
            minimum_mail::send_auction_item(
                &mut tx,
                assets,
                cid,
                ids.next().ok_or_else(fail)?,
                &class,
                row.item_count,
                false,
                &attributes,
                equipment,
                &title,
                &content,
            )
            .await?;
            trade_log(&mut *tx, log::GET_EXPIRED_ITEM_GIVE_SUCCESS, Some(row.tid), Some(cid), None, None).await;
            mailed += 1;
        }
        trade_log(&mut *tx, log::GET_EXPIRED_ITEM_SP_SUCCESS, Some(-1), Some(cid), Some(-1), Some("GetExpiredItemSPSuccess ")).await;
        if mailed != 0 {
            packets.push(console(market::EXPIRED_CONSOLE, &[mailed.to_string()])?);
        }
        // MyGold(CID): TradeGold with its order and the item, history Status 2, deleted.
        let rows: Vec<GoldRow> = sqlx::query_as("SELECT g.gid,g.tid,g.oid,g.gold,o.buy_count,i.item_class FROM game_trade_gold g JOIN game_trade_orders o ON o.oid=g.oid JOIN LATERAL (SELECT item_class FROM game_trade_items WHERE tid=g.tid UNION ALL SELECT item_class FROM game_trade_item_history WHERE tid=g.tid LIMIT 1) i ON true WHERE g.cid=$1 AND g.gold>0 ORDER BY g.gid LIMIT $2")
            .bind(cid)
            .bind(i64::try_from(budget - expired.len()).map_err(|_| fail())?)
            .fetch_all(&mut *tx)
            .await?;
        let mut sum: i64 = 0;
        let mut sold = 0;
        for r in &rows {
            sqlx::query("WITH d AS (DELETE FROM game_trade_gold WHERE gid=$1 RETURNING *) INSERT INTO game_trade_gold_history(gid,cid,tid,oid,gold,status,log_ticks) SELECT gid,cid,tid,oid,gold,2,$2 FROM d")
                .bind(r.gid)
                .bind(at)
                .execute(&mut *tx)
                .await?;
            sum += i64::from(r.gold);
            let buy_count = i64::from(r.buy_count.ok_or_else(fail)?);
            let (title, content) = market::sold_mail(&r.item_class, buy_count, i64::from(r.gold));
            minimum_mail::send_auction_item(
                &mut tx,
                assets,
                cid,
                ids.next().ok_or_else(fail)?,
                &gold,
                r.gold,
                false,
                &item_attribute::EMPTY,
                None,
                &title,
                &content,
            )
            .await?;
            trade_log(&mut *tx, log::GET_GOLD_GIVE_SUCCESS, Some(r.tid), Some(cid), Some(r.oid), None).await;
            sold += 1;
        }
        // DELETE FROM TradeGold WHERE CID = @CID also drops the rows without gold.
        sqlx::query("DELETE FROM game_trade_gold WHERE cid=$1 AND gold<=0")
            .bind(cid)
            .execute(&mut *tx)
            .await?;
        let summary = if sum > 0 { format!("Gold Sum : {sum}") } else { String::new() };
        trade_log(&mut *tx, log::GET_GOLD_SP_SUCCESS, Some(-1), Some(cid), Some(-1), Some(&format!("GetGoldSPSuccess {summary}"))).await;
        if sold != 0 {
            packets.push(console(market::SOLD_CONSOLE, &[sold.to_string()])?);
        }
        tx.commit().await?;
        Ok(Outcome { packets })
    })
    .await
    .map_err(|_| fail())?
}

// --- Laboratory extension: the server's own listings (Ajustes de jogo). ---

/// The panel's settings and the last run.
pub struct AutoState {
    pub settings: market::auto::Settings,
    /// DateTime ticks of the last run and how many listings it made.
    pub last_run: Option<i64>,
    pub last_listed: i32,
}
type AutoRow = (bool, i32, i32, i32, Option<i64>, i32);
fn auto_state(r: AutoRow) -> AutoState {
    AutoState {
        settings: market::auto::Settings {
            enabled: r.0,
            interval_hours: r.1,
            items: r.2,
            days: r.3,
        },
        last_run: r.4,
        last_listed: r.5,
    }
}
const AUTO_COLUMNS: &str = "enabled,interval_hours,items,days,last_run_ticks,last_listed";
pub async fn auto_settings(pool: &PgPool) -> Result<AutoState, MinimumError> {
    let row: AutoRow = sqlx::query_as(&format!(
        "SELECT {AUTO_COLUMNS} FROM game_trade_auto_settings WHERE singleton"
    ))
    .fetch_one(pool)
    .await?;
    Ok(auto_state(row))
}
/// Replaces the settings in the caller's transaction (the panel audits it); the previous
/// values are returned. Turning it on lists at the next tick.
pub async fn save_auto_settings(
    tx: &mut Transaction<'_, Postgres>,
    settings: &market::auto::Settings,
) -> Result<AutoState, MinimumError> {
    if !settings.valid() {
        return Err(fail());
    }
    let before: AutoRow = sqlx::query_as(&format!(
        "SELECT {AUTO_COLUMNS} FROM game_trade_auto_settings WHERE singleton FOR UPDATE"
    ))
    .fetch_one(&mut **tx)
    .await?;
    let reset = settings.enabled && !before.0;
    sqlx::query("UPDATE game_trade_auto_settings SET enabled=$1,interval_hours=$2,items=$3,days=$4,last_run_ticks=CASE WHEN $5 THEN NULL ELSE last_run_ticks END WHERE singleton")
        .bind(settings.enabled)
        .bind(settings.interval_hours)
        .bind(settings.items)
        .bind(settings.days)
        .bind(reset)
        .execute(&mut **tx)
        .await?;
    Ok(auto_state(before))
}
/// One class the server may list: effective (its Feature enabled in the region), with a
/// trade category, tradable, without expiry, deliverable by GiveItem.
struct Candidate {
    class: String,
    equippable: bool,
    max_stack: i32,
    category: String,
    sub: String,
    required_level: i32,
    class_price: i32,
}
fn candidates(assets: &Assets) -> Vec<Candidate> {
    let Some(economy) = assets.economy.as_ref() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (index, row) in assets.catalog.classes.iter().enumerate() {
        let Some(key) = row.item_class.as_ref().map(|c| c.as_bytes()) else {
            continue;
        };
        let Some(indexed) = assets.stats_index.class(Some(key)) else {
            continue;
        };
        // Only the effective row of the class (the catalog keeps repeated rows).
        if indexed.source_index != index || assets.inventory.is_gold(key) {
            continue;
        }
        let Some(e) = economy.get(index) else {
            continue;
        };
        let (Some(category), Some(sub)) =
            (e.trade_category.as_ref(), e.trade_category_sub.as_ref())
        else {
            continue;
        };
        let class = text(key);
        if category.is_empty()
            || market::auto::EXCLUDED_CATEGORIES.contains(&category.to_uppercase().as_str())
            || e.trade_restriction.is_some()
            || e.expire_in.is_some_and(|x| x > 0)
            || class == market::ENCHANT_SCROLL
            || class.encode_utf16().count() > 50
            || !class.is_ascii()
        {
            continue;
        }
        out.push(Candidate {
            class,
            equippable: indexed.equippable,
            max_stack: indexed.max_stack,
            category: category.clone(),
            sub: sub.clone(),
            required_level: e.required_level,
            class_price: e.price,
        });
    }
    out
}
/// The auto-listing tick: removes the server's closed listings and, when due, lists
/// `items` random classes at their average price (else their default price) for `days`.
/// Returns how many listings were made.
pub async fn auto_tick(pool: &PgPool, assets: &Assets) -> Result<usize, MinimumError> {
    use rand_core::{OsRng, RngCore};
    let Some(contents) = marketplace(assets) else {
        return Ok(0);
    };
    let prices = refresh_or_cached_prices(pool).await;
    let mut tx = pool.begin().await?;
    sqlx::query("SET LOCAL lock_timeout='5s'")
        .execute(&mut *tx)
        .await?;
    let state = auto_state(
        sqlx::query_as(&format!(
            "SELECT {AUTO_COLUMNS} FROM game_trade_auto_settings WHERE singleton FOR UPDATE"
        ))
        .fetch_one(&mut *tx)
        .await?,
    );
    let at = now()?;
    // Closed server listings are deleted (nobody collects them).
    sqlx::query("DELETE FROM game_trade_items WHERE cid=$1 AND close_ticks<=$2")
        .bind(market::auto::SELLER_CID)
        .bind(at)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM game_trade_items_expired WHERE cid=$1")
        .bind(market::auto::SELLER_CID)
        .execute(&mut *tx)
        .await?;
    if !state.settings.due(state.last_run, at) {
        tx.commit().await?;
        return Ok(0);
    }
    let features = &contents.features;
    let mut pool_of = candidates(assets);
    let wanted = usize::try_from(state.settings.items).map_err(|_| fail())?;
    // Partial Fisher–Yates with the OS generator.
    let picks = wanted.min(pool_of.len());
    for i in 0..picks {
        let span = u64::try_from(pool_of.len() - i).map_err(|_| fail())?;
        let j = i + usize::try_from(OsRng.next_u64() % span).map_err(|_| fail())?;
        pool_of.swap(i, j);
    }
    let close = at
        .checked_add(i64::from(state.settings.days) * market::DAY_TICKS)
        .filter(|c| *c <= MAX_TICKS)
        .ok_or_else(fail)?;
    let mut listed = 0;
    for c in &pool_of[..picks] {
        let limit = features.use_price_limit.then(|| {
            features.price_limit(
                contents.max_limit_value(&c.class),
                shop_gold_price(assets, &c.class),
            )
        });
        let average = prices.get(&c.class).map(|r| r.price);
        let Some(price) = market::auto::unit_price(
            average,
            c.class_price,
            shop_gold_price(assets, &c.class),
            limit,
        ) else {
            continue;
        };
        let Some(count) = market::auto::units(
            c.equippable,
            c.max_stack,
            OsRng.next_u32(),
            price,
            features.money_limit,
        ) else {
            continue;
        };
        let options = [("PRICE".to_owned(), price)];
        let tid = insert_registration(
            &mut tx,
            &Registration {
                cid: market::auto::SELLER_CID,
                name: market::auto::SELLER_NAME,
                item_id: -1,
                category: &c.category,
                sub: &c.sub,
                class: &c.class,
                required_level: c.required_level,
                count,
                price,
                open: at,
                close,
                attribute_ex: "",
                colors: [-1; 3],
                bonus: 0,
                trade_type: 0,
                options: &options,
            },
        )
        .await?;
        trade_log(
            &mut *tx,
            log::ADD_ITEM_SUCCESS,
            Some(tid),
            Some(market::auto::SELLER_CID),
            None,
            Some("auto"),
        )
        .await;
        listed += 1;
    }
    sqlx::query(
        "UPDATE game_trade_auto_settings SET last_run_ticks=$1,last_listed=$2 WHERE singleton",
    )
    .bind(at)
    .bind(i32::try_from(listed).map_err(|_| fail())?)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(listed)
}
/// TradeManager.Prices whatever ItemPriceInfoInItemSlot says (the average price of a
/// plain class is keyed by the class itself).
async fn refresh_or_cached_prices(pool: &PgPool) -> Prices {
    let Ok(at) = now() else {
        return Prices::new();
    };
    {
        let cached = PRICES.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some((refreshed, prices)) = cached.as_ref()
            && at.saturating_sub(*refreshed) < market::PRICE_REFRESH_TICKS
        {
            return prices.clone();
        }
    }
    match refresh_prices(pool, at).await {
        Ok(prices) => {
            *PRICES.lock().unwrap_or_else(PoisonError::into_inner) = Some((at, prices.clone()));
            prices
        }
        Err(_) => Prices::new(),
    }
}
/// How many classes the server may list with the loaded contents (shown by the panel).
pub fn auto_candidates(assets: &Assets) -> usize {
    candidates(assets).len()
}
