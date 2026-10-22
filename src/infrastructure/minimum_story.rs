//! StoryEntity state of the selected first Fiona: StoryProgress/StoryGuide storage,
//! the login auto-give of storylines (OpenStoryLine + SyncStoryProgress) and
//! QueryNpcList. Story logs (StoryTelling journals) are not migrated.
use super::{character_lobby::UserKey, minimum_assets::Assets, minimum_character::MinimumError};
use crate::{
    domain::{
        map_partition::OutgoingPacket,
        npc_script::{NpcScripts, Progress},
    },
    protocol::{
        DEFAULT_MAX_BODY,
        inventory::{
            self as wire, BriefStoryLine, Building, Kind, Message, NpcInfo, StoryInfo, StoryLines,
        },
    },
};
use sqlx::{PgPool, Postgres, Transaction};

fn fail() -> MinimumError {
    MinimumError::Conflict
}
fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
fn text(raw: &[u8]) -> Result<String, MinimumError> {
    if !raw.len().is_multiple_of(2) || raw.len() > 400 {
        return Err(fail());
    }
    String::from_utf16(
        &raw.as_chunks::<2>()
            .0
            .iter()
            .map(|a| u16::from_le_bytes(*a))
            .collect::<Vec<_>>(),
    )
    .map_err(|_| fail())
}
fn packet(kind: Kind, message: Message) -> Result<OutgoingPacket, MinimumError> {
    Ok(OutgoingPacket {
        category: kind.category(),
        body: wire::encode(kind, Some(&message), DEFAULT_MAX_BODY)
            .map_err(|_| fail())?
            .to_vec(),
    })
}

/// Entry.StoryProgress in storage order.
async fn progress(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
) -> Result<Vec<Progress>, MinimumError> {
    let rows: Vec<(Vec<u8>, Option<Vec<u8>>, i16)> = sqlx::query_as(
        "SELECT story_line_utf16,phase_utf16,status FROM game_story_progress WHERE cid=$1 ORDER BY row_id LIMIT 4097",
    )
    .bind(cid)
    .fetch_all(&mut **tx)
    .await?;
    if rows.len() > 4096 {
        return Err(fail());
    }
    rows.into_iter()
        .map(|(line, phase, status)| {
            Ok(Progress {
                story_line: text(&line)?,
                phase: phase.as_deref().map(text).transpose()?,
                status: u8::try_from(status).map_err(|_| fail())?,
            })
        })
        .collect()
}

fn npc_list_packet(
    scripts: &NpcScripts,
    policy: &crate::domain::feature_policy::FeaturePolicy,
    progress: &[Progress],
) -> Result<OutgoingPacket, MinimumError> {
    let buildings = scripts
        .building_list(progress, policy)
        .map_err(|_| fail())?
        .into_iter()
        .map(|(building, npcs)| {
            Some(Building {
                building: Some(building),
                npcs: Some(
                    npcs.into_iter()
                        .map(|n| {
                            Some(NpcInfo {
                                npc: Some(n.npc),
                                feature: Some(n.feature),
                                story_lines: Some(
                                    n.story_lines
                                        .into_iter()
                                        .map(|(line, revealing)| {
                                            Some(StoryInfo {
                                                story_line: Some(line),
                                                is_revealing: revealing,
                                            })
                                        })
                                        .collect(),
                                ),
                            })
                        })
                        .collect(),
                ),
            })
        })
        .collect();
    packet(Kind::NpcList, Message::NpcList(Some(buildings)))
}

struct Character {
    level: i32,
    class: i32,
    hint: i16,
}
async fn character(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
) -> Result<Character, MinimumError> {
    let (connected, level, class): (bool, i32, i32) = sqlx::query_as(
        "SELECT is_connected,level,class::integer FROM game_character_info WHERE id=$1",
    )
    .bind(cid)
    .fetch_one(&mut **tx)
    .await?;
    // StoryEntity: Entry = StoryEntry.Where(CID).Single().
    let hint: Option<i16> =
        sqlx::query_scalar("SELECT hint_category FROM game_story_entries WHERE cid=$1 FOR UPDATE")
            .bind(cid)
            .fetch_optional(&mut **tx)
            .await?;
    if !connected {
        return Err(fail());
    }
    Ok(Character {
        level,
        class,
        hint: hint.ok_or_else(fail)?,
    })
}

