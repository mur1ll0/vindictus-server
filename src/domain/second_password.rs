//! Recovered second-password decisions. This is distinct from primary authentication.
//! StringEncrypter version zero uses an explicit nonrandomized Framework identity profile.
use super::legacy_user_data::{LegacyUserDataError, SqlDateTime2};
use crate::protocol::identity::{IdentityProfile, framework_string_hash};
use sha2::{Digest, Sha512};
use thiserror::Error;
use zeroize::Zeroizing;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HashProfile {
    Legacy(IdentityProfile),
    Sha512,
    AccountSha512,
}
impl TryFrom<i32> for HashProfile {
    type Error = SecondPasswordError;
    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Sha512),
            2 => Ok(Self::AccountSha512),
            _ => Err(SecondPasswordError::HashProfile),
        }
    }
}
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SecondPasswordError {
    #[error("second password requires a recovered supported hash profile")]
    HashProfile,
    #[error("second password input exceeds the bounded compatibility contract")]
    Input,
    #[error("second password counter is invalid or would overflow; mutation refused")]
    Counter,
    #[error("second password retry interval exceeds the supported clock range")]
    Clock,
    #[error("second password retry is currently restricted")]
    Locked,
    #[error("second password is not initialized; query is required before checking")]
    Uninitialized,
    #[error(transparent)]
    Date(#[from] LegacyUserDataError),
}
#[derive(Clone, Copy)]
pub struct ClockSamples {
    pub operation_now: SqlDateTime2,
    pub retry_compare_now: SqlDateTime2,
    pub retry_remaining_now: SqlDateTime2,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueryResult {
    /// Captured entity restriction, retained even when first-query initialization clears SQL fields.
    pub restricted_until: Option<SqlDateTime2>,
    pub first: bool,
    pub has_password: bool,
    pub fail_count: i32,
    pub retry_seconds: i32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CheckResult {
    pub passed: bool,
    pub fail_count: i32,
    pub retry_seconds: i32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientState {
    Unknown,
    NoPassword,
    Locked,
    Passed,
}
impl ClientState {
    pub fn passed(self, enabled: bool) -> bool {
        !enabled || matches!(self, Self::NoPassword | Self::Passed)
    }
    pub fn essential_passed(self, enabled: bool, essential: bool) -> bool {
        !(enabled && essential) || self == Self::Passed
    }
}
/// Source uses UTF16 string Length, not the encoded byte count or a prefix parser.
pub fn stored_version(units: usize) -> i32 {
    if units < 128 {
        0
    } else if units == 128 {
        1
    } else {
        2
    }
}
/// Source ASCIIEncoding fallback operates over UTF16 units, including unpaired surrogates.
/// Passwords and hashes are never Debug/Serialize'd and temporary buffers are zeroized.
pub fn hash(
    name: &str,
    password: &[u16],
    profile: HashProfile,
) -> Result<Zeroizing<Vec<u8>>, SecondPasswordError> {
    if !name.is_ascii() || name.len() > 24 || password.len() > 256 {
        return Err(SecondPasswordError::Input);
    }
    if let HashProfile::Legacy(identity) = profile {
        let key = Zeroizing::new(name.to_ascii_lowercase().encode_utf16().collect::<Vec<_>>());
        let mut random = FrameworkRandom128::from_key(&key, identity);
        let mut output = Zeroizing::new(Vec::with_capacity(password.len() * 4));
        const HEX: &[u8; 16] = b"0123456789ABCDEF";
        for &unit in password {
            let n = (i32::from(unit) + random.next()) % 128;
            output.extend([HEX[(n / 16) as usize], 0, HEX[(n % 16) as usize], 0]);
        }
        return Ok(output);
    }
    let mut input = Zeroizing::new(Vec::with_capacity(name.len() + password.len()));
    if profile == HashProfile::AccountSha512 {
        input.extend(name.bytes().map(|b| b.to_ascii_lowercase()));
    }
    input.extend(
        password
            .iter()
            .map(|&c| if c <= 127 { c as u8 } else { b'?' }),
    );
    let digest = Zeroizing::new(Sha512::digest(&*input).to_vec());
    let mut output = Zeroizing::new(Vec::with_capacity(260));
    if profile == HashProfile::AccountSha512 {
        output.extend([b'2', 0, b':', 0]);
    }
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for &b in digest.iter() {
        output.extend([HEX[(b >> 4) as usize], 0, HEX[(b & 15) as usize], 0]);
    }
    Ok(output)
}
/// Constant work over the computed hash; length/version is already part of the source format.
pub fn matches(
    name: &str,
    password: &[u16],
    stored: &[u8],
    identity: IdentityProfile,
) -> Result<bool, SecondPasswordError> {
    if stored.len() > 512 || !stored.len().is_multiple_of(2) {
        return Err(SecondPasswordError::Input);
    }
    if stored_version(stored.len() / 2) == 0 {
        if !name.is_ascii()
            || name.len() > 24
            || password.len() > 256
            || !(stored.len() / 2).is_multiple_of(2)
        {
            return Err(SecondPasswordError::Input);
        }
        let mut encoded = Zeroizing::new(Vec::with_capacity(stored.len() / 2));
        for pair in stored.as_chunks::<2>().0 {
            if pair[1] != 0 || !matches!(pair[0], b'0'..=b'9'|b'A'..=b'F') {
                return Err(SecondPasswordError::Input);
            }
            encoded.push(if pair[0] < b'A' {
                pair[0] - b'0'
            } else {
                pair[0] - b'A' + 10
            });
        }
        let variants = [
            Zeroizing::new(name.to_owned()),
            Zeroizing::new(name.to_ascii_lowercase()),
            Zeroizing::new(name.to_ascii_uppercase()),
        ];
        let mut any = false;
        for key in variants {
            let key = Zeroizing::new(key.encode_utf16().collect::<Vec<_>>());
            let mut random = FrameworkRandom128::from_key(&key, identity);
            let mut difference = (encoded.len() / 2) ^ password.len();
            for (n, hex) in encoded.as_chunks::<2>().0.iter().enumerate() {
                let unit = (i32::from(hex[0]) * 16 + i32::from(hex[1]) + 128 - random.next()) % 128;
                difference |=
                    usize::from((unit as u16) ^ password.get(n).copied().unwrap_or_default());
            }
            any |= difference == 0;
        }
        return Ok(any);
    }
    let profile = HashProfile::try_from(stored_version(stored.len() / 2))?;
    let expected = hash(name, password, profile)?;
    let mut difference = stored.len() ^ expected.len();
    for (i, &b) in expected.iter().enumerate() {
        difference |= usize::from(b ^ stored.get(i).copied().unwrap_or_default());
    }
    Ok(difference == 0)
}
pub fn retry_until(
    count: i32,
    last: SqlDateTime2,
) -> Result<Option<SqlDateTime2>, SecondPasswordError> {
    if (0..=4).contains(&count) {
        return Ok(None);
    }
    let ticks = last
        .ticks()
        .checked_add(6_000_000_000)
        .ok_or(SecondPasswordError::Clock)?;
    Ok(Some(SqlDateTime2::from_ticks(ticks)?))
}
pub fn retry_seconds(
    until: Option<SqlDateTime2>,
    clock: ClockSamples,
) -> Result<i32, SecondPasswordError> {
    let Some(until) = until.filter(|v| v.ticks() > clock.retry_compare_now.ticks()) else {
        return Ok(-1);
    };
    // The source separately samples UtcNow for comparison and remaining TimeSpan.TotalSeconds.
    let seconds = (until.ticks() - clock.retry_remaining_now.ticks()) / 10_000_000;
    i32::try_from(seconds).map_err(|_| SecondPasswordError::Clock)
}
pub fn query(
    stored: Option<&[u8]>,
    count: Option<i32>,
    last: Option<SqlDateTime2>,
    clock: ClockSamples,
) -> Result<QueryResult, SecondPasswordError> {
    if stored.is_some_and(|b| b.len() > 512 || !b.len().is_multiple_of(2)) {
        return Err(SecondPasswordError::Input);
    }
    let fail_count = count.unwrap_or_default();
    let last = last.unwrap_or(SqlDateTime2::from_ticks(552_877_920_000_000_000)?); // SqlDateTime.MinValue = 1753-01-01.
    let restricted_until = retry_until(fail_count, last)?;
    Ok(QueryResult {
        restricted_until,
        first: stored.is_none(),
        has_password: stored.is_some_and(|b| !b.is_empty()),
        fail_count,
        retry_seconds: retry_seconds(restricted_until, clock)?,
    })
}
pub fn checked_count(count: Option<i32>, matched: bool) -> Result<i32, SecondPasswordError> {
    let count = count.unwrap_or_default();
    if count < 0 {
        return Err(SecondPasswordError::Counter);
    } // Never turn source -1+1 into a false success.
    if matched {
        return Ok(0);
    }
    let count = if count == 5 { 0 } else { count };
    count.checked_add(1).ok_or(SecondPasswordError::Counter)
}

// System.Random constructor/InternalSample/Next(128), recovered from Framework mscorlib.
// Seed is the source's signed remainder of wrapping(hash+126991), bounded to +/-1048575.
struct FrameworkRandom128 {
    seed: Zeroizing<[i32; 56]>,
    next: usize,
    nextp: usize,
}
impl FrameworkRandom128 {
    fn from_key(key: &[u16], profile: IdentityProfile) -> Self {
        let value = framework_string_hash(key, profile.architecture).wrapping_add(126991) % 1048576;
        let mut seed = Zeroizing::new([0; 56]);
        let mut mj = 161803398 - value.abs();
        seed[55] = mj;
        let mut mk = 1;
        for i in 1..55 {
            let ii = 21 * i % 55;
            seed[ii] = mk;
            mk = mj - mk;
            if mk < 0 {
                mk += i32::MAX
            };
            mj = seed[ii];
        }
        for _ in 1..5 {
            for k in 1..56 {
                seed[k] -= seed[1 + (k + 30) % 55];
                if seed[k] < 0 {
                    seed[k] += i32::MAX
                };
            }
        }
        Self {
            seed,
            next: 0,
            nextp: 21,
        }
    }
    fn next(&mut self) -> i32 {
        self.next += 1;
        if self.next >= 56 {
            self.next = 1
        };
        self.nextp += 1;
        if self.nextp >= 56 {
            self.nextp = 1
        };
        let mut sample = self.seed[self.next] - self.seed[self.nextp];
        if sample == i32::MAX {
            sample -= 1
        };
        if sample < 0 {
            sample += i32::MAX
        };
        self.seed[self.next] = sample;
        ((f64::from(sample) * 4.656612875245797E-10) * 128.0) as i32
    }
}
