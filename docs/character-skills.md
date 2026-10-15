# Skill/SPSkill preservation and Fiona base statistics

This component does not enable character creation in the client. The normal Fiona initializer still needs default grants, their effective callbacks, title handling, appearance persistence, cleanup, Summary and the TCP lifecycle.

Migration 0019 preserves the six Skill columns and four SPSkill columns. Names retain the original UTF16 bytes, including empty values and unpaired surrogates. Skill names are non-null; SPSkill names are nullable. Only source primary keys are unique: duplicate skill names and duplicate slots are preserved. Both CID foreign keys reference Character and cascade on update/delete. Skill IDs have no database identity; runtime grants must use the shared service entity authority.

SPSkill uses its own integer sequence. The initial value comes from `sys.identity_columns.last_value`, never a maximum row ID. Import locks the preservation state and both tables, compares every field, initializes the sequence transactionally and commits the entire snapshot. Exact replay and verification never reseed it. Conflicts are rejected without repair. The bounded stdin reader rejects malformed, unknown, incomplete or oversized private input with static errors.

`scripts/database/Import-CharacterSkills.ps1` reads SQL Server using SELECT queries inside a read transaction. The export remains in an ACL-protected cache. Import, replay, typed verification and a second readonly export comparison produce only aggregate public evidence. Use `-Mode Verify` after restarting the new server/database.

The restricted slot initializer checks preserved identity state, the synthetic account origin, canonical account name, account revision and an owned normal Fiona at level 1. It creates slot 0 with explicit SQL NULL when the collection is empty. Exact replay creates no row and consumes no sequence value. Other existing slot states are rejected. It does not grant default skills.

The original `CharacterContents.GetCharacterStats` supplies the six class values plus LUCK=100. `CharacterStats.Add` applies title contributions using unchecked Int32 arithmetic. The Rust composer requires the seven title contributions explicitly; it does not assume that an account has no titles. The guarded database boundary writes all seven statistics and Dirty=false only from a pristine newly created base row, or accepts exact replay. It preserves unrelated fields and rejects conflicts without repair. Loading the actual source-bound class/title profile is a later integration dependency.

The owned oracle template injects the recovered original constructor, GetCharacterStats and Add methods only into ignored framework cache. Public fixtures contain 256 synthetic inputs/results per Framework x86/x64 architecture. Original bodies, raw content values, accounts and exports are not published. Regenerate with `scripts/reverse/generate-character-base-stats-fixtures.ps1` from the repository root.

Source references: ServiceCore-be65d59d2385 / ServiceCore.CharacterServiceOperations / CharacterStats.cs; PlayerService-f23d8b9df77c / PlayerService.CharacterServiceCore / CharacterContents.cs, StatManager.cs and SkillManager.cs. Source digests are in the generated fixture; SQL metadata provenance is linked by the import evidence.

Verification is recorded in docs/evidence/verification/REPORT.md and gate.json after the complete gate. All raw diagnostics remain under .cache/verification. No game compatibility claim follows from these component checks.
