//! In-game guild messages recovered from the Framework serializer
//! (ServiceCore.EndPointNetwork.GuildService): OpenGuild (338), GuildResult (281), GuildInfo
//! (278, InGameGuildInfo), GuildMemberList (336, GuildMemberInfo), QueryGuildList (339),
//! GuildList (279), JoinGuild (284), LeaveGuild (285), CloseGuild (277), ReconnectGuild
//! (286), GuildOperation (337, GuildOperationInfo), ChangeMaster (84), InviteGuild (283),
//! GuildNoticeEdit (280), GuildStoryRequest (282) and the guild chat: GuildChatMemberInfo
//! (85), GuildChat (86) and JoinGuildChatRoom (88).
use super::{
    inventory::{Wire, object},
    wire::{PayloadError, Reader, Writer},
};
use thiserror::Error;
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

const MAX_ROWS: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    OpenGuild,
    GuildResult,
    GuildInfo,
    GuildMemberList,
    QueryGuildList,
    GuildList,
    JoinGuild,
    LeaveGuild,
    CloseGuild,
    ReconnectGuild,
    GuildOperation,
    ChangeMaster,
    InviteGuild,
    GuildNoticeEdit,
    GuildStoryRequest,
    GuildChatMemberInfo,
    GuildChat,
    JoinGuildChatRoom,
}
impl Kind {
    /// Messages the client sends (GuildServiceProxy and its states).
    pub const INGRESS: [Kind; 13] = [
        Kind::OpenGuild,
        Kind::QueryGuildList,
        Kind::JoinGuild,
        Kind::LeaveGuild,
        Kind::CloseGuild,
        Kind::ReconnectGuild,
        Kind::GuildOperation,
        Kind::ChangeMaster,
        Kind::InviteGuild,
        Kind::GuildNoticeEdit,
        Kind::GuildStoryRequest,
        Kind::GuildChat,
        Kind::JoinGuildChatRoom,
    ];
    pub const fn category(self) -> i32 {
        match self {
            Self::OpenGuild => 338,
            Self::GuildResult => 281,
            Self::GuildInfo => 278,
            Self::GuildMemberList => 336,
            Self::QueryGuildList => 339,
            Self::GuildList => 279,
            Self::JoinGuild => 284,
            Self::LeaveGuild => 285,
            Self::CloseGuild => 277,
            Self::ReconnectGuild => 286,
            Self::GuildOperation => 337,
            Self::ChangeMaster => 84,
            Self::InviteGuild => 283,
            Self::GuildNoticeEdit => 280,
            Self::GuildStoryRequest => 282,
            Self::GuildChatMemberInfo => 85,
            Self::GuildChat => 86,
            Self::JoinGuildChatRoom => 88,
        }
    }
    pub const fn type_id(self) -> Uuid {
        Uuid::from_u128(match self {
            Self::OpenGuild => 0x37c909f5_c8f6_33d2_8f2c_7dda5b1fe08d,
            Self::GuildResult => 0x4d108cd7_8273_3e5b_afcc_2d077d73e7e0,
            Self::GuildInfo => 0x647a5f98_b588_328f_b58d_ddc8164d3014,
            Self::GuildMemberList => 0xe9a63382_19a4_39b9_915a_765a485a84b0,
            Self::QueryGuildList => 0x34da3a6d_e9a2_3f2f_83a6_e73965a7b636,
            Self::GuildList => 0xd74f17bb_616b_3a80_a17f_46217557ad03,
            Self::JoinGuild => 0x7abea10d_5632_3388_bea1_c342d53465f8,
            Self::LeaveGuild => 0xeb9a9076_bd3a_3358_87b9_14a00200e1f8,
            Self::CloseGuild => 0x89f37dba_2fc1_3b7d_9512_465648c48278,
            Self::ReconnectGuild => 0x918435ec_3213_3b76_b1b8_569b1f7657cb,
            Self::GuildOperation => 0x0c77318b_cd6a_3bd6_8d6c_1e26f68f8717,
            Self::ChangeMaster => 0x8f635448_72bd_3685_bcea_b6cd22d0db8d,
            Self::InviteGuild => 0x51b65671_7ce1_3ca7_8572_1b02fb80076a,
            Self::GuildNoticeEdit => 0x3df39210_4621_3c4a_b85a_a1c864c4f86d,
            Self::GuildStoryRequest => 0xefc67527_b0af_310f_a8b7_b39c1922e670,
            Self::GuildChatMemberInfo => 0x95a85ad2_b272_3422_9b92_3a664dc0cba9,
            Self::GuildChat => 0x76a1e908_29cf_3ecd_8275_9453e1534766,
            Self::JoinGuildChatRoom => 0x6ea314db_4675_36a1_979a_d0942107da04,
        })
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GuildWireError {
    #[error(transparent)]
    Payload(#[from] PayloadError),
    #[error("message does not match its kind")]
    Kind,
    #[error("unsupported guild category/type")]
    Type,
}

/// GuildResultEnum.
pub mod result {
    pub const CONNECT_SUCCESS: i32 = 0;
    pub const CONNECT_FAIL: i32 = 1;
    pub const GUILD_SEARCH_FAIL: i32 = 2;
    pub const JOIN_GUILD_SUCCESS: i32 = 3;
    pub const JOIN_GUILD_FAIL: i32 = 4;
    pub const OPEN_GUILD_SUCCESS: i32 = 5;
    pub const OPEN_GUILD_FAIL: i32 = 6;
    pub const LEAVE_GUILD_SUCCESS: i32 = 7;
    pub const LEAVE_GUILD_FAIL: i32 = 8;
    pub const CLOSE_GUILD_SUCCESS: i32 = 9;
    pub const CLOSE_GUILD_FAIL: i32 = 10;
    pub const GUILD_OPERATION_SUCCESS: i32 = 11;
    pub const GUILD_OPERATION_FAIL: i32 = 12;
    pub const GIVE_STORY_SUCCESS: i32 = 13;
    pub const GIVE_STORY_FAIL: i32 = 14;
    pub const JOIN_GUILD_CHAT_SUCCESS: i32 = 15;
    pub const JOIN_GUILD_CHAT_FAIL: i32 = 16;
}

/// GuildMemberRank.
pub mod rank {
    pub const UNKNOWN: i32 = 0;
    pub const MASTER: i32 = 1;
    pub const OPERATOR: i32 = 2;
    pub const MEMBER: i32 = 3;
    pub const WAIT: i32 = 4;
    pub const IGNORABLE: i32 = 5;
}

/// IDictionary<byte, int>: count, then unique byte keys and values.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ByteDict(pub Option<Vec<(u8, i32)>>);
impl Wire for ByteDict {
    fn write(&self, w: &mut Writer) -> Result<(), PayloadError> {
        let Some(rows) = &self.0 else {
            return w.append(&[0]);
        };
        if rows.len() > MAX_ROWS {
            return Err(PayloadError::TooLarge);
        }
        w.append(&super::encode_integer((rows.len() + 1) as i32))?;
        for (i, (k, v)) in rows.iter().enumerate() {
            if rows[..i].iter().any(|(other, _)| other == k) {
                return Err(PayloadError::DuplicateKey);
            }
            k.write(w)?;
            v.write(w)?;
        }
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PayloadError> {
        let Some(n) = r.count()? else {
            return Ok(Self(None));
        };
        if n > MAX_ROWS || n > r.remaining.len() {
            return Err(PayloadError::TooLarge);
        }
        let mut rows: Vec<(u8, i32)> = Vec::with_capacity(n);
        for _ in 0..n {
            let k = u8::read(r)?;
            if rows.iter().any(|(other, _)| *other == k) {
                return Err(PayloadError::DuplicateKey);
            }
            rows.push((k, i32::read(r)?));
        }
        Ok(Self(Some(rows)))
    }
}

/// InGameGuildInfo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuildInfo {
    pub guild_sn: i32,
    pub guild_name: Option<String>,
    pub guild_level: i32,
    pub member_count: i32,
    pub master_name: Option<String>,
    pub max_member_count: i32,
    pub is_newbie_recommend: bool,
    pub guild_point: i64,
    pub guild_notice: Option<String>,
    /// GPGainType → today's gain.
    pub daily_gain_gp: ByteDict,
}
object!(GuildInfo {
    guild_sn,
    guild_name,
    guild_level,
    member_count,
    master_name,
    max_member_count,
    is_newbie_recommend,
    guild_point,
    guild_notice,
    daily_gain_gp
});

/// GuildMemberInfo (LastLoginTime is the number of days since the last login).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuildMember {
    pub character_name: Option<String>,
    pub game_level: i32,
    pub rank: i32,
    pub point: i64,
    pub last_login_time: i32,
    pub is_online: bool,
}
object!(GuildMember {
    character_name,
    game_level,
    rank,
    point,
    last_login_time,
    is_online
});

/// GuildOperationInfo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Operation {
    pub command: Option<String>,
    pub target: Option<String>,
    pub value: i32,
}
object!(Operation {
    command,
    target,
    value
});

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenGuild {
    pub guild_name: Option<String>,
    pub guild_name_id: Option<String>,
    pub guild_intro: Option<String>,
}
object!(OpenGuild {
    guild_name,
    guild_name_id,
    guild_intro
});

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuildResult {
    pub result: i32,
    pub arg: Option<String>,
    pub guild_id: i64,
}
object!(GuildResult {
    result,
    arg,
    guild_id
});

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemberList {
    pub is_full_update: bool,
    pub members: Option<Vec<Option<GuildMember>>>,
}
object!(MemberList {
    is_full_update,
    members
});

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueryGuildList {
    pub query_type: i32,
    pub search_key: Option<String>,
    pub page: i32,
    pub page_size: u8,
}
object!(QueryGuildList {
    query_type,
    search_key,
    page,
    page_size
});

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuildList {
    pub guild_list: Option<Vec<Option<GuildInfo>>>,
    pub page: i32,
    pub total_page: i32,
}
object!(GuildList {
    guild_list,
    page,
    total_page
});

/// GuildChatMessage (both directions; the server fills Sender).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuildChat {
    pub sender: Option<String>,
    pub message: Option<String>,
}
object!(GuildChat { sender, message });

