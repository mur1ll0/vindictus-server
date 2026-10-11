VERIFICATION REPORT
-------------------
Claim: M13.2d.2b.2b.1 verificada: conteúdo privado/FeatureMatrix/planner/SqlClient; executor e cliente pendentes.
Command: scripts/content/Export-RewardPolicy.ps1; scripts/reverse/generate-reward-policy-fixtures.ps1; scripts/dev/verify.ps1 -DatabaseIntegration; docker compose build --quiet server; docker compose up -d --wait server; scripts/content/Verify-RewardPolicy.ps1 -Docker; scripts/dev/smoke-docker.ps1; scripts/database/Import-Characters.ps1 -Mode Verify; scripts/database/Import-Users.ps1 -Mode Verify; git diff --cached --check
Executed: Gate 2026-10-11T20:04:02.5955513+00:00; Docker 2026-10-11T20:06:57.2624727+00:00; política 2026-10-11T20:06:12.4752930+00:00.
Exit code: 0
Output summary: rust-tests:exit=0;passed=128;failed=0;ignored=31. postgres-integration:exit=0;passed=31;failed=0;ignored=0. client-python:exit=0;passed=7. content-python:exit=0;passed=8. Format/Clippy/doc/build exit=0. Oracle Framework x86/x64;81versionpairs/5cultures;21integercases/profile;908SqlClientdates/profile. Private plan catalog24/characters79/calls74; native/Docker/Framework equal. Docker build/migrate/readiness/smoke/restart PASS. Character verify287rows/64fields/inserted0/sourceunchanged; User verify29rows/15fields/inserted0. Content/config/oracle/artifact hash mismatches0. Queue identity remains140source/141unused/140rows/0receipts.
Warnings: none; doctests have0examples.31DBtests ignored in general command, all31executed separately.
Errors: none in final commands. Earlier differential mismatch identified numeric whitespace/NUL handling; fixed before final gate. Parallel PG scenarios produced two different lock/observation timeouts. The final gate runs PG cases sequentially (--test-threads=1), preserving each internal concurrency test and all production/test lock deadlines.
Verdict: PASS for M13.2d.2b.2b.1; parent migration partial.

Contracts: docs/dormant-reward-policy.md; docs/dormant-item-queue.md.
Evidence: docs/evidence/content/reward-policy.json; docs/evidence/verification/gate.json; docker-smoke.json; docs/evidence/database/character-import.json; user-import.json.
Pending: real character/dormancy observations integrated with account executor/receipts/revision, inventory/mail/expiry/logging, new-user/session/frontend and real-client acceptance. No original game service executed; SQL Server has no persistent writes from this oracle.