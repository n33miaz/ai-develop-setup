//! The adapter's settings for the core spend guard: on or off and the monthly cap.

use ads_core::spend::SpendLimit;

use crate::config::ApiConfig;

impl From<&ApiConfig> for SpendLimit {
    fn from(config: &ApiConfig) -> Self {
        config.spend_limit
    }
}
