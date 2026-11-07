//! The EN-EU PvP modes of the private content export (B5 PV2); skipped without the export.
use vindictus_server::{domain::pvp::Matching, infrastructure::npc_scripts::Profile};

#[test]
#[ignore = "requires the private export .cache/private-content/npc-scripts.json"]
fn private_export_loads_the_en_eu_pvp_modes() {
    let file = std::fs::File::open(".cache/private-content/npc-scripts.json").unwrap();
    let profile = Profile::read(file).unwrap();
    let arena = profile.pvp.get("p05").unwrap();
    assert_eq!(
        (
            arena.matching,
            arena.max_size,
            arena.team_size,
            arena.level_min
        ),
        (Matching::Arena, 32, 1, 40)
    );
    assert_eq!((arena.game_length, arena.arena_channel_count), (90, 3));
    assert_eq!(arena.reward_item, "honor_stamp_limit>honor_stamp");
    let dm = profile.pvp.get("p03").unwrap();
    assert_eq!(
        (dm.matching, dm.team_size, dm.is_guild),
        (Matching::Full, 8, true)
    );
    let gm = profile.pvp.get("p04").unwrap();
    assert_eq!(
        (gm.matching, gm.team_size, gm.is_guild),
        (Matching::Full, 6, false)
    );
    // CTF, Survivor and FruitMatch modes are not migrated.
    assert!(profile.pvp.get("p01").is_none() && profile.pvp.get("p06").is_none());
}
