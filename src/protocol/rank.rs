//! Ranking messages recovered from the Framework serializer (ServiceCore.EndPointNetwork with
//! the ServiceCore.RankServiceOperations RankResultInfo, RandomRankResultInfo and
//! RankAlarmInfo): the client's QueryRank* (147, 333, 382–388), RequestSetRankFavoriteInfo
//! (396) and RequestSetRankGoalInfo (397), and the server's RandomRankInfo (148),
//! RankAlarmInfo (334) and Rank*Info (389–395).
use super::{
    inventory::{Wire, object},
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
            pub const fn category(self) -> i32 { match self { $(Self::$name => $cat),* } }
            pub const fn type_id(self) -> Uuid { Uuid::from_u128(match self { $(Self::$name => $guid),* }) }
        }
    };
}
kinds! {
    QueryRandomRank = 147, 0x051924be_e16a_3172_9a8b_0e65b13edb6f;
    RandomRank = 148, 0x7411ad56_8be6_3a81_bdd3_a69f242c3fe2;
    QueryRankAlarm = 333, 0x81ccd497_b4c1_35d7_b08c_77ddf2073089;
    RankAlarm = 334, 0xb1f6d4f1_e607_30fa_915f_1e5e1c14ed0c;
    QueryRankAll = 382, 0x76e547c7_7cf9_3523_96be_0d5367e969aa;
    QueryRankCharacter = 383, 0xd2180bc7_06fd_38e3_95e9_7801bcb01860;
    QueryRankFavorites = 384, 0x22ab80f4_a54c_3a02_8a37_daad929429a9;
    QueryRankGoal = 385, 0x05c1e7bd_6448_31a6_9798_2d9c2cbec28e;
    QueryRankHeavyFavorites = 386, 0xecdf4e69_d15e_37a0_a873_534ca32e79dd;
    QueryRank = 387, 0x9a9b066c_03e8_3ebf_a2ab_e171a2b6665d;
    QueryRankOther = 388, 0x46da8595_f780_3157_86e8_ed68f230bd60;
    RankCharacter = 389, 0x069fe707_679c_3fad_a66f_f2cae935c923;
    RankFavorites = 390, 0xbb38aa48_cb2a_3db0_a98a_862a571abda7;
    RankGoal = 391, 0x5652a397_6ea3_3b22_9c37_7c6969a8902f;
    RankHeavyFavorites = 392, 0xff2edfd7_44a3_356d_9f05_6dcfbd9447a1;
    RankAll = 393, 0x61439335_cfa2_35b9_9d41_d26c4778dd90;
    RankInfo = 394, 0x7d4ee0f1_991f_3ca7_ae3d_2fd9ec4be411;
    RankOther = 395, 0x70712a6c_a1c3_330a_bb24_c8bb9d0b6830;
    SetRankFavorite = 396, 0x8824e598_669b_3370_87a4_1407030f5525;
    SetRankGoal = 397, 0xaf164256_d1f0_311f_9894_896845db1f36;
}
impl Kind {
    /// Messages the client sends (InGame).
    pub const INGRESS: [Kind; 11] = [
        Kind::QueryRandomRank,
        Kind::QueryRankAlarm,
        Kind::QueryRankAll,
        Kind::QueryRankCharacter,
        Kind::QueryRankFavorites,
        Kind::QueryRankGoal,
        Kind::QueryRankHeavyFavorites,
        Kind::QueryRank,
        Kind::QueryRankOther,
        Kind::SetRankFavorite,
        Kind::SetRankGoal,
    ];
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RankWireError {
    #[error(transparent)]
    Payload(#[from] PayloadError),
    #[error("message does not match its kind")]
    Kind,
    #[error("unsupported rank category/type")]
    Type,
}

/// RankResultInfo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RankResult {
    pub event_id: i32,
    pub rank: i32,
    pub rank_prev: i32,
    pub score: i64,
    pub character_name: Option<String>,
}
object!(RankResult {
    event_id,
    rank,
    rank_prev,
    score,
    character_name
});

/// IList<RankResultInfo> (nullable list of nullable objects).
pub type Results = Option<Vec<Option<RankResult>>>;

/// RandomRankResultInfo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RandomRank {
    pub random_rank_result: Results,
    pub event_id: i32,
    pub period_type: i32,
}
object!(RandomRank {
    random_rank_result,
    event_id,
    period_type
});

