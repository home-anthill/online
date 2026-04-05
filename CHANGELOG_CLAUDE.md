# Changelog (AI-assisted changes)

## [2026-04-05] Redis config via env vars

### Redis URL sourced from `.env` instead of `Rocket.toml`
`REDIS_URI`, `REDIS_USERNAME`, `REDIS_PASSWORD` added to `.env_template` (and `.env`). Credentials
are URL-encoded and injected into the URI at startup using the same pattern as `online-alarm`.
`main.rs` now uses `rocket::custom(figment)` with `figment.merge(("databases.redis_pool.url", …))`
so `rocket_db_pools` picks up the programmatically built URL; the `[default.databases.redis_pool]`
section has been removed from `Rocket.toml`. All route handlers and tests are unchanged.
`urlencoding = "^2.1.3"` added to `Cargo.toml`. `redact_redis_uri` helper added to `config/mod.rs`
with the same logic as `online-alarm`; custom `Debug` impl updated to redact the password field.

## [2026-04-02] Security fixes

### `apiToken` removed from GET response (`src/routes/api.rs`)
The `GET /online/{device_uuid}/features/{feature_uuid}` endpoint no longer returns the `apiToken`
field in its JSON response. It was a sensitive credential that had no business being exposed to
callers. The field is still validated as present in Redis (returning 404 if missing), but is not
forwarded to the client. Integration test updated accordingly.

### Full Redis scan in FCM token endpoint replaced with targeted lookup (`src/db/online.rs`, `src/routes/api.rs`)
`POST /fcmtoken` previously called `find_all()`, which loaded every Redis record (all fields) into
memory and then filtered by `apiToken` in-process. Replaced with a new
`update_fcm_token_by_api_token()` function that scans keys but issues `HGET key apiToken` per key,
only updating `fcmToken` on matching records. This avoids deserializing every `Online` struct and
reduces memory and bandwidth usage significantly for large datasets.

### Hardcoded secret keys removed from `Rocket.toml`
Both `[debug]` and `[release]` sections contained placeholder `secret_key` values committed to
source control. These have been removed. Set the `ROCKET_SECRET_KEY` environment variable instead
(Rocket 0.5 reads it automatically). Generate a value with: `openssl rand -base64 32`.

### Sensitive data removed from logs (`src/routes/api.rs`)
- `device_uuid` and `feature_uuid` URL parameters no longer appear in `INFO`-level log messages
  for `get_online` and `delete_online`.
- The full Redis hash map (containing `apiToken`) is no longer debug-logged in `get_online`.
- The full `Online` struct (containing `apiToken` and `fcmToken`) is no longer debug-logged in
  `post_init_fcmtoken`.

### Custom `Debug` impl for `Online` redacts sensitive fields (`src/models/online.rs`)
Replaced the derived `#[derive(Debug)]` on `Online` with a manual `impl fmt::Debug` that prints
`<redacted>` for `apiToken` and `fcmToken`, preventing accidental credential leakage via any future
debug-log usage.

### Input validation for FCM token endpoint (`src/routes/api.rs`)
`POST /fcmtoken` now rejects requests where `apiToken` or `fcmToken` is empty or exceeds 512
characters, returning `400 Bad Request`.

### Uniform 404 for corrupt Redis records (`src/routes/api.rs`)
`GET /online/…` previously returned `500` with descriptive messages (`"ApiToken is missing in db
object"`, `"Cannot parse dates"`) when a key existed but held malformed data. This allowed callers
to distinguish "record does not exist" (404) from "record exists but is corrupt" (500). All
non-existence / corrupt-data cases now return a generic `404 Not found`.

### `format = "json"` added to POST route (`src/routes/api.rs`)
The `POST /fcmtoken` route now declares `format = "json"`, so Rocket enforces the `Content-Type:
application/json` requirement at the framework level before the handler is invoked.

## [2026-04-03] Security fixes

### UUID path parameter validation (`src/routes/api.rs`, `Cargo.toml`)
`device_uuid` and `feature_uuid` path parameters in `GET /online/…` and `DELETE /online/…` changed
from `&str` to `uuid::Uuid`. Rocket now rejects non-UUID segments automatically before the handler
runs, preventing arbitrary strings from being embedded in Redis keys. `uuid` crate added to
`[dependencies]` (was dev-only).

### `InitFCMTTokenInput` no longer leaks tokens via `Debug` (`src/models/inputs.rs`)
Replaced `#[derive(Debug)]` with a manual `impl fmt::Debug` that prints `<redacted>` for both
`apiToken` and `fcmToken`, matching the pattern already in place on `Online`.

### `update_fcm_token_by_api_token` returns `Result<(), DbError>` (`src/db/online.rs`, `src/routes/api.rs`)
The function previously returned `()`, silently swallowing Redis scan failures and always causing
`POST /fcmtoken` to respond `200 OK` regardless of outcome. It now returns `Err(DbError::DbScanError)`
on scan failure; the route handler maps this to `500 Internal Server Error`. New `DbScanError`
variant added to `DbError`.

