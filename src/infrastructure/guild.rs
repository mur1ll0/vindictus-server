//! In-game guilds (GuildService with HeroesGuildAPI, B5 social G1): the guild tables of
//! 0062, the FE GuildServiceProxy connection on InGame entry (QueryGuildID → ConnectGuild or
//! OutGuild), OpenGuild, QueryGuildList and the online presence the GuildEntity keeps. Every
//! change other members see goes through `GuildHub`, which pushes through the party hub
//! mailboxes (the single broadcast point of the frontend process).
use super::minimum_character::MinimumError;
use super::party_hub::{Notice, PartyHub};
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
use std::collections::{BTreeMap, BTreeSet, HashMap};
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

/// GuildResultMessage(result, arg) with a null Arg (an operation that failed with a
/// FailMessage leaves ErrorType null).
pub fn result_packet_arg(
    code: i32,
    arg: Option<&str>,
    guild_id: i64,
) -> Result<OutgoingPacket, MinimumError> {
    packet(&Message::GuildResult(GuildResult {
        result: code,
        arg: arg.map(str::to_owned),
        guild_id,
    }))
}

/// SystemMessage(Dialog, "GameUI_Heroes_" + ChangeGuildMaster.ResultCode).
pub fn change_master_message(code: &str) -> Result<OutgoingPacket, MinimumError> {
    super::minimum_shop::system_message(4, &format!("GameUI_Heroes_{code}"), &[])
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
    },
}

/// The sync state of a loaded GuildEntity (a guild with online members).
#[derive(Default)]
struct Entity {
    /// OnlineMembers: character name → CID.
    online: BTreeMap<String, i64>,
    /// ChangedGuildMember.
    changed: BTreeSet<String>,
    /// NotSynchedGuildMember.
    not_synched: BTreeSet<String>,
    /// IsGuildInfoChanged.
    info_changed: bool,
}
/// What one Sync starts from (the flags are cleared when it is taken).
struct Pending {
    online: BTreeMap<String, i64>,
    changed: BTreeSet<String>,
    not_synched: BTreeSet<String>,
    info_changed: bool,
}

