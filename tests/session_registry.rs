use serde::Deserialize;
use std::{
    sync::{Arc, Barrier},
    time::{Duration, Instant},
};
use vindictus_server::{
    application::{
        frontend_session::{FrontendSession, LabAdmission, SessionError, SessionPhase},
        session_registry::{MAX_REGISTRY_CAPACITY, RegistryError, SessionRegistry},
    },
    domain::passport::LoginRejection,
    protocol::identity::{
        AsciiCulture, FrameworkArchitecture, IdentityProfile, LabIdentityOptions,
    },
};
fn config() -> LabAdmission {
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
        allow_cheat: false,
        allow_during_maintenance: false,
        is_online: true,
    }
}
fn admit(registry: &SessionRegistry, name: &str) -> Result<FrontendSession, SessionError> {
    let mut session = FrontendSession::new(registry);
    let now = Instant::now();
    let token = session.begin(now, Duration::from_secs(10))?;
    session.admit_lab(
        token,
        now,
        Some(name),
        "127.0.0.1".parse().unwrap(),
        &config(),
    )?;
    Ok(session)
}
#[derive(Deserialize)]
struct Fixture {
    profiles: Vec<Profile>,
}
#[derive(Deserialize)]
struct Profile {
    duplicates: Vec<Duplicate>,
    lifecycle: Vec<Lifecycle>,
}
#[derive(Deserialize)]
struct Duplicate {
    remote: Option<String>,
    tag: String,
    feedback: Vec<String>,
    disconnects: usize,
    queued: usize,
    close_before: usize,
    close_after: usize,
}
#[derive(Deserialize)]
struct Lifecycle {
    id: i32,
    uses: i32,
    throws: bool,
    closed: i32,
    tag_cleared: bool,
    calls: i32,
    before: i32,
    after: i32,
}
#[test]
fn recovered_duplicate_reason_matches_while_active_session_is_preserved_deliberately() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("fixtures/framework-session-lifecycle.json")).unwrap();
    for profile in fixture.profiles {
        assert_eq!(profile.duplicates.len(), 12);
        for case in profile.duplicates {
            if case.remote.as_deref() == Some("FrontendServiceCore.FrontendService")
                && case.tag == "user"
            {
                let registry = SessionRegistry::with_capacity(2).unwrap();
                let old = admit(&registry, "duplicate").unwrap();
                let result = admit(&registry, "duplicate");
                assert_eq!(
                    result.err(),
                    Some(SessionError::Rejected(LoginRejection::DuplicateLogin))
                );
                assert_eq!(
                    case.feedback,
                    vec![(LoginRejection::DuplicateLogin as i32).to_string()]
                );
                assert_eq!(
                    (
                        case.disconnects,
                        case.queued,
                        case.close_before,
                        case.close_after
                    ),
                    (1, 1, 0, 1)
                );
                assert_eq!(old.phase(), SessionPhase::CheckingPolicies); // intentional departure from old-session disconnect.
                assert_eq!(registry.active_count().unwrap(), 1);
            } else if case.remote.as_deref() != Some("FrontendServiceCore.FrontendService") {
                assert_eq!(case.feedback, vec!["False"]);
            } else {
                assert_eq!(case.feedback, vec!["continue"]);
            }
        }
        assert_eq!(profile.lifecycle.len(), 18);
        assert_eq!(
            profile.lifecycle.iter().filter(|case| case.throws).count(),
            9
        );
        for case in profile.lifecycle {
            assert!([i32::MIN, 0, i32::MAX].contains(&case.id));
            assert!([0, 1, 3].contains(&case.uses));
            assert_eq!(
                (
                    case.closed,
                    case.tag_cleared,
                    case.calls,
                    case.before,
                    case.after
                ),
                (1, true, 1, 42, -1)
            );
            // All nine simulated AcquireService failures above still clear the recovered Tag.
        }
    }
}
#[test]
fn racing_sixteen_connections_keeps_one_owner_and_fifteen_duplicate_rejections() {
    let registry = SessionRegistry::with_capacity(16).unwrap();
    let barrier = Arc::new(Barrier::new(16));
    let handles: Vec<_> = (0..16)
        .map(|_| {
            let registry = registry.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                admit(&registry, "same_user")
            })
        })
        .collect();
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|r| matches!(
                r,
                Err(SessionError::Rejected(LoginRejection::DuplicateLogin))
            ))
            .count(),
        15
    );
    assert_eq!(registry.active_count().unwrap(), 1);
    drop(results);
    assert_eq!(registry.active_count().unwrap(), 0);
}
#[test]
fn bounded_capacity_recovers_after_drop_and_does_not_grow_with_churn() {
    assert!(matches!(
        SessionRegistry::with_capacity(0),
        Err(RegistryError::InvalidCapacity)
    ));
    assert!(matches!(
        SessionRegistry::with_capacity(MAX_REGISTRY_CAPACITY + 1),
        Err(RegistryError::InvalidCapacity)
    ));
    let registry = SessionRegistry::with_capacity(2).unwrap();
    let first = admit(&registry, "first").unwrap();
    let second = admit(&registry, "second").unwrap();
    assert_eq!(
        admit(&registry, "third").err(),
        Some(SessionError::Registry(RegistryError::AtCapacity))
    );
    assert_eq!(
        admit(&registry, "first").err(),
        Some(SessionError::Rejected(LoginRejection::DuplicateLogin))
    );
    drop(first);
    let third = admit(&registry, "third").unwrap();
    assert_eq!(registry.active_count().unwrap(), 2);
    drop((second, third));
    for i in 0..512 {
        drop(admit(&registry, &format!("churn_{i}")).unwrap());
    }
    assert_eq!(registry.active_count().unwrap(), 0);
}
#[test]
fn late_old_disconnect_and_callback_cannot_release_reconnected_owner() {
    let registry = SessionRegistry::with_capacity(2).unwrap();
    let now = Instant::now();
    let mut old = FrontendSession::new(&registry);
    let old_token = old.begin(now, Duration::from_secs(10)).unwrap();
    old.admit_lab(
        old_token,
        now,
        Some("reconnect"),
        "127.0.0.1".parse().unwrap(),
        &config(),
    )
    .unwrap();
    old.close();
    let mut new = admit(&registry, "reconnect").unwrap();
    old.close();
    drop(old);
    assert_eq!(
        new.reject(old_token, now, LoginRejection::Banned),
        Err(SessionError::StaleCallback)
    );
    assert_eq!(new.phase(), SessionPhase::CheckingPolicies);
    assert_eq!(
        admit(&registry, "reconnect").err(),
        Some(SessionError::Rejected(LoginRejection::DuplicateLogin))
    );
    new.close();
    drop(admit(&registry, "reconnect").unwrap());
    assert_eq!(registry.active_count().unwrap(), 0);
}
#[test]
fn rejected_and_expired_attempts_release_and_unrelated_users_remain_owned() {
    let registry = SessionRegistry::with_capacity(2).unwrap();
    let other = admit(&registry, "unrelated").unwrap();
    let now = Instant::now();
    let mut session = FrontendSession::new(&registry);
    let token = session.begin(now, Duration::from_secs(10)).unwrap();
    session
        .admit_lab(
            token,
            now,
            Some("retry"),
            "127.0.0.1".parse().unwrap(),
            &config(),
        )
        .unwrap();
    let _ = session.reject(token, now, LoginRejection::Banned);
    let token = session.begin(now, Duration::from_secs(10)).unwrap();
    session
        .admit_lab(
            token,
            now,
            Some("retry"),
            "127.0.0.1".parse().unwrap(),
            &config(),
        )
        .unwrap();
    assert_eq!(
        session.reject(token, now + Duration::from_secs(10), LoginRejection::Banned),
        Err(SessionError::Timeout)
    );
    let replacement = admit(&registry, "retry").unwrap();
    assert_eq!(registry.active_count().unwrap(), 2);
    drop((replacement, other));
    assert_eq!(registry.active_count().unwrap(), 0);
}
#[tokio::test]
async fn aborting_task_that_owns_session_releases_without_async_destructor() {
    let registry = SessionRegistry::with_capacity(1).unwrap();
    let runtime = registry.clone();
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        let session = admit(&runtime, "task_abort").unwrap();
        ready_tx.send(()).unwrap();
        std::future::pending::<()>().await;
        drop(session);
    });
    ready_rx.await.unwrap();
    assert_eq!(registry.active_count().unwrap(), 1);
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    drop(admit(&registry, "task_abort").unwrap());
    assert_eq!(registry.active_count().unwrap(), 0);
}
