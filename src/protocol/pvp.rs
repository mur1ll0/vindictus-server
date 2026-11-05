//! PvP messages recovered from the Framework serializer (ServiceCore.EndPointNetwork and its
//! Pvp namespace): FreeMatchReportMessage (276, the MicroPlay host's free duel report) and the
//! PvpService match messages (B5 PV2-PV4): channel list, register/unregister and its result,
//! confirm join, host and member info, commands, reports, team info, kick, penalty, the Arena
//! result and waiting queue. The waiting queue messages are unregistered (category 0, GUID
//! envelope only).
use super::{
    inventory::{Dict, HeroesString, Wire, object},
    mission::{GameInfo, JoinMember, StrDict},
    wire::{PayloadError, Reader, Writer},
};
use thiserror::Error;
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

macro_rules! kinds {
    ($($name:ident = $cat:literal, $guid:literal;)*) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Kind { $($name),* }
        impl Kind {
            /// The registered category; 0 for an unregistered message.
            pub const fn category(self) -> i32 { match self { $(Self::$name => $cat),* } }
            pub const fn type_id(self) -> Uuid { Uuid::from_u128(match self { $(Self::$name => $guid),* }) }
        }
    };
}
kinds! {
    FreeMatchReport = 276, 0x8673db49_f849_349b_acea_3794efe7ece5;
    QueryChannelList = 130, 0x28e86f3f_74a2_3fc0_a9ae_860806aafb63;
    ChannelList = 129, 0xa6711fc7_3b8b_35c6_a8a5_2a2d222a9b41;
    Register = 442, 0x352de823_b4f4_337d_a212_bdfdef5624ce;
    Unregister = 611, 0x28670757_a5f2_3ad1_9bde_5031f6606d6c;
    ConfirmJoin = 143, 0xdd1dab96_b042_3e0a_b622_d09b812d90c5;
    RegisterResult = 378, 0xc4cee3b5_48bb_3b17_a6b7_0b05e7c4e803;
    Command = 376, 0xb27a17ad_9e01_361b_933e_701ccaec625d;
    Kicked = 377, 0x542086a0_b1ac_3f38_8f87_ea16e3b162c7;
    Report = 379, 0x8e68a8d9_9e06_3199_a19a_9e9cdf506748;
    TeamInfo = 380, 0xf851a062_d98a_314d_a92b_060567a6b060;
    GameHosted = 435, 0xb8b23b05_1468_316e_950f_b00af304ca2c;
    HostInfo = 436, 0x3b54c4d3_e989_3926_b130_9312ae448ff2;
    MemberInfo = 437, 0x52a61a16_45c8_30e2_a833_9c2a79e3d2d1;
    ArenaResultInfo = 647, 0xc36c9bb6_997d_3d08_9a9a_eddab5a6dadf;
    Penalty = 648, 0x6b85dc42_733c_3660_8645_9ed785d1b7e3;
    RegisterGameWaitingQueue = 0, 0x7717e97d_fb9b_3adb_97a2_3be8d0efff52;
    UnregisterGameWaitingQueue = 0, 0xf02655c1_174e_33fd_ad02_563444bf7469;
}
impl Kind {
    /// Messages the client sends (InGame and the PvpServiceProxy states).
    pub const INGRESS: [Kind; 9] = [
        Kind::FreeMatchReport,
        Kind::QueryChannelList,
        Kind::Register,
        Kind::Unregister,
        Kind::ConfirmJoin,
        Kind::Report,
        Kind::Penalty,
        Kind::RegisterGameWaitingQueue,
        Kind::UnregisterGameWaitingQueue,
    ];
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PvpWireError {
    #[error(transparent)]
    Payload(#[from] PayloadError),
    #[error("message does not match its kind")]
    Kind,
    #[error("unsupported PvP category/type")]
    Type,
}

/// FreeMatchReportMessage: WinnerTag, LoserTag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FreeMatchReport {
    pub winner_tag: i32,
    pub loser_tag: i32,
}
object!(FreeMatchReport {
    winner_tag,
    loser_tag
});

/// PvpQueryChannelListMessage: PvpMode, Key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueryChannelList {
    pub pvp_mode: Option<String>,
    pub key: Option<String>,
}
object!(QueryChannelList { pvp_mode, key });

/// PvpChannelInfo: Desc, ChannelID.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChannelInfo {
    pub desc: Option<String>,
    pub channel_id: i32,
}
object!(ChannelInfo { desc, channel_id });

