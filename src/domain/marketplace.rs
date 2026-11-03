//! Marketplace rules of the PlayerService TradeSystem (TradeInsertOrderProcessor,
//! ItemTradeProxy.InsertTradeOrder, TradeManager, ItemGenerator, TradeItem) and of the
//! heroesMarketPlace procedures (AddTradeItem, BuyTradeItem, CancelTradeItem,
//! TradeItemList*, TradeItemListByCategory*, MyTradeItemList, MyGold, ExpiredTradeItem,
//! JobExpireTradeItem, JobTradeItemAvgPrice). No storage here.
use super::feature_policy::FeaturePolicy;
use regex::Regex;

/// InsertTradeOrderResultMessage.ErrorCode.
pub mod order {
    pub const OK: i32 = 0;
    pub const PRICE_OUT_OF_RANGE: i32 = 1;
    pub const INTERNAL_ERROR: i32 = 2;
    pub const INTERNAL_ERROR_TIR: i32 = 3;
    pub const WEB_ERROR: i32 = 4;
    pub const NOT_TRADABLE: i32 = 7;
    pub const TIME_LIMIT_ITEM: i32 = 8;
    pub const NOT_ENOUGH_LEVEL: i32 = 9;
    pub const PREMIUM_TICKET_DISABLE: i32 = 10;
    pub const NO_PREMIUM_TICKET: i32 = 11;
}
/// TradeResultCode.
pub mod result {
    pub const SUCCESS: i32 = 0;
    pub const EXCEPTION_OCCURED: i32 = 1;
    pub const NOT_ENOUGH_MONEY: i32 = 2;
    pub const SYSTEM_ERROR: i32 = 3;
    pub const ALREADY_SOLD_OUT: i32 = 4;
    pub const EMPTY_LIST: i32 = 7;
    pub const SEARCH_OVERHEAD: i32 = 8;
    /// The frontend's UseSecondPassword failure answer of TradePurchaseItemMessage.
    pub const SECOND_PASSWORD_FAILED: i32 = 9;
}
/// TradeItem_History.Status: 1 AddTradeItem, 2 BuyTradeItem, 3 CancelTradeItem,
/// 4 ExpiredTradeItem.
pub mod status {
    pub const ADDED: i16 = 1;
    pub const BOUGHT: i16 = 2;
    pub const CANCELED: i16 = 3;
    pub const EXPIRED: i16 = 4;
}
/// TradeLogCategory (AddTradeLog Event).
pub mod log {
    pub const ADD_ITEM_SUCCESS: i32 = 3;
    pub const PURCHASE_MONEY_DEDUCTION: i32 = 9;
    pub const PURCHASE_REQUEST: i32 = 11;
    pub const PURCHASE_SUCCESS: i32 = 12;
    pub const PURCHASE_FAIL: i32 = 13;
    pub const CANCEL_REQUEST: i32 = 15;
    pub const CANCEL_SUCCESS: i32 = 16;
    pub const CANCEL_FAIL: i32 = 17;
    pub const INVALID_ITEM_CLASS_NAME_QUERY: i32 = 20;
    pub const GET_EXPIRED_ITEM_SP_SUCCESS: i32 = 21;
    pub const GET_EXPIRED_ITEM_GIVE_SUCCESS: i32 = 23;
    pub const GET_GOLD_SP_SUCCESS: i32 = 25;
    pub const GET_GOLD_GIVE_SUCCESS: i32 = 27;
}
/// ItemServiceProxy: `int chunkSize = 100` of both searches.
pub const CHUNK_SIZE: i32 = 100;
/// AddTradeItem: `IF ISNULL(@AlreadyCnt,0)+1 > 20` over the open registrations.
pub const MAX_OPEN_ORDERS: i64 = 20;
/// TradeManager.QueryTradeItemList: itemClassList.Count() and the CSV length bound.
pub const MAX_CLASS_LIST: usize = 2048;
/// FN_SPLIT returns NVARCHAR(52) values (longer pieces are truncated).
pub const SPLIT_VALUE_UNITS: usize = 52;
/// TradeItemList*_Attribute: `SELECT TOP 10000` of the price (Under) branch.
pub const UNDER_TOP: i64 = 10_000;
/// The premium registration ticket (TradeType > 0).
pub const PREMIUM_TICKET: &str = "pre_trade_ticket";
/// ItemClassConst.EnchantScroll.
pub const ENCHANT_SCROLL: &str = "enchant_scroll";
/// SearchOverheadChecker: HistoryMax 20, TimeLimitSecond 10.
pub const SEARCH_HISTORY: usize = 20;
pub const SEARCH_WINDOW_TICKS: i64 = 10 * 10_000_000;
/// JobTradeItemAvgPrice: sales of the last day with at least ten units.
pub const AVG_PRICE_MIN_BUYS: i64 = 10;
pub const DAY_TICKS: i64 = 864_000_000_000;
/// TradeManager.ScheduleUpdatePrice: every hour.
pub const PRICE_REFRESH_TICKS: i64 = 36_000_000_000;

