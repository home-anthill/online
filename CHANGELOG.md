# Changelog

## 4.0.0

### Features

- Renamed the service, Cargo package/binary, Docker image, and repository references from `online` to `alarm-api` while preserving the existing `/online` heartbeat routes and models.
- Added a dedicated Redis DB 3 pool for alarm settings and pending-alarm token migration.
- Moved the per-feature silence endpoint to `PUT /alarms/{device_uuid}/features/{feature_uuid}/notifications`; `/online` remains reserved for heartbeat state.
- Added repository-specific `AGENTS.md` guidance for coding agents, covering the service
  architecture, Redis-backed integration test setup, security conventions, configuration, and
  CI/CD workflow.
- Added `GET /notifications/{apiToken}` to list profile notification history from the
  notifications Redis store, returning newest notifications first.
- Added a dedicated `notifications_redis_pool` and `NOTIFICATIONS_REDIS_URI` configuration for
  reading notification history independently from online-state Redis data.
- Stored `notificationSilenced` in `alarm-settings:{device_uuid}:{feature_uuid}` hashes in Redis DB 3.

### Bug fixes

- Migrated notification history during `PUT /api-token` by merging the old
  `notifications:by_api_token:<oldToken>` index into the new token index, updating referenced
  notification hashes, and deleting the old index.

### Tests

- Added `POST /fcmtoken` integration tests for invalid `apiToken` and empty `fcmToken` request
  bodies, asserting the expected `400` JSON errors.
- Added `PUT /api-token` integration tests for invalid `oldApiToken`, `newApiToken`,
  `deviceUuid`, and `featureUuid` values.
- Added `GET /online/{device_uuid}/features/{feature_uuid}` integration tests for missing Redis
  records and corrupt records with missing `apiToken` or invalid `createdAt`, asserting secure
  `404` responses.
- Added `DELETE /online/{device_uuid}/features/{feature_uuid}` coverage for idempotent deletion
  when the Redis record is already missing.
- Added a Redis test helper for inserting partial online hashes so corrupt-record scenarios can be
  covered directly.
- Added integration coverage for preserving notification history when a profile API token is
  regenerated.
- Added integration coverage for updating the per-feature notification silence flag.


## 3.0.0

### Features

- Added internal `PUT /api-token` support to move Redis online-state `apiToken` fields and
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
