use rocket::serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct DbError {
    pub message: String,
    pub code: u16,
}

impl DbError {
    pub fn new(message: String, code: u16) -> Self {
        Self { message, code }
    }
}