/// RankAlarmInfo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Alarm {
    pub period_type: i32,
    pub event_id: i32,
    pub rank: i32,
}
object!(Alarm {
    period_type,
    event_id,
    rank
});

/// RankCharacter/Favorites/All/Info: the results and the period type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeriodResults {
    pub rank_result: Results,
    pub period_type: i32,
}
object!(PeriodResults {
    rank_result,
    period_type
});

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Goal {
    pub event_id: i32,
    pub rival_name: Option<String>,
    pub rival_score: i64,
    pub rank: i32,
    pub rank_prev: i32,
}
object!(Goal {
    event_id,
    rival_name,
    rival_score,
    rank,
    rank_prev
});

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Other {
    pub level: i32,
    pub base_character: i32,
    pub character_name: Option<String>,
    pub rank_result: Results,
    pub requester_rank_result: Results,
    pub period_type: i32,
}
object!(Other {
    level,
    base_character,
    character_name,
    rank_result,
    requester_rank_result,
    period_type
});

/// QueryRankCharacter/Favorites: CID and period type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CidPeriod {
    pub cid: i64,
    pub period_type: i32,
}
object!(CidPeriod { cid, period_type });

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueryGoal {
    pub cid: i64,
    pub event_id: i32,
    pub period_type: i32,
}
object!(QueryGoal {
    cid,
    event_id,
    period_type
});

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueryRank {
    pub rank_id: Option<String>,
    pub period_type: i32,
}
object!(QueryRank {
    rank_id,
    period_type
});

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueryOther {
    pub requester_cid: i64,
    pub character_name: Option<String>,
    pub period_type: i32,
}
object!(QueryOther {
    requester_cid,
    character_name,
    period_type
});

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SetFavorite {
    pub cid: i64,
    pub rank_id: Option<String>,
    pub is_addition: bool,
}
object!(SetFavorite {
    cid,
    rank_id,
    is_addition
});

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SetGoal {
    pub cid: i64,
    pub rank_id: Option<String>,
}
object!(SetGoal { cid, rank_id });

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    QueryRandomRank,
    RandomRank(Option<Vec<Option<RandomRank>>>),
    /// QueryRankAlarmInfoMessage.CID.
    QueryRankAlarm(i64),
    RankAlarm(Option<Vec<Option<Alarm>>>),
    /// QueryRankAllInfoMessage.PeriodType.
    QueryRankAll(i32),
    QueryRankCharacter(CidPeriod),
    QueryRankFavorites(CidPeriod),
    QueryRankGoal(QueryGoal),
    QueryRankHeavyFavorites,
    QueryRank(QueryRank),
    QueryRankOther(QueryOther),
    RankCharacter(PeriodResults),
    RankFavorites(PeriodResults),
    RankGoal(Goal),
    RankHeavyFavorites(Results),
    RankAll(PeriodResults),
    RankInfo(PeriodResults),
    RankOther(Other),
    SetRankFavorite(SetFavorite),
    SetRankGoal(SetGoal),
}
impl Message {
    pub const fn kind(&self) -> Kind {
        match self {
            Self::QueryRandomRank => Kind::QueryRandomRank,
            Self::RandomRank(_) => Kind::RandomRank,
            Self::QueryRankAlarm(_) => Kind::QueryRankAlarm,
            Self::RankAlarm(_) => Kind::RankAlarm,
            Self::QueryRankAll(_) => Kind::QueryRankAll,
            Self::QueryRankCharacter(_) => Kind::QueryRankCharacter,
            Self::QueryRankFavorites(_) => Kind::QueryRankFavorites,
            Self::QueryRankGoal(_) => Kind::QueryRankGoal,
            Self::QueryRankHeavyFavorites => Kind::QueryRankHeavyFavorites,
            Self::QueryRank(_) => Kind::QueryRank,
            Self::QueryRankOther(_) => Kind::QueryRankOther,
            Self::RankCharacter(_) => Kind::RankCharacter,
            Self::RankFavorites(_) => Kind::RankFavorites,
            Self::RankGoal(_) => Kind::RankGoal,
            Self::RankHeavyFavorites(_) => Kind::RankHeavyFavorites,
            Self::RankAll(_) => Kind::RankAll,
            Self::RankInfo(_) => Kind::RankInfo,
            Self::RankOther(_) => Kind::RankOther,
            Self::SetRankFavorite(_) => Kind::SetRankFavorite,
            Self::SetRankGoal(_) => Kind::SetRankGoal,
        }
    }
}

