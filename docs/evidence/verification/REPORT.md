VERIFICATION REPORT
-------------------
Claim: M13.3d.2b restricted laboratory TCP login, second password and empty lobby; actual client login/second-password/creation-screen observed; complete acceptance pending.
Command: generate-fixtures.ps1; verify.ps1 -DatabaseIntegration -DatabaseIntegrationDocker; Docker build/up; smoke-docker.ps1; preservation wrappers; Import-HomeCafeBonus.ps1 Verify; start-lab-frontend.ps1 with protected private input; loopback endpoint HTTP check.
Executed: full gate 2026-10-13T18:59:59.1376915+00:00; Docker comparison 2026-10-13T19:07:20.0220581+00:00; native launch 2026-10-13T19:07:36.5815313+00:00.
Exit code:0.
Output summary:213Rust/0failed;68PostgreSQL/0failed/0ignored;19Python; format/Clippy/doctests/build pass. Four native TCP and three isolated PostgreSQL TCP tests. Nullable password original methods x86/x64:15catch cases and18comparison cases per architecture. Native restricted listener ready; copy endpointHTTP200. Docker migration/readiness/duplicate/non-root/restart pass; source snapshots preserved; one synthetic origin after real login; imported origins0 and non-laboratory reward receipts0.
Warnings:none in final gate.
Errors:none in final gate. Earlier Clippy type complexity/argument count and missing feature-version evidence corrected; actual native launch passed afterward. Failed/incomplete runs do not count.
Verdict:PASS for stated restricted runtime scope. Character creation/selection/PlayerLoader/MMO/movement and complete failure lifecycle remain pending. No actual client compatibility claim.
Evidence:docs/lab-frontend-tcp.md; docs/evidence/auth/lab-frontend-tcp.json; lab-frontend-launch.json; gate.json/docker-smoke.json and preservation summaries. Complete logs in.cache/verification. Originals and SQLServer read-only; no private exports or credentials published; no push; full migration incomplete.
