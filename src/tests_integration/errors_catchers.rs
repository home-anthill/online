use super::rocket;
use rocket::http::Status;
use rocket::local::asynchronous::{Client, LocalRequest, LocalResponse};

#[rocket::async_test]
async fn error_catcher_not_found() {
    let client: Client = Client::tracked(rocket()).await.unwrap();

    let req: LocalRequest = client.get("/unknownpath");
    let res: LocalResponse = req.dispatch().await;
    assert_eq!(res.status(), Status::NotFound);
    assert_eq!(res.into_string().await.unwrap(), String::from("Not found"));
}
