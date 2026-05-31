use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProfileNotification {
    pub id: String,
    pub sent_at: u64,
    pub title: String,
    pub body: String,
    pub device_count: u64,
    pub devices: Value,
    pub provider: String,
    pub provider_message_id: String,
}
