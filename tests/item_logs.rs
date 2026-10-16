use serde_json::{Value, json};
use vindictus_server::domain::{
    item_logs::{Counts, Data, Ledger, Manager},
    legacy_user_data::SqlDateTime2,
};
fn utf(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
fn data(mask: u64) -> [Data; 3] {
    [
        Data::new(
            101,
            utf("Syn"),
            2,
            [i32::MIN, 0, i32::MAX],
            -7,
            9,
            mask & 32 != 0,
        )
        .unwrap(),
        Data::new(-1, utf("synthetic"), 3, [-1; 3], 0, 0, false).unwrap(),
        Data::requested(utf("Syn"), -3).unwrap(),
    ]
}
#[test]
fn item_log_projection_matches_all_original_framework_scenarios() {
    let f: Value = serde_json::from_str(include_str!("fixtures/framework-item-logs.json")).unwrap();
    let cases = f["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 64);
    for c in cases {
        let mask = c["mask"].as_u64().unwrap();
        let jid = if mask & 4 != 0 { -1 } else { 17 };
        let enabled = mask & 1 != 0;
        let mut m = Manager::default();
        m.set_journal(jid, 19);
        let mut tick = 0;
        for d in data(mask) {
            m.add(d, enabled, || {
                let t = 637999999999999999 + tick * 17001;
                tick += 1;
                SqlDateTime2::from_ticks(t).unwrap()
            })
            .unwrap();
        }
        let counts = Counts {
            inventory: if mask & 8 == 0 { Some(11) } else { None },
            shared: if mask & 24 == 0 { Some(7) } else { None },
        };
        let logs = m
            .prepare_write(enabled, |_| counts)
            .unwrap_or(&[])
            .iter()
            .map(|r| {
                json!([
                    r.journal,
                    23,
                    r.event,
                    r.detail,
                    r.before_id,
                    r.data.item_id,
                    String::from_utf16(
                        &r.data
                            .class()
                            .as_chunks::<2>()
                            .0
                            .iter()
                            .map(|c| u16::from_le_bytes([c[0], c[1]]))
                            .collect::<Vec<_>>()
                    )
                    .unwrap(),
                    r.data.count,
                    r.after.inventory,
                    r.after.shared,
                    r.data.colors[0],
                    r.data.colors[1],
                    r.data.colors[2],
                    r.data.reduced,
                    r.data.bonus,
                    null,
                    r.captured.ticks()
                ])
            })
            .collect::<Vec<_>>();
        assert_eq!(json!(logs), c["logs"], "mask{mask}");
        let mut ledgers = Vec::new();
        for d in data(mask) {
            let l = Ledger::creation(jid, d, counts.inventory);
            if !l.suppressed(mask & 2 != 0) {
                ledgers.push(json!([
                    jid,
                    l.data.item_id,
                    if l.data.class() == utf("Syn") {
                        "Syn"
                    } else {
                        "synthetic"
                    },
                    23,
                    2,
                    l.data.count,
                    l.data.colors[0],
                    l.data.colors[1],
                    l.data.colors[2],
                    l.data.reduced,
                    l.data.bonus,
                    23,
                    l.bundle
                ]));
            }
        }
        let failure = Ledger::creation_failure(jid, utf("giveDefaultItemFail(save)")).unwrap();
        if !failure.suppressed(mask & 2 != 0) {
            ledgers.push(json!([
                jid,
                -1,
                "giveDefaultItemFail(save)",
                23,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                23,
                null
            ]));
        }
        assert_eq!(json!(ledgers), c["ledgers"], "mask{mask}");
        m.clear();
        assert_eq!(m.pending(), 0);
    }
}
#[test]
fn item_log_manager_preserves_disabled_queue_live_last_counts_and_wrapping_shared_delta() {
    let now = || SqlDateTime2::from_ticks(638000000000000000).unwrap();
    let mut m = Manager::default();
    m.set_journal(17, 4);
    m.add_detail(Data::requested(utf("A"), i32::MAX).unwrap(), 4, true, now)
        .unwrap();
    m.add_detail(Data::requested(utf("B"), 1).unwrap(), 4, true, now)
        .unwrap();
    m.set_journal(18, 19);
    m.add(Data::requested(utf("C"), 0).unwrap(), true, now)
        .unwrap();
    assert!(
        m.prepare_write(false, |_| panic!("disabled Write must not query owner"))
            .is_none()
    );
    assert_eq!(m.pending(), 3);
    let rows = m
        .prepare_write(true, |d| {
            assert_eq!(d.class(), utf("C"));
            Counts {
                inventory: Some(13),
                shared: Some(i32::MAX),
            }
        })
        .unwrap();
    assert_eq!(rows[0].after.inventory, None);
    assert_eq!(rows[1].after.shared, None);
    assert_eq!(rows[2].after.shared, Some(-1));
    assert_eq!(rows[2].after.inventory, Some(13));
    assert_eq!(rows[2].detail, 2);
    assert_eq!(m.pending(), 3);
    assert!(Data::requested(vec![0; 2049], 1).is_err());
    assert!(Data::requested(vec![0; 2048], 1).is_ok());
}
