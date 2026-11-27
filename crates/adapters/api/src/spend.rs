//! The adapter's settings for the core spend guard: on or off and the monthly cap.

use ads_core::spend::SpendLimit;

use crate::config::ApiConfig;

impl From<&ApiConfig> for SpendLimit {
    fn from(config: &ApiConfig) -> Self {
        Self {
            enabled: config.enabled,
            monthly_cap_micro_usd: config.monthly_cap_micro_usd,
        }
    }
}
