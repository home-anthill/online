use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OnlineBulkDeviceFeatureInput {
    pub device_uuid: Uuid,
    pub feature_uuid: Uuid,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OnlineBulkInput {
    pub device_features: Vec<OnlineBulkDeviceFeatureInput>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InitFCMTTokenInput {
    pub api_token: String,
    pub fcm_token: String,
}

impl fmt::Debug for InitFCMTTokenInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InitFCMTTokenInput").field("apiToken", &"<redacted>").field("fcmToken", &"<redacted>").finish()
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateApiTokenDeviceFeature {
    pub device_uuid: Uuid,
    pub feature_uuid: Uuid,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateApiTokenInput {
    pub old_api_token: String,
    pub new_api_token: String,
    pub device_features: Vec<UpdateApiTokenDeviceFeature>,
}

impl fmt::Debug for UpdateApiTokenInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UpdateApiTokenInput")
            .field("oldApiToken", &"<redacted>")
            .field("newApiToken", &"<redacted>")
            .finish()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateFeatureNotificationInput {
    pub notification_silenced: bool,
}