/// QueryNpcListProcessor: SendNpcListMessage(GetBuildingList()).
pub async fn npc_list(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
) -> Result<OutgoingPacket, MinimumError> {
    let npc = assets.npc.as_ref().ok_or_else(fail)?;
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
        character(&mut tx, cid).await?;
        let rows = progress(&mut tx, cid).await?;
        let out = npc_list_packet(&npc.scripts, &npc.policy, &rows)?;
        tx.commit().await?;
        Ok(out)
    })
    .await
    .map_err(|_| fail())?
}

/// InGame.OnEnter QueryStoryProgress and QueryStoryGuide: SyncStoryProgress
/// (StoryLinesMessage, NpcListMessage, StoryVariablesMessage when NewStoryLineDialog is
/// on) and SyncGuideInfo (UpdateStoryGuideMessage, empty strings without a guide).
pub async fn in_game_sync(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let npc = assets.npc.as_ref().ok_or_else(fail)?;
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
        let c = character(&mut tx, cid).await?;
        let rows = progress(&mut tx, cid).await?;
        let mut out = vec![
            story_lines_packet(npc, &rows, c.hint)?,
            npc_list_packet(&npc.scripts, &npc.policy, &rows)?,
        ];
        if npc
            .policy
            .is_enabled(Some("NewStoryLineDialog"))
            .map_err(|_| fail())?
        {
            let variables = variables(&mut tx, cid).await?;
            out.push(packet(
                Kind::StoryVariables,
                Message::StoryVariables(Some(
                    variables
                        .into_iter()
                        .map(|(line, key, value)| {
                            Some(wire::StoryVariable {
                                story_line: Some(line),
                                key: Some(key),
                                value,
                            })
                        })
                        .collect(),
                )),
            )?);
        }
        let (target, message) = guide(&mut tx, cid).await?.unwrap_or_default();
        out.push(packet(
            Kind::UpdateStoryGuide,
            Message::UpdateStoryGuide(wire::StoryGuide {
                target: Some(target),
                message: Some(message),
            }),
        )?);
        tx.commit().await?;
        Ok(out)
    })
    .await
    .map_err(|_| fail())?
}

/// SetStoryLineStatusProcessor: for an owned storyline, StoryEntity.SetStoryLineStatus
/// (Progress.Status = status; the watching list follows it), Save and
/// SyncWatchingStoryLines. An unknown storyline answers nothing.
pub async fn set_story_line_status(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    story_line: &str,
    status: u8,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let npc = assets.npc.as_ref().ok_or_else(fail)?;
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
        let c = character(&mut tx, cid).await?;
        let mut rows = progress(&mut tx, cid).await?;
        let Some(row) = rows.iter_mut().find(|p| p.story_line == story_line) else {
            tx.commit().await?;
            return Ok(Vec::new());
        };
        row.status = status;
        sqlx::query(
            "UPDATE game_story_progress SET status=$3 WHERE cid=$1 AND story_line_utf16=$2",
        )
        .bind(cid)
        .bind(utf16(story_line))
        .bind(i16::from(status))
        .execute(&mut *tx)
        .await?;
        let out = story_lines_packet(npc, &rows, c.hint)?;
        tx.commit().await?;
        Ok(vec![out])
    })
    .await
    .map_err(|_| fail())?
}

/// SendStoryLinesMessage(GetWatchingStoryLines(), HintCategory).
fn story_lines_packet(
    npc: &super::npc_scripts::Profile,
    rows: &[Progress],
    hint: i16,
) -> Result<OutgoingPacket, MinimumError> {
    let lines = npc
        .scripts
        .watched_lines(rows, &npc.policy)
        .map_err(|_| fail())?
        .into_iter()
        .map(|l| {
            Some(BriefStoryLine {
                story_line: Some(l.story_line),
                phase: Some(l.phase),
                status: l.status,
                phase_text: Some(l.goal.unwrap_or_default()),
            })
        })
        .collect();
    packet(
        Kind::StoryLines,
        Message::StoryLines(StoryLines {
            status: Some(lines),
            hint_category: i32::from(hint),
        }),
    )
}

