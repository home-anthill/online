use uuid::Uuid;

use super::rocket;
use mongodb::Database;
use rocket::http::Status;
use rocket::local::asynchronous::{Client, LocalRequest, LocalResponse};

use serde_json::{json, Value};

use crate::tests_integration::db_utils::{connect, drop_all_collections, find_online_by_uuid, insert_online};

#[rocket::async_test]
async fn get_online() {
    // init
    let client: Client = Client::tracked(rocket()).await.unwrap();
    let db: Database = connect().await.unwrap();
    drop_all_collections(&db).await;
    // inputs
    let sensor_uuid: String = Uuid::new_v4().to_string();
    let api_token: String = Uuid::new_v4().to_string();
    // fill db with a sensor with default zero value
    let _ = insert_online(&db, &sensor_uuid, &api_token, true).await;
    // read again the sensor document, previously updated
    let document = find_online_by_uuid(&db, &sensor_uuid).await.unwrap().unwrap();
    assert_eq!(document.get("online").unwrap().as_bool().unwrap(), true);

    // read dates from db
    let created_at = document.get_datetime("createdAt").unwrap().timestamp_millis();
    let modified_at = document.get_datetime("modifiedAt").unwrap().timestamp_millis();

    // test api
    let req: LocalRequest = client.get(format!("/online/{}", sensor_uuid));
    let res: LocalResponse = req.dispatch().await;

    // check results
    assert_eq!(res.status(), Status::Ok);
    let expected = json!({
        "online": true,
        "createdAt": created_at,
        "modifiedAt": modified_at,
    });
    assert_eq!(res.into_json::<Value>().await.unwrap(), expected);

    // cleanup
    drop_all_collections(&db).await;
}