/// GuildChatMemberInfoMessage: a member joined or left the chat room.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatMemberInfo {
    pub sender: Option<String>,
    pub is_online: bool,
}
object!(ChatMemberInfo { sender, is_online });

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    OpenGuild(OpenGuild),
    GuildResult(GuildResult),
    /// GuildInfoMessage.GuildInfo (null without a guild).
    GuildInfo(Option<GuildInfo>),
    GuildMemberList(MemberList),
    QueryGuildList(QueryGuildList),
    GuildList(GuildList),
    /// JoinGuildMessage.GuildSN.
    JoinGuild(i32),
    LeaveGuild,
    CloseGuild,
    ReconnectGuild,
    GuildOperation(Option<Vec<Option<Operation>>>),
    /// ChangeMasterMessage.NewMasterName.
    ChangeMaster(Option<String>),
    /// InviteGuildMessage.Name.
    InviteGuild(Option<String>),
    /// GuildNoticeEditMessage.Text.
    GuildNoticeEdit(Option<String>),
    GuildStoryRequest,
    GuildChatMemberInfo(ChatMemberInfo),
    GuildChat(GuildChat),
    /// JoinGuildChatRoomMessage.GuildKey.
    JoinGuildChatRoom(i64),
}
impl Message {
    pub const fn kind(&self) -> Kind {
        match self {
            Self::OpenGuild(_) => Kind::OpenGuild,
            Self::GuildResult(_) => Kind::GuildResult,
            Self::GuildInfo(_) => Kind::GuildInfo,
            Self::GuildMemberList(_) => Kind::GuildMemberList,
            Self::QueryGuildList(_) => Kind::QueryGuildList,
            Self::GuildList(_) => Kind::GuildList,
            Self::JoinGuild(_) => Kind::JoinGuild,
            Self::LeaveGuild => Kind::LeaveGuild,
            Self::CloseGuild => Kind::CloseGuild,
            Self::ReconnectGuild => Kind::ReconnectGuild,
            Self::GuildOperation(_) => Kind::GuildOperation,
            Self::ChangeMaster(_) => Kind::ChangeMaster,
            Self::InviteGuild(_) => Kind::InviteGuild,
            Self::GuildNoticeEdit(_) => Kind::GuildNoticeEdit,
            Self::GuildStoryRequest => Kind::GuildStoryRequest,
            Self::GuildChatMemberInfo(_) => Kind::GuildChatMemberInfo,
            Self::GuildChat(_) => Kind::GuildChat,
            Self::JoinGuildChatRoom(_) => Kind::JoinGuildChatRoom,
        }
    }
}

