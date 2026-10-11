use serde::Deserialize;
use vindictus_server::protocol::identity::{
    AsciiCulture, FrameworkArchitecture, IdentityError, IdentityProfile, LabIdentityOptions,
    derive_lab_identity, framework_string_hash,
};

#[derive(Deserialize)]
struct Fixture {
    implementations: Vec<Implementation>,
}
#[derive(Deserialize)]
struct Implementation {
    architecture: String,
    profiles: Vec<Profile>,
}
#[derive(Deserialize)]
struct Profile {
    culture: String,
    cases: Vec<Case>,
}
#[derive(Deserialize)]
struct Case {
    passport: Option<String>,
    supported_ascii: bool,
    raw_utf16: Option<Vec<u16>>,
    lower_utf16: Option<Vec<u16>>,
    raw_hash: Option<i32>,
    generic: Expected,
    zh_cn: Expected,
}
#[derive(Deserialize)]
struct Expected {
    error: Option<String>,
    nexon_id: Option<String>,
    nexon_sn: Option<i32>,
}
fn fixture() -> Fixture {
    serde_json::from_str(include_str!("fixtures/framework-identity.json"))
        .expect("synthetic Framework fixture")
}
fn architecture(value: &str) -> FrameworkArchitecture {
    match value {
        "x86" => FrameworkArchitecture::X86,
        "x64" => FrameworkArchitecture::X64,
        _ => panic!("unknown architecture"),
    }
}
fn culture(value: &str) -> Option<AsciiCulture> {
    match value {
        "" => Some(AsciiCulture::Invariant),
        "en-US" => Some(AsciiCulture::EnUs),
        "zh-TW" => Some(AsciiCulture::ZhTw),
        "zh-CN" => Some(AsciiCulture::ZhCn),
        _ => None,
    }
}
fn options() -> LabIdentityOptions {
    LabIdentityOptions {
        profile: IdentityProfile {
            architecture: FrameworkArchitecture::X86,
            culture: AsciiCulture::Invariant,
        },
        ngm_disable: true,
        zh_cn: false,
        max_passport_bytes: 4096,
    }
}

#[test]
fn framework_hash_matches_bcl_on_both_architectures_including_unicode_and_embedded_nul() {
    for implementation in fixture().implementations {
        for profile in implementation.profiles {
            for case in profile.cases {
                if let (Some(units), Some(expected)) = (case.raw_utf16, case.raw_hash) {
                    assert_eq!(
                        framework_string_hash(&units, architecture(&implementation.architecture)),
                        expected
                    );
                }
            }
        }
    }
}

#[test]
fn ascii_identity_and_zh_cn_restrictions_match_synthetic_framework_bcl() {
    for implementation in fixture().implementations {
        for profile in implementation.profiles {
            let Some(culture) = culture(&profile.culture) else {
                continue;
            };
            for case in profile
                .cases
                .into_iter()
                .filter(|case| case.supported_ascii)
            {
                for (zh_cn, expected) in [(false, &case.generic), (true, &case.zh_cn)] {
                    let result = derive_lab_identity(
                        case.passport.as_deref(),
                        LabIdentityOptions {
                            profile: IdentityProfile {
                                architecture: architecture(&implementation.architecture),
                                culture,
                            },
                            zh_cn,
                            ..options()
                        },
                    );
                    if let Some(error) = expected.error.as_deref() {
                        let expected_error = match error {
                            "NullPassport" => IdentityError::NullPassport,
                            "NoUserInfo" => IdentityError::NoUserInfo,
                            "PassportFail" => IdentityError::PassportFail,
                            _ => panic!("unknown oracle failure"),
                        };
                        assert_eq!(result.err(), Some(expected_error));
                    } else {
                        let value = result.expect("synthetic passport accepted");
                        assert_eq!(
                            (Some(value.nexon_id), Some(value.nexon_sn)),
                            (expected.nexon_id.clone(), expected.nexon_sn)
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn online_mode_requires_separate_authentication_even_for_valid_lab_passport() {
    assert_eq!(
        derive_lab_identity(
            Some("LabUser"),
            LabIdentityOptions {
                ngm_disable: false,
                ..options()
            }
        )
        .err(),
        Some(IdentityError::OnlineMode)
    );
}
#[test]
fn unicode_and_control_passports_are_rejected_instead_of_guessing_culture_casing() {
    for passport in ["I\u{0130}", "É", "😀", "a\0b", "a\nb"] {
        assert_eq!(
            derive_lab_identity(Some(passport), options()).err(),
            Some(IdentityError::UnsupportedCharacters)
        );
    }
}
#[test]
fn input_is_bounded_before_casing_allocation() {
    assert_eq!(
        derive_lab_identity(
            Some("aaaa"),
            LabIdentityOptions {
                max_passport_bytes: 3,
                ..options()
            }
        )
        .err(),
        Some(IdentityError::TooLarge)
    );
}
#[test]
fn identity_preserves_case_and_truncates_name_but_hashes_entire_passport() {
    let prefix = "ABCDEFGHIJKLMNOPQRSTUVWX";
    let first = derive_lab_identity(Some(&format!("{prefix}a")), options())
        .ok()
        .unwrap();
    let second = derive_lab_identity(Some(&format!("{prefix}b")), options())
        .ok()
        .unwrap();
    assert_eq!(first.nexon_id, prefix);
    assert_eq!(second.nexon_id, prefix);
    assert_ne!(first.nexon_sn, second.nexon_sn);
}
#[test]
fn architecture_profile_changes_legacy_identifier() {
    let x86 = derive_lab_identity(Some("ab"), options())
        .ok()
        .unwrap()
        .nexon_sn;
    let x64 = derive_lab_identity(
        Some("ab"),
        LabIdentityOptions {
            profile: IdentityProfile {
                architecture: FrameworkArchitecture::X64,
                culture: AsciiCulture::Invariant,
            },
            ..options()
        },
    )
    .ok()
    .unwrap()
    .nexon_sn;
    assert_eq!((x86, x64), (-840386625, 1093630535));
}

#[test]
fn turkish_oracle_demonstrates_why_ascii_culture_must_be_selected_explicitly() {
    for implementation in fixture().implementations {
        for profile in implementation.profiles {
            let case = profile
                .cases
                .iter()
                .find(|case| case.passport.as_deref() == Some("I"))
                .unwrap();
            let expected = if profile.culture == "tr-TR" { 305 } else { 105 };
            assert_eq!(case.lower_utf16.as_deref(), Some([expected].as_slice()));
        }
    }
}
