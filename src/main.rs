#[macro_use]
extern crate rocket;

use rocket::{Build, Rocket};
use rocket_db_pools::Database;
use tracing::info;

use online::catchers;
use online::config::init;
use online::db::RedisPool;
use online::routes;

#[rocket::launch]
fn rocket() -> Rocket<Build> {
    // 1. Init logger and env
    init();

    // 2. Init Rocket
    // a) assign Database to Rocket (you can get a reference inside REST functions)
    // b) define APIs
    // c) define error handlers
    info!(target: "app", "Starting Rocket...");
    rocket::build()
        .attach(RedisPool::init())
        .mount(
            "/",
            routes![
                routes::api::get_online,
                routes::api::delete_online,
                routes::api::post_init_fcmtoken,
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