/// The TradeSystem's FeatureMatrix values for the loaded region.
#[derive(Debug)]
pub struct Features {
    pub money_limit: i32,
    pub register_fee: i32,
    pub use_price_limit: bool,
    pub price_limit_percentage: i32,
    pub premium_ticket: bool,
    /// TradeItem price fields: ItemPriceInfoInItemSlot && !ItemPriceInfoInService.
    pub price_info_in_trade_item: bool,
    level_limits: Vec<(Regex, i32)>,
}
impl Features {
    /// FeatureMatrix.GetInteger/IsEnable over the exported values; the
    /// LevelLimitToTradeItem list is ItemContents.LoadLevelLimitToTradeItem.
    pub fn new(policy: &FeaturePolicy) -> Result<Self, crate::error::Error> {
        Ok(Self {
            money_limit: policy.get_integer("MoneyLimitToTradeItem")?,
            register_fee: policy.get_integer("TradeRegisterFee")?,
            use_price_limit: policy.is_enabled(Some("UseTradePriceLimit"))?,
            price_limit_percentage: policy.get_integer("UseTradePriceLimitPercentage")?,
            premium_ticket: policy.is_enabled(Some("PremiumTradeTicket"))?,
            price_info_in_trade_item: policy.is_enabled(Some("ItemPriceInfoInItemSlot"))?
                && !policy.is_enabled(Some("ItemPriceInfoInService"))?,
            level_limits: level_limits(policy.get_string("LevelLimitToTradeItem"))?,
        })
    }
    /// ItemContents.GetLevelLimitToTradeItem: the first pattern (in load order) that
    /// matches anywhere in the class, else 0.
    pub fn level_limit(&self, class: &str) -> i32 {
        self.level_limits
            .iter()
            .find(|(r, _)| r.is_match(class))
            .map_or(0, |(_, level)| *level)
    }
    /// TradeInsertOrderProcessor's UseTradePriceLimit maximum: the ItemTradeLimitInfo
    /// MaxLimitValue and the gold shop price times UseTradePriceLimitPercentage / 100
    /// (Int32 arithmetic, unchecked as the source).
    pub fn price_limit(&self, max_limit_value: Option<i32>, shop_gold_price: i32) -> i32 {
        let mut max = i32::MAX;
        if let Some(v) = max_limit_value {
            max = max.min(v);
        }
        if shop_gold_price >= 0 && self.price_limit_percentage > 0 {
            max = max.min(shop_gold_price.wrapping_mul(self.price_limit_percentage) / 100);
        }
        max
    }
    /// ItemTradeProxy.InsertTradeOrder: `(ulong)unitPrice * (ulong)num > (ulong)limit`.
    pub fn over_money_limit(&self, unit_price: i32, num: i32) -> bool {
        (unit_price as u64).wrapping_mul(num as u64) > self.money_limit as u64
    }
}

