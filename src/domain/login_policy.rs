//! Recovered login policies with explicit civil clock samples. No authentication or clock inference.
use crate::{domain::legacy_user_data::*, error::Error};
use std::net::IpAddr;

/// Two DateTime.Now calls in CheckLastLoginIP may observe different values.
#[derive(Clone, Copy)]
pub struct ApplicationClockSamples {
    pub set_lock_now: SqlDateTime2,
    pub check_lock_now: SqlDateTime2,
}
pub struct IpRequest {
    pub connection_ip: IpAddr,
    pub secured: bool,
    pub lock_minutes_feature: i32,
    pub has_cheat_permission: bool,
    pub clock: ApplicationClockSamples,
}
#[derive(Default, PartialEq, Eq)]
pub struct IpState {
    pub last_login_ip: Option<LegacyBytes<50, true>>,
    pub lock_until: Option<SqlDateTime2>,
}
pub struct IpEvaluation {
    pub state: IpState,
    pub previous_ip: Option<LegacyBytes<50, true>>,
    /// Raw CheckLastLoginIP result; empty prior IP suppresses this later in UserLoginProcessor.
    pub lock_seconds: i32,
    pub client_lock_seconds: i32,
}
fn legacy_ip_text(ip: IpAddr) -> String {
    if let IpAddr::V6(v6) = ip {
        let segments = v6.segments();
        if segments[..6] == [0; 6] && segments[6] != 0 {
            // Framework formats IPv4-compatible IPv6 as dotted decimal; std formats hex.
            let bytes = v6.octets();
            return format!(
                "::{}",
                std::net::Ipv4Addr::new(bytes[12], bytes[13], bytes[14], bytes[15])
            );
        }
    }
    ip.to_string()
}
fn ip_bytes(ip: IpAddr) -> Result<LegacyBytes<50, true>, Error> {
    Ok(LegacyBytes::from_bytes(
        legacy_ip_text(ip)
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect(),
    )?)
}
pub fn check_last_login_ip(mut state: IpState, request: &IpRequest) -> Result<IpEvaluation, Error> {
    let previous_ip = state
        .last_login_ip
        .as_ref()
        .map(|b| LegacyBytes::from_bytes(b.as_bytes().to_vec()))
        .transpose()?;
    let current = ip_bytes(request.connection_ip)?;
    if state
        .last_login_ip
        .as_ref()
        .is_some_and(|old| old != &current)
    {
        let minutes = if request.has_cheat_permission {
            1
        } else if request.lock_minutes_feature <= 0 {
            10
        } else {
            request.lock_minutes_feature
        };
        let ticks = request
            .clock
            .set_lock_now
            .ticks()
            .checked_add(i64::from(minutes) * 600_000_000)
            .ok_or(Error::LegacyClockOverflow)?;
        state.lock_until =
            Some(SqlDateTime2::from_ticks(ticks).map_err(|_| Error::LegacyClockOverflow)?);
    }
    let mut lock_seconds = 0;
    if let Some(until) = state.lock_until {
        if request.secured {
            state.lock_until = None;
            state.last_login_ip = Some(ip_bytes(request.connection_ip)?);
        } else {
            // Preserve the recovered Framework TimeSpan.TotalSeconds double + unchecked int conversion.
            // Dates themselves remain exact integers. Both supported CLR profiles agree on overflow.
            let seconds =
                ((until.ticks() - request.clock.check_lock_now.ticks()) as f64 * 1e-7).trunc();
            let converted = if seconds >= f64::from(i32::MIN) && seconds <= f64::from(i32::MAX) {
                seconds as i32
            } else {
                i32::MIN
            };
            if converted > 0 {
                lock_seconds = converted;
            }
        }
    }
    if state.last_login_ip.is_none() {
        state.last_login_ip = Some(current);
    }
    let client_lock_seconds = if previous_ip
        .as_ref()
        .is_some_and(|ip| ip.as_bytes().is_empty())
    {
        0
    } else {
        lock_seconds
    };
    Ok(IpEvaluation {
        state,
        previous_ip,
        lock_seconds,
        client_lock_seconds,
    })
}
/// Recovered UpdateLastLoginIP only clears an existing lock at/after expiry.
pub fn update_expired_ip_lock(
    mut state: IpState,
    ip: IpAddr,
    now: SqlDateTime2,
) -> Result<IpState, Error> {
    if state
        .lock_until
        .is_some_and(|until| until.ticks() <= now.ticks())
    {
        state.lock_until = None;
        state.last_login_ip = Some(ip_bytes(ip)?);
    }
    Ok(state)
}