### Constant-time `apiToken` comparison (`src/db/online.rs`, `Cargo.toml`)
`stored_token == api_token` (short-circuit string equality) replaced with
`subtle::ConstantTimeEq::ct_eq` on the raw byte slices, eliminating a timing side-channel that
could be used to enumerate valid API tokens. `subtle = "2"` added to `[dependencies]`.

### `apiToken` in `POST /fcmtoken` validated as UUID format (`src/routes/api.rs`)
Replaced the loose length check (`len > 512`) with `Uuid::parse_str()`. Only a valid UUID passes;
the canonical string form is forwarded to the DB layer so both sides of the constant-time comparison
are always normalised 36-byte lowercase strings.

### Separate length constant for `fcmToken` (`src/routes/api.rs`)
`MAX_TOKEN_LEN` renamed to `MAX_FCM_TOKEN_LEN` and now applies exclusively to `fcmToken`.
The `apiToken` constraint is now enforced structurally by UUID parsing rather than a shared numeric
limit.

### Production log level lowered to INFO (`src/config/mod.rs`)
`with_max_level` changed from `tracing::Level::DEBUG` to `tracing::Level::INFO`. `debug!` lines
that log Redis key paths (containing device/feature UUIDs) are now filtered at runtime and never
written to the rolling log files.

### Redis URL production warning added (`Rocket.toml`)
Comment added above the default `redis://localhost:6379` URL explaining it is for local development
only, with examples of authenticated (`redis://:PASSWORD@host:6379`) and TLS-secured
(`rediss://…`) URLs for production deployments.

## [2026-04-03] Idiomatic Rust refactors

### `snake_case` struct fields with `#[serde(rename_all = "camelCase")]` (`src/models/`)
`Online` and `InitFCMTTokenInput` previously used camelCase field names suppressed with
`#[allow(non_snake_case)]`. Fields renamed to `snake_case` (`api_token`, `fcm_token`,
`device_uuid`, etc.) and `#[serde(rename_all = "camelCase")]` added so the JSON wire format is
unchanged. All call sites updated (`db/online.rs`, `routes/api.rs`, `tests_integration/fcmtoken.rs`).

### `get_all_keys_pattern()` returns `&'static str` (`src/db/online.rs`)
Both branches return string literals; the function was allocating a `String` via `.to_owned()` on
every call. Return type changed to `&'static str` and callers dropped the now-redundant `.as_str()`
suffix.

### `env::var` comparison no longer allocates (`src/config/mod.rs`)
`env::var("ENV") != Ok("testing".to_string())` replaced with
`env::var("ENV").as_deref() != Ok("testing")`, avoiding a heap allocation for the comparison.

### `unwrap()` removed from `ApiError::respond_to` (`src/errors/api_error.rs`)
`self.message.respond_to(req).unwrap()` replaced with `?`, propagating errors through the
`Responder` return type instead of panicking.

### Route handlers use `mut db` directly, no internal clone (`src/routes/api.rs`, `src/db/online.rs`)
All handlers previously did `let mut con = db.clone()` to obtain a mutable connection, silently
cloning a `MultiplexedConnection` on every request. Handlers now take `mut db: Connection<RedisPool>`
and call Redis methods directly via auto-deref. `find_all` and `update_fcm_token_by_api_token` in
`db/online.rs` likewise changed from `&MultiplexedConnection` (with internal clone) to
`&mut MultiplexedConnection`.

### `db_key.as_str()` replaced with `&db_key` (`src/db/online.rs`)
`.as_str()` calls on `String` values passed to Redis commands replaced with direct borrows —
`&db_key` coerces to `&str` via `Deref` automatically.

### `is_testing()` reads `ENV` once via `OnceLock` (`src/db/online.rs`)
`is_testing()` previously called `env::var("ENV")` on every invocation (called multiple times per
request). A `static IS_TESTING: OnceLock<bool>` now caches the result on first call.

### `HashMap` imported at module level in `routes/api.rs`
`std::collections::HashMap` was used by its full path in a function body. Added
`use std::collections::HashMap;` at the top of the file.

## [2026-04-03] Redis authentication support

### Authenticated Redis URL in `Rocket.toml`
The default Redis URL changed from `redis://localhost:6379` to `redis://redisuser:Password1!@localhost:6379`
for local development, using a named Redis ACL user (`redisuser`) instead of the legacy `requirepass`
default-user approach. The local Docker run command uses `--user redisuser on '>Password1!' '~*' '+@all'`.
In production (Kubernetes), the URL is injected via the Helm ConfigMap in `online.yaml` using
`redis://{{ .Values.redis.username }}:{{ .Values.redis.password }}@...`.