/// Entity IDs the login auto-give needs (one per storyline it opens).
pub async fn login_openings(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
) -> Result<usize, MinimumError> {
    let npc = assets.npc.as_ref().ok_or_else(fail)?;
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
    let c = character(&mut tx, cid).await?;
    let rows = progress(&mut tx, cid).await?;
    let opened = npc
        .scripts
        .login_openings(&rows, c.level, c.class)
        .map_err(|_| fail())?;
    tx.commit().await?;
    Ok(opened.len())
}

/// StoryEntity construction for the entering character: the first-story guide when no
/// progress exists, OpenStoryLine for every login auto-give storyline, then
/// SyncStoryProgress (StoryLines, NpcList) when one was opened.
pub async fn login(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    entity_ids: Vec<i64>,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let npc = assets.npc.as_ref().ok_or_else(fail)?;
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
        let c = character(&mut tx, cid).await?;
        let mut rows = progress(&mut tx, cid).await?;
        if rows.is_empty() {
            sqlx::query("INSERT INTO game_story_guides(cid,guide_target_utf16,guide_message_utf16) VALUES($1,$2,$3) ON CONFLICT(cid) DO UPDATE SET guide_target_utf16=EXCLUDED.guide_target_utf16,guide_message_utf16=EXCLUDED.guide_message_utf16")
                .bind(cid)
                .bind(utf16("landmark_adv_guild"))
                .bind(utf16("GameUI_Heroes_Storyline_Character_First_Story_Guide"))
                .execute(&mut *tx)
                .await?;
        }
        let opened = npc
            .scripts
            .login_openings(&rows, c.level, c.class)
            .map_err(|_| fail())?;
        if opened.len() != entity_ids.len() {
            return Err(fail());
        }
        for (p, id) in opened.iter().zip(entity_ids) {
            sqlx::query("INSERT INTO game_story_progress(row_id,cid,story_line_utf16,phase_utf16,status,last_update_units300) VALUES($1,$2,$3,$4,$5,floor((extract(epoch FROM clock_timestamp())+2208988800)*300+0.5)::BIGINT)")
                .bind(id)
                .bind(cid)
                .bind(utf16(&p.story_line))
                .bind(p.phase.as_deref().map(utf16))
                .bind(i16::from(p.status))
                .execute(&mut *tx)
                .await?;
        }
        let mut packets = Vec::new();
        if !opened.is_empty() {
            rows.extend(opened);
            packets.push(story_lines_packet(npc, &rows, c.hint)?);
            packets.push(npc_list_packet(&npc.scripts, &npc.policy, &rows)?);
        }
        tx.commit().await?;
        Ok(packets)
    })
    .await
    .map_err(|_| fail())?
}

/// Client requests handled by the story service query.
pub enum TalkRequest<'a> {
    /// QueryNpcTalkMessage: Location, NpcID, StoryLine, Command.
    TalkTo {
        location: Option<&'a str>,
        npc: &'a str,
        story_line: Option<&'a str>,
        command: Option<&'a str>,
    },
    SelectButton(i32),
    ExecuteServerCommand,
}

struct Facts<'a> {
    quests: Option<&'a crate::domain::quest::QuestContents>,
    raid_coupons: i64,
    class: i32,
    level: i32,
    titles: std::collections::BTreeSet<i32>,
    items: std::collections::BTreeMap<String, i32>,
    repair_price: i32,
    gold: i32,
    manufactures: std::collections::BTreeMap<String, (i32, i32)>,
}
impl crate::domain::npc_talk::Facts for Facts<'_> {
    fn class(&self) -> i32 {
        self.class
    }
    fn level(&self) -> i32 {
        self.level
    }
    fn title_acquired(&self, id: i32) -> bool {
        self.titles.contains(&id)
    }
    fn item_count(&self, class: &str) -> i32 {
        self.items.get(class).copied().unwrap_or(0)
    }
    fn manufacture(&self, id: &str) -> Option<(i32, i32)> {
        self.manufactures.get(id).copied()
    }
    fn quests(&self) -> Option<&crate::domain::quest::QuestContents> {
        self.quests
    }
    fn quest_context(&self) -> (bool, i64) {
        (false, self.raid_coupons)
    }
    fn repair_price(&self) -> i32 {
        self.repair_price
    }
    fn gold(&self) -> i32 {
        self.gold
    }
}

