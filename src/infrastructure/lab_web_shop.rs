//! Laboratory replacement for the Nexon web shop that InGameCashShopDialog_V2 calls for a
//! purchase: an HTTP GET of ingame_cashshop_purchaseURL (`Purchase.aspx?passport=%s&
//! ServerCode=%d&productno=%d&quantity=%d&characterid=%I64d&giftcharacterid=%d&
//! giftmessage=%s&ruleid=%d`). client.dll 0x38479040 reads the body: empty is the SSO error,
//! a decimal 0 is success (the dialog then refreshes balance and depot over the frontend),
//! another number is a result code (0x38476b50) and anything else the "outside" error.
//! The NEXONplug passport cannot be verified here, so the authority is the frontend: only a
//! character selected in a live connection of this process can buy, for its own account.
use super::{
    character_lobby::UserKey,
    minimum_assets::LiveAssets,
    minimum_cash_shop::{self, Purchased},
};
use crate::protocol::inventory::CashShopPurchase;
use axum::{
    Router,
    extract::{ConnectInfo, Query, State},
    routing::get,
};
use sqlx::PgPool;
use std::{
    collections::HashMap,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    sync::{Arc, Mutex, PoisonError},
};

/// Loopback endpoint of the web shop (published like the relay in compose.yaml).
pub const BIND: SocketAddr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 27080));
/// ingame_cashshop_purchaseURL with the original format specifiers.
pub const PURCHASE_URL: &str = "http://127.0.0.1:27080/Purchase.aspx?passport=%s&ServerCode=%d&productno=%d&quantity=%d&characterid=%I64d&giftcharacterid=%d&giftmessage=%s&ruleid=%d";
/// Result codes of client.dll 0x38476b50.
const OK: &str = "0";
const NO_ITEM: &str = "1";
const NEGATIVE_BALANCE: &str = "21";
const PARAMETER: &str = "98";
const NISMS: &str = "99";
const NO_USER: &str = "1001104";
/// A purchase quantity above this is refused as a parameter error.
const MAX_QUANTITY: i16 = 999;

/// Account of a character selected in a live frontend connection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Owner {
    pub uid: i32,
    pub name: String,
    pub revision: i64,
}

/// Selected characters by CID, maintained by the frontend connections.
#[derive(Clone, Default)]
pub struct Sessions(Arc<Mutex<HashMap<i64, Owner>>>);

impl Sessions {
    pub fn insert(&self, cid: i64, owner: Owner) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(cid, owner);
    }
    pub fn remove(&self, cid: i64) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&cid);
    }
    fn get(&self, cid: i64) -> Option<Owner> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&cid)
            .cloned()
    }
}

#[derive(Clone)]
struct Shop {
    sessions: Sessions,
    pool: PgPool,
    /// The game's current contents (a live tuning swaps them).
    assets: Arc<LiveAssets>,
}

pub fn router(sessions: Sessions, pool: PgPool, assets: Arc<LiveAssets>) -> Router {
    Router::new()
        .route("/Purchase.aspx", get(purchase))
        .with_state(Shop {
            sessions,
            pool,
            assets,
        })
}

/// Serves the web shop until the listener fails or the task is aborted.
pub async fn serve(
    listener: tokio::net::TcpListener,
    sessions: Sessions,
    pool: PgPool,
    assets: Arc<LiveAssets>,
) {
    let app = router(sessions, pool, assets).into_make_service_with_connect_info::<SocketAddr>();
    if axum::serve(listener, app).await.is_err() {
        tracing::warn!("laboratory web shop stopped");
    }
}

fn number<T: std::str::FromStr>(query: &[(String, String)], key: &str) -> Option<T> {
    query
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .and_then(|(_, v)| v.trim().parse().ok())
}

async fn purchase(
    State(shop): State<Shop>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Query(query): Query<Vec<(String, String)>>,
) -> &'static str {
    if super::lab_peer::effective_peer(peer.ip()).is_none() || query.len() > 16 {
        return PARAMETER;
    }
    let (Some(product_no), Some(quantity), Some(cid), Some(gift)) = (
        number::<i32>(&query, "productno"),
        number::<i16>(&query, "quantity"),
        number::<i64>(&query, "characterid"),
        number::<i64>(&query, "giftcharacterid"),
    ) else {
        return PARAMETER;
    };
    // Gifts are not migrated.
    if gift != 0 || !(1..=MAX_QUANTITY).contains(&quantity) {
        return PARAMETER;
    }
    let Some(owner) = shop.sessions.get(cid) else {
        tracing::warn!("laboratory web shop purchase without a selected character; values omitted");
        return NO_USER;
    };
    let key = UserKey {
        uid: owner.uid,
        name: &owner.name,
        revision: owner.revision,
    };
    let request = CashShopPurchase {
        product_no,
        quantity,
        attribute0: None,
        attribute1: None,
        attribute2: None,
        attribute3: None,
        attribute4: None,
        is_for_common_inven: false,
    };
    let code = match minimum_cash_shop::purchase_order(
        &shop.pool,
        &key,
        &shop.assets.current(),
        cid,
        &request,
    )
    .await
    {
        Ok(Purchased::Bought { .. }) => OK,
        Ok(Purchased::UnknownProduct) => NO_ITEM,
        Ok(Purchased::InvalidQuantity) => PARAMETER,
        Ok(Purchased::NotEnoughCash) => NEGATIVE_BALANCE,
        Err(_) => NISMS,
    };
    tracing::info!(
        result = code,
        "laboratory web shop purchase answered; values omitted"
    );
    code
}
