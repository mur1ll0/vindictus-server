VERIFICATION REPORT
-------------------
Claim: M13.3d.2a.1 atomic local laboratory account provenance before login effects, internal scoped session composition and source preservation. No TCP/client compatibility claim.
Command: scripts/dev/verify.ps1 -DatabaseIntegration -DatabaseIntegrationDocker; docker compose build --quiet server; docker compose up -d --wait server; scripts/dev/smoke-docker.ps1; .cache/verify-login-reward-docker.ps1; .cache/verify-lobby-docker.ps1; scripts/database/Import-HomeCafeBonus.ps1 -Mode Verify; sanitized origin metadata query.
Executed: full gate 2026-10-13T14:27:00.0625276+00:00; Docker comparison after restart 2026-10-13T14:32:25.1448145+00:00.
Exit code:0.
Output summary:204Rust/0failed;65PostgreSQL/0failed/0ignored;19Python; format/Clippy/doctests/build pass;0warnings. Six new isolated database tests, eight concurrent creators/one winner, real identity-lock race, rollback, orphan/profile/unmarked refusal and scoped session reconnect/reservation release. Docker migrate/readiness/duplicate/non-root/restart pass; origins0;29users15fields/287bundle64fields/queue140-next141unused/lobby25-0-2/HomeCafe2x4 preserved.
Warnings:none in final verification.
Errors:none in final verification. A test constructor used new instead of the existing with_capacity API; corrected before the final full gate. Failed/incomplete runs are not counted. Full logs remain local.
Verdict:PASS for internal provenance/metadata/preservation only. Runtime protected configuration/allowlist/clock and TCP remain pending; no actual client authentication, character creation/selection or town/movement.
Evidence:docs/lab-account-provenance.md; docs/evidence/auth/lab-account-provenance.json; docs/evidence/database/lab-account-origins.json; gate.json/docker-smoke.json and preservation summaries. Original sources/SQL Server read-only; private exports/credentials not published; no push; full migration incomplete.
