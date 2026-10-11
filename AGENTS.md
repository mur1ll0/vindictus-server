# Project instructions

Scope: all development under vindictus-server. The requested deliverable is a complete Rust migration; the current foundation is not that deliverable.

## Session memory
- Read MIGRATION.md before changing code. Continue the earliest actionable incomplete step.
- Update MIGRATION.md after each verified step with command, evidence, remaining dependencies and next action.
- Never mark a service migrated merely because ILSpy exported C# or a Rust placeholder compiles.
- Game compatibility requires a reproducible client login, character creation, battle and persistence test.

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
- Original Client/, Vindictus1.69EN/, ServidorVindictus/ and backups are read-only evidence.
- Decompilation output is local ignored reference, not production code. Retain source hashes.
- Do not overwrite restored databases, remove Docker volumes, run global Docker cleanup, or alter unrelated containers.
- Never execute unreviewed legacy services merely to inspect metadata.

## Verification
- Run scripts/dev/verify.ps1 before reporting a code step complete.
- Database integration tests must execute against an isolated schema; skipped tests are not passing evidence.
- Regenerate protocol fixtures from recovered C# via scripts/reverse/generate-fixtures.ps1 when changing the recovered protocol.
- Docker verification: build --locked, migrate exit 0, server readiness, create-account and duplicate rejection, restart persistence.
- Native protocol and client compatibility remain pending until exercised, regardless of successful infrastructure tests.

## Git and stage completion
- User authorized local commits after each completed, verified migration stage.
- Commit each stage with its code, tests, sanitized evidence and MIGRATION.md status together.
- Run the full gate before commits. Stage explicit paths and review the staged diff and file list.
- Never commit credentials, private exports, restored raw SQL catalogs, original game assets or decompiled proprietary sources.
- Use the configured Git author. No push or remote creation is implied by local commit authorization.
