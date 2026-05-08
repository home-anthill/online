# Changelog (AI-assisted changes)

## Security

### API token rotation endpoint added
Added internal `POST /api-token/rotate` support so `api-server` can move Redis online-state `apiToken`
fields and the `fcm_by_api_token` lookup to a regenerated token. The route validates both tokens and
the supplied device/feature UUIDs, avoids logging token values, and updates existing targeted online
hashes even when Redis still contains a stale token from an earlier partial rotation.

### Online Redis timestamp invariant
Online hashes now always contain `modifiedAt`. On creation, `createdAt` and `modifiedAt` are written
with the same Unix-ms timestamp; subsequent updates preserve `createdAt` and refresh `modifiedAt`.

### `apiToken` removed from GET response
The `GET /online/{device_uuid}/features/{feature_uuid}` endpoint no longer returns the `apiToken`
field in its JSON response. It was a sensitive credential that had no business being exposed to
callers. The field is still validated as present in Redis (returning 404 if missing), but is not
forwarded to the client. Integration test updated accordingly.

### Hardcoded secret keys removed from `Rocket.toml`
Both `[debug]` and `[release]` sections contained placeholder `secret_key` values committed to
source control. These have been removed. Set the `ROCKET_SECRET_KEY` environment variable instead
(Rocket 0.5 reads it automatically). Generate a value with: `openssl rand -base64 32`.

### Sensitive data removed from logs
- `device_uuid` and `feature_uuid` URL parameters no longer appear in `INFO`-level log messages
  for `get_online` and `delete_online`.
- The full Redis hash map (containing `apiToken`) is no longer debug-logged in `get_online`.
- The full `Online` struct (containing `apiToken` and `fcmToken`) is no longer debug-logged in
  `post_init_fcmtoken`.

### Custom `Debug` impls redact sensitive fields on `Online` and `InitFCMTTokenInput`
Replaced the derived `#[derive(Debug)]` on both structs with a manual `impl fmt::Debug` that prints
`<redacted>` for `apiToken` and `fcmToken`, preventing accidental credential leakage via any future
debug-log usage.

### Production log level lowered to INFO
`with_max_level` changed from `tracing::Level::DEBUG` to `tracing::Level::INFO`. `debug!` lines
that log Redis key paths (containing device/feature UUIDs) are now filtered at runtime and never
written to the rolling log files.

### UUID path parameter validation
`device_uuid` and `feature_uuid` path parameters in `GET /online/…` and `DELETE /online/…` changed
from `&str` to `uuid::Uuid`. Rocket now rejects non-UUID segments automatically before the handler
runs, preventing arbitrary strings from being embedded in Redis keys. `uuid` crate promoted from
dev-only to a regular dependency.

### Input validation for FCM token endpoint
`POST /fcmtoken` now validates `apiToken` via `Uuid::parse_str()` instead of a loose length check.
Only a valid UUID passes; the canonical string form is forwarded to the DB layer so both sides of
the constant-time comparison are always normalised 36-byte lowercase strings.
`fcmToken` is checked against `MAX_FCM_TOKEN_LEN = 512` (renamed from the previous shared
`MAX_TOKEN_LEN`). Requests failing either check return `400 Bad Request`.

### `format = "json"` added to POST route
The `POST /fcmtoken` route now declares `format = "json"`, so Rocket enforces the `Content-Type:
application/json` requirement at the framework level before the handler is invoked.

### Constant-time `apiToken` comparison
`stored_token == api_token` (short-circuit string equality) replaced with
`subtle::ConstantTimeEq::ct_eq` on the raw byte slices, eliminating a timing side-channel that
could be used to enumerate valid API tokens. `subtle = "2"` added to dependencies.

### Uniform 404 for corrupt Redis records
`GET /online/…` previously returned `500` with descriptive messages (`"ApiToken is missing in db
object"`, `"Cannot parse dates"`) when a key existed but held malformed data. This allowed callers
to distinguish "record does not exist" (404) from "record exists but is corrupt" (500). All
non-existence / corrupt-data cases now return a generic `404 Not found`.

