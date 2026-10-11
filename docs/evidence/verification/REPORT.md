VERIFICATION REPORT
-------------------
Claim: M13.2d.2b.2a verificada: criação de entradas SendDormantItem, identity explícito e replay durável; etapa pai parcial.
Command: scripts/database/Compare-DormantItems.ps1; scripts/dev/verify.ps1 -DatabaseIntegration; docker compose build server; docker compose up -d --wait server; docker compose run --rm --no-deps server initialize-queue-identity --source-last-value 140 (antes/depois de restart); scripts/dev/smoke-docker.ps1; scripts/database/Import-Characters.ps1 -Mode Verify; scripts/database/Import-Users.ps1 -Mode Verify; git diff --cached --check
Executed: Gate 2026-10-11T19:23:58.6805928+00:00; Docker 2026-10-11T19:25:32.8819746+00:00; identity/restart 2026-10-11T19:25:38.3355440+00:00; snapshots 2026-10-11T19:25:39.4104182+00:00.
Exit code: 0
Output summary: Oracle PASS:4synthetic rows/18columns; rollback identity gap; UTC within bounds; source140rows/counter140 unchanged. rust-tests:exit=0; passed=120; failed=0; ignored=31. postgres-integration:exit=0; passed=31; failed=0; ignored=0. client-python:exit=0; passed=7; failed=0; ignored=0. Format/Clippy/doc/build exit=0. Docker build exit=0. migrate/readiness/smoke/restart PASS. Queue identity Docker PASS:initializedfrom140/next141unused; replay does not reseed/restart preserves. Character verify:287rows/64fields/inserted0/sourceunchanged. User verify:29rows/15fields/inserted0/sourceunchanged. Hash mismatches=0.
Warnings: none; doctests have0examples. General rust-tests ignore31DBtests; all31execute explicitly in postgres-integration.
Errors: none in final commands. Initial TEMP oracle reseeded a never-used identity and emitted140; harness now models the used source identity, giving141 and consuming144 on rollback. No persistent reference writes.
Verdict: PASS for M13.2d.2b.2a; parents partial, client authentication unavailable.

Contracts: docs/dormant-item-queue.md; docs/character-login-storage.md.
Evidence: docs/evidence/auth/dormant-item-queue.json; docs/evidence/database/dormant-item-oracle.json; queue-identity-docker.json; character-import.json; user-import.json; gate.json; docker-smoke.json.
Pending: DormantReward/FeatureMatrix, character reward planning, mail/UTF16, SQL datetime callback conversion, account executor integration, inventory/mail/expiry/logging and client login. Source removal marks Processed/ProcessedTime; this stage does not mark delivery. Local receipts add retries without claiming they existed in the original procedure.
