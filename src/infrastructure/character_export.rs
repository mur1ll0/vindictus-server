//! Bounded private source bundle; missing nullable columns are errors.
use super::user_export::{PrivateText, hex, integer, nullable};
use crate::{domain::character_data::*, domain::legacy_user_data::*, error::Error};
use serde::Deserialize;
use std::io::Read;
use zeroize::Zeroizing;
pub const MAX_ROWS: usize = 1000;
pub const MAX_BYTES: usize = 6 * 1024 * 1024;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Counts {
    characters: usize,
    character_info: usize,
    connections: usize,
    queued_items: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Export {
    format_version: u32,
    source: String,
    source_counts: Counts,
    characters: Vec<CharacterIdentityRow>,
    character_info: Vec<CharacterSnapshotRow>,
    connections: Vec<ConnectionSnapshotRow>,
    queued_items: Vec<QueuedItemSnapshotRow>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CharacterIdentityRow {
    id: PrivateText,
}
impl CharacterIdentityRow {
    fn into_snapshot(self) -> Result<CharacterIdentity, Error> {
        Ok(CharacterIdentity {
            id: integer(self.id)?,
        })
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CharacterSnapshotRow {
    id: PrivateText,
    uid: i32,
    character_sn: i32,
    #[serde(deserialize_with = "nullable")]
    name_utf16: Option<PrivateText>,
    class: i16,
    level: i32,
    exp: PrivateText,
    str: i32,
    dex: i32,
    int: i32,
    will: i32,
    luck: i32,
    hp: i32,
    stamina: i32,
    dirty: bool,
    play_time: PrivateText,
    #[serde(deserialize_with = "nullable")]
    selected_title: Option<i32>,
    ap: i32,
    total_used_ap: i32,
    last_ap_update_units300: PrivateText,
    permission: i32,
    status: i32,
    #[serde(deserialize_with = "nullable")]
    quote_utf16: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    delete_time_units300: Option<PrivateText>,
    region_code: i32,
    is_connected: bool,
    #[serde(deserialize_with = "nullable")]
    legacy_last_update: Option<PrivateText>,
    create_time_ticks: PrivateText,
    free_match_win_count: i32,
    free_match_lose_count: i32,
    use_preset: bool,
    is_premium: bool,
    #[serde(deserialize_with = "nullable")]
    free_title_name_utf16: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    today_play_time: Option<i32>,
    #[serde(deserialize_with = "nullable")]
    channel: Option<i16>,
    #[serde(deserialize_with = "nullable")]
    order_num: Option<i16>,
    map_state_info: i16,
}
impl CharacterSnapshotRow {
    fn into_snapshot(self) -> Result<CharacterSnapshot, Error> {
        Ok(CharacterSnapshot {
            id: integer(self.id)?,
            uid: self.uid,
            character_sn: self.character_sn,
            name: self
                .name_utf16
                .map(|v| Ok::<_, Error>(LegacyBytes::from_bytes(hex(v, 100)?)?))
                .transpose()?,
            class: self.class,
            level: self.level,
            exp: integer(self.exp)?,
            str: self.str,
            dex: self.dex,
            int: self.int,
            will: self.will,
            luck: self.luck,
            hp: self.hp,
            stamina: self.stamina,
            dirty: self.dirty,
            play_time: integer(self.play_time)?,
            selected_title: self.selected_title,
            ap: self.ap,
            total_used_ap: self.total_used_ap,
            last_ap_update: Ok::<_, Error>(SqlDateTime::from_units300(integer(
                self.last_ap_update_units300,
            )?)?)?,
            permission: self.permission,
            status: self.status,
            quote: self
                .quote_utf16
                .map(|v| Ok::<_, Error>(LegacyBytes::from_bytes(hex(v, 100)?)?))
                .transpose()?,
            delete_time: self
                .delete_time_units300
                .map(|v| Ok::<_, Error>(SqlDateTime::from_units300(integer(v)?)?))
                .transpose()?,
            region_code: self.region_code,
            is_connected: self.is_connected,
            last_update: self
                .legacy_last_update
                .map(|v| Ok::<_, Error>(SourceRowVersion::from_bytes(&hex(v, 8)?)?))
                .transpose()?,
            create_time: Ok::<_, Error>(SqlDateTime2::from_ticks(integer(
                self.create_time_ticks,
            )?)?)?,
            free_match_win_count: self.free_match_win_count,
            free_match_lose_count: self.free_match_lose_count,
            use_preset: self.use_preset,
            is_premium: self.is_premium,
            free_title_name: self
                .free_title_name_utf16
                .map(|v| Ok::<_, Error>(LegacyBytes::from_bytes(hex(v, 100)?)?))
                .transpose()?,
            today_play_time: self.today_play_time,
            channel: self.channel,
            order_num: self.order_num,
            map_state_info: self.map_state_info,
        })
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConnectionSnapshotRow {
    id: PrivateText,
    last_connect_date_units300: PrivateText,
    #[serde(deserialize_with = "nullable")]
    last_cafe_bonus_units300: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    last_cafe_login_units300: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    last_cafe_logout_units300: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    last_home_cafe_login_units300: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    last_home_cafe_logout_units300: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    last_logout_units300: Option<PrivateText>,
}
impl ConnectionSnapshotRow {
    fn into_snapshot(self) -> Result<ConnectionSnapshot, Error> {
        Ok(ConnectionSnapshot {
            id: integer(self.id)?,
            last_connect_date: Ok::<_, Error>(SqlDateTime::from_units300(integer(
                self.last_connect_date_units300,
            )?)?)?,
            last_cafe_bonus: self
                .last_cafe_bonus_units300
                .map(|v| Ok::<_, Error>(SqlDateTime::from_units300(integer(v)?)?))
                .transpose()?,
            last_cafe_login: self
                .last_cafe_login_units300
                .map(|v| Ok::<_, Error>(SqlDateTime::from_units300(integer(v)?)?))
                .transpose()?,
            last_cafe_logout: self
                .last_cafe_logout_units300
                .map(|v| Ok::<_, Error>(SqlDateTime::from_units300(integer(v)?)?))
                .transpose()?,
            last_home_cafe_login: self
                .last_home_cafe_login_units300
                .map(|v| Ok::<_, Error>(SqlDateTime::from_units300(integer(v)?)?))
                .transpose()?,
            last_home_cafe_logout: self
                .last_home_cafe_logout_units300
                .map(|v| Ok::<_, Error>(SqlDateTime::from_units300(integer(v)?)?))
                .transpose()?,
            last_logout: self
                .last_logout_units300
                .map(|v| Ok::<_, Error>(SqlDateTime::from_units300(integer(v)?)?))
                .transpose()?,
        })
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QueuedItemSnapshotRow {
    row_id: i32,
    cid: PrivateText,
    item_class_ex_utf16: PrivateText,
    count: i32,
    is_character_binded: bool,
    #[serde(deserialize_with = "nullable")]
    mail_title_utf16: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    mail_content_utf16: Option<PrivateText>,
    color1: i32,
    color2: i32,
    color3: i32,
    reduced_durability: i32,
    max_durability_bonus: i32,
    #[serde(deserialize_with = "nullable")]
    item_id: Option<PrivateText>,
    processed: bool,
    #[serde(deserialize_with = "nullable")]
    event_code_utf16: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    processed_time_units300: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    expire_time_units300: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    log_date_units300: Option<PrivateText>,
}
impl QueuedItemSnapshotRow {
    fn into_snapshot(self) -> Result<QueuedItemSnapshot, Error> {
        Ok(QueuedItemSnapshot {
            row_id: self.row_id,
            cid: integer(self.cid)?,
            item_class_ex: Ok::<_, Error>(LegacyBytes::from_bytes(hex(
                self.item_class_ex_utf16,
                2048,
            )?)?)?,
            count: self.count,
            is_character_binded: self.is_character_binded,
            mail_title: self
                .mail_title_utf16
                .map(|v| Ok::<_, Error>(LegacyBytes::from_bytes(hex(v, 512)?)?))
                .transpose()?,
            mail_content: self
                .mail_content_utf16
                .map(|v| Ok::<_, Error>(LegacyBytes::from_bytes(hex(v, 2048)?)?))
                .transpose()?,
            color1: self.color1,
            color2: self.color2,
            color3: self.color3,
            reduced_durability: self.reduced_durability,
            max_durability_bonus: self.max_durability_bonus,
            item_id: self.item_id.map(integer).transpose()?,
            processed: self.processed,
            event_code: self
                .event_code_utf16
                .map(|v| Ok::<_, Error>(LegacyBytes::from_bytes(hex(v, 100)?)?))
                .transpose()?,
            processed_time: self
                .processed_time_units300
                .map(|v| Ok::<_, Error>(SqlDateTime::from_units300(integer(v)?)?))
                .transpose()?,
            expire_time: self
                .expire_time_units300
                .map(|v| Ok::<_, Error>(SqlDateTime::from_units300(integer(v)?)?))
                .transpose()?,
            log_date: self
                .log_date_units300
                .map(|v| Ok::<_, Error>(SqlDateTime::from_units300(integer(v)?)?))
                .transpose()?,
        })
    }
}
pub fn read_export(reader: impl Read) -> Result<CharacterBundle, Error> {
    let mut input = Zeroizing::new(Vec::new());
    reader
        .take((MAX_BYTES + 1) as u64)
        .read_to_end(&mut input)
        .map_err(|_| Error::CharacterExport)?;
    if input.len() > MAX_BYTES {
        return Err(Error::CharacterExport);
    }
    let export: Export = serde_json::from_slice(&input).map_err(|_| Error::CharacterExport)?;
    if export.format_version != 1 || export.source != "study_heroes.character-login-bundle" {
        return Err(Error::CharacterExport);
    }
    for (count, size) in [
        (export.source_counts.characters, export.characters.len()),
        (
            export.source_counts.character_info,
            export.character_info.len(),
        ),
        (export.source_counts.connections, export.connections.len()),
        (export.source_counts.queued_items, export.queued_items.len()),
    ] {
        if count != size || size > MAX_ROWS {
            return Err(Error::CharacterExport);
        }
    }
    let rows = CharacterBundle {
        characters: export
            .characters
            .into_iter()
            .map(CharacterIdentityRow::into_snapshot)
            .collect::<Result<_, _>>()?,
        character_info: export
            .character_info
            .into_iter()
            .map(CharacterSnapshotRow::into_snapshot)
            .collect::<Result<_, _>>()?,
        connections: export
            .connections
            .into_iter()
            .map(ConnectionSnapshotRow::into_snapshot)
            .collect::<Result<_, _>>()?,
        queued_items: export
            .queued_items
            .into_iter()
            .map(QueuedItemSnapshotRow::into_snapshot)
            .collect::<Result<_, _>>()?,
    };
    rows.validate()?;
    Ok(rows)
}