/// QueryNpcTalk/SelectButton/ExecuteNpcServerCommand processors. A refused request
/// (CanStartQuery, DEV without cheat permission, unmigrated branch) sends nothing.
#[allow(clippy::too_many_arguments)]
pub async fn talk(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    session: &mut crate::domain::npc_talk::Session,
    request: TalkRequest<'_>,
    entity_ids: Vec<i64>,
    growth: super::minimum_growth::GrowthContext,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let clock = growth.clock;
    use crate::domain::npc_talk::{StoryState, Talker};
    let npc = assets.npc.as_ref().ok_or_else(fail)?;
    if let TalkRequest::TalkTo { npc: id, .. } = &request
        && (!session.can_start_query || *id == "DEV")
    {
        return Err(fail());
    }
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
        let c = character(&mut tx, cid).await?;
        let rows = progress(&mut tx, cid).await?;
        let titles: Vec<i32> = sqlx::query_scalar(
            "SELECT title_id FROM game_titles WHERE cid=$1 AND acquired LIMIT 65537",
        )
        .bind(cid)
        .fetch_all(&mut *tx)
        .await?;
        let items: Vec<(Vec<u8>, i64)> = sqlx::query_as(
            "SELECT item_class_utf16,sum(count)::BIGINT FROM game_items WHERE owner_id=$1 AND expire_ticks IS NULL GROUP BY item_class_utf16 LIMIT 4097",
        )
        .bind(cid)
        .fetch_all(&mut *tx)
        .await?;
        let manufactures: Vec<(Vec<u8>, i32, i32)> = sqlx::query_as(
            "SELECT manufacture_utf16,grade,experience_point FROM game_manufacture WHERE cid=$1 ORDER BY row_id LIMIT 4097",
        )
        .bind(cid)
        .fetch_all(&mut *tx)
        .await?;
        let repair_price = super::minimum_repair::price_of_all(&mut tx, owner, assets, cid).await?;
        let gold_key = assets.inventory.gold_class().to_vec();
        let gold: i64 = sqlx::query_scalar(
            "SELECT COALESCE(sum(count),0)::BIGINT FROM game_items WHERE owner_id=$1 AND item_class_utf16=$2",
        )
        .bind(cid)
        .bind(&gold_key)
        .fetch_one(&mut *tx)
        .await?;
        // QuestOwner after Load (InitQuestStatus) with the NotifyLevel CharacterLevel.
        let contents = npc.quests.as_ref();
        let raw_quests = match contents {
            Some(c) => super::minimum_quest::load(&mut tx, cid, c).await?,
            None => crate::domain::quest::QuestOwner::default(),
        };
        let mut quests = raw_quests.clone();
        if let Some(q) = contents {
            quests.init_status(q);
            quests.level = c.level;
        }
        let raid_coupons = super::minimum_quest::raid_coupons(&mut tx, cid).await?;
        let mut state = StoryState {
            progress: rows,
            saves: saves(&mut tx, cid).await?,
            variables: variables(&mut tx, cid).await?,
            guide: guide(&mut tx, cid).await?,
            item_delta: Vec::new(),
            quests,
        };
        let original = state.clone();
        let facts = Facts {
            quests: contents,
            raid_coupons,
            repair_price,
            gold: i32::try_from(gold).map_err(|_| fail())?,
            class: c.class,
            level: c.level,
            titles: titles.into_iter().collect(),
            items: items
                .into_iter()
                .map(|(k, n)| Ok((text(&k)?, i32::try_from(n).map_err(|_| fail())?)))
                .collect::<Result<_, MinimumError>>()?,
            manufactures: manufactures
                .into_iter()
                .map(|(k, g, e)| Ok((text(&k)?, (g, e))))
                .collect::<Result<_, MinimumError>>()?,
        };
        // DoRandom: a fresh System.Random (Environment.TickCount seed).
        let mut random = crate::domain::item_colors::ColorRandom::from_seed(
            (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| fail())?
                .as_millis()
                & 0x7fff_ffff) as i32,
        );
        let mut pick = |n: usize| {
            random
                .next(i32::try_from(n).unwrap_or(i32::MAX))
                .ok()
                .and_then(|v| usize::try_from(v).ok())
                .unwrap_or(0)
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| fail())?
            .as_secs();
        let minute_second = (now % 3600) as i32;
        let backup = session.clone();
        let talker = Talker::new(
            &npc.scripts,
            &npc.policy,
            &facts,
            &mut state,
            &mut pick,
            minute_second,
            session,
        );
        let pushed = match request {
            TalkRequest::TalkTo {
                location,
                npc: id,
                story_line,
                command,
            } => talker.talk_to(location, id, story_line, command),
            TalkRequest::SelectButton(index) => talker.select_button(index),
            TalkRequest::ExecuteServerCommand => talker.execute_server_command(),
        };
        let pushed = match pushed {
            Ok(p) => p,
            Err(e) => {
                // An unmigrated branch leaves the StoryEntity as it was; a source halt
                // keeps the open query, as the original does.
                if matches!(e, crate::domain::npc_talk::TalkError::Unsupported(_)) {
                    *session = backup;
                }
                tracing::warn!(reason = ?e, "minimum NPC talk branch not completed; nothing sent");
                return Err(fail());
            }
        };
        let mut ids = entity_ids.into_iter();
        let quest_ctx = contents.map(|contents| crate::domain::quest::Context {
            contents,
            policy: &npc.policy,
            cafe: false,
            raid_coupons,
        });
        let mut rewards = Vec::new();
        let applied = apply_outputs(
            &mut tx,
            owner,
            assets,
            cid,
            c.hint,
            pushed.outputs,
            &mut ids,
            (quest_ctx, clock),
            &mut rewards,
        )
        .await;
        let mut packets = match applied {
            Ok(p) => p,
            Err(e) => {
                *session = backup;
                tracing::warn!("minimum NPC talk effects not applied; nothing sent");
                return Err(e);
            }
        };
        if let Err(e) = save_state(&mut tx, cid, &original, &state, &mut ids).await {
            *session = backup;
            return Err(e);
        }
        if let Err(e) =
            super::minimum_quest::save(&mut tx, cid, &raw_quests, &state.quests, &mut ids).await
        {
            *session = backup;
            return Err(e);
        }
        // GiveReward runs on the saved story and quest state; its packets keep their
        // position among the query outputs.
        for (at, effect) in rewards.into_iter().rev() {
            let applied = match effect {
                crate::domain::npc_talk::Effect::NoticeTitle(title) => {
                    super::minimum_growth::notice_title(&mut tx, owner, assets, cid, title, growth)
                        .await
                }
                crate::domain::npc_talk::Effect::Reward { exp, ap } => {
                    super::minimum_growth::give_reward(
                        &mut tx,
                        owner,
                        assets,
                        cid,
                        i64::from(exp),
                        ap,
                        growth,
                        &mut ids,
                    )
                    .await
                }
                _ => Err(fail()),
            };
            match applied {
                Ok(reward) => {
                    let tail = packets.split_off(at);
                    packets.extend(reward);
                    packets.extend(tail);
                }
                Err(e) => {
                    *session = backup;
                    tracing::warn!("minimum story reward not applied; nothing sent");
                    return Err(e);
                }
            }
        }
        tx.commit().await?;
        Ok(packets)
    })
    .await
    .map_err(|_| fail())?
}

