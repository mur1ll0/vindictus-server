VERIFICATION REPORT
-------------------
Claim: M13.3d.2c.2b.1 reservation/history schema and actual admitted QueryReservedInfo handler; character creation and client acceptance pending.
Command: scripts/reverse/generate-reservation-query-fixtures.ps1; scripts/dev/verify.ps1 -DatabaseIntegration -DatabaseIntegrationDocker; .cache/verify-reservations-docker.ps1.
Executed: full gate 10/13/2026 20:45:13; Docker preservation 10/13/2026 20:50:53, after all code changes.
Exit code:0.
Output summary:217Rust/0failed;75PostgreSQL/0failed/0ignored;19Python/0failed; format/Clippy/doctests/build pass.96 SELECT-only synthetic SQL oracle cases; isolated storage/schema/ownership/cancellation and actual encrypted TCP tests pass, including nonempty and empty lists before password verification. Docker build/migrate/readiness/duplicate/non-root/restart pass; original users29x15/character bundle287x64/queue140/lobby25/0/2/HomeCafe2 preserved; one synthetic prologue row, zero created characters and three synthetic reward receipts. Source and target reservation tables0/0/0, source identity unused and target next1 unused; no invented FK or title identity.
Warnings:none in final gate.
Errors:none in final gate. First run failed four tests: old downgrade fixtures retained migration14 and SQL JSON omitted empty arrays. Fixtures corrected and full gate repeated; failed run retained locally, excluded from passing evidence.
Verdict:PASS for schema and restricted reservation query. Full availability Check, forbidden/collation/history/consumption, Create/defaults/Summary/CID/SN, selection/PlayerLoader/MMO/movement pending. Operator last observed creation timeout/disconnect; this change has not yet been exercised by the actual client. No creation compatibility claimed.
Evidence:docs/character-reservations.md;docs/evidence/auth/character-reservations-query.json;docs/evidence/database/character-reservations.json;gate.json/docker-smoke.json and preservation summaries. Full logs in .cache/verification. Originals and SQL Server remain read-only; no private exports/credentials published; no push; M01-M24 remains the final objective.
