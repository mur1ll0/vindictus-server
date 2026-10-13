//! Bounded private lobby export reader. Preserves NULLs and lossless source representations.
use super::{
    character_lobby::LobbyError,
    user_export::{PrivateText, hex, integer, nullable},
};
use crate::domain::{character_lobby::*, legacy_user_data::*};
use serde::Deserialize;
use std::io::Read;
use zeroize::Zeroizing;
const MAX_BYTES: usize = 2 * 1024 * 1024;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Export {
    format_version: u32,
    source: String,
    user_info: Vec<Info>,
    user_products: Vec<Product>,
    user_care_info: Vec<Care>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Info {
    nexon_sn: i32,
    #[serde(deserialize_with = "nullable")]
    last_login_cid: Option<PrivateText>,
    prologue_played: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Product {
    id: PrivateText,
    nexon_sn: i32,
    product_name_utf16_hex: PrivateText,
    #[serde(deserialize_with = "nullable")]
    argument_utf16_hex: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    expire_time_ticks: Option<PrivateText>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Care {
    uid: i32,
    #[serde(rename = "type")]
    kind: u8,
    state: i16,
    last_update_units300: PrivateText,
}
fn number(v: PrivateText) -> Result<i64, LobbyError> {
    integer(v).map_err(|_| LobbyError::Export)
}
fn text(v: PrivateText) -> Result<LegacyBytes<128, true>, LobbyError> {
    Ok(LegacyBytes::from_bytes(
        hex(v, 256).map_err(|_| LobbyError::Export)?,
    )?)
}
pub fn read_snapshot(input: impl Read) -> Result<LobbySnapshot, LobbyError> {
    let mut bytes = Zeroizing::new(Vec::new());
    input.take((MAX_BYTES + 1) as u64).read_to_end(&mut bytes)?;
    if bytes.len() > MAX_BYTES {
        return Err(LobbyError::Export);
    }
    let e: Export = serde_json::from_slice(&bytes).map_err(|_| LobbyError::Export)?;
    if e.format_version != 1
        || e.source != "study_heroes.character-lobby"
        || e.user_info.len() > MAX_ROWS
        || e.user_products.len() > MAX_ROWS
        || e.user_care_info.len() > MAX_ROWS
    {
        return Err(LobbyError::Export);
    }
    let user_info = e
        .user_info
        .into_iter()
        .map(|r| {
            Ok(UserInfo {
                nexon_sn: r.nexon_sn,
                last_login_cid: r.last_login_cid.map(number).transpose()?,
                prologue_played: r.prologue_played,
            })
        })
        .collect::<Result<_, LobbyError>>()?;
    let user_products = e
        .user_products
        .into_iter()
        .map(|r| {
            Ok(UserProduct {
                id: number(r.id)?,
                nexon_sn: r.nexon_sn,
                product_name: text(r.product_name_utf16_hex)?,
                argument: r.argument_utf16_hex.map(text).transpose()?,
                expire_time: r
                    .expire_time_ticks
                    .map(|v| Ok::<_, LobbyError>(SqlDateTime2::from_ticks(number(v)?)?))
                    .transpose()?,
            })
        })
        .collect::<Result<_, LobbyError>>()?;
    let user_care_info = e
        .user_care_info
        .into_iter()
        .map(|r| {
            Ok(UserCareInfo {
                uid: r.uid,
                kind: r.kind,
                state: r.state,
                last_update: SqlDateTime::from_units300(number(r.last_update_units300)?)?,
            })
        })
        .collect::<Result<_, LobbyError>>()?;
    let s = LobbySnapshot {
        user_info,
        user_products,
        user_care_info,
    };
    s.validate()?;
    Ok(s)
}