/// The GuildEntity of each guild with online members: presence and the Sync flags.
#[derive(Default)]
pub struct GuildHub {
    entities: Mutex<HashMap<i32, Entity>>,
}
impl GuildHub {
    fn lock(&self) -> MutexGuard<'_, HashMap<i32, Entity>> {
        self.entities.lock().unwrap_or_else(PoisonError::into_inner)
    }
    /// GuildEntity.Connect of a known member: OnlineMembers, NotSynched and Changed.
    fn connect(&self, guild_sn: i32, name: &str, cid: i64) {
        let mut entities = self.lock();
        for e in entities.values_mut() {
            e.online.retain(|_, c| *c != cid);
        }
        let e = entities.entry(guild_sn).or_default();
        e.online.insert(name.to_owned(), cid);
        e.not_synched.insert(name.to_owned());
        e.changed.insert(name.to_owned());
    }
    /// GuildEntity.Disconnect: the member leaves OnlineMembers and is reported changed for
    /// the next Sync; an entity without online members closes.
    fn disconnect(&self, cid: i64) -> Option<i32> {
        let mut entities = self.lock();
        let (sn, name) = entities.iter().find_map(|(sn, e)| {
            e.online
                .iter()
                .find(|(_, c)| **c == cid)
                .map(|(n, _)| (*sn, n.clone()))
        })?;
        let close = entities.get_mut(&sn).is_some_and(|e| {
            e.online.remove(&name);
            e.changed.insert(name);
            e.online.is_empty()
        });
        if close {
            entities.remove(&sn);
        }
        Some(sn)
    }
    /// ReportGuildMemberChanged.
    fn report_changed(&self, guild_sn: i32, name: &str) {
        if let Some(e) = self.lock().get_mut(&guild_sn) {
            e.changed.insert(name.to_owned());
        }
    }
    /// ReportGuildInfoChanged.
    fn report_info_changed(&self, guild_sn: i32) {
        if let Some(e) = self.lock().get_mut(&guild_sn) {
            e.info_changed = true;
        }
    }
    pub fn is_online(&self, cid: i64) -> bool {
        self.lock()
            .values()
            .any(|e| e.online.values().any(|c| *c == cid))
    }
    /// The online members of a guild (name → CID).
    fn online(&self, guild_sn: i32) -> BTreeMap<String, i64> {
        self.lock()
            .get(&guild_sn)
            .map(|e| e.online.clone())
            .unwrap_or_default()
    }
    fn take(&self, guild_sn: i32) -> Option<Pending> {
        let mut entities = self.lock();
        let e = entities.get_mut(&guild_sn)?;
        Some(Pending {
            online: e.online.clone(),
            changed: std::mem::take(&mut e.changed),
            not_synched: std::mem::take(&mut e.not_synched),
            info_changed: std::mem::take(&mut e.info_changed),
        })
    }
    /// The closed guild's entity goes away; its online members.
    fn close(&self, guild_sn: i32) -> Vec<i64> {
        self.lock()
            .remove(&guild_sn)
            .map(|e| e.online.into_values().collect())
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
/// order; IsOnline from the entity's online members. (CID, line) pairs.
async fn member_rows(
    pool: &PgPool,
    online: &BTreeMap<String, i64>,
    guild_sn: i32,
) -> Result<Vec<(i64, GuildMember)>, MinimumError> {
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
            let name = name
                .as_deref()
                .map(super::minimum_party::decode)
                .transpose()?;
            let is_online = name.as_ref().is_some_and(|n| online.contains_key(n));
            Ok((
                cid,
                GuildMember {
                    character_name: name,
                    game_level: level,
                    rank: rules::rank_of(t),
                    point: point.unwrap_or(0),
                    last_login_time: last.map_or(0, |last| rules::days_since(now, last)),
                    is_online,
                },
            ))
        })
        .collect()
}

/// The member lines of a guild (MakeGuildMemberMessage_Full order).
pub async fn members(
    pool: &PgPool,
    hub: &GuildHub,
    guild_sn: i32,
) -> Result<Vec<GuildMember>, MinimumError> {
    let online = hub.online(guild_sn);
    Ok(member_rows(pool, &online, guild_sn)
        .await?
        .into_iter()
        .map(|(_, m)| m)
        .collect())
}

/// GuildMemberInfo.InvalidMember: a name no longer in the guild.
fn invalid_member(name: &str) -> GuildMember {
    GuildMember {
        character_name: Some(name.to_owned()),
        game_level: 0,
        rank: wire::rank::UNKNOWN,
        point: 0,
        last_login_time: 0,
        is_online: false,
    }
}

/// What a Sync gave the member that caused it.
#[derive(Debug, Default)]
pub struct Synced {
    pub packets: Vec<OutgoingPacket>,
    /// GuildKicked(LeaveGuildSuccess) for the caller (it is no longer in the guild).
    pub kicked: bool,
}

