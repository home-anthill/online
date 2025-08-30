use std::time::{SystemTime, UNIX_EPOCH};

use super::rocket;
use rocket::http::Status;
use rocket::local::asynchronous::{Client, LocalRequest, LocalResponse};
use rocket_db_pools::deadpool_redis::{Config, Connection, Runtime, redis::aio::MultiplexedConnection};
use serde_json::{Map, Value};
use uuid::Uuid;

use crate::tests_integration::db_utils::{drop_all_test_keys, insert_online};

#[rocket::async_test]
#[test_log::test]
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

#[rocket::async_test]
#[test_log::test]
async fn delete_online() {
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
    // verify the inserted online
    let get_req: LocalRequest = client.get(format!("/online/{}", &uuid));
    let get_res: LocalResponse = get_req.dispatch().await;
    assert_eq!(get_res.status(), Status::Ok);

    // test api
    let req: LocalRequest = client.delete(format!("/online/{}", &uuid));
    let res: LocalResponse = req.dispatch().await;
    assert_eq!(res.status(), Status::Ok);

    // verify that online has been removed
    let get_req2: LocalRequest = client.get(format!("/online/{}", &uuid));
    let get_res2: LocalResponse = get_req2.dispatch().await;
    assert_eq!(get_res2.status(), Status::NotFound);

    // cleanup
    drop_all_test_keys(&con).await;
}
