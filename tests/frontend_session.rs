use serde::Deserialize;
use std::{
    net::{IpAddr, Ipv4Addr},
    time::{Duration, Instant},
};
use vindictus_server::application::session_registry::SessionRegistry;
use vindictus_server::{
    application::frontend_session::{FrontendSession, LabAdmission, SessionError, SessionPhase},
    domain::passport::{
        LoginPermissions, LoginRejection, frontend_passport_gate, is_secured_login,
    },
    protocol::identity::{
        AsciiCulture, FrameworkArchitecture, IdentityProfile, LabIdentityOptions,
    },
};
pub fn config() -> LabAdmission {
    LabAdmission {
        identity_options: LabIdentityOptions {
            profile: IdentityProfile {
                architecture: FrameworkArchitecture::X86,
                culture: AsciiCulture::Invariant,
            },
            ngm_disable: true,
            zh_cn: false,
            max_passport_bytes: 128,
        },
        allow_during_maintenance: false,
        allow_cheat: false,
        is_online: true,
    }
}
fn peer() -> IpAddr {
    Ipv4Addr::LOCALHOST.into()
}
#[derive(Deserialize)]
struct Fixture {
    profiles: Vec<Profile>,
}
#[derive(Deserialize)]
struct Profile {
    cases: Vec<Case>,
    secured: Vec<Secured>,
}
#[derive(Deserialize)]
struct Case {
    bits: i32,
    online: bool,
    reason: i32,
    result: i32,
}
#[derive(Deserialize)]
struct Secured {
    code: u8,
    value: bool,
}
fn fixture() -> Fixture {
    serde_json::from_str(include_str!("fixtures/framework-frontend-gate.json")).unwrap()
}
#[test]
fn gates_match_unchanged_recovered_frontend_for_both_architectures() {
    for profile in fixture().profiles {
        assert_eq!(profile.cases.len(), 144);
        for case in profile.cases {
            let reason = match case.reason {
                1 => LoginRejection::PassportFail,
                4 => LoginRejection::Banned,
                13 => LoginRejection::PermissionFailed,
                _ => panic!("unexpected fixture"),
            };
            let result = frontend_passport_gate(
                LoginPermissions::from_recovered_bits(case.bits),
                case.online,
                reason,
            )
            .map_or_else(|e| e.0 as i32, |()| -1);
            assert_eq!(result, case.result);
        }
    }
}
#[test]
fn secured_login_matches_all_recovered_byte_values() {
    for profile in fixture().profiles {
        assert_eq!(profile.secured.len(), 256);
        for case in profile.secured {
            assert_eq!(is_secured_login(case.code), case.value);
        }
    }
}
#[test]
fn callbacks_from_failed_attempt_or_another_connection_cannot_admit_identity() {
    let registry = SessionRegistry::with_capacity(32).unwrap();
    let now = Instant::now();
    let mut session = FrontendSession::new(&registry);
    let old = session.begin(now, Duration::from_secs(10)).unwrap();
    assert_eq!(
        session.reject(old, now, LoginRejection::Banned),
        Err(SessionError::Rejected(LoginRejection::Banned))
    );
    let current = session.begin(now, Duration::from_secs(10)).unwrap();
    assert_eq!(
        session.admit_lab(old, now, Some("synthetic"), peer(), &config()),
        Err(SessionError::StaleCallback)
    );
    let mut other = FrontendSession::new(&registry);
    let foreign = other.begin(now, Duration::from_secs(10)).unwrap();
    assert_eq!(
        session.reject(foreign, now, LoginRejection::Banned),
        Err(SessionError::StaleCallback)
    );
    session
        .admit_lab(current, now, Some("synthetic"), peer(), &config())
        .unwrap();
    assert_eq!(session.phase(), SessionPhase::CheckingPolicies);
}
#[test]
fn scope_and_invalid_passports_reset_without_granting_permissions() {
    let registry = SessionRegistry::with_capacity(32).unwrap();
    let now = Instant::now();
    let mut session = FrontendSession::new(&registry);
    let token = session.begin(now, Duration::from_secs(10)).unwrap();
    assert_eq!(
        session.admit_lab(
            token,
            now,
            Some("synthetic"),
            "192.168.0.5".parse().unwrap(),
            &config()
        ),
        Err(SessionError::LabScope)
    );
    assert_eq!(session.phase(), SessionPhase::Initial);
    for value in [None, Some(""), Some("não-ascii"), Some("a\n")] {
        let token = session.begin(now, Duration::from_secs(10)).unwrap();
        assert!(matches!(
            session.admit_lab(token, now, value, peer(), &config()),
            Err(SessionError::Identity(_))
        ));
        assert_eq!(session.phase(), SessionPhase::Initial);
    }
    let mut disabled = config();
    disabled.identity_options.ngm_disable = false;
    let token = session.begin(now, Duration::from_secs(10)).unwrap();
    assert_eq!(
        session.admit_lab(token, now, Some("synthetic"), peer(), &disabled),
        Err(SessionError::LabScope)
    );
}
#[test]
fn maintenance_bypass_requires_explicit_operator_permission() {
    let registry = SessionRegistry::with_capacity(32).unwrap();
    let now = Instant::now();
    let mut cfg = config();
    cfg.is_online = false;
    cfg.allow_cheat = true;
    let mut session = FrontendSession::new(&registry);
    let token = session.begin(now, Duration::from_secs(10)).unwrap();
    assert_eq!(
        session.admit_lab(token, now, Some("synthetic"), peer(), &cfg),
        Err(SessionError::Rejected(LoginRejection::WebShutdown))
    );
    cfg.allow_during_maintenance = true;
    let token = session.begin(now, Duration::from_secs(10)).unwrap();
    session
        .admit_lab(token, now, Some("synthetic"), "::1".parse().unwrap(), &cfg)
        .unwrap();
    assert_eq!(session.phase(), SessionPhase::CheckingPolicies);
}
#[test]
fn deadline_is_absolute_at_equality_and_expired_callbacks_close_connection() {
    let registry = SessionRegistry::with_capacity(32).unwrap();
    let now = Instant::now();
    let mut session = FrontendSession::new(&registry);
    let token = session.begin(now, Duration::from_secs(10)).unwrap();
    session
        .admit_lab(
            token,
            now + Duration::from_secs(9),
            Some("synthetic"),
            peer(),
            &config(),
        )
        .unwrap();
    assert_eq!(
        session.reject(token, now + Duration::from_secs(10), LoginRejection::Banned),
        Err(SessionError::Timeout)
    );
    assert_eq!(session.phase(), SessionPhase::Closed);
}
#[test]
fn phases_prevent_parallel_login_retry_after_success_or_reopening_closed_connection() {
    let registry = SessionRegistry::with_capacity(32).unwrap();
    let now = Instant::now();
    let mut session = FrontendSession::new(&registry);
    assert_eq!(
        session.begin(now, Duration::ZERO).err(),
        Some(SessionError::Deadline)
    );
    assert_eq!(
        session.begin(now, Duration::from_secs(31)).err(),
        Some(SessionError::Deadline)
    );
    let token = session.begin(now, Duration::from_secs(10)).unwrap();
    assert_eq!(
        session.begin(now, Duration::from_secs(10)).err(),
        Some(SessionError::InvalidPhase)
    );
    session
        .admit_lab(token, now, Some("synthetic"), peer(), &config())
        .unwrap();
    assert_eq!(
        session.admit_lab(token, now, Some("other"), peer(), &config()),
        Err(SessionError::InvalidPhase)
    );
    session.close();
    assert_eq!(
        session.begin(now, Duration::from_secs(10)).err(),
        Some(SessionError::InvalidPhase)
    );
}
