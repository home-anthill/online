#[macro_use]
extern crate rocket;

use rocket::{Build, Rocket};
use rocket_db_pools::Database;
use tracing::{info, warn};
use urlencoding::encode;

use online::catchers;
use online::config::{init, redact_redis_uri};
use online::db::RedisPool;
use online::routes;

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
    let redis_url = if env.redis_password.is_empty() {
        env.redis_uri.clone()
    } else {
        match env.redis_uri.find("://") {
            Some(scheme_end) => format!(
                "{scheme}{username}:{password}@{rest}",
                scheme = &env.redis_uri[..scheme_end + 3],
                username = encode(&env.redis_username),
                password = encode(&env.redis_password),
                rest = &env.redis_uri[scheme_end + 3..],
            ),
            None => {
                warn!(target: "app", "REDIS_URI has no recognizable scheme (missing '://'), skipping credential injection");
                env.redis_uri.clone()
            }
        }
    };
    info!(target: "app", "Redis URL (redacted) = {}", redact_redis_uri(&redis_url));

    // 3. Inject the Redis URL into Rocket's figment so rocket_db_pools picks it up,
    //    overriding any value in Rocket.toml.
    let figment = rocket::Config::figment().merge(("databases.redis_pool.url", redis_url));

    // 4. Init Rocket
    // a) assign Database to Rocket (you can get a reference inside REST functions)
    // b) define APIs
    // c) define error handlers
    info!(target: "app", "Starting Rocket...");
    rocket::custom(figment)
        .attach(RedisPool::init())
        .mount(
            "/",
            routes![
                routes::api::get_online,
                routes::api::delete_online,
                routes::api::post_init_fcmtoken,
                routes::api::post_rotate_api_token,
                routes::api::keep_alive
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
