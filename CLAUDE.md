# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Rust microservice for the home-anthill smart home platform. Tracks online status of devices/features in Redis and manages FCM (Firebase Cloud Messaging) tokens. Built with Rocket 0.5 async web framework.

## Build & Development Commands

All commands are in the Makefile:

```bash
make build          # fmt + lint + cargo build (default target)
make release        # optimized release build (fat LTO, opt-level 3)
make run            # watch mode with hot reload (cargo-watch)
make test           # integration tests (ENV=testing, single-threaded)
make test-coverage  # integration tests + grcov HTML coverage report in coverage/html/
make fmt            # rustfmt
make lint           # clippy
make check          # cargo audit (vulnerability scan)
make doc            # rustdoc
make deps           # install all dev dependencies (cargo-watch, grcov, cargo-audit, clippy, rustfmt)
make clean          # remove build artifacts
```

### Integration Tests

All tests are integration tests that require a **real Redis instance** on `localhost:6379`. There is no mocking of Redis or external services.

**Setup:**
- Copy `.env_template` to `.env` (only `LOG_LEVEL=debug` is needed for tests)
- Start Redis with ACL authentication:
  ```bash
  docker run --name redis -p 6379:6379 -d redis redis-server \
    --save 60 1 \
    --loglevel warning \
    --user redisuser on '>Password1!' '~*' '+@all'
  ```

**Running tests:**
- All tests: `make test` (single-threaded, full backtraces, console output)
- Single test: `ENV=testing RUST_BACKTRACE=full cargo test <test_name> -- --nocapture --test-threads 1`
- With coverage: `make test-coverage` (generates HTML report in `coverage/html/`)

Test utilities are in `src/tests_integration/db_utils.rs` — helpers for cleanup and key isolation. Test keys use a `test_` prefix for isolation from development data.

## Architecture

**Single-crate Rust service** with these modules under `src/`:

- **routes/api.rs** — All 4 REST endpoints:
  - `GET /keepalive` — health check
  - `GET /online/{device_uuid}/features/{feature_uuid}` — returns `createdAt`, `modifiedAt`, `currentTime` (intentionally omits `apiToken` from response)
  - `DELETE /online/{device_uuid}/features/{feature_uuid}` — delete online record; returns `200 {}` even if not found
  - `POST /fcmtoken` — set FCM token for devices matching an apiToken (requires `Content-Type: application/json`)
