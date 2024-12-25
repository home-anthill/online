use serde::{Deserialize, Serialize};

#[allow(non_snake_case)]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InitFCMTTokenInput {
    pub apiToken: String,
    pub fcmToken: String,
}