/// LoadLevelLimitToTradeItem: `level=regex` entries separated by ';'. Invalid entries are
/// skipped (the source logs them); a pattern that does not compile refuses the content,
/// as `new Regex` throws while loading.
fn level_limits(text: &str) -> Result<Vec<(Regex, i32)>, crate::error::Error> {
    let mut out = Vec::new();
    if text.is_empty() {
        return Ok(out);
    }
    for entry in text.split(';') {
        let parts: Vec<&str> = entry.split('=').collect();
        let [level, pattern] = parts.as_slice() else {
            continue;
        };
        let Some(level) = parse_int(level) else {
            continue;
        };
        if out.len() >= 256 || pattern.len() > 512 {
            return Err(crate::error::Error::Configuration(
                "LevelLimitToTradeItem bound".into(),
            ));
        }
        let regex = Regex::new(pattern).map_err(|_| {
            crate::error::Error::Configuration("LevelLimitToTradeItem pattern".into())
        })?;
        out.push((regex, level));
    }
    Ok(out)
}
/// int.TryParse (NumberStyles.Integer): surrounding whitespace and one sign.
fn parse_int(text: &str) -> Option<i32> {
    let t = text.trim_matches(|c| matches!(c, ' ' | '\u{0009}'..='\u{000d}'));
    let digits = t.strip_prefix(['+', '-']).unwrap_or(t);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    t.parse().ok()
}

/// ItemTradeProxy.InsertTradeOrder fee of a gold registration (TradeType 0):
/// `checked((int)(fee/10000.0 * unitPrice * num * (durationMin/60/24)))`, refused when
/// negative, then reduced by the guild's TradePrice bonus. None is InternalError.
pub fn register_fee(
    fee_rate: i32,
    unit_price: i32,
    num: i32,
    duration_min: i32,
    guild_bonus: i32,
) -> Option<i32> {
    let rate = f64::from(fee_rate) / 10000.0;
    let days = duration_min / 60 / 24;
    let value = rate * f64::from(unit_price) * f64::from(num) * f64::from(days);
    // conv.ovf.i4: the truncated value must fit Int32 (NaN throws).
    if !(value > -2_147_483_649.0 && value < 2_147_483_648.0) {
        return None;
    }
    let fee = value as i32;
    if fee < 0 || unit_price < 0 || num < 0 || days < 0 {
        return None;
    }
    let ratio = f64::from(100 - guild_bonus) / 100.0;
    let fee = framework_int(f64::from(fee) * ratio);
    (fee >= 0).then_some(fee)
}
/// Unchecked (int)double of the Framework JIT: truncation, int.MinValue when outside.
fn framework_int(value: f64) -> i32 {
    if value.is_nan() || !(-2_147_483_648.0..2_147_483_648.0).contains(&value) {
        i32::MIN
    } else {
        value as i32
    }
}

/// TradeItem.GetRemainTimeFromNow: whole seconds left, int.MaxValue above it, 0 when past.
pub fn remain_seconds(close_ticks: i64, now_ticks: i64) -> i32 {
    let seconds = (close_ticks.saturating_sub(now_ticks)) as f64 * (1.0 / 10_000_000.0);
    if 2_147_483_647.0 < seconds {
        return i32::MAX;
    }
    framework_int(seconds).max(0)
}

