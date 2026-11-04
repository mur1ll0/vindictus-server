//! In-game guilds (GuildService with HeroesGuildAPI, B5 social G1): the guild tables of
//! 0062, the FE GuildServiceProxy connection on InGame entry (QueryGuildID → ConnectGuild or
//! OutGuild), OpenGuild, QueryGuildList and the online presence the GuildEntity keeps. Every
//! change other members see goes through `GuildHub`, which pushes through the party hub
//! mailboxes (the single broadcast point of the frontend process).
use super::minimum_character::MinimumError;
use super::party_hub::PartyHub;
use crate::domain::guild::{self as rules, open_error, user_type};
use crate::domain::map_partition::OutgoingPacket;
use crate::protocol::{
    DEFAULT_MAX_BODY,
    guild::{
        self as wire, ByteDict, GuildInfo, GuildList, GuildMember, GuildResult, MemberList,
        Message, result,
    },
};
use sqlx::PgPool;
use std::collections::{BTreeSet, HashMap};
use std::sync::{Mutex, MutexGuard, PoisonError};

/// Guild name, RealUserCount, master name, MaxMemberLimit, NewbieRecommend, Level, Exp, Notice.
type InfoRow = (String, i32, Option<Vec<u8>>, i32, bool, i32, i64, String);
/// CID, codeGroupUserType, name, level, guild point, last connect and now (units of 1/300 s).
type MemberRow = (
    i64,
    i16,
    Option<Vec<u8>>,
    i32,
    Option<i64>,
    Option<i64>,
    i64,
);
/// GuildSN, guild name, owner CID, owner name and owner status.
type SearchRow = (i32, String, i64, Option<Vec<u8>>, i32);

fn fail() -> MinimumError {
    MinimumError::Conflict
}
pub fn packet(message: &Message) -> Result<OutgoingPacket, MinimumError> {
    let kind = message.kind();
    Ok(OutgoingPacket {
        category: kind.category(),
        body: wire::encode(kind, Some(message), DEFAULT_MAX_BODY)
            .map_err(|_| fail())?
            .to_vec(),
    })
}
/// GuildResultMessage(result, arg, guildID).
pub fn result_packet(code: i32, arg: &str, guild_id: i64) -> Result<OutgoingPacket, MinimumError> {
    packet(&Message::GuildResult(GuildResult {
        result: code,
        arg: Some(arg.to_owned()),
        guild_id,
    }))
}

/// The GuildServiceProxy state of a connection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum State {
    /// Initial (also after a failed QueryGuildID/ConnectGuild): only ReconnectGuild works.
    #[default]
    Initial,
    OutGuild,
    InGuild {
        guild_sn: i32,
        rank: i32,
    },
}

/// Online guild members by guild (OnlineGuildMember of each GuildEntity).
#[derive(Default)]
pub struct GuildHub {
    online: Mutex<HashMap<i32, BTreeSet<i64>>>,
}
impl GuildHub {
    fn lock(&self) -> MutexGuard<'_, HashMap<i32, BTreeSet<i64>>> {
        self.online.lock().unwrap_or_else(PoisonError::into_inner)
    }
    fn join(&self, guild_sn: i32, cid: i64) {
        let mut online = self.lock();
        for members in online.values_mut() {
            members.remove(&cid);
        }
        online.entry(guild_sn).or_default().insert(cid);
    }
    /// The PlayerConn closed: the member leaves its guild's online set.
    fn part(&self, cid: i64) -> Option<i32> {
        let mut online = self.lock();
        let sn = online
            .iter()
            .find(|(_, members)| members.contains(&cid))
            .map(|(sn, _)| *sn)?;
        if let Some(members) = online.get_mut(&sn) {
            members.remove(&cid);
            if members.is_empty() {
                online.remove(&sn);
            }
        }
        Some(sn)
    }
    pub fn is_online(&self, cid: i64) -> bool {
        self.lock().values().any(|members| members.contains(&cid))
    }
    fn others(&self, guild_sn: i32, cid: i64) -> Vec<i64> {
        self.lock()
            .get(&guild_sn)
            .map(|members| members.iter().copied().filter(|c| *c != cid).collect())
            .unwrap_or_default()
    }
}

