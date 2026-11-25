//! One call from request to settled spend: build, reserve, send once, then settle or release.

use std::fmt;
use std::sync::{Mutex, MutexGuard, PoisonError};

use ads_contracts::RecordApiUsageV1;

use crate::chat::{
    ChatRequest, ChatResponse, MAX_RESPONSE_BYTES, MapError, build_request, parse_response,
};
use crate::config::ProviderConfig;
use crate::spend::{Month, Refusal, SpendGuard};
use crate::transport::{Transport, TransportError, TransportResponse};
use crate::usage::usage_record;

/// Why a call gave no response. Carries no request text, response body or key reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallError {
    /// The request could not be built; nothing was sent.
    Request(MapError),
    /// The timestamp or the provider settings do not fit the usage record; nothing was sent.
    Record,
    /// The spend guard refused; nothing was sent.
    Refused(Refusal),
    /// The transport is certain the request did not leave; nothing is charged.
    NotSent,
    /// The transport failed after the provider may have received the request.
    Uncertain,
    /// The provider answered outside 2xx.
    Status(u16),
    /// The answer is too large or cannot be read.
    Response(MapError),
    /// The guard was replaced while the call was open, so the cost in the record is not in it.
    GuardReplaced,
}

impl fmt::Display for CallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Request(err) | Self::Response(err) => err.fmt(f),
            Self::Record => f.write_str("the usage record of this call cannot be built"),
            Self::Refused(refusal) => refusal.fmt(f),
            Self::NotSent => TransportError::NotSent.fmt(f),
            Self::Uncertain => TransportError::MaybeSent.fmt(f),
            Self::Status(status) => write!(f, "the provider answered with status {status}"),
            Self::GuardReplaced => f.write_str("the spend guard was replaced during the call"),
        }
    }
}

impl std::error::Error for CallError {}

/// How a call ended, successful or not.
#[derive(Debug, Clone, PartialEq)]
#[must_use = "write the usage record to the ledger"]
pub struct CallOutcome {
    pub result: Result<ChatResponse, CallError>,
    /// Spend of the guard right after this call; holds of calls still open are not in it.
    pub spent_micro_usd: u64,
    /// Present whenever the call costs something, for the caller to write to the ledger.
    pub record: Option<RecordApiUsageV1>,
}

/// Run one call. The guard is locked to reserve and to settle, never while the transport sends.
///
/// `now` and `timestamp` are taken before the call; the record carries `timestamp`. There is no
/// retry and no change of key. If the transport panics, the reservation stays held in the guard
/// and there is no record.
pub fn run_call<T: Transport + ?Sized>(
    transport: &T,
    guard: &Mutex<SpendGuard>,
    provider: &ProviderConfig,
    request: &ChatRequest,
    now: Month,
    timestamp: &str,
) -> CallOutcome {
    let unsent = |error| CallOutcome {
        result: Err(error),
        spent_micro_usd: lock(guard).spent_micro_usd(),
        record: None,
    };
    let prepared = match build_request(provider, request) {
        Ok(prepared) => prepared,
        Err(err) => return unsent(CallError::Request(err)),
    };
    // Built before sending, so a call that may be billed can never end up without a record.
    let Ok(reserved) = usage_record(provider, timestamp, None, prepared.worst_case_micro_usd)
    else {
        return unsent(CallError::Record);
    };
    // A statement of its own: the lock is gone before the transport runs.
    let authorized = lock(guard).authorize(now, prepared.worst_case_micro_usd);
    let reservation = match authorized {
        Ok(reservation) => reservation,
        Err(refusal) => return unsent(CallError::Refused(refusal)),
    };

    // The reservation has no `Drop`: a panic in here leaves the amount held.
    let (record, result) = match transport.send(&prepared) {
        Err(TransportError::NotSent) => {
            let mut guard = lock(guard);
            // A replacement guard never held this amount, so there is nothing to undo in it.
            let _ = guard.release(reservation);
            return CallOutcome {
                result: Err(CallError::NotSent),
                spent_micro_usd: guard.spent_micro_usd(),
                record: None,
            };
        }
        Err(TransportError::MaybeSent) => (reserved, Err(CallError::Uncertain)),
        Ok(answer) => match read(&answer) {
            Ok(response) => {
                let record = usage_record(
                    provider,
                    timestamp,
                    response.usage,
                    prepared.worst_case_micro_usd,
                )
                .unwrap_or(reserved);
                (record, Ok(response))
            }
            Err(error) => (reserved, Err(error)),
        },
    };

    let mut guard = lock(guard);
    let settled = guard.settle(reservation, record.cost_estimate_micro_usd);
    CallOutcome {
        result: match settled {
            Ok(()) => result,
            Err(_) => Err(CallError::GuardReplaced),
        },
        spent_micro_usd: guard.spent_micro_usd(),
        record: Some(record),
    }
}

fn read(answer: &TransportResponse) -> Result<ChatResponse, CallError> {
    if !(200..300).contains(&answer.status) {
        return Err(CallError::Status(answer.status));
    }
    if answer.body.len() > MAX_RESPONSE_BYTES {
        return Err(CallError::Response(MapError::ResponseTooLarge));
    }
    let body = std::str::from_utf8(&answer.body)
        .map_err(|_| CallError::Response(MapError::MalformedResponse))?;
    parse_response(body).map_err(CallError::Response)
}

// Each guard method leaves the counters consistent, so a poisoned lock is still usable.
fn lock(guard: &Mutex<SpendGuard>) -> MutexGuard<'_, SpendGuard> {
    guard.lock().unwrap_or_else(PoisonError::into_inner)
}
