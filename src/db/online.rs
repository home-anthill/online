use log::info;

use mongodb::bson::{doc, Document};
use mongodb::Database;

use crate::errors::db_error::DbError;

pub async fn find_online_by_uuid(db: &Database, uuid: &str) -> Result<Document, DbError> {
    info!(target: "app", "find_online_by_uuid - Called with uuid = {} to get online status from db", uuid);
    let collection = db.collection::<Document>("online");

    let filter = doc! { "uuid": uuid };
    let projection = doc! {"_id": 0, "online": 1, "createdAt": 1, "modifiedAt": 1};
    match collection.find_one(filter).projection(projection).await {
        Ok(doc_result) => match doc_result {
            Some(doc) => Ok(doc),
            None => Err(DbError::new(String::from("Cannot find online"), 404)),
        },
        Err(err) => Err(DbError::new(err.to_string(), 500)),
    }
}
