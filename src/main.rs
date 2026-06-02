#[macro_use]
extern crate rocket;

use rocket::{Build, Rocket};
use rocket_db_pools::Database;
use tracing::{info, warn};
use urlencoding::encode;

use online::catchers;
use online::config::{init, redact_redis_uri};
use online::db::{NotificationsRedisPool, RedisPool};
use online::routes;

fn redis_url_with_optional_credentials(redis_uri: &str, redis_username: &str, redis_password: &str) -> String {
    if redis_password.is_empty() {
        redis_uri.to_string()
    } else {
        match redis_uri.find("://") {
            Some(scheme_end) => format!(
                "{scheme}{username}:{password}@{rest}",
                scheme = &redis_uri[..scheme_end + 3],
                username = encode(redis_username),
                password = encode(redis_password),
                rest = &redis_uri[scheme_end + 3..],
            ),
            None => {
                warn!(target: "app", "Redis URI has no recognizable scheme (missing '://'), skipping credential injection");
                redis_uri.to_string()
            }
        }
    }
}

#[rocket::launch]
fn rocket() -> Rocket<Build> {
    // 1. Init logger and env
    let (env, _app_env) = init();

    // 2. Build the authenticated Redis URL from env vars.
    // If credentials are configured, inject them into the URI:
    //   redis://host:port -> redis://username:password@host:port
    if !env.redis_username.is_empty() && env.redis_password.is_empty() {
        warn!(target: "app", "REDIS_USERNAME is set but REDIS_PASSWORD is empty — no authentication will be attempted");
    }
    let redis_url = redis_url_with_optional_credentials(&env.redis_uri, &env.redis_username, &env.redis_password);
    let notifications_redis_url =
        redis_url_with_optional_credentials(&env.notifications_redis_uri, &env.redis_username, &env.redis_password);
    info!(target: "app", "Redis URL (redacted) = {}", redact_redis_uri(&redis_url));
    info!(target: "app", "Notifications Redis URL (redacted) = {}", redact_redis_uri(&notifications_redis_url));

    // 3. Inject the Redis URL into Rocket's figment so rocket_db_pools picks it up,
    //    overriding any value in Rocket.toml.
    let figment = rocket::Config::figment()
        .merge(("databases.redis_pool.url", redis_url))
        .merge(("databases.notifications_redis_pool.url", notifications_redis_url));

    // 4. Init Rocket
    // a) assign Database to Rocket (you can get a reference inside REST functions)
    // b) define APIs
    // c) define error handlers
    info!(target: "app", "Starting Rocket...");
    rocket::custom(figment)
        .attach(RedisPool::init())
        .attach(NotificationsRedisPool::init())
        .mount(
            "/",
            routes![
                routes::online::get_online,
                routes::online::delete_online,
                routes::api::post_init_fcmtoken,
                routes::api::post_rotate_api_token,
                routes::api::put_feature_notification,
                routes::keepalive::keep_alive,
                routes::notification::get_profile_notifications
            ],
        )
        .register(
            "/",
            catchers![
                catchers::bad_request,
                catchers::not_found,
                catchers::internal_server_error,
                catchers::service_unavailable,
            ],
        )
}

// testing
#[cfg(test)]
mod tests_integration;
