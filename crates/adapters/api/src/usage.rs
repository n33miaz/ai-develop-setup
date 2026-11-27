//! Usage record of a call: tokens, estimated cost, provider alias and outcome, never any text.

use ads_contracts::{
    CommonTimestampV1, ContractError, RecordApiUsageV3, RecordApiUsageV3CostBasis,
    RecordApiUsageV3Fields, RecordApiUsageV3Model, RecordApiUsageV3Outcome,
    RecordApiUsageV3PriceVersion, RecordApiUsageV3SafeInteger, RecordApiUsageV3Version,
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
    outcome: RecordApiUsageV3Outcome,
) -> Result<RecordApiUsageV3, ContractError> {
    let zero = RecordApiUsageV3SafeInteger::new(0)?;
    let mut record = RecordApiUsageV3Fields {
        version: RecordApiUsageV3Version,
        ts: CommonTimestampV1::new(timestamp)?,
        provider_alias: provider.alias().clone(),
        model: RecordApiUsageV3Model::new(provider.model.clone())?,
        tokens_in: zero,
        tokens_out: zero,
        cost_estimate_micro_usd: RecordApiUsageV3SafeInteger::new(reserved_micro_usd)?,
        cost_basis: RecordApiUsageV3CostBasis::Reserved,
        price_version: RecordApiUsageV3PriceVersion::new(provider.price_version.clone())?,
        outcome,
    };
    if let Some(usage) = usage {
        report_usage(&mut record, provider, usage)?;
    }
    RecordApiUsageV3::new(record)
}

pub(crate) fn report_usage(
    record: &mut RecordApiUsageV3Fields,
    provider: &ProviderConfig,
    usage: TokenUsage,
) -> Result<(), ContractError> {
    let tokens_in = RecordApiUsageV3SafeInteger::new(usage.tokens_in)?;
    let tokens_out = RecordApiUsageV3SafeInteger::new(usage.tokens_out)?;
    let cost = RecordApiUsageV3SafeInteger::new(
        provider.cost_micro_usd(usage.tokens_in, usage.tokens_out),
    )?;

    record.tokens_in = tokens_in;
    record.tokens_out = tokens_out;
    record.cost_estimate_micro_usd = cost;
    record.cost_basis = RecordApiUsageV3CostBasis::Reported;
    Ok(())
}
