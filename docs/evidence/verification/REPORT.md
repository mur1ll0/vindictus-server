VERIFICATION REPORT
-------------------
Claim: M13.2d.2b.2c.2b.3b.1 effective existence/equipment/storage/binding catalog projection, with readonly native and Docker inspection of actual supplied content.
Command: ./scripts/content/Export-ItemCatalog.ps1; ./scripts/reverse/generate-item-catalog-fixtures.ps1; ./scripts/dev/verify.ps1 -DatabaseIntegration; docker compose build --quiet server; docker compose up -d --wait server; ./scripts/content/Verify-ItemCatalog.ps1 -Docker; ./.cache/verify-login-reward-docker.ps1.
Executed: gate 2026-10-12T17:33:23.3721282+00:00; catalog native/Docker 2026-10-12T17:35:21.8031315+00:00; preservation 2026-10-12T17:36:11.5032886+00:00, after final Rust changes.
Exit code: 0 for final source oracle, gate, Docker locked build/up, actual projection and preservation.
Output summary: rust-tests passed=168 failed=0 ignored=40; postgres-integration passed=40 failed=0 ignored=0; Python passed=19. Six new Rust/four Python tests. Framework2architectures x391scenarios and24new finite version pairs x5cultures x2architectures:1022 comparisons. Actual12377equip/19702class/6extra rows;10967effective equip/18547effective classes;54feature atoms. Native/Framework/Docker counts and digest equal.287snapshots/64fields and29users/15fields preserved;queue140/next141unused/0real batch receipts.
Warnings: none in final verification.
Errors: none in final verification. Initial Clippy byte-string suggestions corrected before fresh gate.
Verdict: PASS for the bounded catalog projection and diagnostic integration. Full DTO conversion, GetItemStat, random rates, remaining ItemContents startup, inventory import/delivery and game-client acceptance remain pending.

Evidence: docs/evidence/auth/item-catalog.json; docs/evidence/content/item-catalog.json; docs/item-catalog.md; gate.json and docker-smoke.json.
SELECT * observed sequence preserved, no invented ORDER BY or deduplication before filtering. Historical runtime provider order not observed. Feature/IsValid/dictionary bodies recovered; stats replaced by a null-returning shim and rates/remaining tables excluded. Source content/private exports never published. No source services/VM executed or inventory/Processed writes performed.