/// Entity IDs a talk request reserves for opened storylines, quests and created items
/// (a level-up reward can open storylines and give quests).
pub const TALK_ENTITY_IDS: usize = 512;

/// StoryImpl.NotifyLevelUp: open the storylines auto-given at levels (last, cur]; when
/// any opened, SyncStoryProgress (StoryLines, NpcList).
pub(super) async fn level_up(
    tx: &mut Transaction<'_, Postgres>,
    assets: &Assets,
    cid: i64,
    last: i32,
    cur: i32,
    class: i32,
    ids: &mut (dyn Iterator<Item = i64> + Send),
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let npc = assets.npc.as_ref().ok_or_else(fail)?;
    let c = character(tx, cid).await?;
    let mut rows = progress(tx, cid).await?;
    let opened = npc
        .scripts
        .level_up_openings(&rows, last, cur, class)
        .map_err(|_| fail())?;
    if opened.is_empty() {
        return Ok(Vec::new());
    }
    for p in &opened {
        sqlx::query("INSERT INTO game_story_progress(row_id,cid,story_line_utf16,phase_utf16,status,last_update_units300) VALUES($1,$2,$3,$4,$5,floor((extract(epoch FROM clock_timestamp())+2208988800)*300+0.5)::BIGINT)")
            .bind(ids.next().ok_or_else(fail)?)
            .bind(cid)
            .bind(utf16(&p.story_line))
            .bind(p.phase.as_deref().map(utf16))
            .bind(i16::from(p.status))
            .execute(&mut **tx)
            .await?;
    }
    rows.extend(opened);
    Ok(vec![
        story_lines_packet(npc, &rows, c.hint)?,
        npc_list_packet(&npc.scripts, &npc.policy, &rows)?,
    ])
}

