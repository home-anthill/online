# AGENTS.md

This file provides guidance to coding agents when working with code in this repository.

## Project Overview

Rust microservice for the home-anthill smart home platform. Tracks online status, manages FCM tokens, stores per-feature alarm silence flags in Redis DB 3, and reads notification history from Redis DB 1. Built with Rocket 0.5 async web framework.

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

All tests are integration tests that require a **real Redis instance** on `localhost:6379`. There is no mocking of Redis or external services. Tests use Redis database `0` for online state, database `1` for notification history, and database `3` for alarm settings.

**Setup:**
- Copy `.env_template` to `.env` (`REDIS_URI`, `NOTIFICATIONS_REDIS_URI`, and `ALARMS_REDIS_URI` select DB 0, 1, and 3)
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

Test utilities are in `src/tests_integration/db_utils.rs` — helpers for cleanup and key isolation. Test keys use a `test_` prefix for isolation from development data. Note: test helper functions in `db_utils.rs` connect directly to `redis://localhost:6379` (no credentials) — this works because the CI Redis service has no ACL auth, and the default Redis user remains enabled alongside any named ACL users added locally.

## Architecture

**Single-crate Rust service** with these modules under `src/`:

- **routes/api.rs**, **routes/online.rs**, **routes/notification.rs**, and **routes/keepalive.rs** — All 7 REST endpoints:
  - `GET /keepalive` — health check
  - `GET /online/{device_uuid}/features/{feature_uuid}` — returns `createdAt`, `modifiedAt`, `currentTime` (intentionally omits `apiToken` from response)
  - `DELETE /online/{device_uuid}/features/{feature_uuid}` — delete online record; returns `200 {}` even if not found
  - `POST /fcmtoken` — set FCM token for devices matching an apiToken (requires `Content-Type: application/json`)
  - `PUT /alarms/{device_uuid}/features/{feature_uuid}/notifications` — store the per-feature `notificationSilenced` flag used by `alarm-notifier`
  - `PUT /api-token` — internal endpoint used by `api-server` after profile token regeneration; updates the supplied device/feature Redis hashes, moves `fcm_by_api_token` from stale token fields to the new token, and migrates notification history indexes/hashes
  - `GET /notifications/{apiToken}` — list profile notification history from the notifications Redis store, newest first
