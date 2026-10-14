//! Isolated recovered allocators. Callers still own service state and database admission.
//! These decisions do not create a character or grant ownership of an existing CID.
use super::legacy_user_data::{LegacyUserDataError, SqlDateTime2};

const UNIX_EPOCH_TICKS: i64 = 621_355_968_000_000_000;
const TICKS_PER_SECOND: i64 = 10_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EntityAllocation {
    pub entity_time: SqlDateTime2,
    pub delay_ticks: i64,
}

/// Original Service.MakeEntityID mutates its counter before constructing DateTime.
/// Retain that mutation even if DateTime rejects the resulting value.
/// `service_id` is Service.ID, not FeatureMatrix.ServerCode or a user's NexonSN.
pub fn make_entity_id(
    previous: &mut i64,
    service_id: i32,
    utc_now: SqlDateTime2,
) -> Result<EntityAllocation, LegacyUserDataError> {
    let ticks = utc_now.ticks();
    *previous = if *previous < ticks {
        let suffix = i64::from(service_id as u16) + 1;
        (ticks.wrapping_sub(suffix) | 0xFFFF).wrapping_add(suffix)
    } else {
        previous.wrapping_add(65_536)
    };
    let delay_ticks = previous.wrapping_sub(ticks);
    let entity_time = SqlDateTime2::from_ticks(*previous)?;
    Ok(EntityAllocation {
        entity_time,
        delay_ticks,
    })
}

/// Extention.ToInt32 for the UTC samples used by CharacterMaker.
/// Truncation toward zero and unchecked int32 conversion are intentional.
pub fn utc_unix_seconds(utc_now: SqlDateTime2) -> i32 {
    ((utc_now.ticks() - UNIX_EPOCH_TICKS) / TICKS_PER_SECOND) as i32
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CharacterSerial {
    pub serial: i32,
    pub sampled_at: SqlDateTime2,
    pub delay_ticks: i64,
}

/// Original overload uses two independent DateTime.UtcNow samples.
pub fn first_character_serial(
    first_now: SqlDateTime2,
    second_now: SqlDateTime2,
    server_code: i32,
) -> CharacterSerial {
    next_character_serial(
        second_now,
        server_code,
        utc_unix_seconds(first_now).wrapping_sub(1),
    )
}

/// Original CharacterMaker.MakeCharacterSN overload, including unchecked overflow.
/// Character count, zero-SN retry, name/slot checks and SQL collision loop remain caller work.
pub fn next_character_serial(
    utc_now: SqlDateTime2,
    server_code: i32,
    previous: i32,
) -> CharacterSerial {
    let seconds = utc_unix_seconds(utc_now);
    let serial = if seconds.wrapping_sub(previous) <= 0 {
        previous.wrapping_add(256)
    } else if !(0..=255).contains(&server_code) {
        seconds
    } else {
        let suffix = server_code + 1;
        (seconds.wrapping_sub(suffix) | 0xFF).wrapping_add(suffix)
    };
    CharacterSerial {
        serial,
        sampled_at: utc_now,
        delay_ticks: i64::from(serial.wrapping_sub(seconds)) * TICKS_PER_SECOND,
    }
}
