use std::time::{SystemTime, UNIX_EPOCH};

use super::rocket;
use rocket::http::Status;
use rocket::local::asynchronous::{Client, LocalRequest, LocalResponse};
use rocket_db_pools::deadpool_redis::{redis::aio::MultiplexedConnection, Config, Connection, Runtime};
use serde_json::{Map, Value};
use uuid::Uuid;

use crate::tests_integration::db_utils::{drop_all_test_keys, insert_online};

#[rocket::async_test]
async fn get_online() {
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
    let req: LocalRequest = client.get(format!("/online/{}", &uuid));
    let res: LocalResponse = req.dispatch().await;

    // check response status
    assert_eq!(res.status(), Status::Ok);
    // check response
    let json_val: Value = res.into_json::<Value>().await.unwrap();
    let result: &Map<String, Value> = json_val.as_object().unwrap();
    assert_eq!(result.get("apiToken").unwrap(), api_token.as_str());
    assert_eq!(
        result.get("createdAt").unwrap().to_string().as_str(),
        date.to_string().as_str()
    );

    // cleanup
    drop_all_test_keys(&con).await;
}
