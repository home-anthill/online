use super::rocket;
use pretty_assertions::assert_eq;
use rocket::http::Status;
use rocket::local::asynchronous::{Client, LocalRequest, LocalResponse};
use rocket_db_pools::deadpool_redis::{Config, Connection, Runtime, redis::aio::MultiplexedConnection};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::tests_integration::db_utils::{delete_notifications_by_api_token, insert_notification_for_api_token};

#[rocket::async_test]
#[test_log::test]
async fn get_profile_notifications_returns_notifications_for_api_token() {
    let client: Client = Client::tracked(rocket()).await.unwrap();
    let cfg: Config = Config::from_url("redis://localhost:6379/1");
    let pool = cfg.create_pool(Some(Runtime::Tokio1)).unwrap();
    let connection: Connection = pool.get().await.unwrap();
    let con: MultiplexedConnection = connection.clone();

    let api_token = Uuid::new_v4().to_string();
    delete_notifications_by_api_token(&con, &api_token).await;

    let devices = json!([{
        "deviceUuid": Uuid::new_v4().to_string(),
        "featureUuid": Uuid::new_v4().to_string(),
        "createdAt": 1710000000001u64,
        "modifiedAt": 1710000000002u64,
    }])
    .to_string();
    insert_notification_for_api_token(
        &con,
        &api_token,
        "test-notification-a",
        1717000000000,
        "home anthill",
        "Device is offline",
        &devices,
    )
    .await;

    let req: LocalRequest = client.get(format!("/notifications/{api_token}"));
    let res: LocalResponse = req.dispatch().await;

    assert_eq!(res.status(), Status::Ok);
    assert_eq!(
        res.into_json::<Value>().await.unwrap(),
        json!({
            "notifications": [{
                "id": "test-notification-a",
                "sentAt": 1717000000000u64,
                "title": "home anthill",
                "body": "Device is offline",
                "deviceCount": 1u64,
                "devices": serde_json::from_str::<Value>(&devices).unwrap(),
                "provider": "fcm",
                "providerMessageId": "projects/home-anthill/messages/message-a",
            }]
        })
    );

    delete_notifications_by_api_token(&con, &api_token).await;
}

#[rocket::async_test]
#[test_log::test]
async fn get_profile_notifications_rejects_invalid_api_token() {
    let client: Client = Client::tracked(rocket()).await.unwrap();

    let req: LocalRequest = client.get("/notifications/not-a-uuid");
    let res: LocalResponse = req.dispatch().await;

    assert_eq!(res.status(), Status::BadRequest);
    assert_eq!(res.into_json::<Value>().await.unwrap(), json!({ "message": "Invalid apiToken", "code": 400 }));
}
