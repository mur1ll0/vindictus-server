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
            let lines = npc
                .scripts
                .watched_lines(&rows, &npc.policy)
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
            packets.push(packet(
                Kind::StoryLines,
                Message::StoryLines(StoryLines {
                    status: Some(lines),
                    hint_category: i32::from(c.hint),
                }),
            )?);
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

struct Facts {
    class: i32,
    level: i32,
    titles: std::collections::BTreeSet<i32>,
    items: std::collections::BTreeMap<String, i32>,
    manufactures: std::collections::BTreeMap<String, (i32, i32)>,
}
impl crate::domain::npc_talk::Facts for Facts {
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
    /// The quest service is not migrated and no quest can be given yet: no state.
    fn quest_progress(&self, _: &str) -> i32 {
        -1
    }
}

/// QueryNpcTalk/SelectButton/ExecuteNpcServerCommand processors. A refused request
/// (CanStartQuery, DEV without cheat permission, unmigrated branch) sends nothing.
pub async fn talk(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    session: &mut crate::domain::npc_talk::Session,
    request: TalkRequest<'_>,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    use crate::domain::npc_talk::Talker;
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
        tx.commit().await?;
        let facts = Facts {
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
            &rows,
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
        let mut packets = Vec::new();
        for message in pushed.messages {
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
        if pushed.sync_story {
            return Err(fail());
        }
        Ok(packets)
    })
    .await
    .map_err(|_| fail())?
}
