VERIFICATION REPORT
-------------------
Claim: M13.3d.2a.2 bounded protected laboratory configuration, stable provenance context and explicit Framework-compatible clock; no live TCP/client claim.
Command: generate-lab-clock-fixtures.ps1; prepare-lab-frontend-profile.ps1 with bounded private stdin; inspect-lab-frontend-profile on Windows and Linux; verify.ps1 -DatabaseIntegration -DatabaseIntegrationDocker; Docker build/up; smoke-docker.ps1; reward/lobby preservation wrappers; Import-HomeCafeBonus.ps1 Verify.
Executed: full gate 2026-10-13T18:12:09.5469394+00:00; profile inspection 2026-10-13T18:13:46.6818951+00:00; Docker comparison 2026-10-13T18:14:33.9362553+00:00.
Exit code:0.
Output summary:208Rust/0failed;65PostgreSQL/0failed/0ignored;19Python; format/Clippy/doctests/build pass;140clock cases x2architectures/0differences; one protected synthetic passport validated native/Docker; no DB/listener opened by inspector. Docker migration/readiness/duplicate/non-root/restart pass; original snapshots unchanged; origins0.
Warnings:none in final verification.
Errors:none in final verification. Intermediate Clippy constant chunks and PowerShell CurrentVer selector errors corrected; final gate and actual preparation/inspection passed. Incomplete/failed runs are not counted.
Verdict:PASS for configuration/clock/preservation only. TCP authentication, creation, selection and town/movement require implementation and actual client execution.
Evidence:docs/lab-frontend-runtime-profile.md; docs/evidence/auth/lab-frontend-runtime-profile.json; lab-runtime-preparation.json; gate.json/docker-smoke.json and preservation summaries. Full logs in.cache/verification; originals/SQL Server read-only; no private exports/credentials published; no push; full migration incomplete.
