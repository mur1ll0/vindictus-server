VERIFICATION REPORT
-------------------
Claim: M13.2d.2b.2c.1 queue dependency recovery and pure selection/event/route/expiry/default-binding decisions.
Command: ./scripts/dev/verify.ps1 -DatabaseIntegration; ./scripts/reverse/generate-queue-decisions-fixtures.ps1; py -3.13 scripts/reverse/catalog-queue-consumers.py; docker compose build --quiet server; docker compose up -d --wait server; ./.cache/verify-login-reward-docker.ps1 (read-only policy, smoke/restart and snapshot Verify).
Executed: gate 2026-10-12T04:21:35.5545606+00:00; Docker preservation 2026-10-12T04:25:53.2178870+00:00, after final code changes.
Exit code: 0 for gate, oracle, metadata recovery, Docker build/up/smoke/restart/preservation.
Output summary: rust-tests passed=133 failed=0 ignored=40; postgres-integration passed=40 failed=0 ignored=0; Python passed=15; Framework profiles2 with72attempts/504routes/56binders/8selections each; Docker PASS;287snapshots/64fields and29users/15fields unchanged;queue140/next141unused/0real batch receipts.
Warnings: none in final verification.
Errors: none in final verification.
Verdict: PASS for this substage; delivery/processed updates, inventory/mail/log execution, session and real-client acceptance remain pending.

Evidence: docs/evidence/auth/queue-consumers.json; queue-consumer-contract.json; docs/evidence/verification/gate.json and docker-smoke.json; docs/evidence/database/login-reward-docker.json.
The oracle executes extracted decisions with synthetic shims and in-memory LINQ selection, not the delivery body or source services. Docker remains non-root/locked; no private catalog, assets or source fragments added.
