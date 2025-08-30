use std::time::{SystemTime, UNIX_EPOCH};

use super::rocket;
use rocket::http::Status;
use rocket::local::asynchronous::{Client, LocalRequest, LocalResponse};
use rocket_db_pools::deadpool_redis::{Config, Connection, Runtime, redis::aio::MultiplexedConnection};
use uuid::Uuid;

use crate::tests_integration::db_utils::{drop_all_test_keys, get_fcmtoken_by_uuid, insert_online};
use online::models::inputs::InitFCMTTokenInput;

#[rocket::async_test]
#[test_log::test]
async fn post_fcmtoken() {
    // init
    let client: Client = Client::tracked(rocket()).await.unwrap();
    let cfg: Config = Config::from_url("redis://localhost:6379");
    let pool = cfg.create_pool(Some(Runtime::Tokio1)).unwrap();
    let connection: Connection = pool.get().await.unwrap();
    let con: MultiplexedConnection = connection.clone();

    // cleanup
    drop_all_test_keys(&con).await;

    // inputs
    let uuid: String = Uuid::new_v4().to_string();
    let db_key: String = "test-".to_owned() + &uuid;
    let api_token: String = Uuid::new_v4().to_string();
    let date: u128 = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
    // insert in db
    insert_online(&con, &db_key, &api_token, date).await;

    // test api
    let fcm_token: String = "mocked_fcm_token".to_owned();
    let body = InitFCMTTokenInput {
        apiToken: api_token.clone(),
        fcmToken: fcm_token.clone(),
    };
    let req: LocalRequest = client.post("/fcmtoken").json(&body);
    let res: LocalResponse = req.dispatch().await;

    // check response status
    assert_eq!(res.status(), Status::Ok);

    // read from db to check if fcmToken has been set
    let fcm_token_db: String = get_fcmtoken_by_uuid(&con, &db_key).await;
    assert_eq!(fcm_token_db, fcm_token);

    // cleanup
    drop_all_test_keys(&con).await;
}
