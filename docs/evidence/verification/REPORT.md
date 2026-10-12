VERIFICATION REPORT
-------------------
Claim: M13.2d.2b.2c.2b.1 ToArgInt decision chain and ToInt32/relative-second arithmetic with explicit temporal ports; general cultural date grammar not migrated.
Command: ./scripts/reverse/generate-item-argument-fixtures.ps1; ./scripts/dev/verify.ps1 -DatabaseIntegration; docker compose build --quiet server; docker compose up -d --wait server; ./.cache/verify-login-reward-docker.ps1.
Executed: gate 2026-10-12T15:34:47.4756456+00:00; Docker preservation 2026-10-12T15:38:12.2366966+00:00, after final source changes.
Exit code: 0 for final oracle, gate, locked Docker build/up/smoke/restart/preservation.
Output summary: rust-tests passed=147 failed=0 ignored=40; postgres-integration passed=40 failed=0 ignored=0; Python passed=15. Framework2architectures x5cultures x3zones x100cases:3000 comparisons,180 range errors,1320 explicit local conversions. Docker PASS;287snapshots/64fields and29users/15fields preserved;queue140/next141unused/0real batch receipts.
Warnings: none in final verification.
Errors: none in final verification. Initial C# harness static-class generic compilation corrected before generating final fixtures.
Verdict: PASS for temporal decision/arithmetic substage only. Date parsing and cultural suffix observations are injected into Rust tests; they are not a migrated date grammar, DST adapter, queue delivery or game-client acceptance.

Evidence: docs/evidence/auth/item-argument.json; docs/item-argument.md; docs/evidence/verification/gate.json and docker-smoke.json.
No inventory schema changes or writes; source services not executed; source databases/assets remain read-only.
