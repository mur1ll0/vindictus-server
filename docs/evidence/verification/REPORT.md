VERIFICATION REPORT
-------------------
Claim: M13.2d.2b.2c.2b.2 bounded reference en-US/Gregorian/UTC+8 temporal adapter; complete numeric dates and ASCII seconds, fail-closed for unknown grammar.
Command: py -3.13 scripts/reverse/read-offline-item-profile.py; ./scripts/reverse/generate-item-argument-profile-fixtures.ps1; ./scripts/dev/verify.ps1 -DatabaseIntegration; docker compose build --quiet server; docker compose up -d --wait server; ./.cache/verify-login-reward-docker.ps1.
Executed: gate 2026-10-12T16:29:00.6082204+00:00; Docker preservation 2026-10-12T16:31:19.3957052+00:00, after final source changes.
Exit code: 0 for final profile recovery, oracle, gate, locked Docker build/up/smoke/restart/preservation.
Output summary: rust-tests passed=155 failed=0 ignored=40; postgres-integration passed=40 failed=0 ignored=0; Python passed=15. Framework2architectures x4clocks x575cases:4600 comparisons;16 deliberate refusals/architecture. Six clean private hives/five regional configurations agree en-US; UTC+8 from unchanged clock evidence; private queue census140rows/0attributes/0argument delimiters. Docker PASS;287snapshots/64fields and29users/15fields preserved;queue140/next141unused/0real batch receipts.
Warnings: none in final verification.
Errors: none in final verification. Initial offline-reader path escaping fixed; integration-test expectation corrected to preserve source colon-discard behavior; review exposed slash-date plus ISO T mismatch, corrected with six extra cases and full gate/Docker repeated.
Verdict: PASS for bounded explicitly selected offline profile. General DateTime.TryParse/NLS/DST, runtime thread culture, live inventory/queue delivery and game-client acceptance remain unverified.

Evidence: docs/evidence/auth/item-argument-profile.json and item-temporal-profile.json; docs/item-argument-profile.md; gate.json and docker-smoke.json.
Rust parses dates itself; Framework parser/suffix observations are not injected into these adapter tests. Unsupported formats are explicit errors. No source VM/services executed, registry modified or inventory/Processed writes performed.