- **db/online.rs** — Redis operations (HGETALL, HSET, HGET, DEL, EXISTS, SCAN). Key format: `online_{device_uuid}_feature_{feature_uuid}`
- **models/** — `Online` (6-field struct) and `InitFCMTTokenInput` (request body for fcmtoken)
- **errors/** — `ApiError`/`ApiResponse` responders, `DbError` (thiserror) enums
- **catchers/** — HTTP error catchers (400, 404, 500, 503)
- **config/** — Tracing logger setup with daily-rotating file appenders (disabled when `ENV=testing`)
- **tests_integration/** — Rocket async integration tests with test-log; `db_utils.rs` has Redis test helpers

**Key dependencies:** rocket, rocket_db_pools (deadpool_redis), serde/serde_json, tracing, thiserror, uuid, subtle

## Key Patterns & Conventions

### Models
- Struct fields use `snake_case` (Rust convention). JSON serialisation uses `camelCase` via `#[serde(rename_all = "camelCase")]` on each struct — do not use `#[allow(non_snake_case)]` with camelCase field names.
- **Both `Online` and `InitFCMTTokenInput` have manual `Debug` impls that print `<redacted>` for sensitive fields (`apiToken`, `fcmToken`)** to prevent credential leakage in logs. Always add custom `Debug` impls to any new types that hold credentials.

### Route handlers
- **Path parameters for device/feature UUIDs are typed as `uuid::Uuid`** — Rocket automatically rejects non-UUID segments before the handler runs, preventing invalid data from reaching Redis.
- Handlers take `mut db: Connection<RedisPool>` and call Redis commands directly via auto-deref (no `db.clone()`). When passing the connection to a `db/` function, use `&mut db`.
- `POST /fcmtoken` validates `apiToken` by parsing it as `uuid::Uuid` (not just checking length); `fcmToken` is checked against `MAX_FCM_TOKEN_LEN = 512`.
- `POST /fcmtoken` declares `format = "json"` so Rocket enforces `Content-Type: application/json` at the framework level.

### DB layer
- `find_all` and `update_fcm_token_by_api_token` take `&mut MultiplexedConnection` directly — no internal clone. This is more efficient than cloning for every request.
- **`update_fcm_token_by_api_token` uses `subtle::ConstantTimeEq` for `apiToken` comparison** to prevent timing side-channel attacks when comparing sensitive tokens.
- `get_all_keys_pattern()` returns `&'static str` (not `String`) — the function returns compile-time string literals, so heap allocation is unnecessary.
- `is_testing()` reads `ENV` once via `static IS_TESTING: OnceLock<bool>` — it is cached on first call to avoid repeated `env::var` invocations.
- Redis keys use the pattern `online_{device_uuid}_feature_{feature_uuid}` and store data as Redis hashes containing fields: `apiToken`, `deviceUuid`, `featureUuid`, `fcmToken`, `createdAt`, `modifiedAt`.

### Logging & Security
- Production log level is `INFO`. `debug!` lines (containing device/feature UUIDs) are compiled in but filtered at runtime and never reach rolling log files.
- **Sensitive credential fields are never logged** — remove any `debug!` or `info!` calls that would print `apiToken`, `fcmToken`, or full request bodies. Use custom `Debug` impls when printing structs containing credentials.
- `GET /online/…` returns only `createdAt`, `modifiedAt`, and `currentTime` — **`apiToken` is intentionally omitted** even though it is validated as present in Redis.
- Corrupt Redis records (missing or unparseable required fields) return a generic `404` to avoid revealing whether a key exists but holds malformed data.

### Errors
- `DbError` variants: `DbNotFound`, `DbStrToNumError`, `UnknownFieldNameError`, `DbScanError`.
- `update_fcm_token_by_api_token` returns `Result<(), DbError>`; callers must handle the error (especially `DbScanError` from Redis scan failures) and return an appropriate HTTP response.

## Configuration

- **Rocket.toml** — Framework config (port, address, Redis URL per profile).
  - Local dev Redis URL uses ACL authentication: `redis://redisuser:Password1!@localhost:6379`
  - In production (Kubernetes), the URL is injected via the Helm ConfigMap
  - For TLS in production, use `rediss://username:password@host:6380`
  - Debug profile listens on port 8089; release profile listens on 0.0.0.0:80
  - **Do NOT commit hardcoded secret keys to `Rocket.toml`** — use the `ROCKET_SECRET_KEY` environment variable instead

- **`ROCKET_SECRET_KEY`** environment variable
  - Required for the release profile (Rocket 0.5 automatically reads this)
  - Generate with: `openssl rand -base64 32`
  - For local development, Rocket auto-generates a temporary key if not set
  - In Kubernetes, injected via ConfigMap

- **rustfmt.toml** — Max width 120, 4-space indent, hard tabs disabled

- **.env_template** — Contains `LOG_LEVEL=debug` for local development. Copy to `.env` on first setup; do not commit `.env` itself (contains secrets in production).

## Security Considerations

This service handles sensitive credentials (`apiToken`, `fcmToken`) that are never exposed to external clients:

1. **Credentials are never returned in HTTP responses** — `GET /online/…` explicitly omits `apiToken` from the JSON response even though it is validated as present in Redis.
2. **Prevent timing side-channels** — Use `subtle::ConstantTimeEq` when comparing sensitive tokens (e.g., `apiToken`) instead of short-circuit string equality.
3. **Never log credentials** — Use custom `Debug` impls to redact sensitive fields. Do not add debug-log statements that would print full request bodies or Redis records containing credentials.
4. **Validate at boundaries** — UUID path parameters and request body tokens are validated by Rocket/Serde before the handler runs, preventing invalid input from reaching the database layer.
5. **Fail securely** — Corrupt or missing records return `404` (not found) instead of `500` (server error), preventing callers from distinguishing "record does not exist" from "record exists but is malformed".

When modifying this service, assume all credentials are hostile and untrusted — validate early, log minimally, and fail securely.

## CI/CD

GitHub Actions workflow (`.github/workflows/docker-image.yml`):
1. **Test job** — Ubuntu + Redis 8.x service; installs grcov + cargo-audit via cargo-binstall; runs `cp .env_template .env && make test-coverage`
2. **Build job** — Multi-stage Docker build (GHA cache for layer caching), publishes to DockerHub as `ks89/online`

Triggers on `master`, `develop`, `ft**` branches, pull requests to `master`/`develop`, and `v*.*.*` tags. Markdown changes are ignored.

---

## Changelog & Recent Improvements

See `CHANGELOG_CLAUDE.md` for a detailed history of AI-assisted improvements, including:
- **Security fixes** (April 2026) — removed credential leakage from responses and logs, added constant-time token comparison, improved validation
- **Idiomatic Rust refactors** — eliminated unnecessary allocations (`OnceLock` for cached config, `&'static str` for literal patterns), removed internal clones in DB functions, refactored models to use `snake_case` fields with serde `rename_all`
- **Redis authentication support** — switched from legacy Redis default user to named ACL user for local dev and production

When adding new features or fixing bugs, follow these established patterns:
- Validate input at Rocket's framework level (e.g., `uuid::Uuid` path parameters, `format = "json"` on routes)
- Use typed wrapper types (e.g., UUIDs) instead of raw strings for sensitive identifiers
- Cache static data via `OnceLock` rather than computing on every request
- Avoid unnecessary allocations and clones — prefer borrowing and auto-deref
- Redact sensitive fields in custom `Debug` impls and log messages