/// GuildEntity.Sync: GuildInfo to every online member when it changed (else to the members
/// not synced yet), the changed lines to the synced regular members, the full list to each
/// changed member still in the guild and GuildKicked to each one that left, then the full
/// list to the regular members not synced yet. The caller's packets come back; the other
/// members get theirs through the party hub.
async fn sync(
    pool: &PgPool,
    party: &PartyHub,
    hub: &GuildHub,
    guild_sn: i32,
    caller: i64,
) -> Result<Synced, MinimumError> {
    let Some(p) = hub.take(guild_sn) else {
        return Ok(Synced::default());
    };
    let rows = member_rows(pool, &p.online, guild_sn).await?;
    let dict: BTreeMap<String, GuildMember> = rows
        .iter()
        .filter_map(|(_, m)| Some((m.character_name.clone()?, m.clone())))
        .collect();
    let regular = |name: &str| dict.get(name).is_some_and(|m| rules::is_regular(m.rank));
    let mut outbox: BTreeMap<i64, Vec<OutgoingPacket>> = BTreeMap::new();
    let mut kicked = Vec::new();
    if p.info_changed || !p.not_synched.is_empty() {
        let mut guild = info(pool, guild_sn).await?;
        if let Some(g) = guild.as_mut() {
            // MakeGuildInfoMessage: MemberCount = GetRegularMemberCount().
            g.member_count = dict.values().filter(|m| rules::is_regular(m.rank)).count() as i32;
        }
        let message = packet(&Message::GuildInfo(guild))?;
        for (name, cid) in &p.online {
            if p.info_changed || p.not_synched.contains(name) {
                outbox.entry(*cid).or_default().push(message.clone());
            }
        }
    }
    let full = packet(&Message::GuildMemberList(MemberList {
        is_full_update: true,
        members: Some(
            dict.values()
                .filter(|m| m.rank != wire::rank::UNKNOWN && m.rank != wire::rank::IGNORABLE)
                .cloned()
                .map(Some)
                .collect(),
        ),
    }))?;
    if !p.changed.is_empty() {
        let update = packet(&Message::GuildMemberList(MemberList {
            is_full_update: false,
            members: Some(
                p.changed
                    .iter()
                    .map(|n| Some(dict.get(n).cloned().unwrap_or_else(|| invalid_member(n))))
                    .collect(),
            ),
        }))?;
        for (name, cid) in &p.online {
            if !p.not_synched.contains(name) && regular(name) {
                outbox.entry(*cid).or_default().push(update.clone());
            }
        }
        for name in &p.changed {
            let Some(cid) = p.online.get(name) else {
                continue;
            };
            match dict.get(name) {
                Some(m) if m.rank != wire::rank::UNKNOWN && m.rank != wire::rank::IGNORABLE => {
                    outbox.entry(*cid).or_default().push(full.clone());
                }
                _ => kicked.push(*cid),
            }
        }
    }
    for name in &p.not_synched {
        if let Some(cid) = p.online.get(name)
            && regular(name)
        {
            outbox.entry(*cid).or_default().push(full.clone());
        }
    }
    let mut own = Synced::default();
    for (cid, packets) in outbox {
        if cid == caller {
            own.packets = packets;
        } else {
            party.deliver(cid, packets);
        }
    }
    for cid in kicked {
        // GuildKicked, then OnlineGuildMember.Disconnect.
        hub.disconnect(cid);
        if cid == caller {
            own.kicked = true;
        } else {
            party.notify(
                cid,
                Notice::GuildKicked {
                    result: result::LEAVE_GUILD_SUCCESS,
                },
            );
        }
    }
    Ok(own)
}

/// ConnectGuildProcessor: GuildEntity.Connect (with its Sync), then the member's info
/// again (UpdateGroupMemberInfo) and a second Sync; the FE then enters InGuild with
/// ConnectSuccess(GuildSN).
async fn connect_member(
    pool: &PgPool,
    party: &PartyHub,
    hub: &GuildHub,
    cid: i64,
    guild_sn: i32,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let name = super::minimum_party::name_of(pool, cid).await?;
    if info(pool, guild_sn).await?.is_none() {
        return Err(fail());
    }
    hub.connect(guild_sn, &name, cid);
    let mut out = sync(pool, party, hub, guild_sn, cid).await?.packets;
    hub.report_changed(guild_sn, &name);
    out.extend(sync(pool, party, hub, guild_sn, cid).await?.packets);
    out.push(result_packet(
        result::CONNECT_SUCCESS,
        "",
        i64::from(guild_sn),
    )?);
    Ok(out)
}