/// RewardCharacter's StoryEntity.NotifyEvent(Event_QuestFinished) in the caller's
/// transaction: the moved progress rows, then SyncStoryProgress when one moved. Returns
/// the story sector ID (−1 without one).
pub(super) async fn quest_finished(
    tx: &mut Transaction<'_, Postgres>,
    assets: &Assets,
    cid: i64,
    event: &crate::domain::npc_script::QuestFinished<'_>,
) -> Result<(i32, Vec<OutgoingPacket>), MinimumError> {
    let npc = assets.npc.as_ref().ok_or_else(fail)?;
    let mut rows = progress(tx, cid).await?;
    let original = rows.clone();
    let outcome = npc.scripts.quest_finished(&mut rows, event);
    if outcome.skipped_on_enter > 0 {
        tracing::warn!(
            phases = outcome.skipped_on_enter,
            "story OnEnterPhase after a quest event is not migrated; values omitted"
        );
    }
    if !outcome.changed {
        return Ok((outcome.story_sector, Vec::new()));
    }
    for (p, o) in rows.iter().zip(&original) {
        if p == o {
            continue;
        }
        sqlx::query("UPDATE game_story_progress SET phase_utf16=$3,status=$4,last_update_units300=floor((extract(epoch FROM clock_timestamp())+2208988800)*300+0.5)::BIGINT WHERE cid=$1 AND story_line_utf16=$2")
            .bind(cid)
            .bind(utf16(&p.story_line))
            .bind(p.phase.as_deref().map(utf16))
            .bind(i16::from(p.status))
            .execute(&mut **tx)
            .await?;
    }
    let c = character(tx, cid).await?;
    Ok((
        outcome.story_sector,
        vec![
            story_lines_packet(npc, &rows, c.hint)?,
            npc_list_packet(&npc.scripts, &npc.policy, &rows)?,
        ],
    ))
}