### Full Redis scan in FCM token endpoint replaced with targeted lookup
`POST /fcmtoken` previously called `find_all()`, which loaded every Redis record (all fields) into
memory and then filtered by `apiToken` in-process. Replaced with a new
`update_fcm_token_by_api_token()` function that scans keys but issues `HGET key apiToken` per key,
only updating `fcmToken` on matching records. This avoids deserializing every `Online` struct and
reduces memory and bandwidth usage significantly for large datasets.

### `update_fcm_token_by_api_token` returns `Result<(), DbError>`
The function previously returned `()`, silently swallowing Redis scan failures and always causing
`POST /fcmtoken` to respond `200 OK` regardless of outcome. It now returns `Err(DbError::DbScanError)`
on scan failure; the route handler maps this to `500 Internal Server Error`. New `DbScanError`
variant added to `DbError`.

---

## Idiomatic Rust

### `snake_case` struct fields with `#[serde(rename_all = "camelCase")]`
`Online` and `InitFCMTTokenInput` previously used camelCase field names suppressed with
`#[allow(non_snake_case)]`. Fields renamed to `snake_case` (`api_token`, `fcm_token`,
`device_uuid`, etc.) and `#[serde(rename_all = "camelCase")]` added so the JSON wire format is
unchanged. All call sites updated.

### `get_all_keys_pattern()` returns `&'static str`
Both branches return string literals; the function was allocating a `String` via `.to_owned()` on
every call. Return type changed to `&'static str` and callers dropped the now-redundant `.as_str()`
suffix.

### `env::var` comparison no longer allocates
`env::var("ENV") != Ok("testing".to_string())` replaced with
`env::var("ENV").as_deref() != Ok("testing")`, avoiding a heap allocation for the comparison.

### `unwrap()` removed from `ApiError::respond_to`
`self.message.respond_to(req).unwrap()` replaced with `?`, propagating errors through the
`Responder` return type instead of panicking.

### Route handlers use `mut db` directly, no internal clone
All handlers previously did `let mut con = db.clone()` to obtain a mutable connection, silently
cloning a `MultiplexedConnection` on every request. Handlers now take `mut db: Connection<RedisPool>`
and call Redis methods directly via auto-deref. `find_all` and `update_fcm_token_by_api_token` in
`db/online.rs` likewise changed from `&MultiplexedConnection` (with internal clone) to
`&mut MultiplexedConnection`.

### `db_key.as_str()` replaced with `&db_key`
`.as_str()` calls on `String` values passed to Redis commands replaced with direct borrows —
`&db_key` coerces to `&str` via `Deref` automatically.

### `is_testing()` reads `ENV` once via `OnceLock`
`is_testing()` previously called `env::var("ENV")` on every invocation (called multiple times per
request). A `static IS_TESTING: OnceLock<bool>` now caches the result on first call.

### `HashMap` imported at module level
`std::collections::HashMap` was used by its full path in a function body. Moved to a module-level
`use` statement.

---

## Configuration

### Redis URL sourced from `.env` instead of `Rocket.toml`
`REDIS_URI`, `REDIS_USERNAME`, `REDIS_PASSWORD` added to `.env_template` (and `.env`). Credentials
are URL-encoded and injected into the URI at startup. `main.rs` now uses `rocket::custom(figment)`
with `figment.merge(("databases.redis_pool.url", …))` so `rocket_db_pools` picks up the
programmatically built URL; the `[default.databases.redis_pool]` section has been removed from
`Rocket.toml`. All route handlers and tests are unchanged. `urlencoding = "^2.1.3"` added to
dependencies. `redact_redis_uri` helper added to `config/mod.rs`; custom `Debug` impl updated to
redact the password field.

### Authenticated Redis URL for local development
The default Redis URL switched from `redis://localhost:6379` to
`redis://redisuser:Password1!@localhost:6379`, using a named Redis ACL user (`redisuser`) instead
of the legacy `requirepass` default-user approach. The local Docker run command uses
`--user redisuser on '>Password1!' '~*' '+@all'`. In production (Kubernetes), the URL is injected
via the Helm ConfigMap.