/// QueryGuildID: the guild and rank of the character (GuildUserGroupGetList of the account,
/// filtered by the character).
pub async fn membership(pool: &PgPool, cid: i64) -> Result<Option<(i32, i32)>, MinimumError> {
    let row: Option<(i32, i16)> = sqlx::query_as(
        "SELECT u.guild_sn,u.group_user_type FROM game_guild_user u \
         JOIN game_guild_info g ON g.guild_sn=u.guild_sn WHERE u.cid=$1",
    )
    .bind(cid)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(sn, t)| (sn, rules::rank_of(t))))
}

/// HeroesGuildInfo.ToGuildInfo (NewbieGuildRecommend and GuildLevel on): GuildGetInfoBySn
/// with the master's name, GetMaxMemberLimit and GetInGameGuildInfo. The daily GP gains
/// (GuildDailyGainGP) arrive with the GP step; until then the dictionary is empty.
pub async fn info(pool: &PgPool, guild_sn: i32) -> Result<Option<GuildInfo>, MinimumError> {
    let row: Option<InfoRow> = sqlx::query_as(
        "SELECT g.guild_name,g.real_user_count,c.name_utf16,d.max_member_limit,\
             d.newbie_recommend,d.level,d.exp,d.notice FROM game_guild_info g \
             JOIN game_character_info c ON c.id=g.owner_cid \
             JOIN game_guild d ON d.oid_guild=g.guild_sn WHERE g.guild_sn=$1",
    )
    .bind(guild_sn)
    .fetch_optional(pool)
    .await?;
    let Some((name, count, master, limit, newbie, level, exp, notice)) = row else {
        return Ok(None);
    };
    let master = master
        .as_deref()
        .map(super::minimum_party::decode)
        .transpose()?;
    Ok(Some(GuildInfo {
        guild_sn,
        guild_name: Some(name),
        guild_level: level,
        member_count: count,
        master_name: master,
        max_member_count: rules::max_member_limit(limit),
        is_newbie_recommend: newbie,
        guild_point: exp,
        guild_notice: Some(notice),
        daily_gain_gp: ByteDict(Some(Vec::new())),
    }))
}

/// GuildAllMemberGetList with each member's level, guild point (GuildCharacterInfo) and
/// GetLatestConnectedDateByName (the later of LastConnectDate and LastCafeLogin), in CID
/// order; IsOnline from the online set.
pub async fn members(
    pool: &PgPool,
    hub: &GuildHub,
    guild_sn: i32,
) -> Result<Vec<GuildMember>, MinimumError> {
    let rows: Vec<MemberRow> = sqlx::query_as(
        "SELECT u.cid,u.group_user_type,c.name_utf16,c.level,p.point,\
             GREATEST(l.last_connect_date_units300,l.last_cafe_login_units300),\
             floor((extract(epoch FROM clock_timestamp())+2208988800)*300+0.5)::BIGINT \
             FROM game_guild_user u JOIN game_character_info c ON c.id=u.cid \
             LEFT JOIN game_guild_character_info p ON p.cid=u.cid \
             LEFT JOIN game_last_connect_info l ON l.id=u.cid \
             WHERE u.guild_sn=$1 ORDER BY u.cid LIMIT 1000",
    )
    .bind(guild_sn)
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|(cid, t, name, level, point, last, now)| {
            Ok(GuildMember {
                character_name: name
                    .as_deref()
                    .map(super::minimum_party::decode)
                    .transpose()?,
                game_level: level,
                rank: rules::rank_of(t),
                point: point.unwrap_or(0),
                last_login_time: last.map_or(0, |last| rules::days_since(now, last)),
                is_online: hub.is_online(cid),
            })
        })
        .collect()
}

