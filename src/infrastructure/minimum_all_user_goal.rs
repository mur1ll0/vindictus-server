//! AllUserGoal events for the laboratory frontend: RefreshAllUserGoalEvent at selection,
//! the MicroPlayInformer → InformAllUserGoalEventProcessor count of a player's event (the
//! heroesShare AddAllUserEventCount procedure, EventAttendUpdate, the announce and the
//! reward through AddQueuedItemEventCurrentConnections), AllUserGoalReward's queued item
//! processing and the scheduler's AllUserGoalEventModify pushes. The windows are the ones the
//! panel's switches moved (`all_user_goal`).
use super::{minimum_assets::Assets, minimum_character::MinimumError};
use crate::domain::{
    all_user_goal::{Goals, SLOTS},
    map_partition::OutgoingPacket,
};
use crate::protocol::{
    DEFAULT_MAX_BODY,
    all_user_goal::{self as wire, Message},
    inventory::Dict,
};
use sqlx::{PgPool, Postgres, Transaction};
use std::sync::{Mutex, PoisonError};

/// SystemMessageCategory.Notice.
const NOTICE: u8 = 1;

/// MicroPlayContents.AllUserGoalTracker IsEndEvent (SetEndEventByID), process-wide as the
/// MicroPlayService's static tracker; a restart clears it as Initialize does.
static ENDED: Mutex<std::collections::BTreeSet<i32>> =
    Mutex::new(std::collections::BTreeSet::new());

fn fail() -> MinimumError {
    MinimumError::Conflict
}
fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
fn ended(goal: i32) -> bool {
    ENDED
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .contains(&goal)
}
fn now() -> Result<i64, MinimumError> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| fail())?;
    i64::try_from(now.as_secs()).map_err(|_| fail())
}
pub fn packet(message: &Message) -> Result<OutgoingPacket, MinimumError> {
    Ok(OutgoingPacket {
        category: message.kind().category(),
        body: wire::encode(message.kind(), Some(message), DEFAULT_MAX_BODY)
            .map_err(|_| fail())?
            .to_vec(),
    })
}

/// FeatureMatrix.IsEnable("AllUserGoalEvent") and the tracker.
fn goals(assets: &Assets) -> Option<&Goals> {
    let npc = assets.npc.as_ref()?;
    npc.policy
        .is_enabled(Some("AllUserGoalEvent"))
        .unwrap_or(false)
        .then_some(&npc.goals)
}

/// AddAllUserEventCount(slotID, increment, endTime): a new slot row, an increment of the
/// same window, or a new window starting at `increment`.
async fn add_count(
    tx: &mut Transaction<'_, Postgres>,
    slot: i32,
    increment: i32,
    end: i64,
) -> Result<i32, MinimumError> {
    Ok(sqlx::query_scalar(
        "INSERT INTO game_all_user_goal_event(slot_id,goal_count,end_seconds) VALUES($1,$2,$3)
         ON CONFLICT (slot_id) DO UPDATE SET
           goal_count=CASE WHEN game_all_user_goal_event.end_seconds=EXCLUDED.end_seconds
                           THEN game_all_user_goal_event.goal_count+EXCLUDED.goal_count
                           ELSE EXCLUDED.goal_count END,
           end_seconds=EXCLUDED.end_seconds
         RETURNING goal_count",
    )
    .bind(slot)
    .bind(increment)
    .bind(end)
    .fetch_one(&mut **tx)
    .await?)
}

/// CharacterEntity.RefreshAllUserGoalEvent (SelectCharacterProcessor): the count of each
/// slot's current goal, or -1 for its next goal.
pub async fn login_message(
    pool: &PgPool,
    assets: &Assets,
) -> Result<Option<OutgoingPacket>, MinimumError> {
    let Some(goals) = goals(assets) else {
        return Ok(None);
    };
    let now = now()?;
    let mut tx = pool.begin().await?;
    let mut rows = Vec::new();
    for slot in SLOTS {
        if let Some(goal) = goals.current_by_slot(slot, now) {
            let value = add_count(&mut tx, goal.slot, 0, goal.end).await?;
            rows.push((goal.id, value));
        } else if let Some(goal) = goals.next_by_slot(slot, now) {
            rows.push((goal.id, -1));
        }
    }
    tx.commit().await?;
    Ok(Some(packet(&Message::Event(Dict(Some(rows))))?))
}

/// What one counted event changes: the AllUserGoalEventModify and announce every client
/// receives, and the goal whose reward is due.
#[derive(Default)]
pub struct Informed {
    pub broadcast: Vec<OutgoingPacket>,
    pub reward: Option<i32>,
}

