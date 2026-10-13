VERIFICATION REPORT
-------------------
Claim: M13.3d.2c.2a recovered EN-EU CheckValidName domain and reproducible synthetic Framework oracle; full name availability and client acceptance pending.
Command: scripts/reverse/generate-character-name-fixtures.ps1; scripts/dev/verify.ps1 -DatabaseIntegration -DatabaseIntegrationDocker; .cache/verify-prologue-docker.ps1.
Executed: full gate 2026-10-13T20:32:23.2951073+00:00; Docker preservation completed 2026-10-13T20:43:14.033037+00:00, after all source changes.
Exit code:0.
Output summary:217Rust/0failed;71PostgreSQL/0failed/0ignored;19Python; format/Clippy/doctests/build pass.1050cases per architecture x86/x64,0differences. Two Rust tests compare the original method and reject unsupported patterns. Four effective naming declarations read from original SQLite in read-only mode, hashes unchanged. Docker build/migrate/readiness/duplicate/non-root/restart pass; users29x15, characters287x64, queue140, original lobby25/0/2 and HomeCafe2 preserved; one synthetic prologue row and zero unmarked additional UserInfo; two synthetic reward receipts with no queued rows; imported origins0/nonlab receipts0.
Warnings:none in final gate.
Errors:none in final gate. Initial Clippy int-plus-one warning corrected with equivalent expression; full gate repeated. Failed runs do not count.
Verdict:PASS for observed-pattern CheckValidName scope. Forbidden/reserved/duplicate/history checks and wire handlers remain pending; Create/selection/PlayerLoader/MMO/movement pending. Last actual operator test: repeated tutorial, creation timeout/disconnect and zero created characters. No new client acceptance claimed for this domain-only step.
Evidence:docs/character-naming.md;docs/evidence/auth/character-naming-rule.json;character-prologue-client.json;gate.json/docker-smoke.json and preservation summaries. Full logs in .cache/verification. Original services not executed; original files and SQLServer read-only; no credentials/private exports published; no push; complete migration remains pending.