/// GuildEntity.Connect + Sync for a connecting member, and ConnectSuccess of the FE
/// Connecting state: GuildInfo, the full member list when the member is regular, then
/// GuildResult(ConnectSuccess, GuildSN). The other online members get the member's line.
async fn connect_member(
    pool: &PgPool,
    party: &PartyHub,
    hub: &GuildHub,
    cid: i64,
    guild_sn: i32,
    member_rank: i32,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    hub.join(guild_sn, cid);
    let Some(guild) = info(pool, guild_sn).await? else {
        hub.part(cid);
        return Err(fail());
    };
    let list = members(pool, hub, guild_sn).await?;
    let mut out = vec![packet(&Message::GuildInfo(Some(guild)))?];
    if rules::is_regular(member_rank) {
        out.push(packet(&Message::GuildMemberList(MemberList {
            is_full_update: true,
            members: Some(list.iter().cloned().map(Some).collect()),
        }))?);
    }
    out.push(result_packet(
        result::CONNECT_SUCCESS,
        "",
        i64::from(guild_sn),
    )?);
    notify_presence(pool, party, hub, guild_sn, cid, &list).await?;
    Ok(out)
}

/// Sync's partial list: the changed member's line to the other online regular members.
async fn notify_presence(
    pool: &PgPool,
    party: &PartyHub,
    hub: &GuildHub,
    guild_sn: i32,
    cid: i64,
    list: &[GuildMember],
) -> Result<(), MinimumError> {
    let others = hub.others(guild_sn, cid);
    if others.is_empty() {
        return Ok(());
    }
    let name = super::minimum_party::name_of(pool, cid).await?;
    let Some(line) = list
        .iter()
        .find(|m| m.character_name.as_deref() == Some(name.as_str()))
    else {
        return Ok(());
    };
    let partial = packet(&Message::GuildMemberList(MemberList {
        is_full_update: false,
        members: Some(vec![Some(line.clone())]),
    }))?;
    let regular: BTreeSet<i64> = sqlx::query_scalar(
        "SELECT cid FROM game_guild_user WHERE guild_sn=$1 AND group_user_type IN (5,15,125)",
    )
    .bind(guild_sn)
    .fetch_all(pool)
    .await?
    .into_iter()
    .collect();
    for other in others.into_iter().filter(|c| regular.contains(c)) {
        party.deliver(other, vec![partial.clone()]);
    }
    Ok(())
}

/// GuildServiceProxy on InGame entry (and Connecting after OpenGuildSuccess or
/// ReconnectGuild): QueryGuildID, then Connecting → InGuild for a member, or OutGuild
/// (ConnectSuccess with GuildID 0 and GuildInfo null). A failed lookup answers ConnectFail
/// and stays Initial.
pub async fn enter(
    pool: &PgPool,
    party: &PartyHub,
    hub: &GuildHub,
    cid: i64,
) -> (State, Vec<OutgoingPacket>) {
    let outcome = async {
        match membership(pool, cid).await? {
            Some((sn, member_rank)) => Ok((
                State::InGuild {
                    guild_sn: sn,
                    rank: member_rank,
                },
                connect_member(pool, party, hub, cid, sn, member_rank).await?,
            )),
            None => Ok::<_, MinimumError>((
                State::OutGuild,
                vec![
                    result_packet(result::CONNECT_SUCCESS, "", 0)?,
                    packet(&Message::GuildInfo(None))?,
                ],
            )),
        }
    }
    .await;
    match outcome {
        Ok(v) => v,
        Err(_) => {
            tracing::warn!("guild connection failed; values omitted");
            (
                State::Initial,
                result_packet(result::CONNECT_FAIL, "", 0)
                    .map(|p| vec![p])
                    .unwrap_or_default(),
            )
        }
    }
}