pub fn encode(
    kind: Kind,
    message: Option<&Message>,
    max_body: usize,
) -> Result<Zeroizing<Vec<u8>>, RankWireError> {
    let mut w = Writer {
        bytes: Vec::new(),
        limit: max_body,
    };
    let result = (|| -> Result<(), RankWireError> {
        let Some(m) = message else {
            w.append(&[0])?;
            return Ok(());
        };
        if kind != m.kind() {
            return Err(RankWireError::Kind);
        }
        w.append(&[1])?;
        match m {
            Message::QueryRandomRank | Message::QueryRankHeavyFavorites => {}
            Message::RandomRank(v) => v.write(&mut w)?,
            Message::QueryRankAlarm(v) => v.write(&mut w)?,
            Message::RankAlarm(v) => v.write(&mut w)?,
            Message::QueryRankAll(v) => v.write(&mut w)?,
            Message::QueryRankCharacter(v) | Message::QueryRankFavorites(v) => v.write(&mut w)?,
            Message::QueryRankGoal(v) => v.write(&mut w)?,
            Message::QueryRank(v) => v.write(&mut w)?,
            Message::QueryRankOther(v) => v.write(&mut w)?,
            Message::RankCharacter(v)
            | Message::RankFavorites(v)
            | Message::RankAll(v)
            | Message::RankInfo(v) => v.write(&mut w)?,
            Message::RankGoal(v) => v.write(&mut w)?,
            Message::RankHeavyFavorites(v) => v.write(&mut w)?,
            Message::RankOther(v) => v.write(&mut w)?,
            Message::SetRankFavorite(v) => v.write(&mut w)?,
            Message::SetRankGoal(v) => v.write(&mut w)?,
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
pub fn decode(kind: Kind, body: &[u8], max_body: usize) -> Result<Option<Message>, RankWireError> {
    decode_body(kind, body, max_body, true)
}

fn decode_body(
    kind: Kind,
    body: &[u8],
    max_body: usize,
    strict: bool,
) -> Result<Option<Message>, RankWireError> {
    if body.len() > max_body {
        return Err(PayloadError::TooLarge.into());
    }
    let mut r = Reader { remaining: body };
    let result = if r.byte()? == 0 {
        None
    } else {
        let r = &mut r;
        Some(match kind {
            Kind::QueryRandomRank => Message::QueryRandomRank,
            Kind::RandomRank => Message::RandomRank(Wire::read(r)?),
            Kind::QueryRankAlarm => Message::QueryRankAlarm(Wire::read(r)?),
            Kind::RankAlarm => Message::RankAlarm(Wire::read(r)?),
            Kind::QueryRankAll => Message::QueryRankAll(Wire::read(r)?),
            Kind::QueryRankCharacter => Message::QueryRankCharacter(Wire::read(r)?),
            Kind::QueryRankFavorites => Message::QueryRankFavorites(Wire::read(r)?),
            Kind::QueryRankGoal => Message::QueryRankGoal(Wire::read(r)?),
            Kind::QueryRankHeavyFavorites => Message::QueryRankHeavyFavorites,
            Kind::QueryRank => Message::QueryRank(Wire::read(r)?),
            Kind::QueryRankOther => Message::QueryRankOther(Wire::read(r)?),
            Kind::RankCharacter => Message::RankCharacter(Wire::read(r)?),
            Kind::RankFavorites => Message::RankFavorites(Wire::read(r)?),
            Kind::RankGoal => Message::RankGoal(Wire::read(r)?),
            Kind::RankHeavyFavorites => Message::RankHeavyFavorites(Wire::read(r)?),
            Kind::RankAll => Message::RankAll(Wire::read(r)?),
            Kind::RankInfo => Message::RankInfo(Wire::read(r)?),
            Kind::RankOther => Message::RankOther(Wire::read(r)?),
            Kind::SetRankFavorite => Message::SetRankFavorite(Wire::read(r)?),
            Kind::SetRankGoal => Message::SetRankGoal(Wire::read(r)?),
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
) -> Result<Option<Message>, RankWireError> {
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
    Err(RankWireError::Type)
}