/// Packets of the query outputs in source order; item effects run in the caller's
/// transaction, so a failed give (no mail fallback) refuses the whole request.
#[allow(clippy::too_many_arguments)]
async fn apply_outputs(
    tx: &mut Transaction<'_, Postgres>,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    hint: i16,
    outputs: Vec<crate::domain::npc_talk::Output>,
    ids: &mut std::vec::IntoIter<i64>,
    quests: (
        Option<crate::domain::quest::Context<'_>>,
        super::minimum_quest::QuestClock,
    ),
    rewards: &mut Vec<(usize, crate::domain::npc_talk::Effect)>,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    use super::minimum_give::Request;
    use crate::domain::npc_talk::{Effect, Output};
    let npc = assets.npc.as_ref().ok_or_else(fail)?;
    let mut packets = Vec::new();
    for output in outputs {
        match output {
            Output::Talk(message) => {
                let content = message
                    .into_iter()
                    .map(|e| {
                        Some(wire::TalkEntity {
                            command: Some(e.command),
                            arg1: Some(e.arg1),
                            arg2: Some(e.arg2),
                        })
                    })
                    .collect();
                packets.push(packet(Kind::NpcTalk, Message::NpcTalk(Some(content)))?);
            }
            Output::SyncStory(rows) => {
                packets.push(story_lines_packet(npc, &rows, hint)?);
                packets.push(npc_list_packet(&npc.scripts, &npc.policy, &rows)?);
            }
            Output::QuestProgress(snapshot) => {
                let (ctx, clock) = quests;
                let ctx = ctx.ok_or_else(fail)?;
                // PushQuestProgressMessage needs CharacterLevel > 0.
                if snapshot.level > 0 {
                    packets.push(super::minimum_quest::progress_packet(
                        &snapshot,
                        ctx,
                        clock,
                        super::minimum_quest::now_units()?,
                    )?);
                }
            }
            Output::Guide(target, message) => packets.push(packet(
                Kind::UpdateStoryGuide,
                Message::UpdateStoryGuide(wire::StoryGuide {
                    target: Some(target),
                    message: Some(message),
                }),
            )?),
            Output::Variables(variables) => packets.push(packet(
                Kind::StoryVariables,
                Message::StoryVariables(Some(
                    variables
                        .into_iter()
                        .map(|(line, key, value)| {
                            Some(wire::StoryVariable {
                                story_line: Some(line),
                                key: Some(key),
                                value,
                            })
                        })
                        .collect(),
                )),
            )?),
            Output::Effect(effect @ (Effect::Reward { .. } | Effect::NoticeTitle(_))) => {
                rewards.push((packets.len(), effect))
            }
            Output::Effect(effect @ (Effect::GiveItem { .. } | Effect::TakeItem { .. })) => {
                let (class, count, take) = match &effect {
                    Effect::GiveItem { class, count } => (class, *count, false),
                    Effect::TakeItem { class, count } => (class, *count, true),
                    _ => return Err(fail()),
                };
                let key = utf16(class);
                let request = [Request {
                    class: &key,
                    count,
                    bound: false,
                    equipment: None,
                }];
                let (destroys, gives): (&[Request<'_>], &[Request<'_>]) = if take {
                    (&request, &[])
                } else {
                    (&[], &request)
                };
                let applied = super::minimum_give::apply(
                    tx,
                    owner,
                    assets,
                    cid,
                    destroys,
                    gives,
                    &mut || ids.next().ok_or_else(fail),
                )
                .await?
                .map_err(|reason| {
                    tracing::warn!(?reason, "minimum story item effect refused; values omitted");
                    fail()
                })?;
                packets.push(packet(
                    Kind::UpdateInventoryInfo,
                    Message::UpdateInventoryInfo(Some(applied.slots)),
                )?);
            }
            Output::Effect(Effect::RepairAll) => {
                let repaired = super::minimum_repair::repair(
                    tx,
                    owner,
                    assets,
                    cid,
                    super::minimum_repair::Selection {
                        all_repairable: true,
                        ..Default::default()
                    },
                )
                .await?
                .ok_or_else(fail)?;
                packets.push(packet(
                    Kind::UpdateInventoryInfo,
                    Message::UpdateInventoryInfo(Some(repaired.slots)),
                )?);
            }
        }
    }
    Ok(packets)
}

/// Entry.StorySave rows: (NpcID, StoryLineID, CmdID).
async fn saves(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
) -> Result<Vec<(String, String, String)>, MinimumError> {
    let rows: Vec<(Vec<u8>, Vec<u8>, Vec<u8>)> = sqlx::query_as(
        "SELECT npc_utf16,story_line_utf16,cmd_utf16 FROM game_story_saves WHERE cid=$1 ORDER BY row_id LIMIT 4097",
    )
    .bind(cid)
    .fetch_all(&mut **tx)
    .await?;
    if rows.len() > 4096 {
        return Err(fail());
    }
    rows.into_iter()
        .map(|(n, l, c)| Ok((text(&n)?, text(&l)?, text(&c)?)))
        .collect()
}

/// Entry.StoryVariable rows: (StoryLineID, Key, Value).
async fn variables(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
) -> Result<Vec<(String, String, i32)>, MinimumError> {
    let rows: Vec<(Vec<u8>, Vec<u8>, i32)> = sqlx::query_as(
        "SELECT story_line_utf16,key_utf16,value FROM game_story_variables WHERE cid=$1 ORDER BY row_id LIMIT 4097",
    )
    .bind(cid)
    .fetch_all(&mut **tx)
    .await?;
    if rows.len() > 4096 {
        return Err(fail());
    }
    rows.into_iter()
        .map(|(l, k, v)| Ok((text(&l)?, text(&k)?, v)))
        .collect()
}

