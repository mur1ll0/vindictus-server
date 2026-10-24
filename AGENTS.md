# Project instructions

Scope: all development under vindictus-server. The requested deliverable is a complete Rust migration; the current foundation is not that deliverable.

## Session memory
- Read MIGRATION.md and docs/playable-game-roadmap.md before changing code. The roadmap's blocks (B0–B5) set the work order; continue the earliest actionable incomplete step of the current block.
- Update MIGRATION.md after each verified step with command, evidence, remaining dependencies, client-test status (checklist entry or "cliente: não necessário") and next action.
- Never mark a service migrated merely because ILSpy exported C# or a Rust placeholder compiles.
- Game compatibility requires a reproducible client login, character creation, battle and persistence test.

## Continuous execution (user instruction 2026-10-21)
- After committing a verified step, start the next step of the current block in the same turn. Do not ask the operator to continue and do not end the turn with only a summary.
- Steps with client-visible effects add an entry (preconditions, client steps, expected result) to the block's test script, docs/client-tests/<block>.md, instead of waiting for a client test. At the gate, complete that script with preparation, gameplay order and reporting instructions (format in docs/playable-game-roadmap.md).
- Stop and hand control back only when:
  1. the current block is complete: run the full gate, finish the block's test script, commit, and ask for the client gameplay session;
  2. the next step cannot be verified without confirming an earlier step in the client (for example, the first dungeon load before combat inside it);
  3. a decision belongs to the operator (scope, trade-off without evidence), or an action is destructive or outside existing authorization;
  4. required evidence is missing and cannot be recovered;
  5. a test or gate keeps failing after one focused investigation.
- When stopping, record the reason and the resumable next action in MIGRATION.md.
- Design every state change visible to other players through a single broadcast/authority point so B5 multiplayer does not require rewrites; multiplayer is tested only in B5.

## Skills
- Rust: .agents/skills/rust-best-practices/SKILL.md and relevant reference chapters.
- Database: .agents/skills/supabase-postgres-best-practices/SKILL.md before schema/query changes.
- Docker: .agents/skills/docker-build-strategies/SKILL.md for image review.
- Skill source/provenance: docs/skills.md. These project skills become available to discovery on the next turn; they can be read directly now.

## Architecture
- Modular monolith: domain owns invariants, application coordinates use cases, infrastructure owns persistence, HTTP is operational management, protocol owns recovered wire formats.
- Keep game TCP protocol separate from operational HTTP. Do not invent opcode values or claim HTTP account provisioning implements game login.
- Ports and data representations must be recovered from evidence and tested against the recovered reference implementation.
- SQL Server is the unchanged legacy reference. PostgreSQL is the new implementation target; existing .bak files are not PostgreSQL backups.
- Keep SQLite content databases versioned separately from transactional character state.
- Use bounded parsing, typed errors, prepared SQL, bounded connection pools, secure password storage, and graceful shutdown.
- No unsafe Rust, production unwrap/expect, secret logging, plaintext password arguments, or real credentials in source.

## Preservation
- Original Client/, Vindictus1.69EN/, ServidorVindictus/, the Windictus 2023 reference client at `C:\Windictus 2023` (see docs/client-compatibility.md, "Clientes de referência") and backups are read-only evidence.
- Decompilation output is local ignored reference, not production code. Retain source hashes.
- Do not overwrite restored databases, remove Docker volumes, run global Docker cleanup, or alter unrelated containers.
- Never execute unreviewed legacy services merely to inspect metadata.

## Verification
- User instruction (2026-10-15): minimize terminal/test output collected into context. Redirect complete logs to .cache/verification; inspect only exit status, aggregate counts and bounded failure excerpts. Do not dump successful test output or repeatedly poll unchanged logs. Preserve this rule across sessions/compaction.
- User instruction (2026-10-21, replaces 2026-10-20): the full gate (scripts/dev/verify.ps1 -DatabaseIntegration plus the Docker smoke) runs only when closing a roadmap block (B1 NPCs/money/shops/quests, B2 dungeons/combat, B3 craft/enhance/enchant, B4 admin panel, B5 multiplayer/remaining), to validate integration with the other game features. Every step inside a block runs only fast focused tests (affected test targets and filtered PostgreSQL tests). The operator may also request a full gate explicitly.
- User authorization (2026-10-20): builds/tests may use multiple CPU cores; CARGO_BUILD_JOBS=1 is no longer required.
- User instruction (2026-10-24): the operator's machine runs hot (memory, CPU, disk). Rebuild Docker (`COMPOSE_PARALLEL_LIMIT=1 docker compose build`, then `docker compose up -d --wait`) only when closing a group of steps, not after every step. Avoid redundant compilations (batch edits before building the test crate; do not rebuild just to re-check). Prioritize delivery over extra verification passes inside a step.
- The disposable tmpfs PostgreSQL test container (vindictus-map-verification-postgres-1) accumulates WAL; restart it before a full suite run when usage is high.
- Database integration tests must execute against an isolated schema; skipped tests are not passing evidence.
- Regenerate protocol fixtures from recovered C# via scripts/reverse/generate-fixtures.ps1 when changing the recovered protocol.
- Docker verification: build --locked, migrate exit 0, server readiness, create-account and duplicate rejection, restart persistence.
- Native protocol and client compatibility remain pending until exercised, regardless of successful infrastructure tests.

## Git and stage completion
- User authorized local commits after each completed, verified migration stage.
- Commit each step with its code, focused tests, sanitized evidence, block test-script entry and MIGRATION.md status together. The block's closing commit records the full gate result.
- Stage explicit paths and review the staged diff and file list.
- Never commit credentials, private exports, restored raw SQL catalogs, original game assets or decompiled proprietary sources.
- Use the configured Git author. No push or remote creation is implied by local commit authorization.
