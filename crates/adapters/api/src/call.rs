//! One call from request to settled spend: build, reserve, send once, then settle or release.

use std::fmt;
use std::sync::{Mutex, MutexGuard, PoisonError};

use ads_contracts::{RecordApiUsageV3, RecordApiUsageV3Outcome};

use crate::chat::{
    ChatRequest, ChatResponse, MAX_RESPONSE_BYTES, MapError, build_request, parse_response,
};
use crate::config::ProviderConfig;
use crate::transport::{Transport, TransportError, TransportResponse};
use crate::usage::{report_usage, usage_record};
use ads_core::spend::{Month, Refusal, SpendGuard};

/// Why a call gave no response. Carries no request text, response body or key reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallError {
    /// The request could not be built; nothing was sent.
    Request(MapError),
    /// The metadata or reservation does not fit the usage record, or the timestamp is not
    /// in the month of the call; nothing was sent.
    Record,
    /// The spend guard refused; nothing was sent.
    Refused(Refusal),
    /// The guard is already past the month of the call; nothing was sent.
    MonthPassed,
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
            Self::Refused(Refusal::Disabled) => f.write_str("the api adapter is disabled"),
            Self::Refused(refusal) => refusal.fmt(f),
            Self::MonthPassed => f.write_str("the spend guard is past the month of the call"),
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
    /// Spend of the guard's month right after this call. Holds of calls still open are not in
    /// it, nor is the cost of a call whose reserved month the guard has already left.
    pub spent_micro_usd: u64,
    /// Present whenever the call costs something, for the caller to write to the ledger.
    pub record: Option<RecordApiUsageV3>,
}

/// Run one call. The guard is locked to reserve and to settle, never while the transport sends.
///
/// `now` and `timestamp` are taken before the call, from the same instant: the record carries
/// `timestamp`, which has to fall in `now`. There is no retry and no change of key. If the
/// transport panics, the reservation stays held until the guard's month changes and there is no
/// record.
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
    let Ok(record) = usage_record(
        provider,
        timestamp,
        None,
        prepared.worst_case_micro_usd,
        RecordApiUsageV3Outcome::ChargedUncertain,
    ) else {
        return unsent(CallError::Record);
    };
    // The ledger totals by the record's month, so it must be the one reserved against.
    if month_of(&record.ts.to_string()) != Some(now) {
        return unsent(CallError::Record);
    }
    let mut record = record.into_fields();
    // A block of its own: the lock is gone before the transport runs.
    let authorized = {
        let mut guard = lock(guard);
        // A later guard month would be charged for a record dated in an earlier one.
        if now < guard.month() {
            Err(CallError::MonthPassed)
        } else {
            guard
                .authorize(now, prepared.worst_case_micro_usd)
                .map_err(CallError::Refused)
        }
    };
    let reservation = match authorized {
        Ok(reservation) => reservation,
        Err(error) => return unsent(error),
    };

    // The reservation has no `Drop`: a panic in here leaves the amount held.
    let result = match transport.send(&prepared) {
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
        Err(TransportError::MaybeSent) => Err(CallError::Uncertain),
        Ok(answer) if !(200..300).contains(&answer.status) => {
            record.outcome = RecordApiUsageV3Outcome::ChargedErrorStatus;
            Err(CallError::Status(answer.status))
        }
        Ok(answer) => match read(&answer) {
            Ok(mut response) => {
                record.outcome = if response
                    .usage
                    .is_some_and(|usage| report_usage(&mut record, provider, usage).is_ok())
                {
                    RecordApiUsageV3Outcome::SucceededReported
                } else {
                    response.usage = None;
                    RecordApiUsageV3Outcome::SucceededReserved
                };
                Ok(response)
            }
            Err(error) => {
                record.outcome = RecordApiUsageV3Outcome::ChargedBadResponse;
                Err(CallError::Response(error))
            }
        },
    };

    let (result, spent_micro_usd) = {
        let mut guard = lock(guard);
        let result = match guard.settle(reservation, record.cost_estimate_micro_usd.get()) {
            Ok(()) => result,
            Err(_) => {
                record.outcome = RecordApiUsageV3Outcome::ChargedGuardReplaced;
                Err(CallError::GuardReplaced)
            }
        };
        (result, guard.spent_micro_usd())
    };
    // The metadata was checked before sending, and each outcome sets a compatible cost basis.
    let record = RecordApiUsageV3::new(record).expect("call outcome and cost basis agree");
    CallOutcome {
        result,
        spent_micro_usd,
        record: Some(record),
    }
}

/// Month of an RFC 3339 UTC timestamp.
fn month_of(timestamp: &str) -> Option<Month> {
    let year = timestamp.get(..4)?.parse().ok()?;
    let month = timestamp.get(5..7)?.parse().ok()?;
    Month::new(year, month)
}

fn read(answer: &TransportResponse) -> Result<ChatResponse, MapError> {
    if answer.body.len() > MAX_RESPONSE_BYTES {
        return Err(MapError::ResponseTooLarge);
    }
    let body = std::str::from_utf8(&answer.body).map_err(|_| MapError::MalformedResponse)?;
    parse_response(body)
}

// Each guard method leaves the counters consistent, so a poisoned lock is still usable.
fn lock(guard: &Mutex<SpendGuard>) -> MutexGuard<'_, SpendGuard> {
    guard.lock().unwrap_or_else(PoisonError::into_inner)
}