/// Source UserBan.ID is nvarchar(50); the procedure's varchar(50) conversion is proven only for ASCII.
#[derive(PartialEq, Eq)]
pub struct UserBan {
    pub name: String,
    pub status: i32,
    pub expire_time: Option<SqlDateTime>,
    pub reason: Option<LegacyBytes<256, true>>,
    pub source_rowversion: Option<SourceRowVersion>,
}
impl UserBan {
    pub fn name_key(&self) -> Result<String, Error> {
        if self.name.len() > 50 || self.name.bytes().any(|b| !(32..=126).contains(&b)) {
            return Err(Error::LegacyBanData);
        }
        Ok(self.name.trim_end_matches(' ').to_ascii_lowercase())
    }
    pub fn selected_at(&self, sql_local_now: SqlDateTime) -> bool {
        self.status != 0
            && self
                .expire_time
                .is_none_or(|t| t.units300() > sql_local_now.units300())
    }
}
pub enum BanReason<'a> {
    Empty,
    AccountStolen,
    Source(&'a [u8]),
}
pub enum MemberState<'a> {
    Unknown,
    Clear,
    Banned(BanReason<'a>),
}
pub fn member_state(source: Result<Option<&UserBan>, ()>, notify_reason: bool) -> MemberState<'_> {
    match source {
        Err(()) => MemberState::Unknown,
        Ok(None) => MemberState::Clear,
        Ok(Some(ban)) if ban.status <= 0 => MemberState::Clear,
        Ok(Some(ban)) => MemberState::Banned(if ban.status == 1 {
            BanReason::AccountStolen
        } else if notify_reason {
            ban.reason
                .as_ref()
                .map_or(BanReason::Empty, |r| BanReason::Source(r.as_bytes()))
        } else {
            BanReason::Empty
        }),
    }
}

/// Explicit fixed civil-time profile. The matching packaged VM is Singapore UTC+8,
/// with zero DST transitions. Restored Linux SQL Server UTC is a different profile.
#[derive(Clone, Copy)]
pub struct FixedLocalClock {
    offset_minutes: i16,
}
impl FixedLocalClock {
    pub fn from_offset_minutes(offset_minutes: i16) -> Result<Self, Error> {
        if !(-840..=840).contains(&offset_minutes) {
            return Err(Error::LegacyClockProfile);
        }
        Ok(Self { offset_minutes })
    }
    pub const fn reference_vm() -> Self {
        Self {
            offset_minutes: 480,
        }
    }
    /// Caller supplies a UTC instant represented in .NET epoch ticks; no host timezone inference.
    pub fn application_local(self, utc: SqlDateTime2) -> Result<SqlDateTime2, Error> {
        let ticks = utc
            .ticks()
            .checked_add(i64::from(self.offset_minutes) * 600_000_000)
            .ok_or(Error::LegacyClockOverflow)?;
        SqlDateTime2::from_ticks(ticks).map_err(|_| Error::LegacyClockOverflow)
    }
    /// Source compatibility100 datetime conversion: round to milliseconds, then units300.
    pub fn sql_local(self, utc: SqlDateTime2) -> Result<SqlDateTime, Error> {
        const SQL_EPOCH_TICKS: i64 = 599_266_080_000_000_000;
        let local = self.application_local(utc)?;
        let milliseconds = (i128::from(local.ticks()) + 5_000).div_euclid(10_000);
        let delta = milliseconds * 10_000 - i128::from(SQL_EPOCH_TICKS);
        let units = (delta * 300 + 5_000_000).div_euclid(10_000_000);
        let units = i64::try_from(units).map_err(|_| Error::LegacyClockOverflow)?;
        SqlDateTime::from_units300(units).map_err(|_| Error::LegacyClockOverflow)
    }
}