pub fn encode(
    kind: Kind,
    message: Option<&Message>,
    max_body: usize,
) -> Result<Zeroizing<Vec<u8>>, GuildWireError> {
    let mut w = Writer {
        bytes: Vec::new(),
        limit: max_body,
    };
    let result = (|| -> Result<(), GuildWireError> {
        let Some(m) = message else {
            w.append(&[0])?;
            return Ok(());
        };
        if kind != m.kind() {
            return Err(GuildWireError::Kind);
        }
        w.append(&[1])?;
        match m {
            Message::OpenGuild(v) => v.write(&mut w)?,
            Message::GuildResult(v) => v.write(&mut w)?,
            Message::GuildInfo(v) => v.write(&mut w)?,
            Message::GuildMemberList(v) => v.write(&mut w)?,
            Message::QueryGuildList(v) => v.write(&mut w)?,
            Message::GuildList(v) => v.write(&mut w)?,
            Message::JoinGuild(v) => v.write(&mut w)?,
            Message::GuildChatMemberInfo(v) => v.write(&mut w)?,
            Message::GuildChat(v) => v.write(&mut w)?,
            Message::JoinGuildChatRoom(v) => v.write(&mut w)?,
            Message::GuildOperation(v) => v.write(&mut w)?,
            Message::ChangeMaster(v) | Message::InviteGuild(v) | Message::GuildNoticeEdit(v) => {
                v.write(&mut w)?
            }
            Message::LeaveGuild
            | Message::CloseGuild
            | Message::ReconnectGuild
            | Message::GuildStoryRequest => {}
        }
        Ok(())
    })();
    match result {
        Ok(()) => Ok(Zeroizing::new(w.bytes)),
        Err(e) => {
            w.bytes.zeroize();
            Err(e)
        }
    }
}

