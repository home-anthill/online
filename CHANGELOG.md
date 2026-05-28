# Changelog

## 3.0.0

### Features

- Added internal `POST /api-token/rotate` support to move Redis online-state `apiToken` fields and
  the `fcm_by_api_token` lookup to a regenerated token, validating both tokens and the supplied
  device/feature UUIDs.
- Ensured online Redis hashes always contain `modifiedAt`; creation writes matching `createdAt`
  and `modifiedAt` timestamps, while later updates preserve `createdAt` and refresh `modifiedAt`.

### Bug fixes

- Changed `update_fcm_token_by_api_token()` to return `Result<(), DbError>` instead of silently
  swallowing Redis scan failures; scan errors now map to `500 Internal Server Error`.

### Security issues

- Removed `apiToken` from the `GET /online/{device_uuid}/features/{feature_uuid}` JSON response
  while still validating that it exists in Redis.
- Removed committed placeholder `secret_key` values from `Rocket.toml`; `ROCKET_SECRET_KEY` must be
  provided via the environment.
- Removed sensitive `device_uuid`, `feature_uuid`, Redis hash maps, `Online`, and
  `InitFCMTTokenInput` data from logs.
- Added manual `Debug` implementations for `Online` and `InitFCMTTokenInput` that redact
  `apiToken` and `fcmToken`.
- Lowered production logging from `DEBUG` to `INFO`, filtering debug logs that include Redis key
  paths.
- Changed `device_uuid` and `feature_uuid` route parameters to `uuid::Uuid` so Rocket rejects
  invalid UUID path segments before handlers run.
- Validated `POST /fcmtoken` input more strictly: `apiToken` must parse as a UUID and `fcmToken`
  must fit `MAX_FCM_TOKEN_LEN = 512`.
- Added `format = "json"` to the `POST /fcmtoken` route so Rocket enforces
  `Content-Type: application/json`.
- Replaced short-circuit `apiToken` equality with `subtle::ConstantTimeEq::ct_eq` to avoid token
  timing side-channels.
- Changed corrupt Redis records in `GET /online/...` to return the same generic `404 Not found` as
  missing records.
- Replaced full Redis record loading in `POST /fcmtoken` with targeted `HGET key apiToken` lookups
  before updating matching `fcmToken` values.

### Idiomatic Rust issues

- Renamed `Online` and `InitFCMTTokenInput` fields to `snake_case` and preserved camelCase JSON via
  `#[serde(rename_all = "camelCase")]`.
- Changed `get_all_keys_pattern()` to return `&'static str` instead of allocating a `String`.
- Replaced `env::var("ENV") != Ok("testing".to_string())` with
  `env::var("ENV").as_deref() != Ok("testing")`.
- Replaced `unwrap()` in `ApiError::respond_to` with `?` so responder errors propagate instead of
  panicking.
- Updated route handlers and Redis helpers to use mutable Redis connections directly instead of
  cloning `MultiplexedConnection`.
- Replaced `db_key.as_str()` calls with direct `&db_key` borrows.
- Cached `is_testing()` with `OnceLock` so `ENV` is read once.
- Moved `HashMap` to a module-level import.

### Chores

- Moved Redis connection configuration from `Rocket.toml` to `.env` variables, building the Redis
  URL at startup and injecting it into Rocket figment configuration.
- Added Redis URI redaction helpers and updated config debug output to redact passwords.
- Added `urlencoding = "^2.1.3"` for Redis credential encoding.
- Switched local development Redis defaults to an authenticated ACL user URL:
  `redis://redisuser:Password1!@localhost:6379`.
- Documented the local Docker Redis ACL user command and kept production Redis URL injection via
  the Helm ConfigMap.

### Tests

- Updated integration coverage for `GET /online/{device_uuid}/features/{feature_uuid}` so tests no
  longer expect the sensitive `apiToken` field in the response body.
