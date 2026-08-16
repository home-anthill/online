use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OnlineStatus {
    Found,
    Missing,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OnlineBulkStatus {
    pub device_uuid: Uuid,
    pub feature_uuid: Uuid,
    pub status: OnlineStatus,
    pub created_at: Option<u128>,
    pub modified_at: Option<u128>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OnlineBulkResponse {
    pub statuses: Vec<OnlineBulkStatus>,
    pub current_time: u128,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Online {
    pub api_token: String,
    pub device_uuid: String,
    pub feature_uuid: String,
    pub fcm_token: String,
    pub created_at: String,
    pub modified_at: String,
}

impl fmt::Debug for Online {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Online")
            .field("apiToken", &"<redacted>")
            .field("deviceUuid", &self.device_uuid)
            .field("featureUuid", &self.feature_uuid)
            .field("fcmToken", &"<redacted>")
            .field("createdAt", &self.created_at)
            .field("modifiedAt", &self.modified_at)
            .finish()
    }
}
