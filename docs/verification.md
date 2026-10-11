# Verification scope

The Rust gate is `pwsh -File scripts/dev/verify.ps1 -DatabaseIntegration` with TEST_DATABASE_URL set privately. It checks rustfmt, Clippy with warnings denied, all targets, doctests, debug build and the explicitly ignored PostgreSQL integration. The integration creates an isolated schema and tests fresh-schema failure, idempotent migrations, readiness, Argon2 verification, case-insensitive duplicate rejection and checksum incompatibility. Running the default gate leaves database integration unverified.

Docker is checked with `docker compose build server` followed by `pwsh -File scripts/dev/smoke-docker.ps1`. The smoke starts only the PostgreSQL/migrate/server dependencies, verifies readiness and capability flags, creates an account using stdin, rejects a duplicate, checks non-root/non-superuser settings, restarts PostgreSQL and server, proves the same account UUID remains and reruns migrations idempotently. It preserves that smoke account and all volumes. It does not execute a game client or prove protocol compatibility.

`docker-smoke.json` records fresh evidence and the source/config SHA256 values observed during verification. If source changes after a build, rebuild and rerun before citing the evidence for the new code. Credentials and password hashes are excluded from this evidence.

Memory M06/M07 may be marked complete only after the final code state passes both gates. The wider migration remains incomplete until its independent game-client acceptance criteria pass.