/// MicroPlayInformer.InformToOtherService (AllUserGoalEvent) for an event of `cid` and the
/// InformAllUserGoalEventProcessor it requests; `party` is every player of the MicroPlay.
pub async fn inform(
    pool: &PgPool,
    assets: &Assets,
    cid: i64,
    event: &str,
    party: &[i64],
) -> Result<Option<Informed>, MinimumError> {
    let Some(goals) = goals(assets) else {
        return Ok(None);
    };
    let now = now()?;
    let id = goals.current_id_by_goal(event, now, &ended);
    let Some(goal) = (id > -1).then(|| goals.goal(id)).flatten() else {
        return Ok(None);
    };
    let mut tx = pool.begin().await?;
    let count = add_count(&mut tx, goal.slot, 1, goal.end).await?;
    let mut out = Informed::default();
    if count <= goal.goal_count {
        out.broadcast.push(packet(&Message::EventModify {
            goal_id: goal.id,
            count,
        })?);
        if count == goal.goal_count {
            out.broadcast.push(super::minimum_shop::system_message(
                NOTICE,
                "GameUI_Heroes_GoalEventDialog_Success_Announce",
                &[goal.name.clone().unwrap_or_default()],
            )?);
        }
        // EventAttendUpdate: every party member for a group goal, else the player.
        let members: Vec<i64> = if Goals::is_group(goal.goal_type) {
            party.to_vec()
        } else {
            vec![cid]
        };
        for member in members {
            sqlx::query(
                "INSERT INTO game_event_attend(event_no,uid,cid,last_update_seconds)
                 SELECT $1,uid,id,$3 FROM game_character_info WHERE id=$2
                 ON CONFLICT (event_no,uid) DO UPDATE SET cid=EXCLUDED.cid,last_update_seconds=EXCLUDED.last_update_seconds",
            )
            .bind(goal.id)
            .bind(member)
            .bind(now)
            .execute(&mut *tx)
            .await?;
        }
    }
    if count >= goal.goal_count {
        ENDED
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(goal.id);
    }
    if count == goal.goal_count
        && let Some(reward) = goal.reward.as_deref()
    {
        queue_reward(&mut tx, reward, goal.reward_num, goal.id, now).await?;
        out.reward = Some(goal.id);
    }
    tx.commit().await?;
    Ok(Some(out))
}

/// AddQueuedItemEventCurrentConnections(reward, count, "AllUserGoalEvent", goalID): one
/// QueuedItem for every connected character and every attendant of an account without a
/// connected one, then the event's attendance of those accounts is cleared.
async fn queue_reward(
    tx: &mut Transaction<'_, Postgres>,
    class_ex: &str,
    count: i32,
    goal: i32,
    now: i64,
) -> Result<(), MinimumError> {
    let initialized: bool =
        sqlx::query_scalar("SELECT initialized FROM game_queue_identity WHERE singleton FOR SHARE")
            .fetch_one(&mut **tx)
            .await?;
    if !initialized {
        return Err(fail());
    }
    let cids: Vec<i64> = sqlx::query_scalar(
        "SELECT DISTINCT cid FROM (
           SELECT id AS cid FROM game_character_info WHERE is_connected
           UNION ALL
           SELECT a.cid FROM game_event_attend a WHERE a.event_no=$1
             AND a.uid NOT IN (SELECT uid FROM game_character_info WHERE is_connected)
         ) t ORDER BY cid LIMIT 100000",
    )
    .bind(goal)
    .fetch_all(&mut **tx)
    .await?;
    let log_date = (now + 2_208_988_800) * 300;
    for cid in cids {
        sqlx::query("INSERT INTO game_queued_items(row_id,cid,item_class_ex_utf16,count,is_character_binded,mail_title_utf16,mail_content_utf16,color1,color2,color3,reduced_durability,max_durability_bonus,item_id,processed,event_code_utf16,processed_time_units300,expire_time_units300,log_date_units300) VALUES(nextval('game_queued_item_row_id')::integer,$1,$2,$3,FALSE,NULL,NULL,-1,-1,-1,0,0,NULL,FALSE,$4,NULL,NULL,$5)")
            .bind(cid)
            .bind(utf16(class_ex))
            .bind(count)
            .bind(utf16("AllUserGoalEvent"))
            .bind(log_date)
            .execute(&mut **tx)
            .await?;
        sqlx::query(
            "DELETE FROM game_event_attend WHERE event_no=$1 AND uid=(SELECT uid FROM game_character_info WHERE id=$2)",
        )
        .bind(goal)
        .bind(cid)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

/// AllUserGoalEventScheduler from `now` for one client: (Unix seconds, GoalID, Count) of the
/// AllUserGoalEventModify pushes, each 5 seconds after its boundary as the source schedules.
pub fn schedule(assets: &Assets, now: i64) -> Vec<(i64, i32, i32)> {
    goals(assets).map_or_else(Vec::new, |g| {
        g.schedule(now)
            .into_iter()
            .map(|(t, goal, count)| (t + 5, goal, count))
            .collect()
    })
}
