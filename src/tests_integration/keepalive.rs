use super::rocket;
use rocket::http::Status;
use rocket::local::asynchronous::{Client, LocalRequest, LocalResponse};

use serde_json::{Value, json};

#[rocket::async_test]
async fn keepalive() {
    let client: Client = Client::tracked(rocket()).await.unwrap();

    let req: LocalRequest = client.get("/keepalive");
    let res: LocalResponse = req.dispatch().await;
    assert_eq!(res.status(), Status::Ok);
    assert_eq!(res.into_json::<Value>().await.unwrap(), json!({ "alive": true }));
}