/// PvpChannelListMessage: PvpChannelInfos, PvpMode, Key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChannelList {
    pub channels: Option<Vec<Option<ChannelInfo>>>,
    pub pvp_mode: Option<String>,
    pub key: Option<String>,
}
object!(ChannelList {
    channels,
    pvp_mode,
    key
});

/// PvpRegisterCheat flags.
pub mod cheat {
    pub const IGNORE_RESTRICTION: i32 = 1;
    pub const FORCE_START: i32 = 2;
    pub const TEST: i32 = 4;
    pub const DEBUG: i32 = 8;
}

/// PvpRegisterMessage: PvpMode, Cheat (PvpRegisterCheat), ChannelID.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Register {
    pub pvp_mode: Option<String>,
    pub cheat: i32,
    pub channel_id: i32,
}
object!(Register {
    pvp_mode,
    cheat,
    channel_id
});

/// A message without fields (PvpUnregister, PvpKicked, PvpUnregisterGameWaitingQueue).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Empty {}
impl Wire for Empty {
    fn write(&self, _: &mut Writer) -> Result<(), PayloadError> {
        Ok(())
    }
    fn read(_: &mut Reader<'_>) -> Result<Self, PayloadError> {
        Ok(Self {})
    }
}

/// PvpConfirmJoinMessage: PvpMode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfirmJoin {
    pub pvp_mode: Option<String>,
}
object!(ConfirmJoin { pvp_mode });

/// PvpRegisterResult.
pub mod register_state {
    pub const MAKE: i32 = 0;
    pub const WAIT: i32 = 1;
    pub const JOINABLE: i32 = 2;
    pub const CANCELED: i32 = 3;
}

/// PvpRegisterResultMessage: PvpMode, UsePendingDialog, StartWaitingTimer, State, Message.
#[derive(Clone, PartialEq, Eq)]
pub struct RegisterResult {
    pub pvp_mode: Option<String>,
    pub use_pending_dialog: bool,
    pub start_waiting_timer: bool,
    pub state: i32,
    pub message: Option<HeroesString>,
}
object!(RegisterResult {
    pvp_mode,
    use_pending_dialog,
    start_waiting_timer,
    state,
    message
});
impl std::fmt::Debug for RegisterResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegisterResult")
            .field("pvp_mode", &self.pvp_mode)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

/// PvpCommandType.
pub mod command {
    pub const START_GAME: i32 = 0;
    pub const ABORT_GAME: i32 = 1;
    pub const END_GAME: i32 = 2;
    pub const ALL_PLAYER_READY: i32 = 3;
    pub const START_ROUND: i32 = 4;
    pub const END_ROUND: i32 = 5;
    pub const REGISTERED_PLAYER_QUEUE: i32 = 6;
    pub const REGISTERED_PLAYER_GAME: i32 = 7;
    pub const UNREGISTERED_PLAYER_QUEUE: i32 = 8;
    pub const UNREGISTERED_PLAYER_GAME: i32 = 9;
    pub const STATE_CHANGED: i32 = 10;
    pub const ARENA_STARTING_PLAYER_INFO: i32 = 11;
    pub const ARENA_END_GAME: i32 = 12;
}

/// PvpCommandMessage: CommandInt (PvpCommandType), Arg.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    pub command: i32,
    pub arg: Option<String>,
}
object!(Command { command, arg });

/// PvpReportType.
pub mod report {
    pub const HOSTED: i32 = 0;
    pub const TEAM_CHANGE: i32 = 1;
    pub const REQUEST_START: i32 = 2;
    pub const SCORE: i32 = 3;
    pub const KILL: i32 = 4;
    pub const ABUSING: i32 = 5;
    pub const REVIVE: i32 = 6;
    pub const GOD_KILL: i32 = 7;
    pub const WIN: i32 = 8;
    pub const DRAW: i32 = 9;
}

/// PvpReportMessage: EventInt (PvpReportType), Subject, Object, Arg.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    pub event: i32,
    pub subject: i32,
    pub object: i32,
    pub arg: Option<String>,
}
object!(Report {
    event,
    subject,
    object,
    arg
});

/// PvpTeamInfoMessage: TeamInfo (slot → PvpTeamID).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamInfo {
    pub team_info: Dict<i32>,
}
object!(TeamInfo { team_info });

/// PvpTeamID.
pub mod team {
    pub const INVALID: i32 = -1;
    pub const BLUE: i32 = 2;
    pub const RED: i32 = 3;
}

