use vindictus_server::domain::character_creation_admission::CreationSlots;
#[test]
fn creation_slot_limit_retains_unchecked_source_addition_and_exact_boundary() {
    let slots = CreationSlots {
        visible: 4,
        purchased: 2,
        premium: 1,
    };
    assert!(slots.at_limit(1));
    assert!(!slots.at_limit(2));
    assert!(slots.at_limit(i32::MAX)); // unchecked addition wraps negative
    assert!(
        !CreationSlots {
            visible: 0,
            purchased: 0,
            premium: 0
        }
        .at_limit(1)
    );
    assert!(
        CreationSlots {
            visible: 0,
            purchased: 0,
            premium: 0
        }
        .at_limit(0)
    );
}