/// GuideTarget and GuideMessage columns.
type GuideRow = (Option<Vec<u8>>, Option<Vec<u8>>);

/// Entry.StoryGuide (NULL columns read as empty strings).
async fn guide(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
) -> Result<Option<(String, String)>, MinimumError> {
    let row: Option<GuideRow> = sqlx::query_as(
        "SELECT guide_target_utf16,guide_message_utf16 FROM game_story_guides WHERE cid=$1",
    )
    .bind(cid)
    .fetch_optional(&mut **tx)
    .await?;
    row.map(|(t, m)| {
        Ok((
            t.as_deref().map(text).transpose()?.unwrap_or_default(),
            m.as_deref().map(text).transpose()?.unwrap_or_default(),
        ))
    })
    .transpose()
}

/// StoryEntity.Save of the changed rows.
async fn save_state(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
    original: &crate::domain::npc_talk::StoryState,
    state: &crate::domain::npc_talk::StoryState,
    ids: &mut std::vec::IntoIter<i64>,
) -> Result<(), MinimumError> {
    for p in &state.progress {
        let before = original
            .progress
            .iter()
            .find(|o| o.story_line == p.story_line);
        match before {
            Some(o) if o == p => {}
            Some(_) => {
                sqlx::query("UPDATE game_story_progress SET phase_utf16=$3,status=$4,last_update_units300=floor((extract(epoch FROM clock_timestamp())+2208988800)*300+0.5)::BIGINT WHERE cid=$1 AND story_line_utf16=$2")
                    .bind(cid)
                    .bind(utf16(&p.story_line))
                    .bind(p.phase.as_deref().map(utf16))
                    .bind(i16::from(p.status))
                    .execute(&mut **tx)
                    .await?;
            }
            None => {
                sqlx::query("INSERT INTO game_story_progress(row_id,cid,story_line_utf16,phase_utf16,status,last_update_units300) VALUES($1,$2,$3,$4,$5,floor((extract(epoch FROM clock_timestamp())+2208988800)*300+0.5)::BIGINT)")
                    .bind(ids.next().ok_or_else(fail)?)
                    .bind(cid)
                    .bind(utf16(&p.story_line))
                    .bind(p.phase.as_deref().map(utf16))
                    .bind(i16::from(p.status))
                    .execute(&mut **tx)
                    .await?;
            }
        }
    }
    if state.saves != original.saves {
        sqlx::query("DELETE FROM game_story_saves WHERE cid=$1")
            .bind(cid)
            .execute(&mut **tx)
            .await?;
        for (npc, line, command) in &state.saves {
            sqlx::query("INSERT INTO game_story_saves(cid,npc_utf16,story_line_utf16,cmd_utf16) VALUES($1,$2,$3,$4)")
                .bind(cid)
                .bind(utf16(npc))
                .bind(utf16(line))
                .bind(utf16(command))
                .execute(&mut **tx)
                .await?;
        }
    }
    if state.variables != original.variables {
        sqlx::query("DELETE FROM game_story_variables WHERE cid=$1")
            .bind(cid)
            .execute(&mut **tx)
            .await?;
        for (line, key, value) in &state.variables {
            sqlx::query("INSERT INTO game_story_variables(cid,story_line_utf16,key_utf16,value) VALUES($1,$2,$3,$4)")
                .bind(cid)
                .bind(utf16(line))
                .bind(utf16(key))
                .bind(value)
                .execute(&mut **tx)
                .await?;
        }
    }
    if state.guide != original.guide
        && let Some((target, message)) = &state.guide
    {
        sqlx::query("INSERT INTO game_story_guides(cid,guide_target_utf16,guide_message_utf16) VALUES($1,$2,$3) ON CONFLICT(cid) DO UPDATE SET guide_target_utf16=EXCLUDED.guide_target_utf16,guide_message_utf16=EXCLUDED.guide_message_utf16")
            .bind(cid)
            .bind(utf16(target))
            .bind(utf16(message))
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}
