//! Usage record of a call: tokens, estimated cost, provider alias and outcome, never any text.

use ads_contracts::{
    CommonTimestampV1, ContractError, RecordApiUsageV2, RecordApiUsageV2CostBasis,
    RecordApiUsageV2Fields, RecordApiUsageV2Model, RecordApiUsageV2Outcome,
    RecordApiUsageV2PriceVersion, RecordApiUsageV2Version,
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
    outcome: RecordApiUsageV2Outcome,
) -> Result<RecordApiUsageV2, ContractError> {
    let mut record = RecordApiUsageV2Fields {
        version: RecordApiUsageV2Version,
        ts: CommonTimestampV1::new(timestamp)?,
        provider_alias: provider.alias.clone(),
        model: RecordApiUsageV2Model::new(provider.model.clone())?,
        tokens_in: 0,
        tokens_out: 0,
        cost_estimate_micro_usd: reserved_micro_usd,
        cost_basis: RecordApiUsageV2CostBasis::Reserved,
        price_version: RecordApiUsageV2PriceVersion::new(provider.price_version.clone())?,
        outcome,
    };
    if let Some(usage) = usage {
        report_usage(&mut record, provider, usage);
    }
    RecordApiUsageV2::new(record)
}

pub(crate) fn report_usage(
    record: &mut RecordApiUsageV2Fields,
    provider: &ProviderConfig,
    usage: TokenUsage,
) {
    record.tokens_in = usage.tokens_in;
    record.tokens_out = usage.tokens_out;
    record.cost_estimate_micro_usd = provider.cost_micro_usd(usage.tokens_in, usage.tokens_out);
    record.cost_basis = RecordApiUsageV2CostBasis::Reported;
}
