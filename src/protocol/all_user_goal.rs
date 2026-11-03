//! Collective goal messages recovered from the Framework serializer: AllUserGoalEvent (52,
//! IDictionary<int, int> of GoalID → count, -1 for a goal that has not started),
//! AllUserGoalEventModify (53, GoalID and Count) and AllUserGoalReward (54, GoalID). The
//! server only sends them.
use super::{
    inventory::{Dict, Wire},
    wire::{PayloadError, Reader, Writer},
};
use thiserror::Error;
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Event,
    EventModify,
    Reward,
}
impl Kind {
    pub const fn category(self) -> i32 {
        match self {
            Self::Event => 52,
            Self::EventModify => 53,
            Self::Reward => 54,
        }
    }
    pub const fn type_id(self) -> Uuid {
        Uuid::from_u128(match self {
            Self::Event => 0x8531c095_3d0a_3e5f_8e05_d60f0c5ade95,
            Self::EventModify => 0xad82781d_ee67_38c2_9674_626448d02d94,
            Self::Reward => 0x22836c87_a7f6_333b_aa9c_dd0987da4bb6,
        })
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AllUserGoalWireError {
    #[error(transparent)]
    Payload(#[from] PayloadError),
    #[error("message does not match its kind")]
    Kind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    /// AllUserGoalEventMessage.AllUserGoalInfo.
    Event(Dict<i32>),
    /// AllUserGoalEventModifyMessage: GoalID, Count.
    EventModify { goal_id: i32, count: i32 },
    /// AllUserGoalRewardMessage.GoalID.
    Reward(i32),
}
impl Message {
    pub const fn kind(&self) -> Kind {
        match self {
            Self::Event(_) => Kind::Event,
            Self::EventModify { .. } => Kind::EventModify,
            Self::Reward(_) => Kind::Reward,
        }
    }
}

pub fn encode(
    kind: Kind,
    message: Option<&Message>,
    max_body: usize,
) -> Result<Zeroizing<Vec<u8>>, AllUserGoalWireError> {
    let mut w = Writer {
        bytes: Vec::new(),
        limit: max_body,
    };
    let result = (|| -> Result<(), AllUserGoalWireError> {
        let Some(m) = message else {
            w.append(&[0])?;
            return Ok(());
        };
        if kind != m.kind() {
            return Err(AllUserGoalWireError::Kind);
        }
        w.append(&[1])?;
        match m {
            Message::Event(d) => d.write(&mut w)?,
            Message::EventModify { goal_id, count } => {
                goal_id.write(&mut w)?;
                count.write(&mut w)?;
            }
            Message::Reward(goal_id) => goal_id.write(&mut w)?,
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

/// Strict decode (encoder round trips and fixtures).
pub fn decode(
    kind: Kind,
    body: &[u8],
    max_body: usize,
) -> Result<Option<Message>, AllUserGoalWireError> {
    if body.len() > max_body {
        return Err(PayloadError::TooLarge.into());
    }
    let mut r = Reader { remaining: body };
    let result = if r.byte()? == 0 {
        None
    } else {
        let r = &mut r;
        Some(match kind {
            Kind::Event => Message::Event(Wire::read(r)?),
            Kind::EventModify => Message::EventModify {
                goal_id: Wire::read(r)?,
                count: Wire::read(r)?,
            },
            Kind::Reward => Message::Reward(Wire::read(r)?),
        })
    };
    if !r.remaining.is_empty() {
        return Err(PayloadError::TrailingBytes.into());
    }
    Ok(result)
}
