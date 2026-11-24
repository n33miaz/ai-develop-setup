//! Usage record of a call: tokens, estimated cost and provider alias, never any text.

use ads_contracts::{
    CommonTimestampV1, ContractError, RecordApiUsageV1, RecordApiUsageV1CostBasis,
    RecordApiUsageV1Model, RecordApiUsageV1PriceVersion, RecordApiUsageV1Version,
};

use crate::chat::TokenUsage;
use crate::config::ProviderConfig;

/// Build the record for a finished call. With no reported usage, the whole reservation is the
/// cost (`reserved`), so a provider that omits counts cannot make calls free.
pub fn usage_record(
    provider: &ProviderConfig,
    timestamp: &str,
    usage: Option<TokenUsage>,
    reserved_micro_usd: u64,
) -> Result<RecordApiUsageV1, ContractError> {
    let (tokens_in, tokens_out, cost, basis) = match usage {
        Some(u) => (
            u.tokens_in,
            u.tokens_out,
            provider.cost_micro_usd(u.tokens_in, u.tokens_out),
            RecordApiUsageV1CostBasis::Reported,
        ),
        None => (
            0,
            0,
            reserved_micro_usd,
            RecordApiUsageV1CostBasis::Reserved,
        ),
    };
    Ok(RecordApiUsageV1 {
        version: RecordApiUsageV1Version,
        ts: CommonTimestampV1::new(timestamp)?,
        provider_alias: provider.alias.clone(),
        model: RecordApiUsageV1Model::new(provider.model.clone())?,
        tokens_in,
        tokens_out,
        cost_estimate_micro_usd: cost,
        cost_basis: basis,
        price_version: RecordApiUsageV1PriceVersion::new(provider.price_version.clone())?,
    })
}