- **db/online.rs** — Online Redis operations (HGETALL, HSET, HGET, DEL, EXISTS, SCAN). Key format: `online_{device_uuid}_feature_{feature_uuid}`
- **db/notification.rs** — Notification Redis operations. Index key format: `notifications:by_api_token:{apiToken}`; notification hash key format: `notification:{id}`
- **db/alarm.rs** — Redis DB 3 alarm settings and pending-alarm token migration. Settings key format: `alarm-settings:{device_uuid}:{feature_uuid}`.
- **models/** — `Online`, `ProfileNotification`, `InitFCMTTokenInput`, `UpdateApiTokenInput`, `UpdateApiTokenDeviceFeature`, and `UpdateFeatureNotificationInput`
- **errors/** — `ApiError`/`ApiResponse` responders, `DbError` (thiserror) enums
- **catchers/** — HTTP error catchers (400, 404, 500, 503)
- **config/** — Tracing logger setup with daily-rotating file appenders (disabled when `ENV=testing`)
- **tests_integration/** — Rocket async integration tests with test-log; `db_utils.rs` has Redis test helpers

**Key dependencies:** rocket, rocket_db_pools (deadpool_redis), serde/serde_json, tracing, thiserror, uuid, subtle, urlencoding

## Key Patterns & Conventions

### Models
- Struct fields use `snake_case` (Rust convention). JSON serialisation uses `camelCase` via `#[serde(rename_all = "camelCase")]` on each struct — do not use `#[allow(non_snake_case)]` with camelCase field names.
- **`Online`, `InitFCMTTokenInput`, and `UpdateApiTokenInput` have manual `Debug` impls that print `<redacted>` for sensitive fields (`apiToken`, `fcmToken`, `oldApiToken`, `newApiToken`)** to prevent credential leakage in logs. Always add custom `Debug` impls to any new types that hold credentials.

### Route handlers
- **Path parameters for device/feature UUIDs are typed as `uuid::Uuid`** — Rocket automatically rejects non-UUID segments before the handler runs, preventing invalid data from reaching Redis.
- Handlers take `mut db: Connection<RedisPool>` and call Redis commands directly via auto-deref (no `db.clone()`). When passing the connection to a `db/` function, use `&mut db`.
- `POST /fcmtoken` validates `apiToken` by parsing it as `uuid::Uuid` (not just checking length); `fcmToken` is checked against `MAX_FCM_TOKEN_LEN = 512`.
- `POST /fcmtoken` declares `format = "json"` so Rocket enforces `Content-Type: application/json` at the framework level.
- `PUT /alarms/{device_uuid}/features/{feature_uuid}/notifications` declares `format = "json"` and accepts `UpdateFeatureNotificationInput { notification_silenced: bool }`; the DB layer stores `"true"` or `"false"` in a DB 3 alarm-settings hash.
- `PUT /api-token` validates `oldApiToken`, `newApiToken`, and each supplied `deviceFeatures[]` UUID pair. When device/features are supplied, it updates matching online hashes and deletes the stale `fcm_by_api_token` field discovered from each hash.
- `GET /notifications/{apiToken}` validates `apiToken` as a UUID, reads from `NotificationsRedisPool`, and returns `{"notifications": [...]}` in newest-first order.

### DB layer
- `find_all` is defined in `db/online.rs` but is **not called by any route handler** — routes call Redis directly via the `Connection<RedisPool>` auto-deref. `find_all` and `update_fcm_token_by_api_token` take `&mut MultiplexedConnection` directly — no internal clone. This is more efficient than cloning for every request.
- **`update_fcm_token_by_api_token` uses `subtle::ConstantTimeEq` for `apiToken` comparison** to prevent timing side-channel attacks when comparing sensitive tokens.
- `update_online_api_token` also uses `subtle::ConstantTimeEq` when comparing stored `apiToken` values.
- `update_notification_api_token` migrates notification history on profile token regeneration: it reads the old sorted-set index, updates referenced notification hashes (`apiToken` and `apiTokens` fields where present), merges old and new sorted sets with `ZUNIONSTORE ... AGGREGATE MAX`, and deletes the old index.
- `get_notifications_by_api_token` reads IDs from `notifications:by_api_token:{apiToken}` with `ZREVRANGE`, then loads `notification:{id}` hashes. Missing or malformed notification hashes are skipped rather than failing the whole request; Redis read failures return `DbScanError`.
- `get_all_keys_pattern()` returns `&'static str` (not `String`) — the function returns compile-time string literals, so heap allocation is unnecessary.
- `is_testing()` reads `ENV` once via `static IS_TESTING: OnceLock<bool>` — it is cached on first call to avoid repeated `env::var` invocations.
- Online Redis keys use the pattern `online_{device_uuid}_feature_{feature_uuid}` and store heartbeat/FCM data only. Notification preferences live exclusively in Redis DB 3 alarm-settings hashes.
- Online Redis hashes must always contain `modifiedAt`. On creation, `createdAt` and `modifiedAt` are set to the same timestamp; on update, `createdAt` is preserved and `modifiedAt` changes.
- Notification Redis uses sorted-set indexes at `notifications:by_api_token:{apiToken}` with notification IDs scored by send time. Notification hashes are stored at `notification:{id}` and include fields such as `id`, `sentAt`, `title`, `body`, `deviceCount`, `devices`, `provider`, `providerMessageId`, and token metadata.

### Logging & Security
- Production log level is `INFO`. `debug!` lines (containing device/feature UUIDs) are compiled in but filtered at runtime and never reach rolling log files.
- **Sensitive credential fields are never logged** — remove any `debug!` or `info!` calls that would print `apiToken`, `oldApiToken`, `newApiToken`, `fcmToken`, Redis URLs with passwords, or full request bodies. Use custom `Debug` impls when printing structs containing credentials.
- `GET /online/…` returns only `createdAt`, `modifiedAt`, and `currentTime` — **`apiToken` is intentionally omitted** even though it is validated as present in Redis.
- Corrupt Redis records (missing or unparseable required fields) return a generic `404` to avoid revealing whether a key exists but holds malformed data.
- Notification history responses can include notification content (`title`, `body`, and device metadata) but must not expose Redis credentials or unrelated tokens.

### Errors
- `DbError` variants: `DbNotFound`, `DbStrToNumError`, `UnknownFieldNameError`, `DbScanError`.
- `update_fcm_token_by_api_token` returns `Result<(), DbError>`; callers must handle the error (especially `DbScanError` from Redis scan failures) and return an appropriate HTTP response.
- Notification-history read and token-migration helpers also return `DbScanError` for Redis failures; route handlers map these to `500` with generic `"Database error"` JSON.

## Configuration

- **Rocket.toml** — Framework config (port, address). Debug profile listens on port 8089; release profile listens on `0.0.0.0:80`. The `databases.redis_pool.url` and `databases.notifications_redis_pool.url` entries are overridden at runtime by URLs built from env vars (see `.env_template` below). **Do NOT commit hardcoded secret keys to `Rocket.toml`** — use the `ROCKET_SECRET_KEY` env var instead. For production TLS, set Redis URIs with the `rediss://host:6380` scheme.

- **`ROCKET_SECRET_KEY`** environment variable
  - Required for the release profile (Rocket 0.5 automatically reads this)
  - Generate with: `openssl rand -base64 32`
  - For local development, Rocket auto-generates a temporary key if not set
  - In Kubernetes, injected via ConfigMap

- **rustfmt.toml** — Max width 120, 4-space indent, hard tabs disabled

- **.env_template** — Copy to `.env` on first setup; do not commit `.env` itself. Contains five variables:
  - `LOG_LEVEL=debug`
  - `REDIS_URI=redis://localhost:6379/0`
  - `NOTIFICATIONS_REDIS_URI=redis://localhost:6379/1`
  - `ALARMS_REDIS_URI=redis://localhost:6379/3`
  - `REDIS_USERNAME=redisuser`
  - `REDIS_PASSWORD=Password1!`

  At startup, `main.rs` reads these via the `Env` struct (`config/mod.rs`), builds authenticated Redis URLs (`redis://username:password@host:port`) for both Redis pools, and injects them into Rocket's figment via `figment.merge(("databases.redis_pool.url", redis_url))` and `figment.merge(("databases.notifications_redis_pool.url", notifications_redis_url))` — overriding URLs in `Rocket.toml`. If `REDIS_PASSWORD` is empty, no credentials are injected and each URI is used as-is.

## Security Considerations

This service handles sensitive credentials (`apiToken`, `fcmToken`) that are never exposed to external clients:

1. **Credentials are never returned in HTTP responses** — `GET /online/…` explicitly omits `apiToken` from the JSON response even though it is validated as present in Redis.
2. **Prevent timing side-channels** — Use `subtle::ConstantTimeEq` when comparing sensitive tokens (e.g., `apiToken`) instead of short-circuit string equality.
3. **Never log credentials** — Use custom `Debug` impls to redact sensitive fields. Do not add debug-log statements that would print full request bodies or Redis records containing credentials.
4. **Validate at boundaries** — UUID path parameters and request body tokens are validated by Rocket/Serde before the handler runs, preventing invalid input from reaching the database layer.
5. **Fail securely** — Corrupt or missing records return `404` (not found) instead of `500` (server error), preventing callers from distinguishing "record does not exist" from "record exists but is malformed".
6. **Token rotation consistency** — When `api-server` regenerates a profile token, it calls `PUT /api-token` with device/feature UUIDs so Redis plaintext token references, `fcm_by_api_token`, and notification-history indexes/hashes do not remain stale.
7. **Notification Redis separation** — Read profile notification history through `NotificationsRedisPool` and `NOTIFICATIONS_REDIS_URI`; do not mix notification-history data with online-state Redis operations unless an explicit migration requires touching both pools.
8. **Alarm Redis separation** — Read/write alarm preferences and pending alarm token references through `AlarmsRedisPool` and `ALARMS_REDIS_URI` (DB 3). DB 15 is test-only.

When modifying this service, assume all credentials are hostile and untrusted — validate early, log minimally, and fail securely.

## CI/CD

GitHub Actions workflow (`.github/workflows/docker-image.yml`):
1. **Test job** — Ubuntu + Redis 8.x service; installs grcov + cargo-audit via cargo-binstall; runs `cp .env_template .env && make test-coverage`
2. **Build job** — Multi-stage Docker build (GHA cache for layer caching), publishes to DockerHub as `ks89/alarm`

Triggers on `master`, `develop`, `ft**` branches, pull requests to `master`/`develop`, and `v*.*.*` tags. Markdown changes are ignored.

---

## Changelog & Recent Improvements

See `CHANGELOG.md` for a detailed history of project changes, including:
- **Security fixes** (April 2026) — removed credential leakage from responses and logs, added constant-time token comparison, improved validation
- **Idiomatic Rust refactors** — eliminated unnecessary allocations (`OnceLock` for cached config, `&'static str` for literal patterns), removed internal clones in DB functions, refactored models to use `snake_case` fields with serde `rename_all`
- **Redis authentication support** — switched from legacy Redis default user to named ACL user for local dev and production
- **Generic alarm support** (4.0.0) — added `AlarmsRedisPool`, Redis DB 3 alarm settings/pending-token migration, and `PUT /alarms/{device_uuid}/features/{feature_uuid}/notifications`

When adding new features or fixing bugs, follow these established patterns:
- Validate input at Rocket's framework level (e.g., `uuid::Uuid` path parameters, `format = "json"` on routes)
- Use typed wrapper types (e.g., UUIDs) instead of raw strings for sensitive identifiers
- Cache static data via `OnceLock` rather than computing on every request
- Avoid unnecessary allocations and clones — prefer borrowing and auto-deref
- Redact sensitive fields in custom `Debug` impls and log messages
- Use DB 0 for online/FCM data, DB 1 for notification history, and DB 3 for alarm settings/pending alarms
