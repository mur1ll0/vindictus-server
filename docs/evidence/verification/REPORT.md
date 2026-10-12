VERIFICATION REPORT
-------------------
Claim: M13.2d.2b.2c.2a common ItemClassEx parser/build/equality, Integer branch, location sentinel and structural inventory recovery.
Command: ./scripts/dev/verify.ps1 -DatabaseIntegration; ./scripts/reverse/generate-item-parser-fixtures.ps1; py -3.13 scripts/reverse/catalog-inventory-contract.py; docker compose build --quiet server; docker compose up -d --wait server; ./.cache/verify-login-reward-docker.ps1.
Executed: gate 2026-10-12T11:50:44.8171548+00:00; Docker preservation 2026-10-12T11:55:54.4705477+00:00, after final code changes.
Exit code: 0 for final gate, oracle, metadata recovery, Docker build/up/smoke/restart/preservation.
Output summary: rust-tests passed=139 failed=0 ignored=40; postgres-integration passed=40 failed=0 ignored=0; Python passed=15; Framework2architectures x5cultures,327syntax/33integer/24equality per culture and48locations per architecture;3936comparison cases. Docker PASS;287snapshots/64fields and29users/15fields unchanged;queue140/next141unused/0real batch receipts.
Warnings: none in final gate.
Errors: none in final verification. Earlier chunks_exact lint corrected using as_chunks; full gate repeated.
Verdict: PASS for this substage; temporal/culture/local-clock resolver, full content/mobile binder, inventory writes, delivery and game-client acceptance remain pending.

Evidence: docs/evidence/auth/item-parser.json and inventory-contract.json; docs/evidence/verification/gate.json and docker-smoke.json; docs/evidence/database/login-reward-docker.json.
The source syntax uses an explicit synthetic resolver, with int.TryParse separately exercised. No real inventory or queue delivery writes; no source services/assets/private data in the image.