/// The connection closed: OnlineGuildMember leaves and the others see it offline.
pub async fn leave(pool: &PgPool, party: &PartyHub, hub: &GuildHub, cid: i64) {
    let Some(sn) = hub.part(cid) else { return };
    let notified = async {
        let list = members(pool, hub, sn).await?;
        notify_presence(pool, party, hub, sn, cid, &list).await
    }
    .await;
    if notified.is_err() {
        tracing::warn!("guild presence update failed; values omitted");
    }
}

fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// OpenGuildProcessor: CheckGroupID, CheckGroupName, GuildCreate (one transaction) and
/// UpdateGuildCharacterInfo(CID, 0). `name` and `id` arrive trimmed by the FE; the ID and
/// the intro are cut to their nvarchar sizes. Ok(GuildSN) or the OpenGuildFail string.
pub async fn open(
    pool: &PgPool,
    cid: i64,
    name: &str,
    id: &str,
    intro: Option<&str>,
) -> Result<Result<i32, &'static str>, MinimumError> {
    let id = rules::truncate_utf16(id, rules::ID_MAX);
    let intro = intro.map(|i| rules::truncate_utf16(i, rules::INTRO_MAX));
    if let Err(e) = rules::check_id(&id) {
        return Ok(Err(e));
    }
    let id_taken: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM game_guild_info WHERE lower(guild_id)=lower($1))",
    )
    .bind(&id)
    .fetch_one(pool)
    .await?;
    if id_taken {
        return Ok(Err(open_error::DUPLICATED_ID));
    }
    if let Err(e) = rules::check_name(name) {
        return Ok(Err(e));
    }
    if name_taken(pool, name).await? {
        return Ok(Err(open_error::DUPLICATED_NAME));
    }
    let mut tx = pool.begin().await?;
    // GuildCreate: name and ID again (-4), then the character's GuildUser row (-6).
    let taken: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM game_guild_info WHERE lower(guild_id)=lower($1)) \
         OR EXISTS(SELECT 1 FROM game_guild_info WHERE lower(guild_name)=lower($2) AND closed_seconds IS NULL)",
    )
    .bind(&id)
    .bind(name)
    .fetch_one(&mut *tx)
    .await?;
    if taken {
        return Ok(Err(open_error::DUPLICATED_NAME));
    }
    let member: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM game_guild_user WHERE cid=$1)")
            .bind(cid)
            .fetch_one(&mut *tx)
            .await?;
    if member {
        return Ok(Err(open_error::REPEATED_CHARACTERS));
    }
    let now = now_seconds();
    let inserted: Result<i32, sqlx::Error> = sqlx::query_scalar(
        "INSERT INTO game_guild_info(guild_id,guild_name,intro,owner_cid,real_user_count,created_seconds,modified_seconds) \
         VALUES($1,$2,$3,$4,1,$5,$5) RETURNING guild_sn",
    )
    .bind(&id)
    .bind(name)
    .bind(intro.as_deref())
    .bind(cid)
    .bind(now)
    .fetch_one(&mut *tx)
    .await;
    let sn = match inserted {
        Ok(sn) => sn,
        // A concurrent GuildCreate took the ID or the name.
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            return Ok(Err(open_error::DUPLICATED_NAME));
        }
        Err(e) => return Err(e.into()),
    };
    sqlx::query(
        "INSERT INTO game_guild_user(cid,guild_sn,intro,created_seconds,modified_seconds,group_user_type) VALUES($1,$2,'',$3,$3,$4)",
    )
    .bind(cid)
    .bind(sn)
    .bind(now)
    .bind(user_type::MASTER)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO game_guild(oid_guild,max_member_limit,newbie_recommend,level,exp,notice,last_daily_gp_reset_seconds) VALUES($1,0,false,1,0,'',$2)",
    )
    .bind(sn)
    .bind(now)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO game_guild_character_info(cid,point) VALUES($1,0) ON CONFLICT (cid) DO UPDATE SET point=0",
    )
    .bind(cid)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Ok(sn))
}