/// Strict decode: the whole body must be consumed (encoder round trips and fixtures).
pub fn decode(kind: Kind, body: &[u8], max_body: usize) -> Result<Option<Message>, GuildWireError> {
    decode_body(kind, body, max_body, true)
}

fn decode_body(
    kind: Kind,
    body: &[u8],
    max_body: usize,
    strict: bool,
) -> Result<Option<Message>, GuildWireError> {
    if body.len() > max_body {
        return Err(PayloadError::TooLarge.into());
    }
    let mut r = Reader { remaining: body };
    let result = if r.byte()? == 0 {
        None
    } else {
        let r = &mut r;
        Some(match kind {
            Kind::OpenGuild => Message::OpenGuild(Wire::read(r)?),
            Kind::GuildResult => Message::GuildResult(Wire::read(r)?),
            Kind::GuildInfo => Message::GuildInfo(Wire::read(r)?),
            Kind::GuildMemberList => Message::GuildMemberList(Wire::read(r)?),
            Kind::QueryGuildList => Message::QueryGuildList(Wire::read(r)?),
            Kind::GuildList => Message::GuildList(Wire::read(r)?),
            Kind::JoinGuild => Message::JoinGuild(Wire::read(r)?),
            Kind::LeaveGuild => Message::LeaveGuild,
            Kind::CloseGuild => Message::CloseGuild,
            Kind::ReconnectGuild => Message::ReconnectGuild,
            Kind::GuildOperation => Message::GuildOperation(Wire::read(r)?),
            Kind::ChangeMaster => Message::ChangeMaster(Wire::read(r)?),
            Kind::InviteGuild => Message::InviteGuild(Wire::read(r)?),
            Kind::GuildNoticeEdit => Message::GuildNoticeEdit(Wire::read(r)?),
            Kind::GuildStoryRequest => Message::GuildStoryRequest,
            Kind::GuildChatMemberInfo => Message::GuildChatMemberInfo(Wire::read(r)?),
            Kind::GuildChat => Message::GuildChat(Wire::read(r)?),
            Kind::JoinGuildChatRoom => Message::JoinGuildChatRoom(Wire::read(r)?),
        })
    };
    if strict && !r.remaining.is_empty() {
        return Err(PayloadError::TrailingBytes.into());
    }
    Ok(result)
}

/// The generic null envelope of the Framework serializer (an unregistered null object).
const NULL_OBJECT: Uuid = Uuid::from_u128(0x81c5fe01_027c_3e1c_98d5_da9c9862aa21);

/// Registered category, the unregistered GUID form, or the generic null envelope;
/// trailing bytes are ignored as SerializeReader.FromBinary does.
pub fn decode_ingress(
    kind: Kind,
    category: i32,
    body: &[u8],
    max_body: usize,
) -> Result<Option<Message>, GuildWireError> {
    if category == kind.category() {
        return decode_body(kind, body, max_body, false);
    }
    if category == 0 && body.len() <= max_body && body.get(..16) == Some(kind.type_id().as_bytes())
    {
        return decode_body(kind, &body[16..], max_body.saturating_sub(16), false);
    }
    if category == 0
        && body.len() <= max_body
        && body.get(..16) == Some(NULL_OBJECT.as_bytes())
        && body.get(16..) == Some(&[0])
    {
        return Ok(None);
    }
    Err(GuildWireError::Type)
}
