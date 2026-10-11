//! Insert-only, full-field preservation of character/login/queue source data.
use crate::{
    domain::{character_data::*, legacy_user_data::*},
    error::Error,
    infrastructure::legacy_users::SourceBytes,
};
use sqlx::{PgConnection, PgPool, Row, postgres::PgRow};
const CHARACTERS_COLUMNS: &str = "id";
fn decode_characters(row: PgRow) -> Result<CharacterIdentity, Error> {
    Ok(CharacterIdentity {
        id: row.try_get("id")?,
    })
}
pub async fn read_characters(pool: &PgPool, id: i64) -> Result<Option<CharacterIdentity>, Error> {
    sqlx::query(&format!(
        "SELECT {CHARACTERS_COLUMNS} FROM game_characters WHERE id=$1"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?
    .map(decode_characters)
    .transpose()
}
async fn preserve_characters(
    connection: &mut PgConnection,
    row: &CharacterIdentity,
    verify: bool,
) -> Result<usize, Error> {
    let inserted = if verify {
        0
    } else {
        sqlx::query("INSERT INTO game_characters(id) VALUES($1) ON CONFLICT DO NOTHING")
            .bind(row.id)
            .execute(&mut *connection)
            .await?
            .rows_affected() as usize
    };
    let stored = sqlx::query(&format!(
        "SELECT {CHARACTERS_COLUMNS} FROM game_characters WHERE id=$1 FOR SHARE"
    ))
    .bind(row.id)
    .fetch_optional(&mut *connection)
    .await?
    .map(decode_characters)
    .transpose()?;
    if stored.as_ref() != Some(row) {
        return Err(Error::CharacterSnapshotConflict);
    }
    Ok(inserted)
}
const CHARACTER_INFO_COLUMNS: &str = "id,uid,character_sn,name_utf16,class,level,exp,str,dex,int,will,luck,hp,stamina,dirty,play_time,selected_title,ap,total_used_ap,last_ap_update_units300,permission,status,quote_utf16,delete_time_units300,region_code,is_connected,legacy_last_update,create_time_ticks,free_match_win_count,free_match_lose_count,use_preset,is_premium,free_title_name_utf16,today_play_time,channel,order_num,map_state_info";
fn decode_character_info(row: PgRow) -> Result<CharacterSnapshot, Error> {
    Ok(CharacterSnapshot {
        id: row.try_get("id")?,
        uid: row.try_get("uid")?,
        character_sn: row.try_get("character_sn")?,
        name: row
            .try_get::<Option<SourceBytes>, _>("name_utf16")?
            .map(|v| Ok::<_, Error>(LegacyBytes::from_bytes(v.into_bytes())?))
            .transpose()?,
        class: row.try_get("class")?,
        level: row.try_get("level")?,
        exp: row.try_get("exp")?,
        str: row.try_get("str")?,
        dex: row.try_get("dex")?,
        int: row.try_get("int")?,
        will: row.try_get("will")?,
        luck: row.try_get("luck")?,
        hp: row.try_get("hp")?,
        stamina: row.try_get("stamina")?,
        dirty: row.try_get("dirty")?,
        play_time: row.try_get("play_time")?,
        selected_title: row.try_get("selected_title")?,
        ap: row.try_get("ap")?,
        total_used_ap: row.try_get("total_used_ap")?,
        last_ap_update: SqlDateTime::from_units300(
            row.try_get::<i64, _>("last_ap_update_units300")?,
        )?,
        permission: row.try_get("permission")?,
        status: row.try_get("status")?,
        quote: row
            .try_get::<Option<SourceBytes>, _>("quote_utf16")?
            .map(|v| Ok::<_, Error>(LegacyBytes::from_bytes(v.into_bytes())?))
            .transpose()?,
        delete_time: row
            .try_get::<Option<i64>, _>("delete_time_units300")?
            .map(SqlDateTime::from_units300)
            .transpose()?,
        region_code: row.try_get("region_code")?,
        is_connected: row.try_get("is_connected")?,
        last_update: row
            .try_get::<Option<SourceBytes>, _>("legacy_last_update")?
            .map(|v| Ok::<_, Error>(SourceRowVersion::from_bytes(&v.into_bytes())?))
            .transpose()?,
        create_time: SqlDateTime2::from_ticks(row.try_get::<i64, _>("create_time_ticks")?)?,
        free_match_win_count: row.try_get("free_match_win_count")?,
        free_match_lose_count: row.try_get("free_match_lose_count")?,
        use_preset: row.try_get("use_preset")?,
        is_premium: row.try_get("is_premium")?,
        free_title_name: row
            .try_get::<Option<SourceBytes>, _>("free_title_name_utf16")?
            .map(|v| Ok::<_, Error>(LegacyBytes::from_bytes(v.into_bytes())?))
            .transpose()?,
        today_play_time: row.try_get("today_play_time")?,
        channel: row.try_get("channel")?,
        order_num: row.try_get("order_num")?,
        map_state_info: row.try_get("map_state_info")?,
    })
}
pub async fn read_character_info(
    pool: &PgPool,
    id: i64,
) -> Result<Option<CharacterSnapshot>, Error> {
    sqlx::query(&format!(
        "SELECT {CHARACTER_INFO_COLUMNS} FROM game_character_info WHERE id=$1"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?
    .map(decode_character_info)
    .transpose()
}
async fn preserve_character_info(
    connection: &mut PgConnection,
    row: &CharacterSnapshot,
    verify: bool,
) -> Result<usize, Error> {
    let inserted = if verify {
        0
    } else {
        sqlx::query("INSERT INTO game_character_info(id,uid,character_sn,name_utf16,class,level,exp,str,dex,int,will,luck,hp,stamina,dirty,play_time,selected_title,ap,total_used_ap,last_ap_update_units300,permission,status,quote_utf16,delete_time_units300,region_code,is_connected,legacy_last_update,create_time_ticks,free_match_win_count,free_match_lose_count,use_preset,is_premium,free_title_name_utf16,today_play_time,channel,order_num,map_state_info) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,$23,$24,$25,$26,$27,$28,$29,$30,$31,$32,$33,$34,$35,$36,$37) ON CONFLICT DO NOTHING")
    .bind(row.id)
    .bind(row.uid)
    .bind(row.character_sn)
    .bind(row.name.as_ref().map(|v|v.as_bytes()))
    .bind(row.class)
    .bind(row.level)
    .bind(row.exp)
    .bind(row.str)
    .bind(row.dex)
    .bind(row.int)
    .bind(row.will)
    .bind(row.luck)
    .bind(row.hp)
    .bind(row.stamina)
    .bind(row.dirty)
    .bind(row.play_time)
    .bind(row.selected_title)
    .bind(row.ap)
    .bind(row.total_used_ap)
    .bind(row.last_ap_update.units300())
    .bind(row.permission)
    .bind(row.status)
    .bind(row.quote.as_ref().map(|v|v.as_bytes()))
    .bind(row.delete_time.map(|v|v.units300()))
    .bind(row.region_code)
    .bind(row.is_connected)
    .bind(row.last_update.as_ref().map(|v|v.as_bytes()))
    .bind(row.create_time.ticks())
    .bind(row.free_match_win_count)
    .bind(row.free_match_lose_count)
    .bind(row.use_preset)
    .bind(row.is_premium)
    .bind(row.free_title_name.as_ref().map(|v|v.as_bytes()))
    .bind(row.today_play_time)
    .bind(row.channel)
    .bind(row.order_num)
    .bind(row.map_state_info)
    .execute(&mut *connection).await?.rows_affected() as usize
    };
    let stored = sqlx::query(&format!(
        "SELECT {CHARACTER_INFO_COLUMNS} FROM game_character_info WHERE id=$1 FOR SHARE"
    ))
    .bind(row.id)
    .fetch_optional(&mut *connection)
    .await?
    .map(decode_character_info)
    .transpose()?;
    if stored.as_ref() != Some(row) {
        return Err(Error::CharacterSnapshotConflict);
    }
    Ok(inserted)
}
const CONNECTIONS_COLUMNS: &str = "id,last_connect_date_units300,last_cafe_bonus_units300,last_cafe_login_units300,last_cafe_logout_units300,last_home_cafe_login_units300,last_home_cafe_logout_units300,last_logout_units300";
fn decode_connections(row: PgRow) -> Result<ConnectionSnapshot, Error> {
    Ok(ConnectionSnapshot {
        id: row.try_get("id")?,
        last_connect_date: SqlDateTime::from_units300(
            row.try_get::<i64, _>("last_connect_date_units300")?,
        )?,
        last_cafe_bonus: row
            .try_get::<Option<i64>, _>("last_cafe_bonus_units300")?
            .map(SqlDateTime::from_units300)
            .transpose()?,
        last_cafe_login: row
            .try_get::<Option<i64>, _>("last_cafe_login_units300")?
            .map(SqlDateTime::from_units300)
            .transpose()?,
        last_cafe_logout: row
            .try_get::<Option<i64>, _>("last_cafe_logout_units300")?
            .map(SqlDateTime::from_units300)
            .transpose()?,
        last_home_cafe_login: row
            .try_get::<Option<i64>, _>("last_home_cafe_login_units300")?
            .map(SqlDateTime::from_units300)
            .transpose()?,
        last_home_cafe_logout: row
            .try_get::<Option<i64>, _>("last_home_cafe_logout_units300")?
            .map(SqlDateTime::from_units300)
            .transpose()?,
        last_logout: row
            .try_get::<Option<i64>, _>("last_logout_units300")?
            .map(SqlDateTime::from_units300)
            .transpose()?,
    })
}
pub async fn read_connections(pool: &PgPool, id: i64) -> Result<Option<ConnectionSnapshot>, Error> {
    sqlx::query(&format!(
        "SELECT {CONNECTIONS_COLUMNS} FROM game_last_connect_info WHERE id=$1"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?
    .map(decode_connections)
    .transpose()
}
async fn preserve_connections(
    connection: &mut PgConnection,
    row: &ConnectionSnapshot,
    verify: bool,
) -> Result<usize, Error> {
    let inserted = if verify {
        0
    } else {
        sqlx::query("INSERT INTO game_last_connect_info(id,last_connect_date_units300,last_cafe_bonus_units300,last_cafe_login_units300,last_cafe_logout_units300,last_home_cafe_login_units300,last_home_cafe_logout_units300,last_logout_units300) VALUES($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT DO NOTHING")
    .bind(row.id)
    .bind(row.last_connect_date.units300())
    .bind(row.last_cafe_bonus.map(|v|v.units300()))
    .bind(row.last_cafe_login.map(|v|v.units300()))
    .bind(row.last_cafe_logout.map(|v|v.units300()))
    .bind(row.last_home_cafe_login.map(|v|v.units300()))
    .bind(row.last_home_cafe_logout.map(|v|v.units300()))
    .bind(row.last_logout.map(|v|v.units300()))
    .execute(&mut *connection).await?.rows_affected() as usize
    };
    let stored = sqlx::query(&format!(
        "SELECT {CONNECTIONS_COLUMNS} FROM game_last_connect_info WHERE id=$1 FOR SHARE"
    ))
    .bind(row.id)
    .fetch_optional(&mut *connection)
    .await?
    .map(decode_connections)
    .transpose()?;
    if stored.as_ref() != Some(row) {
        return Err(Error::CharacterSnapshotConflict);
    }
    Ok(inserted)
}
const QUEUED_ITEMS_COLUMNS: &str = "row_id,cid,item_class_ex_utf16,count,is_character_binded,mail_title_utf16,mail_content_utf16,color1,color2,color3,reduced_durability,max_durability_bonus,item_id,processed,event_code_utf16,processed_time_units300,expire_time_units300,log_date_units300";
fn decode_queued_items(row: PgRow) -> Result<QueuedItemSnapshot, Error> {
    Ok(QueuedItemSnapshot {
        row_id: row.try_get("row_id")?,
        cid: row.try_get("cid")?,
        item_class_ex: {
            let v = row.try_get::<SourceBytes, _>("item_class_ex_utf16")?;
            LegacyBytes::from_bytes(v.into_bytes())?
        },
        count: row.try_get("count")?,
        is_character_binded: row.try_get("is_character_binded")?,
        mail_title: row
            .try_get::<Option<SourceBytes>, _>("mail_title_utf16")?
            .map(|v| Ok::<_, Error>(LegacyBytes::from_bytes(v.into_bytes())?))
            .transpose()?,
        mail_content: row
            .try_get::<Option<SourceBytes>, _>("mail_content_utf16")?
            .map(|v| Ok::<_, Error>(LegacyBytes::from_bytes(v.into_bytes())?))
            .transpose()?,
        color1: row.try_get("color1")?,
        color2: row.try_get("color2")?,
        color3: row.try_get("color3")?,
        reduced_durability: row.try_get("reduced_durability")?,
        max_durability_bonus: row.try_get("max_durability_bonus")?,
        item_id: row.try_get("item_id")?,
        processed: row.try_get("processed")?,
        event_code: row
            .try_get::<Option<SourceBytes>, _>("event_code_utf16")?
            .map(|v| Ok::<_, Error>(LegacyBytes::from_bytes(v.into_bytes())?))
            .transpose()?,
        processed_time: row
            .try_get::<Option<i64>, _>("processed_time_units300")?
            .map(SqlDateTime::from_units300)
            .transpose()?,
        expire_time: row
            .try_get::<Option<i64>, _>("expire_time_units300")?
            .map(SqlDateTime::from_units300)
            .transpose()?,
        log_date: row
            .try_get::<Option<i64>, _>("log_date_units300")?
            .map(SqlDateTime::from_units300)
            .transpose()?,
    })
}
pub async fn read_queued_items(
    pool: &PgPool,
    id: i32,
) -> Result<Option<QueuedItemSnapshot>, Error> {
    sqlx::query(&format!(
        "SELECT {QUEUED_ITEMS_COLUMNS} FROM game_queued_items WHERE row_id=$1"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?
    .map(decode_queued_items)
    .transpose()
}
async fn preserve_queued_items(
    connection: &mut PgConnection,
    row: &QueuedItemSnapshot,
    verify: bool,
) -> Result<usize, Error> {
    let inserted = if verify {
        0
    } else {
        sqlx::query("INSERT INTO game_queued_items(row_id,cid,item_class_ex_utf16,count,is_character_binded,mail_title_utf16,mail_content_utf16,color1,color2,color3,reduced_durability,max_durability_bonus,item_id,processed,event_code_utf16,processed_time_units300,expire_time_units300,log_date_units300) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18) ON CONFLICT DO NOTHING")
    .bind(row.row_id)
    .bind(row.cid)
    .bind(row.item_class_ex.as_bytes())
    .bind(row.count)
    .bind(row.is_character_binded)
    .bind(row.mail_title.as_ref().map(|v|v.as_bytes()))
    .bind(row.mail_content.as_ref().map(|v|v.as_bytes()))
    .bind(row.color1)
    .bind(row.color2)
    .bind(row.color3)
    .bind(row.reduced_durability)
    .bind(row.max_durability_bonus)
    .bind(row.item_id)
    .bind(row.processed)
    .bind(row.event_code.as_ref().map(|v|v.as_bytes()))
    .bind(row.processed_time.map(|v|v.units300()))
    .bind(row.expire_time.map(|v|v.units300()))
    .bind(row.log_date.map(|v|v.units300()))
    .execute(&mut *connection).await?.rows_affected() as usize
    };
    let stored = sqlx::query(&format!(
        "SELECT {QUEUED_ITEMS_COLUMNS} FROM game_queued_items WHERE row_id=$1 FOR SHARE"
    ))
    .bind(row.row_id)
    .fetch_optional(&mut *connection)
    .await?
    .map(decode_queued_items)
    .transpose()?;
    if stored.as_ref() != Some(row) {
        return Err(Error::CharacterSnapshotConflict);
    }
    Ok(inserted)
}
#[derive(Default, Clone, Copy, serde::Serialize)]
pub struct BundleCounts {
    pub characters: usize,
    pub character_info: usize,
    pub connections: usize,
    pub queued_items: usize,
}
impl BundleCounts {
    pub fn total(self) -> usize {
        self.characters + self.character_info + self.connections + self.queued_items
    }
}
#[derive(serde::Serialize)]
pub struct BundleSummary {
    pub source_rows: BundleCounts,
    pub inserted: BundleCounts,
    pub verified: BundleCounts,
}
pub async fn import_bundle(
    pool: &PgPool,
    bundle: &CharacterBundle,
    verify_only: bool,
) -> Result<BundleSummary, Error> {
    bundle.validate()?;
    tokio::time::timeout(
        std::time::Duration::from_secs(30),
        execute_bundle(pool, bundle, verify_only),
    )
    .await
    .map_err(|_| Error::CharacterImportDeadline)?
}
async fn execute_bundle(
    pool: &PgPool,
    bundle: &CharacterBundle,
    verify_only: bool,
) -> Result<BundleSummary, Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET LOCAL lock_timeout='2s'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout='5s'")
        .execute(&mut *tx)
        .await?;
    let mut inserted = BundleCounts::default();

    let mut ordered: Vec<_> = bundle.characters.iter().collect();
    ordered.sort_unstable_by_key(|row| row.id);
    for row in ordered {
        inserted.characters += preserve_characters(&mut tx, row, verify_only).await?;
    }
    let mut ordered: Vec<_> = bundle.character_info.iter().collect();
    ordered.sort_unstable_by_key(|row| row.id);
    for row in ordered {
        inserted.character_info += preserve_character_info(&mut tx, row, verify_only).await?;
    }
    let mut ordered: Vec<_> = bundle.connections.iter().collect();
    ordered.sort_unstable_by_key(|row| row.id);
    for row in ordered {
        inserted.connections += preserve_connections(&mut tx, row, verify_only).await?;
    }
    let mut ordered: Vec<_> = bundle.queued_items.iter().collect();
    ordered.sort_unstable_by_key(|row| row.row_id);
    for row in ordered {
        inserted.queued_items += preserve_queued_items(&mut tx, row, verify_only).await?;
    }
    let counts = BundleCounts {
        characters: bundle.characters.len(),
        character_info: bundle.character_info.len(),
        connections: bundle.connections.len(),
        queued_items: bundle.queued_items.len(),
    };
    tx.commit().await?;
    Ok(BundleSummary {
        source_rows: counts,
        inserted,
        verified: counts,
    })
}
pub async fn get_characters(pool: &PgPool, uid: i32) -> Result<Vec<CharacterSnapshot>, Error> {
    let rows=sqlx::query(&format!("SELECT {CHARACTER_INFO_COLUMNS} FROM game_character_info WHERE uid=$1 ORDER BY COALESCE(order_num,255),id LIMIT 1001")).bind(uid).fetch_all(pool).await?;
    if rows.len() > 1000 {
        return Err(Error::CharacterQueryLimit);
    }
    rows.into_iter().map(decode_character_info).collect()
}
pub async fn get_latest_connected_date(
    pool: &PgPool,
    uid: i32,
) -> Result<Option<SqlDateTime>, Error> {
    let value:Option<i64>=sqlx::query_scalar("SELECT MAX(CASE WHEN l.last_connect_date_units300<l.last_cafe_logout_units300 THEN l.last_cafe_logout_units300 ELSE l.last_connect_date_units300 END) FROM game_last_connect_info l WHERE l.id IN (SELECT id FROM game_character_info WHERE uid=$1)").bind(uid).fetch_one(pool).await?;
    Ok(value.map(SqlDateTime::from_units300).transpose()?)
}
pub async fn check_ready(pool: &PgPool) -> Result<(), Error> {
    sqlx::query("SELECT id FROM game_characters WHERE false")
        .execute(pool)
        .await
        .map_err(|_| Error::SchemaNotReady)?;
    sqlx::query("SELECT id,uid,character_sn,name_utf16,class,level,exp,str,dex,int,will,luck,hp,stamina,dirty,play_time,selected_title,ap,total_used_ap,last_ap_update_units300,permission,status,quote_utf16,delete_time_units300,region_code,is_connected,legacy_last_update,create_time_ticks,free_match_win_count,free_match_lose_count,use_preset,is_premium,free_title_name_utf16,today_play_time,channel,order_num,map_state_info FROM game_character_info WHERE false").execute(pool).await.map_err(|_|Error::SchemaNotReady)?;
    sqlx::query("SELECT id,last_connect_date_units300,last_cafe_bonus_units300,last_cafe_login_units300,last_cafe_logout_units300,last_home_cafe_login_units300,last_home_cafe_logout_units300,last_logout_units300 FROM game_last_connect_info WHERE false").execute(pool).await.map_err(|_|Error::SchemaNotReady)?;
    sqlx::query("SELECT row_id,cid,item_class_ex_utf16,count,is_character_binded,mail_title_utf16,mail_content_utf16,color1,color2,color3,reduced_durability,max_durability_bonus,item_id,processed,event_code_utf16,processed_time_units300,expire_time_units300,log_date_units300 FROM game_queued_items WHERE false").execute(pool).await.map_err(|_|Error::SchemaNotReady)?;
    let valid:bool=sqlx::query_scalar("SELECT COUNT(*)=2 FROM pg_constraint WHERE contype='f' AND convalidated AND NOT condeferrable AND conkey=ARRAY[1]::smallint[] AND confkey=ARRAY[1]::smallint[] AND ((conrelid='game_character_info'::regclass AND confrelid='game_characters'::regclass AND confdeltype='c' AND confupdtype='c') OR (conrelid='game_last_connect_info'::regclass AND confrelid='game_character_info'::regclass AND confdeltype='c' AND confupdtype='a'))").fetch_one(pool).await.map_err(|_|Error::SchemaNotReady)?;
    if !valid {
        return Err(Error::SchemaNotReady);
    }
    Ok(())
}