/// GuildServiceProxy on InGame entry (and Initial after GuildKicked, ReconnectGuild,
/// OpenGuildSuccess or JoinGuildSuccess): QueryGuildID, then Connecting → InGuild for a
/// member, or OutGuild (ConnectSuccess with GuildID 0 and GuildInfo null). A failed lookup
/// answers ConnectFail and stays Initial.
pub async fn enter(
    pool: &PgPool,
    party: &PartyHub,
    hub: &GuildHub,
    cid: i64,
) -> (State, Vec<OutgoingPacket>) {
    let outcome = async {
        match membership(pool, cid).await? {
            Some((sn, _)) => Ok((
                State::InGuild { guild_sn: sn },
                connect_member(pool, party, hub, cid, sn).await?,
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

/// The connection closed: GuildEntity.Disconnect (no Sync; the others see the change at
/// the next one).
pub fn leave(hub: &GuildHub, cid: i64) {
    hub.disconnect(cid);
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

/// The guild's members by name (GuildMemberDict: exact names) with their CID and rank.
async fn roster(
    pool: &PgPool,
    guild_sn: i32,
) -> Result<BTreeMap<String, (i64, i32)>, MinimumError> {
    let rows: Vec<(i64, i16, Option<Vec<u8>>)> = sqlx::query_as(
        "SELECT u.cid,u.group_user_type,c.name_utf16 FROM game_guild_user u \
         JOIN game_character_info c ON c.id=u.cid WHERE u.guild_sn=$1 LIMIT 1000",
    )
    .bind(guild_sn)
    .fetch_all(pool)
    .await?;
    let mut out = BTreeMap::new();
    for (cid, t, name) in rows {
        if let Some(name) = name {
            out.insert(
                super::minimum_party::decode(&name)?,
                (cid, rules::rank_of(t)),
            );
        }
    }
    Ok(out)
}

/// Regular members (Master, Operator, Member) of a roster.
fn regular_count(roster: &BTreeMap<String, (i64, i32)>) -> i32 {
    roster
        .values()
        .filter(|(_, r)| rules::is_regular(*r))
        .count() as i32
}

/// GetMaxMemberLimit of a guild.
async fn max_members(pool: &PgPool, guild_sn: i32) -> Result<i32, MinimumError> {
    let stored: i32 =
        sqlx::query_scalar("SELECT max_member_limit FROM game_guild WHERE oid_guild=$1")
            .bind(guild_sn)
            .fetch_one(pool)
            .await?;
    Ok(rules::max_member_limit(stored))
}

/// UpdateGroupMemberInfo after a change of `name` from rank `before` to `after` (None: no
/// longer a member): the guild info changes when a regular member leaves or a waiting one
/// becomes regular; the member is reported changed.
fn report_member(hub: &GuildHub, guild_sn: i32, name: &str, before: i32, after: Option<i32>) {
    let info_changed = match after {
        None => rules::is_regular(before),
        Some(after) => before == wire::rank::WAIT && rules::is_regular(after),
    };
    if info_changed {
        hub.report_info_changed(guild_sn);
    }
    hub.report_changed(guild_sn, name);
}

/// The SQL that resets a character's guild point (UpdateGuildCharacterInfo(CID, 0)).
const RESET_POINT: &str = "INSERT INTO game_guild_character_info(cid,point) VALUES($1,0) \
     ON CONFLICT (cid) DO UPDATE SET point=0";

/// GuildUserSecede: every GuildUser row of the character and RealUserCount − 1 unless it
/// was waiting; UpdateGuildCharacterInfo(CID, 0). False without a row.
async fn secede(pool: &PgPool, guild_sn: i32, cid: i64) -> Result<bool, MinimumError> {
    let mut tx = pool.begin().await?;
    let removed: Option<i16> =
        sqlx::query_scalar("DELETE FROM game_guild_user WHERE cid=$1 RETURNING group_user_type")
            .bind(cid)
            .fetch_optional(&mut *tx)
            .await?;
    let Some(t) = removed else {
        return Ok(false);
    };
    if t != user_type::WAIT {
        sqlx::query(
            "UPDATE game_guild_info SET real_user_count=real_user_count-1 WHERE guild_sn=$1",
        )
        .bind(guild_sn)
        .execute(&mut *tx)
        .await?;
    }
    sqlx::query(RESET_POINT).bind(cid).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(true)
}

/// GuildUserTypeModify: a new codeGroupUserType (not the current one) with RealUserCount
/// following the Wait boundary.
async fn modify_type(
    pool: &PgPool,
    guild_sn: i32,
    cid: i64,
    to: i16,
) -> Result<bool, MinimumError> {
    let mut tx = pool.begin().await?;
    let current: Option<i16> = sqlx::query_scalar(
        "SELECT group_user_type FROM game_guild_user WHERE cid=$1 AND guild_sn=$2 FOR UPDATE",
    )
    .bind(cid)
    .bind(guild_sn)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(current) = current.filter(|c| *c != to) else {
        return Ok(false);
    };
    sqlx::query("UPDATE game_guild_user SET group_user_type=$2,modified_seconds=$3 WHERE cid=$1")
        .bind(cid)
        .bind(to)
        .bind(now_seconds())
        .execute(&mut *tx)
        .await?;
    let delta = match (current < user_type::WAIT, to < user_type::WAIT) {
        (true, false) => -1,
        (false, true) => 1,
        _ => 0,
    };
    if delta != 0 {
        sqlx::query(
            "UPDATE game_guild_info SET real_user_count=real_user_count+$2 WHERE guild_sn=$1",
        )
        .bind(guild_sn)
        .bind(delta)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(true)
}

/// JoinGuildFail and CloseGuildFail strings of the GuildService processors.
pub mod join_error {
    pub const NOT_INITIALIZE: &str = "GuildNotInitialize";
    pub const GUILD_FULL: &str = "GuildFull";
    /// CloseGuildFail when the guild has other members (waiting ones count).
    pub const HAS_MEMBER: &str = "HasMember";
}

/// JoinGuildProcessor (FE OutGuild JoinGuild): the guild must exist and be open (a closed
/// guild is refused like one that cannot initialize), the character must have no guild
/// and the regular members must be under the limit; GuildUserJoin adds it waiting (155)
/// and the entity syncs. Ok(GuildSN) or the JoinGuildFail string.
pub async fn join(
    pool: &PgPool,
    party: &PartyHub,
    hub: &GuildHub,
    cid: i64,
    guild_sn: i32,
) -> Result<Result<i32, &'static str>, MinimumError> {
    let open: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM game_guild_info WHERE guild_sn=$1 AND closed_seconds IS NULL)",
    )
    .bind(guild_sn)
    .fetch_one(pool)
    .await?;
    if !open {
        return Ok(Err(join_error::NOT_INITIALIZE));
    }
    if membership(pool, cid).await?.is_some() {
        return Ok(Err(open_error::HAS_GUILD));
    }
    let members = roster(pool, guild_sn).await?;
    if regular_count(&members) >= max_members(pool, guild_sn).await? {
        return Ok(Err(join_error::GUILD_FULL));
    }
    let inserted = sqlx::query(
        "INSERT INTO game_guild_user(cid,guild_sn,intro,created_seconds,modified_seconds,group_user_type) \
         VALUES($1,$2,'',$3,$3,$4) ON CONFLICT (cid) DO NOTHING",
    )
    .bind(cid)
    .bind(guild_sn)
    .bind(now_seconds())
    .bind(user_type::WAIT)
    .execute(pool)
    .await?;
    if inserted.rows_affected() != 1 {
        return Ok(Err(open_error::HAS_GUILD));
    }
    let name = super::minimum_party::name_of(pool, cid).await?;
    report_member(
        hub,
        guild_sn,
        &name,
        wire::rank::UNKNOWN,
        Some(wire::rank::WAIT),
    );
    sync(pool, party, hub, guild_sn, cid).await?;
    Ok(Ok(guild_sn))
}

/// LeaveGuildProcessor: GuildUserSecede and the entity sync. None when the character is
/// still a member (LeaveGuildFail); otherwise the caller's sync, whose `kicked` is the
/// GuildKicked(LeaveGuildSuccess) of the FE.
pub async fn leave_guild(
    pool: &PgPool,
    party: &PartyHub,
    hub: &GuildHub,
    cid: i64,
    guild_sn: i32,
) -> Result<Option<Synced>, MinimumError> {
    let name = super::minimum_party::name_of(pool, cid).await?;
    let before = membership(pool, cid)
        .await?
        .map_or(wire::rank::UNKNOWN, |(_, r)| r);
    secede(pool, guild_sn, cid).await?;
    if membership(pool, cid).await?.is_some() {
        return Ok(None);
    }
    report_member(hub, guild_sn, &name, before, None);
    Ok(Some(sync(pool, party, hub, guild_sn, cid).await?))
}

/// CloseGuildProcessor: an online caller and exactly one member (waiting ones count),
/// GuildRemove (dateClosed; the ID stays taken) and SyncCloseGuild (points to 0,
/// GuildKicked(CloseGuildSuccess) to every online member). Err(arg) is the CloseGuildFail
/// argument (None for a FailMessage).
pub async fn close(
    pool: &PgPool,
    party: &PartyHub,
    hub: &GuildHub,
    cid: i64,
    guild_sn: i32,
) -> Result<Result<(), Option<&'static str>>, MinimumError> {
    if !hub.online(guild_sn).values().any(|c| *c == cid) {
        return Ok(Err(None));
    }
    let members = roster(pool, guild_sn).await?;
    if members.len() != 1 {
        return Ok(Err(Some(join_error::HAS_MEMBER)));
    }
    let mut tx = pool.begin().await?;
    let removed: Vec<i64> =
        sqlx::query_scalar("DELETE FROM game_guild_user WHERE guild_sn=$1 RETURNING cid")
            .bind(guild_sn)
            .fetch_all(&mut *tx)
            .await?;
    if removed.len() != 1 {
        return Ok(Err(None));
    }
    sqlx::query(
        "UPDATE game_guild_info SET closed_seconds=$2,modified_seconds=$2 WHERE guild_sn=$1",
    )
    .bind(guild_sn)
    .bind(now_seconds())
    .execute(&mut *tx)
    .await?;
    for (member, _) in members.values() {
        sqlx::query(RESET_POINT)
            .bind(member)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    for other in hub.close(guild_sn).into_iter().filter(|c| *c != cid) {
        party.notify(
            other,
            Notice::GuildKicked {
                result: result::CLOSE_GUILD_SUCCESS,
            },
        );
    }
    Ok(Ok(()))
}

/// The new rank of one GuildOperationInfo target, None when it fails.
async fn apply_operation(
    pool: &PgPool,
    guild_sn: i32,
    operator: i32,
    command: &str,
    value: i32,
    member: i64,
    from: i32,
) -> Result<Option<Option<i32>>, MinimumError> {
    let staff = matches!(operator, wire::rank::MASTER | wire::rank::OPERATOR);
    Ok(match command {
        // AcceptJoin: HasAcceptJoinPermission (Master or Operator on a waiting member).
        "Join" if staff && from == wire::rank::WAIT => {
            if value == 1 {
                modify_type(pool, guild_sn, member, user_type::MEMBER)
                    .await?
                    .then_some(Some(wire::rank::MEMBER))
            } else {
                let rejected = sqlx::query(
                    "DELETE FROM game_guild_user WHERE cid=$1 AND guild_sn=$2 AND group_user_type=$3",
                )
                .bind(member)
                .bind(guild_sn)
                .bind(user_type::WAIT)
                .execute(pool)
                .await?
                .rows_affected()
                    == 1;
                rejected.then_some(None)
            }
        }
        // ChangeRank to Unknown: HasSecedePermission, then GuildUserSecede.
        "Rank" if value == wire::rank::UNKNOWN => {
            let allowed = match operator {
                wire::rank::MASTER => matches!(from, wire::rank::OPERATOR | wire::rank::MEMBER),
                wire::rank::OPERATOR => from == wire::rank::MEMBER,
                _ => false,
            };
            (allowed && secede(pool, guild_sn, member).await?).then_some(None)
        }
        // HasRankChangePermission: Member ↔ Operator by the Master.
        "Rank"
            if operator == wire::rank::MASTER
                && matches!(
                    (from, value),
                    (wire::rank::MEMBER, wire::rank::OPERATOR)
                        | (wire::rank::OPERATOR, wire::rank::MEMBER)
                ) =>
        {
            modify_type(pool, guild_sn, member, rules::user_type_of(value))
                .await?
                .then_some(Some(value))
        }
        _ => None,
    })
}

/// GuildEntity.DoGuildOperation + SyncOperationResult: "Join" (1 accepts, else rejects a
/// waiting member; Master or Operator), "Rank" (0 removes: Master removes Operators and
/// Members, Operator removes Members; Member ↔ Operator by the Master) and
/// "NewbieRecommend" with target " " (Master or Operator). The whole list fails at once
/// when the accepted members would pass the limit. True when every operation succeeded.
pub async fn operate(
    pool: &PgPool,
    party: &PartyHub,
    hub: &GuildHub,
    cid: i64,
    guild_sn: i32,
    operations: &[Option<wire::Operation>],
) -> Result<(bool, Synced), MinimumError> {
    let members = roster(pool, guild_sn).await?;
    let Some(operator) = members.values().find(|(c, _)| *c == cid).map(|(_, r)| *r) else {
        return Ok((false, Synced::default()));
    };
    let ops: Vec<&wire::Operation> = operations.iter().flatten().collect();
    let target = |op: &wire::Operation| op.target.clone().unwrap_or_default();
    let accepts = ops
        .iter()
        .filter(|op| {
            target(op) != " "
                && members.contains_key(&target(op))
                && op.command.as_deref() == Some("Join")
                && op.value == 1
        })
        .count() as i32;
    if accepts + regular_count(&members) > max_members(pool, guild_sn).await? {
        return Ok((false, Synced::default()));
    }
    let mut all = true;
    for op in ops {
        let command = op.command.as_deref().unwrap_or("");
        let name = target(op);
        if name == " " {
            // NewbieGuildRecommend: other commands with the blank target are skipped.
            if command != "NewbieRecommend" {
                continue;
            }
            let staff = matches!(operator, wire::rank::MASTER | wire::rank::OPERATOR);
            let done = staff
                && sqlx::query("UPDATE game_guild SET newbie_recommend=$2 WHERE oid_guild=$1")
                    .bind(guild_sn)
                    .bind(op.value != 0)
                    .execute(pool)
                    .await?
                    .rows_affected()
                    == 1;
            if done {
                hub.report_info_changed(guild_sn);
            } else {
                all = false;
            }
            continue;
        }
        let Some(&(member, from)) = members.get(&name) else {
            all = false;
            continue;
        };
        match apply_operation(pool, guild_sn, operator, command, op.value, member, from).await? {
            Some(after) => report_member(hub, guild_sn, &name, from, after),
            None => all = false,
        }
    }
    let synced = sync(pool, party, hub, guild_sn, cid).await?;
    Ok((all, synced))
}

/// ChangeGuildMaster.ResultCodeEnum names (SystemMessage "GameUI_Heroes_" + code).
pub mod change_master {
    pub const SUCCESS: &str = "ChangeMaster_Success";
    pub const NOT_MATCH_OLD_MASTER: &str = "ChangeMaster_NotMatchOldMaster";
    pub const NOT_FOUND_NEW_MASTER: &str = "ChangeMaster_NotFoundNewMaster";
    pub const LOW_RANK_OLD_MASTER: &str = "ChangeMaster_LowRankOldMaster";
    pub const EQUAL_NEW_AND_OLD_ID: &str = "ChangeMaster_EqualNewAndOldID";
    pub const SYSTEM_ERROR: &str = "ChangeMaster_SystemError";
}

/// ChangeGuildMasterProcessor with the caller's name as the old master: the new master
/// must be an Operator; GuildGroupChangeMaster makes the old master an Operator (15), the
/// new one Master (5) and the owner; the info and both members sync. The caller must be
/// the Master: the source lets any member demote the real owner, which is refused here as
/// NotMatchOldMaster. The result code and the caller's sync.
pub async fn change_master(
    pool: &PgPool,
    party: &PartyHub,
    hub: &GuildHub,
    cid: i64,
    guild_sn: i32,
    new_master: Option<&str>,
) -> Result<(&'static str, Synced), MinimumError> {
    let members = roster(pool, guild_sn).await?;
    let Some((old_name, &(_, old_rank))) = members.iter().find(|(_, (c, _))| *c == cid) else {
        return Ok((change_master::NOT_MATCH_OLD_MASTER, Synced::default()));
    };
    if old_rank != wire::rank::MASTER {
        return Ok((change_master::NOT_MATCH_OLD_MASTER, Synced::default()));
    }
    let Some((new_name, &(new_cid, new_rank))) = new_master.and_then(|n| members.get_key_value(n))
    else {
        return Ok((change_master::NOT_FOUND_NEW_MASTER, Synced::default()));
    };
    if new_cid == cid {
        return Ok((change_master::EQUAL_NEW_AND_OLD_ID, Synced::default()));
    }
    if new_rank != wire::rank::OPERATOR {
        return Ok((change_master::LOW_RANK_OLD_MASTER, Synced::default()));
    }
    let mut tx = pool.begin().await?;
    let owner: Option<Option<i64>> =
        sqlx::query_scalar("SELECT owner_cid FROM game_guild_info WHERE guild_sn=$1 FOR UPDATE")
            .bind(guild_sn)
            .fetch_optional(&mut *tx)
            .await?;
    let Some(Some(owner)) = owner else {
        return Ok((change_master::SYSTEM_ERROR, Synced::default()));
    };
    let set_type = "UPDATE game_guild_user SET group_user_type=$3 WHERE cid=$1 AND guild_sn=$2";
    let demoted = sqlx::query(set_type)
        .bind(owner)
        .bind(guild_sn)
        .bind(user_type::OPERATOR)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    let promoted = sqlx::query(set_type)
        .bind(new_cid)
        .bind(guild_sn)
        .bind(user_type::MASTER)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    if demoted != 1 || promoted != 1 {
        return Ok((change_master::SYSTEM_ERROR, Synced::default()));
    }
    sqlx::query("UPDATE game_guild_info SET owner_cid=$2,modified_seconds=$3 WHERE guild_sn=$1")
        .bind(guild_sn)
        .bind(new_cid)
        .bind(now_seconds())
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    hub.report_info_changed(guild_sn);
    report_member(
        hub,
        guild_sn,
        old_name,
        old_rank,
        Some(wire::rank::OPERATOR),
    );
    report_member(hub, guild_sn, new_name, new_rank, Some(wire::rank::MASTER));
    let synced = sync(pool, party, hub, guild_sn, cid).await?;
    Ok((change_master::SUCCESS, synced))
}

/// EditGuildNoticeProcessor: UpdateGuildInfo with the notice (any member; a null text
/// keeps the stored one, as ISNULL does) and the sync of the changed info. No direct
/// answer.
pub async fn edit_notice(
    pool: &PgPool,
    party: &PartyHub,
    hub: &GuildHub,
    cid: i64,
    guild_sn: i32,
    text: Option<&str>,
) -> Result<Synced, MinimumError> {
    if let Some(text) = text {
        sqlx::query("UPDATE game_guild SET notice=$2 WHERE oid_guild=$1")
            .bind(guild_sn)
            .bind(rules::truncate_utf16(text, rules::NOTICE_MAX))
            .execute(pool)
            .await?;
    }
    hub.report_info_changed(guild_sn);
    sync(pool, party, hub, guild_sn, cid).await
}