/// The ORDER BY of TradeItemList*/TradeItemListByCategory*: @OrderColumn 1 is level,
/// anything else price; @OrderType 2 (isDescending) or 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sort {
    /// TradeType desc, RequiredLevel, ItemClass, ItemPrice.
    LevelAsc,
    /// TradeType desc, RequiredLevel DESC, ItemClass, ItemPrice.
    LevelDesc,
    /// TradeType desc, ItemPrice, RequiredLevel DESC, ItemClass.
    PriceAsc,
    /// TradeType desc, ItemPrice DESC, RequiredLevel DESC, ItemClass.
    PriceDesc,
}
impl Sort {
    pub fn new(order: i32, descending: bool) -> Self {
        match (order == 1, descending) {
            (true, false) => Self::LevelAsc,
            (true, true) => Self::LevelDesc,
            (false, false) => Self::PriceAsc,
            (false, true) => Self::PriceDesc,
        }
    }
}
/// The numbered window of a search page: `TOP (@ListCount*@PageNo)` rows whose idx is
/// BETWEEN `(@PageNo-1)*@ListCount+1` AND `@ListCount*@PageNo`. Err is the SQL error of
/// an Int32 overflow or a negative TOP; Ok(None) an empty page.
/// The SQL error of a search procedure (arithmetic overflow, negative TOP).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcedureError;
pub fn page(page_no: i32, list_count: i32) -> Result<Option<(i64, i64)>, ProcedureError> {
    let top = list_count.checked_mul(page_no).ok_or(ProcedureError)?;
    let start = page_no
        .checked_sub(1)
        .and_then(|p| p.checked_mul(list_count))
        .and_then(|p| p.checked_add(1))
        .ok_or(ProcedureError)?;
    if top < 0 {
        return Err(ProcedureError);
    }
    if top == 0 || start > top {
        return Ok(None);
    }
    let offset = i64::from(start.max(1)) - 1;
    Ok(Some((offset, i64::from(top) - offset)))
}

/// TradeManager.GetCSVFrom then FN_SPLIT and Replace(Value,'''',''): the class names the
/// TradeItemList procedures compare. A name that is not an ItemClassInfo throws
/// ("Verify error!": ExceptionOccured); None elements throw on the quote concatenation's
/// verify call as well. Err(true) is the length/count SystemError.
pub fn class_list(
    classes: &[Option<String>],
    known: impl Fn(&str) -> bool,
) -> Result<Vec<String>, bool> {
    if classes.len() > MAX_CLASS_LIST {
        return Err(true);
    }
    let mut csv_units = 0usize;
    let mut out = Vec::with_capacity(classes.len());
    for (n, class) in classes.iter().enumerate() {
        let class = class.as_deref().ok_or(false)?;
        if !known(class) {
            return Err(false);
        }
        csv_units += class.encode_utf16().count() + 2 + usize::from(n > 0);
        out.push(class.to_owned());
    }
    if csv_units > MAX_CLASS_LIST {
        return Err(true);
    }
    // The VARCHAR(2048) list splits on ','; each piece keeps 52 units with its quotes.
    let mut split = Vec::new();
    for class in out {
        for piece in format!("'{class}'").split(',') {
            let kept: String = char::decode_utf16(piece.encode_utf16().take(SPLIT_VALUE_UNITS))
                .map(|c| c.unwrap_or('\u{fffd}'))
                .collect();
            let value = kept.replace('\'', "");
            if !value.is_empty() {
                split.push(value);
            }
        }
    }
    Ok(split)
}

/// SearchOverheadChecker.Check: more than 20 searches whose oldest is under 10 seconds
/// old refuses this one (the oldest is dequeued either way).
#[derive(Debug, Default)]
pub struct SearchOverhead {
    history: std::collections::VecDeque<i64>,
}
impl SearchOverhead {
    pub fn check(&mut self, now_ticks: i64) -> bool {
        self.history.push_back(now_ticks);
        if self.history.len() > SEARCH_HISTORY
            && let Some(oldest) = self.history.pop_front()
        {
            return now_ticks.saturating_sub(oldest) >= SEARCH_WINDOW_TICKS;
        }
        true
    }
}

