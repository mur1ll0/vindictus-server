# First-Fiona initialization state

M15.1b.2e.10a preserves the source tables needed before connecting Create. It does not enable creation or establish client compatibility.

Migration20 stores Title (6 columns), TitleGoalProgress (3), TitlePreset (2), MicroPlayStatus (12), CharacterMissionInfo (3) and MissionAssignment (11). Source primary keys and nullable fields are preserved. Only Title, TitleGoalProgress and MicroPlayStatus reference Character with update/delete cascades; mission tables have no fabricated FK or identity. UTF16 remains opaque, SQL datetime retains 1/300-second units, datetime2 retains exact ticks. No source defaults are inferred for nullable ReportReceivedCount.

`Import-CharacterState.ps1` selects from the unchanged SQL Server reference. Its bounded private snapshot is ACL restricted to the current Windows user and SYSTEM. The stdin CLI atomically inserts and compares every source field. Initial import rejects unexpected rows; replay and verify allow new runtime rows while requiring historical rows to match. The preservation marker binds the exact export SHA256 and six counts; changing whitespace requires a new reviewed snapshot rather than silently rebinding. Cancellation, failed FKs, late errors and conflicting rows roll back. Replay never repairs historical data.

Internal writers admit only an owned synthetic account revision and its first normal level1 Fiona. They preserve mission-count initialization, distinct UTC timestamps for MicroPlay/chat-report initialization, nullable counters and non-acquired title presets. Exact replay is accepted; conflicting initialized state is rejected. Returning inserted preset IDs does not replace the required GiveTitle logs; these writers are not yet called by Create.

The private initializer profile binds content/configuration/state snapshots and eight original code hashes. Additional version comparisons remain in a bounded private sidecar measured using the original Framework method on x86/x64. They extend only one policy instance, cannot contradict existing evidence and are not committed as real-value fixtures. Unsupported skill effects, unsynchronized presets and active newbie attendance are refused. Base stats require an explicit title contribution. The current projection is deliberately scoped; it is not a migration of the full skill/attendance subsystem.

`lobby_costume::project` recovers CostumeManager.LoadFromDB, CostumeInfo initialization and CostumeDataExtension.Apply. It applies base rows before item rows, preserves missing fields, populates all extra keys and uses the measured Single conversion for body shape. It emits non-null empty dictionaries and enforces the existing wire bounds. It does not serialize new messages. The original constructor/method are extracted only into ignored cache; public fixtures contain hashes of 256 synthetic inputs/outputs per architecture. The fixture generator and tests verify both input generation and the complete output digest.

Verification commands:

- `scripts/reverse/generate-fiona-initializer-sources.ps1`: code paths/hashes only.
- `scripts/content/export_fiona_initializer.py`: actual values only under protected cache.
- `scripts/reverse/measure-fiona-initializer-features.ps1`: private comparisons only.
- `scripts/reverse/generate-lobby-costume-fixtures.ps1`: synthetic original-method oracle.
- `scripts/dev/verify.ps1 -DatabaseIntegration -DatabaseIntegrationDocker`: full gate.
- `scripts/database/Import-CharacterState.ps1`, then `-Mode Verify` after restart.

Next: preserve item/skill/title logs and the original journal counter; integrate default grants and effective callbacks, inventory/appearance, login/logout effects and complete lobby Summary into the guarded lifecycle; then enable Create, test Create/relogin with the operator, implement selection/PlayerLoader/MMO/city and movement. No new client test is useful until the Create path is enabled. The full M01–M24 migration remains required.
