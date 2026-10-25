use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::watch,
    task::JoinHandle,
};
use vindictus_server::{
    infrastructure::{
        lab_frontend::{Runtime, RuntimeError, connection},
        lab_runtime_config::LabRuntimeConfig,
        reward_policy::{RewardPolicyPackage, read_policy},
    },
    protocol::{
        DEFAULT_MAX_BODY, Frame,
        login::{UserLoginMessage, encode_user_login},
        server_profile::USER_LOGIN_MESSAGE_CATEGORY,
        transport::{CipherProfile, LegacyReceiver, LegacySender, OwnedFrame},
    },
};
pub const PASSPORT: &str = "synthetic_tcp_a";
pub fn content(values: &[(&str, Option<&str>)]) -> RewardPolicyPackage {
    let mut v: Value =
        serde_json::from_str(include_str!("../fixtures/synthetic-reward-policy.json")).unwrap();
    for k in [
        "NGM_disable",
        "zhCN",
        "PassportImmigrator",
        "SecondPassword",
        "EssentialSecondPassword",
        "SecondPasswordClear",
        "SecondPassword_UsingSHA",
        "SecondPassword_IP_Limit",
        "MaxCharacterSlot",
        "UserCareSystem",
        "DisableInGameCheat",
        "DisableCharacterSlotCheat",
        "HomeCafeBonus",
        "CafeBonus",
        "CashShop",
        "FraudDetector",
        "NexonGameSecurity",
        "XignCode",
        "OldNexonIDProtect",
        "DisableLogging",
        "AsynchronousLogging",
        "HackshieldOff",
        "NotifyBanReason",
        "IPLockTime",
    ] {
        let value = match k {
            "NGM_disable"
            | "SecondPassword"
            | "EssentialSecondPassword"
            | "SecondPasswordClear"
            | "HackshieldOff"
            | "UserCareSystem" => Some("1"),
            "MaxCharacterSlot" => Some("20"),
            _ => None,
        };
        v["features"]
            .as_array_mut()
            .unwrap()
            .push(json!({"key":k,"value":value}));
    }
    for (key, value) in values {
        v["features"]
            .as_array_mut()
            .unwrap()
            .push(json!({"key":key,"value":value}));
    }
    read_policy(serde_json::to_vec(&v).unwrap().as_slice()).unwrap()
}
pub fn profile(p: &RewardPolicyPackage, ping: bool) -> LabRuntimeConfig {
    let v = json!({"format_version":1,"bind":"127.0.0.1:27015","framework_architecture":"x64","ascii_culture":"invariant","content_database_sha256":"a".repeat(64),"original_config_sha256":"b".repeat(64),"region":"SYNTHETIC","override_region":null,"game_code":1,"server_code":1,"current_version":"3.09.00","service_id":42,"channel_code":0,"civil_offset_seconds":28800,"ping_pong":ping,"allowed_passports":[PASSPORT]});
    LabRuntimeConfig::read(serde_json::to_vec(&v).unwrap().as_slice(), p).unwrap()
}
pub fn runtime() -> Arc<Runtime> {
    let c = content(&[]);
    Arc::new(Runtime::new(profile(&c, false), c).unwrap())
}
pub async fn open(
    r: Arc<Runtime>,
    pool: sqlx::PgPool,
) -> (
    TcpStream,
    JoinHandle<Result<(), RuntimeError>>,
    watch::Sender<bool>,
    LegacyReceiver,
    LegacySender,
) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut client = TcpStream::connect(listener.local_addr().unwrap())
        .await
        .unwrap();
    let (server, _) = listener.accept().await.unwrap();
    let (stop, sig) = watch::channel(false);
    let task = tokio::spawn(connection(server, r, pool, sig));
    let mut receiver =
        LegacyReceiver::with_profile(CipherProfile::VindictusFrontend, DEFAULT_MAX_BODY);
    let registration = read(&mut client, &mut receiver).await;
    assert_eq!(registration.category_id, 0);
    let matrix = read(&mut client, &mut receiver).await;
    assert_eq!(matrix.category_id, 185);
    let decoded = vindictus_server::protocol::features::decode_feature_matrix(
        &matrix.body,
        DEFAULT_MAX_BODY,
        1024,
    )
    .unwrap()
    .unwrap();
    // NGM_disable from the pinned config and the laboratory CashShop override.
    let features = decoded.features.as_ref().unwrap();
    assert_eq!(
        features
            .iter()
            .map(|f| (f.key, f.value))
            .collect::<Vec<_>>(),
        [("NGM_disable", Some("1")), ("CashShop", Some("1.04.08"))]
    );
    let sender =
        LegacySender::with_profile(CipherProfile::VindictusFrontend, 123, DEFAULT_MAX_BODY)
            .unwrap();
    (client, task, stop, receiver, sender)
}
pub async fn read(stream: &mut TcpStream, receiver: &mut LegacyReceiver) -> OwnedFrame {
    tokio::time::timeout(Duration::from_secs(12), async {
        let mut b = [0u8; 1];
        loop {
            stream.read_exact(&mut b).await.unwrap();
            let mut frames = receiver.push(&b).unwrap();
            if !frames.is_empty() {
                return frames.remove(0);
            }
        }
    })
    .await
    .unwrap()
}
pub fn login(passport: &str, sender: &mut LegacySender) -> Vec<u8> {
    let body = encode_user_login(
        Some(&UserLoginMessage {
            passport: Some(passport),
            local_address: 0,
            hw_id: None,
            machine_id: None,
            game_room_client: 0,
            is_character_select_skipped: false,
            nexon_id: Some(""),
            up_to_date_info: None,
            check_sum: 0,
        }),
        DEFAULT_MAX_BODY,
    )
    .unwrap();
    sender
        .encode(&Frame {
            instance_id: 0,
            category_id: USER_LOGIN_MESSAGE_CATEGORY,
            body: &body,
        })
        .unwrap()
}
pub async fn send(stream: &mut TcpStream, sender: &mut LegacySender, category: i32, body: &[u8]) {
    let bytes = sender
        .encode(&Frame {
            instance_id: 0,
            category_id: category,
            body,
        })
        .unwrap();
    stream.write_all(&bytes).await.unwrap();
}