/// TradeItem.GetAttributedItemClassEX: `AttributeEX != null || AttributeEX != ""` is
/// always true, so the class always carries the brackets.
pub fn attributed_class_ex(class: &str, attribute_ex: Option<&str>) -> String {
    format!("{class}[{}]", attribute_ex.unwrap_or(""))
}
/// The mail titles and contents of ItemGenerator and TradeManager.
pub fn buy_mail(class: &str, count: i64) -> (String, String) {
    (
        format!("#GameUI_Heroes_TradeMail_Buy_Title,{class},{count}"),
        format!("#GameUI_Heroes_TradeMail_Buy_Content,{class},{count}"),
    )
}
pub fn canceled_mail(class: &str, count: i64) -> (String, String) {
    (
        format!("#GameUI_Heroes_TradeMail_SellCanceled_Title,{class},{count}"),
        format!("#GameUI_Heroes_TradeMail_SellCanceled_Content,{class},{count}"),
    )
}
pub fn expired_mail(class: &str, count: i64) -> (String, String) {
    (
        format!("#GameUI_Heroes_TradeMail_SellExpired_Title,{class},{count}"),
        format!("#GameUI_Heroes_TradeMail_SellExpired_Content,{class},{count}"),
    )
}
pub fn sold_mail(class: &str, buy_count: i64, gold: i64) -> (String, String) {
    (
        format!("#GameUI_Heroes_TradeMail_Sold_Title,{class},{buy_count}"),
        format!("#GameUI_Heroes_TradeMail_Sold_Content,{gold}"),
    )
}
pub const BUY_CONSOLE: &str = "GameUI_Heroes_TradeMessage_Buy_Console";
pub const BUY_MAIL_CONSOLE: &str = "GameUI_Heroes_TradeMessage_Buy_Mail_Console";
pub const CANCELED_CONSOLE: &str = "GameUI_Heroes_TradeMessage_SellCanceled_Console";
pub const CANCELED_MAIL_CONSOLE: &str = "GameUI_Heroes_TradeMessage_SellCanceled_Mail_Console";
pub const EXPIRED_CONSOLE: &str = "GameUI_Heroes_TradeMessage_SellExpired_Console";
pub const SOLD_CONSOLE: &str = "GameUI_Heroes_TradeMessage_Sold_Console";

/// Color.From: the R, G and B bytes of an Int32 color.
pub fn color_bytes(color: i32) -> [i32; 3] {
    [
        i32::from((color >> 16) as u8),
        i32::from(((color >> 8) & 0xff) as u8),
        i32::from((color & 0xff) as u8),
    ]
}

