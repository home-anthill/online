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