/// PvpGameInfo: TeamInfo, PlayerInfo (IDictionary<int, string>).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PvpGameInfo {
    pub team_info: Dict<Option<String>>,
    pub player_info: Dict<Option<String>>,
}
object!(PvpGameInfo {
    team_info,
    player_info
});

/// PvpGameHostedMessage: GameInfo, HostInfo, TeamID, Tag, Config.
#[derive(Clone, PartialEq, Eq)]
pub struct GameHosted {
    pub game_info: Option<PvpGameInfo>,
    pub host_info: Option<JoinMember>,
    pub team_id: i32,
    pub tag: i32,
    pub config: StrDict<Option<String>>,
}
object!(GameHosted {
    game_info,
    host_info,
    team_id,
    tag,
    config
});

/// PvpHostInfoMessage: GameInfo, MemberInfo, Config.
#[derive(Clone, PartialEq, Eq)]
pub struct HostInfo {
    pub game_info: Option<GameInfo>,
    pub member_info: Option<JoinMember>,
    pub config: StrDict<Option<String>>,
}
object!(HostInfo {
    game_info,
    member_info,
    config
});

/// PvpMemberInfoMessage: MemberInfo, TeamID.
#[derive(Clone, PartialEq, Eq)]
pub struct MemberInfo {
    pub member_info: Option<JoinMember>,
    pub team_id: i32,
}
object!(MemberInfo {
    member_info,
    team_id
});

macro_rules! opaque_debug {
    ($($t:ident),*) => {$(
        impl std::fmt::Debug for $t {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct(stringify!($t)).finish_non_exhaustive()
            }
        }
    )*};
}
opaque_debug!(GameHosted, HostInfo, MemberInfo);

/// ArenaResultInfoMessage: ArenaWinCount, ArenaLoseCount, ArenaSuccessiveWinCount.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArenaResultInfo {
    pub win: i32,
    pub lose: i32,
    pub successive_win: i32,
}
object!(ArenaResultInfo {
    win,
    lose,
    successive_win
});

/// PvpPenaltyMessage: PlayerIndex.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Penalty {
    pub player_index: i32,
}
object!(Penalty { player_index });

/// PvpRegisterGameWaitingQueueMessage: GameIndex.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegisterGameWaitingQueue {
    pub game_index: i32,
}
object!(RegisterGameWaitingQueue { game_index });

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    FreeMatchReport(FreeMatchReport),
    QueryChannelList(QueryChannelList),
    ChannelList(ChannelList),
    Register(Register),
    Unregister(Empty),
    ConfirmJoin(ConfirmJoin),
    RegisterResult(RegisterResult),
    Command(Command),
    Kicked(Empty),
    Report(Report),
    TeamInfo(TeamInfo),
    GameHosted(GameHosted),
    HostInfo(HostInfo),
    MemberInfo(MemberInfo),
    ArenaResultInfo(ArenaResultInfo),
    Penalty(Penalty),
    RegisterGameWaitingQueue(RegisterGameWaitingQueue),
    UnregisterGameWaitingQueue(Empty),
}
impl Message {
    pub const fn kind(&self) -> Kind {
        match self {
            Self::FreeMatchReport(_) => Kind::FreeMatchReport,
            Self::QueryChannelList(_) => Kind::QueryChannelList,
            Self::ChannelList(_) => Kind::ChannelList,
            Self::Register(_) => Kind::Register,
            Self::Unregister(_) => Kind::Unregister,
            Self::ConfirmJoin(_) => Kind::ConfirmJoin,
            Self::RegisterResult(_) => Kind::RegisterResult,
            Self::Command(_) => Kind::Command,
            Self::Kicked(_) => Kind::Kicked,
            Self::Report(_) => Kind::Report,
            Self::TeamInfo(_) => Kind::TeamInfo,
            Self::GameHosted(_) => Kind::GameHosted,
            Self::HostInfo(_) => Kind::HostInfo,
            Self::MemberInfo(_) => Kind::MemberInfo,
            Self::ArenaResultInfo(_) => Kind::ArenaResultInfo,
            Self::Penalty(_) => Kind::Penalty,
            Self::RegisterGameWaitingQueue(_) => Kind::RegisterGameWaitingQueue,
            Self::UnregisterGameWaitingQueue(_) => Kind::UnregisterGameWaitingQueue,
        }
    }
}

