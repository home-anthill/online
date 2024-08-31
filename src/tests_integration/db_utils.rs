use log::info;
use std::env;

use mongodb::bson::DateTime;
use mongodb::bson::{doc, Document};
use mongodb::options::ClientOptions;
use mongodb::{Client, Database};

pub async fn connect() -> mongodb::error::Result<Database> {
    let mongo_uri = env::var("MONGO_URI").expect("MONGO_URI is not found.");
    let mongo_db_name = String::from("online_test");

    let mut client_options = ClientOptions::parse(mongo_uri).await?;
    client_options.app_name = Some("online-test".to_string());
    let client = Client::with_options(client_options)?;
    let database = client.database(mongo_db_name.as_str());

    info!("MongoDB testing connected!");

    Ok(database)
}

pub async fn drop_all_collections(db: &Database) {
    db.collection::<Document>("online")
        .drop()
        .await
        .expect("drop 'online' collection");
}

pub async fn find_online_by_uuid(db: &Database, uuid: &String) -> mongodb::error::Result<Option<Document>> {
    let collection = db.collection::<Document>("online");
    let filter = doc! { "uuid": uuid };
    collection.find_one(filter).await
}

pub async fn insert_online(
    db: &Database,
    uuid: &String,
    api_token: &String,
    online: bool,
) -> mongodb::error::Result<String> {
    let collection = db.collection::<Document>("online");
    let insert_one_result = collection
        .insert_one(doc! {
            "uuid": uuid,
            "apiToken": api_token,
            "createdAt": DateTime::now(),
            "modifiedAt": DateTime::now(),
            "online": online,
        })
        .await?;
    Ok(insert_one_result.inserted_id.as_object_id().unwrap().to_hex())
}