/// TradeManager.UpdatePrice of one TradeItem_Statistics group (the same ItemClassEx key):
/// the source keeps it only with BuyCount and Sales above zero; Price is Sales/BuyCount.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PriceRange {
    pub min: i32,
    pub max: i32,
    pub price: i32,
}
pub fn price_range(rows: &[(i32, i32, i64, i64)]) -> Option<PriceRange> {
    let buys: i64 = rows.iter().map(|r| r.2).fold(0, i64::wrapping_add);
    let sales: i64 = rows.iter().map(|r| r.3).fold(0, i64::wrapping_add);
    if buys <= 0 || sales <= 0 {
        return None;
    }
    Some(PriceRange {
        min: rows.iter().map(|r| r.0).min()?,
        max: rows.iter().map(|r| r.1).max()?,
        price: (sales / buys) as i32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_fee_follows_the_double_arithmetic_and_whole_days() {
        // 300/10000 * 1500 * 3 * 3 days.
        assert_eq!(register_fee(300, 1500, 3, 4320, 0), Some(405));
        // Under one day the fee is zero (durationMin/60/24).
        assert_eq!(register_fee(300, 1500, 3, 1439, 0), Some(0));
        assert_eq!(register_fee(300, 1, 1, -1440, 0), None);
        assert_eq!(register_fee(300, i32::MAX, i32::MAX, 14400, 0), None);
        // The guild TradePrice bonus reduces the fee.
        assert_eq!(register_fee(300, 1000, 10, 1440, 10), Some(270));
    }
    #[test]
    fn price_limit_takes_the_smallest_bound() {
        let policy = FeaturePolicy::new([
            ("CurrentVer".to_owned(), Some("3.09.00".to_owned())),
            (
                "UseTradePriceLimitPercentage".to_owned(),
                Some("200".to_owned()),
            ),
            (
                "MoneyLimitToTradeItem".to_owned(),
                Some("100000000".to_owned()),
            ),
            (
                "LevelLimitToTradeItem".to_owned(),
                Some("11=dye_.*|life_potion_plus_cash_lvl1;x=bad;7".to_owned()),
            ),
        ])
        .unwrap();
        let f = Features::new(&policy).unwrap();
        assert_eq!(f.price_limit(None, -1), i32::MAX);
        assert_eq!(f.price_limit(Some(700), -1), 700);
        assert_eq!(f.price_limit(Some(700), 250), 500);
        assert_eq!(f.price_limit(None, 0), 0);
        assert!(f.over_money_limit(100_000_001, 1));
        assert!(!f.over_money_limit(50_000_000, 2));
        assert_eq!(f.level_limit("dye_red"), 11);
        assert_eq!(f.level_limit("big_life_potion_plus_cash_lvl1_x"), 11);
        assert_eq!(f.level_limit("sword"), 0);
        assert!(!f.use_price_limit && !f.premium_ticket);
    }
    #[test]
    fn pages_number_rows_as_the_procedures() {
        assert_eq!(page(1, 100), Ok(Some((0, 100))));
        assert_eq!(page(3, 100), Ok(Some((200, 100))));
        assert_eq!(page(0, 100), Ok(None));
        assert_eq!(page(-1, 100), Err(ProcedureError));
        assert_eq!(page(i32::MAX, 100), Err(ProcedureError));
    }
    #[test]
    fn remain_seconds_clamps_to_the_int32_range() {
        assert_eq!(remain_seconds(10_000_000 * 90 + 5, 0), 90);
        assert_eq!(remain_seconds(0, 10), 0);
        assert_eq!(remain_seconds(i64::MAX, 0), i32::MAX);
    }
    #[test]
    fn class_lists_verify_and_split_like_fn_split() {
        let known = |c: &str| c != "nope";
        assert_eq!(
            class_list(&[Some("a".into()), Some("b".into())], known),
            Ok(vec!["a".into(), "b".into()])
        );
        assert_eq!(class_list(&[Some("nope".into())], known), Err(false));
        assert_eq!(class_list(&[None], known), Err(false));
        assert_eq!(
            class_list(&vec![Some("a".repeat(700)); 3], known),
            Err(true)
        );
        assert_eq!(
            class_list(&[Some("x".repeat(60))], known),
            Ok(vec!["x".repeat(51)])
        );
    }
    #[test]
    fn search_overhead_refuses_the_twenty_first_within_ten_seconds() {
        let mut c = SearchOverhead::default();
        for n in 0..20 {
            assert!(c.check(n));
        }
        assert!(!c.check(20));
        assert!(c.check(SEARCH_WINDOW_TICKS + 2));
    }
    #[test]
    fn helpers_match_the_source_strings() {
        assert_eq!(attributed_class_ex("sword", Some("")), "sword[]");
        assert_eq!(color_bytes(0x00ff_8001), [255, 128, 1]);
        assert_eq!(color_bytes(-1), [255, 255, 255]);
        assert_eq!(
            sold_mail("sword", 2, 3000).1,
            "#GameUI_Heroes_TradeMail_Sold_Content,3000"
        );
        assert_eq!(Sort::new(2, true), Sort::PriceDesc);
        assert_eq!(Sort::new(99, false), Sort::PriceAsc);
        assert_eq!(
            price_range(&[(10, 30, 4, 80), (5, 20, 6, 70)]),
            Some(PriceRange {
                min: 5,
                max: 30,
                price: 15
            })
        );
        assert_eq!(price_range(&[(10, 30, 0, 80)]), None);
    }
}