pub fn encode(
    kind: Kind,
    message: Option<&Message>,
    max_body: usize,
) -> Result<Zeroizing<Vec<u8>>, PvpWireError> {
    let mut w = Writer {
        bytes: Vec::new(),
        limit: max_body,
    };
    let result = (|| -> Result<(), PvpWireError> {
        let Some(m) = message else {
            w.append(&[0])?;
            return Ok(());
        };
        if kind != m.kind() {
            return Err(PvpWireError::Kind);
        }
        w.append(&[1])?;
        match m {
            Message::FreeMatchReport(v) => v.write(&mut w)?,
            Message::QueryChannelList(v) => v.write(&mut w)?,
            Message::ChannelList(v) => v.write(&mut w)?,
            Message::Register(v) => v.write(&mut w)?,
            Message::Unregister(v) | Message::Kicked(v) | Message::UnregisterGameWaitingQueue(v) => {
                v.write(&mut w)?
            }
            Message::ConfirmJoin(v) => v.write(&mut w)?,
            Message::RegisterResult(v) => v.write(&mut w)?,
            Message::Command(v) => v.write(&mut w)?,
            Message::Report(v) => v.write(&mut w)?,
            Message::TeamInfo(v) => v.write(&mut w)?,
            Message::GameHosted(v) => v.write(&mut w)?,
            Message::HostInfo(v) => v.write(&mut w)?,
            Message::MemberInfo(v) => v.write(&mut w)?,
            Message::ArenaResultInfo(v) => v.write(&mut w)?,
            Message::Penalty(v) => v.write(&mut w)?,
            Message::RegisterGameWaitingQueue(v) => v.write(&mut w)?,
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

/// The packet (category, body) as sent: the registered category, or category 0 with the
/// type's GUID before the body (the envelope of an unregistered message).
pub fn packet(message: &Message, max_body: usize) -> Result<(i32, Vec<u8>), PvpWireError> {
    let kind = message.kind();
    let body = encode(kind, Some(message), max_body)?;
    if kind.category() != 0 {
        return Ok((kind.category(), body.to_vec()));
    }
    let mut out = Vec::with_capacity(16 + body.len());
    out.extend_from_slice(kind.type_id().as_bytes());
    out.extend_from_slice(&body);
    if out.len() > max_body {
        return Err(PayloadError::TooLarge.into());
    }
    Ok((0, out))
}

/// Strict decode: the whole body must be consumed (encoder round trips and fixtures).
pub fn decode(kind: Kind, body: &[u8], max_body: usize) -> Result<Option<Message>, PvpWireError> {
    decode_body(kind, body, max_body, true)
}

fn decode_body(
    kind: Kind,
    body: &[u8],
    max_body: usize,
    strict: bool,
) -> Result<Option<Message>, PvpWireError> {
    if body.len() > max_body {
        return Err(PayloadError::TooLarge.into());
    }
    let mut r = Reader { remaining: body };
    let result = if r.byte()? == 0 {
        None
    } else {
        let r = &mut r;
        Some(match kind {
            Kind::FreeMatchReport => Message::FreeMatchReport(Wire::read(r)?),
            Kind::QueryChannelList => Message::QueryChannelList(Wire::read(r)?),
            Kind::ChannelList => Message::ChannelList(Wire::read(r)?),
            Kind::Register => Message::Register(Wire::read(r)?),
            Kind::Unregister => Message::Unregister(Wire::read(r)?),
            Kind::ConfirmJoin => Message::ConfirmJoin(Wire::read(r)?),
            Kind::RegisterResult => Message::RegisterResult(Wire::read(r)?),
            Kind::Command => Message::Command(Wire::read(r)?),
            Kind::Kicked => Message::Kicked(Wire::read(r)?),
            Kind::Report => Message::Report(Wire::read(r)?),
            Kind::TeamInfo => Message::TeamInfo(Wire::read(r)?),
            Kind::GameHosted => Message::GameHosted(Wire::read(r)?),
            Kind::HostInfo => Message::HostInfo(Wire::read(r)?),
            Kind::MemberInfo => Message::MemberInfo(Wire::read(r)?),
            Kind::ArenaResultInfo => Message::ArenaResultInfo(Wire::read(r)?),
            Kind::Penalty => Message::Penalty(Wire::read(r)?),
            Kind::RegisterGameWaitingQueue => Message::RegisterGameWaitingQueue(Wire::read(r)?),
            Kind::UnregisterGameWaitingQueue => {
                Message::UnregisterGameWaitingQueue(Wire::read(r)?)
            }
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
) -> Result<Option<Message>, PvpWireError> {
    if kind.category() != 0 && category == kind.category() {
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
    Err(PvpWireError::Type)
}
