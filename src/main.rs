#[macro_use]
extern crate rocket;

use log::info;
use rocket::{Build, Rocket};
use rocket_db_pools::Database;

use online::catchers;
use online::config::init;
use online::db::RedisPool;
use online::routes;

#[launch]
fn rocket() -> Rocket<Build> {
    // 1. Init logger
    init();

    // 2. Init Rocket
    // a) assign Database to Rocket (you can get a reference inside REST functions)
    // b) define APIs
    // c) define error handlers
    info!(target: "app", "Starting Rocket...");
    rocket::build()
        .attach(RedisPool::init())
        .mount("/", routes![routes::api::get_online, routes::api::keep_alive])
        .register(
            "/",
            catchers![
                catchers::bad_request,
                catchers::not_found,
                catchers::internal_server_error,
            ],
        )
}

// testing
#[cfg(test)]
mod tests_integration;
