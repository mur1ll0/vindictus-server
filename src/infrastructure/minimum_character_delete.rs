//! Character list DeleteCharacter / DeleteCharacterCancel / SelectOrdersChange: the
//! CharacterMaker.DeleteCharacter/DeleteCharacterCancel and SelectOrdersChangeProcessor storage
//! effects for the laboratory account. Admission (second password) belongs to the frontend.
use super::{
    character_initialization_logs as logs, character_lobby::UserKey,
    character_name_proof::same_name, fiona_character_state as state,
    minimum_character::MinimumError, minimum_character_list,
};
use crate::{
    domain::legacy_user_data::{SqlDateTime, SqlDateTime2},
    protocol::character_delete::{CancelResult, DeleteResult},
};
use sqlx::PgPool;

fn name(raw: &[u8]) -> Result<String, MinimumError> {
    if !raw.len().is_multiple_of(2) {
        return Err(MinimumError::Conflict);
    }
    String::from_utf16(
        &raw.as_chunks::<2>()
            .0
            .iter()
            .map(|a| u16::from_le_bytes(*a))
            .collect::<Vec<_>>(),
    )
    .map_err(|_| MinimumError::Conflict)
}

/// CharacterList.GetCharacterSummary(Name): the listed character whose CharacterID is
/// ordinal-equal to the message name.
pub async fn listed_named(
    pool: &PgPool,
    owner: &UserKey<'_>,
    requested: Option<&str>,
) -> Result<Option<i64>, MinimumError> {
    let Some(requested) = requested else {
        return Ok(None);
    };
    for c in minimum_character_list::characters(pool, owner).await? {
        let stored: Option<Vec<u8>> =
            sqlx::query_scalar("SELECT name_utf16 FROM game_character_info WHERE id=$1")
                .bind(c.cid)
                .fetch_one(pool)
                .await?;
        if stored.map(|s| name(&s)).transpose()?.as_deref() == Some(requested) {
            return Ok(Some(c.cid));
        }
    }
    Ok(None)
}

/// CharacterList with CashShop: QueryCashShopInventory of the listed character and
/// InvolveCharacterCashItem.
pub async fn remain_cash_items(
    pool: &PgPool,
    owner: &UserKey<'_>,
    cid: i64,
) -> Result<bool, MinimumError> {
    let mut tx = state::owned_account(pool, owner).await?;
    let held = super::minimum_cash_shop::holds_character_items(&mut *tx, cid).await?;
    tx.commit().await?;
    Ok(held)
}

/// id, status, DeleteTime (units300) and Name of a CharacterInfo row.
type Row = (i64, i32, Option<i64>, Option<Vec<u8>>);

/// `CharacterInfo.Where(UID, Name, CharacterSN).SingleOrDefault()` (any status): the row id and
/// status, or None; more than one row is the source's SingleOrDefault exception.
async fn find(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: &UserKey<'_>,
    requested: Option<&str>,
    character_sn: i32,
) -> Result<Option<(i64, i32, Option<i64>)>, MinimumError> {
    let Some(requested) = requested else {
        return Ok(None);
    };
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT id,status,delete_time_units300,name_utf16 FROM game_character_info WHERE uid=$1 AND character_sn=$2 ORDER BY id LIMIT 17 FOR UPDATE",
    )
    .bind(owner.uid)
    .bind(character_sn)
    .fetch_all(&mut **tx)
    .await?;
    let mut found = rows
        .into_iter()
        .filter(|r| r.3.as_deref().is_some_and(|n| same_name(requested, n)));
    let first = found.next();
    if found.next().is_some() {
        return Err(MinimumError::Conflict);
    }
    Ok(first.map(|(id, status, deleted, _)| (id, status, deleted)))
}

/// DateTime.UtcNow stored in the SQL datetime column.
fn sql_now(
    utc: &mut impl FnMut() -> Result<SqlDateTime2, MinimumError>,
) -> Result<SqlDateTime, MinimumError> {
    utc()?.sql_parameter().map_err(|_| MinimumError::Conflict)
}