/// GuildCheckGroupName: an open guild holds the name (case-insensitive).
async fn name_taken(pool: &PgPool, name: &str) -> Result<bool, MinimumError> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM game_guild_info WHERE lower(guild_name)=lower($1) AND closed_seconds IS NULL)",
    )
    .bind(name)
    .fetch_one(pool)
    .await?)
}

/// SQL Server LIKE `key + '%'` (case-insensitive; `%` and `_` are wildcards; the
/// bracket characters were stripped).
fn like_prefix(name: &str, key: &str) -> bool {
    fn go(n: &[char], k: &[char]) -> bool {
        match k.split_first() {
            None => true,
            Some(('%', rest)) => (0..=n.len()).any(|i| go(&n[i..], rest)),
            Some(('_', rest)) => !n.is_empty() && go(&n[1..], rest),
            Some((c, rest)) => n.first() == Some(c) && go(&n[1..], rest),
        }
    }
    let n: Vec<char> = name.to_lowercase().chars().collect();
    let k: Vec<char> = key.to_lowercase().chars().collect();
    go(&n, &k)
}

/// QueryGuildListProcessor: GuildGroupGetListByGuildName (QueryType 0: the full-text
/// CONTAINS of the key as a word of the name, or LIKE key%, ordered by name descending and
/// paged) or GuildGroupGetListByGuildMaster (the open guilds of the living master with that
/// name, unpaged, TotalPage 1). None when the operation fails (GuildSearchFail).
pub async fn search(
    pool: &PgPool,
    query: &wire::QueryGuildList,
) -> Result<Option<GuildList>, MinimumError> {
    let Some(key) = query
        .search_key
        .as_deref()
        .filter(|k| rules::search_key_valid(k))
    else {
        return Ok(None);
    };
    let rows: Vec<SearchRow> = sqlx::query_as(
        "SELECT g.guild_sn,g.guild_name,g.owner_cid,c.name_utf16,c.status FROM game_guild_info g \
         JOIN game_character_info c ON c.id=g.owner_cid WHERE g.closed_seconds IS NULL LIMIT 5000",
    )
    .fetch_all(pool)
    .await?;
    let (matched, total_page): (Vec<i32>, i32) = if query.query_type == 0 {
        let key = rules::search_name_key(key);
        let lower = key.to_lowercase();
        let mut hits: Vec<(String, i32)> = rows
            .into_iter()
            .filter(|(_, name, ..)| {
                like_prefix(name, &key)
                    || name
                        .to_lowercase()
                        .split(|c: char| !c.is_alphanumeric())
                        .any(|w| !w.is_empty() && w == lower)
            })
            .map(|(sn, name, ..)| (name, sn))
            .collect();
        hits.sort_by_key(|h| std::cmp::Reverse(h.0.to_lowercase()));
        let Some(total) = rules::total_pages(hits.len() as i64, query.page_size) else {
            return Ok(None);
        };
        let size = i64::from(query.page_size);
        let start = (i64::from(query.page) - 1) * size + 1;
        let end = (i64::from(query.page) - 1) * size + size;
        (
            hits.into_iter()
                .enumerate()
                .filter(|(i, _)| (start..=end).contains(&(*i as i64 + 1)))
                .map(|(_, (_, sn))| sn)
                .collect(),
            total,
        )
    } else {
        let lower = key.to_lowercase();
        let mut hits = Vec::new();
        for (sn, _, _, master, status) in rows {
            let Some(master) = master else { continue };
            if status == 0 && super::minimum_party::decode(&master)?.to_lowercase() == lower {
                hits.push(sn);
            }
        }
        (hits, 1)
    };
    let mut list = Vec::with_capacity(matched.len());
    for sn in matched {
        if let Some(i) = info(pool, sn).await? {
            list.push(Some(i));
        }
    }
    Ok(Some(GuildList {
        guild_list: Some(list),
        page: query.page,
        total_page,
    }))
}
