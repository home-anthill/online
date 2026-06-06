use std::fmt;

use serde::{Deserialize, Serialize};

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
    pub device_uuid: String,
    pub feature_uuid: String,
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