async fn set_status(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    cid: i64,
    status: i32,
    deleted: Option<SqlDateTime>,
) -> Result<(), MinimumError> {
    sqlx::query("UPDATE game_character_info SET status=$2,delete_time_units300=$3 WHERE id=$1")
        .bind(cid)
        .bind(status)
        .bind(deleted.map(SqlDateTime::units300))
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// CharacterMaker.DeleteCharacter. `delay`: FeatureMatrix DeleteCharacterDelay (minutes).
/// Returns the result and WaitingTimeLeft; a storage failure is the source's Fail_Exception.
pub async fn delete(
    pool: &PgPool,
    owner: &UserKey<'_>,
    requested: Option<&str>,
    character_sn: i32,
    delay: i32,
    mut utc: impl FnMut() -> Result<SqlDateTime2, MinimumError>,
) -> (DeleteResult, i32) {
    match delete_inner(pool, owner, requested, character_sn, delay, &mut utc).await {
        Ok(r) => r,
        Err(_) => {
            tracing::warn!("character delete failed; values omitted");
            (DeleteResult::FailException, 0)
        }
    }
}
async fn delete_inner(
    pool: &PgPool,
    owner: &UserKey<'_>,
    requested: Option<&str>,
    character_sn: i32,
    delay: i32,
    utc: &mut impl FnMut() -> Result<SqlDateTime2, MinimumError>,
) -> Result<(DeleteResult, i32), MinimumError> {
    let mut tx = state::owned_account(pool, owner).await?;
    let Some((cid, status, deleted)) = find(&mut tx, owner, requested, character_sn).await? else {
        return Ok((DeleteResult::FailNoSuchCharacter, 0));
    };
    // AddCharacterJournal(ID, "") runs in its own context before the status is read.
    let journal = logs::character_journal(pool, cid, "")
        .await
        .ok_or(MinimumError::Conflict)?;
    let (result, wait, ledger) = match (delay.wrapping_mul(60), status) {
        (0, 0) => {
            set_status(&mut tx, cid, 1, Some(sql_now(utc)?)).await?;
            (DeleteResult::SuccessDeleted, 0, Some(("Deleted", 1)))
        }
        (0, _) => (DeleteResult::FailInvalidStatus, 0, None),
        (_, 0) => {
            set_status(&mut tx, cid, 2, Some(sql_now(utc)?)).await?;
            (DeleteResult::SuccessDeleting, 0, Some(("Deleting", 2)))
        }
        (_, 2) => {
            let deleted = deleted
                .map(SqlDateTime::from_units300)
                .transpose()
                .and_then(|d| d.map(SqlDateTime::framework_date_time).transpose())
                .map_err(|_| MinimumError::Conflict)?;
            let wait = minimum_character_list::delete_wait(delay, deleted, &mut *utc)?;
            if wait > 0 {
                (DeleteResult::FailNeedMoreWaiting, wait, None)
            } else {
                set_status(&mut tx, cid, 1, Some(sql_now(utc)?)).await?;
                (DeleteResult::SuccessDeleted, 0, Some(("Deleted", 1)))
            }
        }
        _ => (DeleteResult::FailInvalidStatus, 0, None),
    };
    tx.commit().await?;
    if let Some((target, value)) = ledger
        && !logs::character_ledger(pool, journal, "DeleteCharacter", target, value).await
    {
        // SubmitChanges already ran; the ledger exception is the source's Fail_Exception.
        return Err(MinimumError::Conflict);
    }
    Ok((result, wait))
}

/// CharacterMaker.DeleteCharacterCancel: status 2 → 0 and DeleteTime null.
pub async fn cancel(
    pool: &PgPool,
    owner: &UserKey<'_>,
    requested: Option<&str>,
    character_sn: i32,
) -> CancelResult {
    let result = async {
        let mut tx = state::owned_account(pool, owner).await?;
        let Some((cid, status, _)) = find(&mut tx, owner, requested, character_sn).await? else {
            return Ok(CancelResult::FailNoSuchCharacter);
        };
        let journal = logs::character_journal(pool, cid, "")
            .await
            .ok_or(MinimumError::Conflict)?;
        if status != 2 {
            return Ok(CancelResult::FailInvalidStatus);
        }
        set_status(&mut tx, cid, 0, None).await?;
        tx.commit().await?;
        if !logs::character_ledger(pool, journal, "DeleteCharacter", "DeleteCancel", 0).await {
            return Err(MinimumError::Conflict);
        }
        Ok::<_, MinimumError>(CancelResult::Success)
    }
    .await;
    result.unwrap_or_else(|_| {
        tracing::warn!("character delete cancel failed; values omitted");
        CancelResult::FailException
    })
}

/// SelectOrdersChangeProcessor: CharacterOrderSet(CID, SelectOrders[n]) for each character in
/// QueryCharacters order. The source's ElementAt throws on a short collection after the earlier
/// rows were set; here the whole change is refused and nothing is written.
pub async fn select_orders(
    pool: &PgPool,
    owner: &UserKey<'_>,
    orders: &[u8],
) -> Result<bool, MinimumError> {
    let mut tx = state::owned_account(pool, owner).await?;
    let rows = minimum_character_list::listed(&mut *tx, owner).await?;
    if orders.len() < rows.len() {
        return Ok(false);
    }
    for (c, order) in rows.iter().zip(orders) {
        sqlx::query("UPDATE game_character_info SET order_num=$2 WHERE id=$1 AND uid=$3")
            .bind(c.cid)
            .bind(i16::from(*order))
            .bind(owner.uid)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(true)
}
