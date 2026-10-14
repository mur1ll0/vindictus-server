VERIFICATION REPORT
-------------------
Claim: M15.1a recovered creation codecs/template and isolated CID/SN arithmetic; no persistent creation or city acceptance claimed.
Command: generate-character-creation-wire-fixtures.ps1;generate-character-allocator-fixtures.ps1;generate-fixtures.ps1;verify.ps1 -DatabaseIntegration -DatabaseIntegrationDocker;.cache/verify-creation-primitives-docker.ps1.
Executed: full gate 2026-10-14T01:40:00.7318277+00:00; Docker/preservation 2026-10-14T01:43:47.5310834+00:00, after the last code change.
Exit code:0.
Output summary:227 Rust passed/0failed;78 PostgreSQL passed/0failed/0ignored;19 Python passed. Format/Clippy/doctests/build exit0.255 wire and851 allocator cases per architecture, x86/x64 zero differences. Docker build/migrate/readiness/account creation/duplicate/nonroot/restart PASS; original user/character/queue/lobby/HomeCafe/reservation snapshots preserved.
Warnings:none in final production gate. Basic fixture regeneration emitted CA2200 in unchanged recovered C# MessageAnalyzer.cs; log retained locally, not suppressed or counted as clean oracle build. New creation/allocator oracle builds have no warnings.
Errors:none in final gate.
Verdict:PASS for these primitives. Actual operator Check of Murael shows That name is available and can be used. Create handler/persistence/defaults/items/skills/appearance/Summary pending; selection/PlayerLoader/MMO/city/movement pending. Full M01-M24 migration remains required.
Evidence:character-creation-primitives.json;character-name-check-client.json;gate.json/docker-smoke.json and preservation evidence. Full logs and private sources/exports remain in .cache/verification or protected private cache. Originals and SQL Server read-only; local commit only; no push.