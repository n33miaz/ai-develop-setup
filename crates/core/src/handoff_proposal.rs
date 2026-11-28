//! Build the proposal of a handoff by context limit.

use std::error::Error;
use std::fmt;
use std::num::NonZeroU64;

use ads_contracts::{
    CommonIdV1, ContractError, EventSessionHandoffProposedV2Data,
    EventSessionHandoffProposedV2DataContextPct as ContextPct,
    EventSessionHandoffProposedV2DataFields, EventSessionHandoffProposedV2DataNumber as Number,
    EventSessionHandoffProposedV2DataThresholdPct as ThresholdPct,
    RecordSessionHandoffV2Trigger as Trigger,
};

use crate::handoff::{Consent, ContextUsage, Decision, HandoffPolicy};

const HUNDRED: NonZeroU64 = NonZeroU64::new(100).unwrap();

#[derive(Debug)]
pub enum ProposalError {
    /// The first session of a chain has no previous document.
    FirstWithPredecessor,
    /// From the second session on, the document that opened the session is required.
    MissingPredecessor,
    Contract(ContractError),
}

impl fmt::Display for ProposalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FirstWithPredecessor => f.write_str("the first session has no previous document"),
            Self::MissingPredecessor => f.write_str("the previous document is missing"),
            Self::Contract(error) => write!(f, "the proposal is outside the contract: {error}"),
        }
    }
}

impl Error for ProposalError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Contract(error) => Some(error),
            _ => None,
        }
    }
}

/// Data of `session.handoff_proposed:2`, or `None` when the policy does not propose a handoff for
/// the measured context. An invalid pair of number and previous document is an error even then.
pub fn proposed(
    policy: &HandoffPolicy,
    session_id: CommonIdV1,
    usage: Option<ContextUsage>,
    number: Number,
    previous_document_id: Option<CommonIdV1>,
) -> Result<Option<EventSessionHandoffProposedV2Data>, ProposalError> {
    match (number.get(), &previous_document_id) {
        (1, Some(_)) => return Err(ProposalError::FirstWithPredecessor),
        (2.., None) => return Err(ProposalError::MissingPredecessor),
        _ => {}
    }
    let Some(usage) = usage.filter(|usage| proposes(policy, *usage)) else {
        return Ok(None);
    };
    let Some(threshold) = threshold_pct(policy) else {
        return Ok(None);
    };
    let measured = u128::from(usage.used) * 100 / u128::from(usage.capacity.get());
    // A proposal implies at least 1%, so only usage above the capacity needs a limit.
    let context = measured.min(100) as u16;
    EventSessionHandoffProposedV2Data::new(EventSessionHandoffProposedV2DataFields {
        session_id,
        threshold_pct: ThresholdPct::new(threshold).map_err(ProposalError::Contract)?,
        context_pct: ContextPct::new(context).map_err(ProposalError::Contract)?,
        number,
        previous_document_id,
    })
    .map(Some)
    .map_err(ProposalError::Contract)
}

fn proposes(policy: &HandoffPolicy, usage: ContextUsage) -> bool {
    policy.evaluate(Trigger::ContextLimit, Some(usage), Consent::Pending) == Decision::Propose
}

/// The policy keeps its threshold private: it is the lowest whole percentage it proposes at.
fn threshold_pct(policy: &HandoffPolicy) -> Option<u16> {
    (1..=100u16).find(|&pct| {
        let usage = ContextUsage {
            used: pct.into(),
            capacity: HUNDRED,
        };
        proposes(policy, usage)
    })
}
