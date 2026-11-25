//! Session handoff rules.

use std::collections::HashSet;
use std::num::NonZeroU64;

use ads_contracts::{
    CommonIdV1, CommonTimestampV1, RecordSessionHandoffV2, RecordSessionHandoffV2Number,
    RecordSessionHandoffV2Trigger as Trigger,
};
use serde::{Deserialize, Deserializer, de};

pub const DEFAULT_POLICY: &str = include_str!("../../../defaults/handoff.toml");

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    Off,
    Propose,
    AutoWithConsent,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandoffPolicy {
    mode: Mode,
    #[serde(deserialize_with = "deserialize_threshold")]
    context_threshold_pct: u8,
}

#[derive(Clone, Copy, Debug)]
pub struct ContextUsage {
    pub used: u64,
    pub capacity: NonZeroU64,
}

#[derive(Clone, Copy, Debug)]
pub enum Consent {
    Pending,
    Granted,
    Declined,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    None,
    Propose,
    Proceed,
}

impl HandoffPolicy {
    pub fn from_toml(input: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(input)
    }

    /// Consent must already have been verified for this handoff by the caller.
    /// For account switches, the caller must first apply the account-switch policy.
    pub fn evaluate(
        &self,
        trigger: Trigger,
        usage: Option<ContextUsage>,
        consent: Consent,
    ) -> Decision {
        if self.mode == Mode::Off || matches!(consent, Consent::Declined) {
            return Decision::None;
        }

        if trigger == Trigger::ContextLimit
            && !usage.is_some_and(|usage| {
                u128::from(usage.used) * 100
                    >= u128::from(usage.capacity.get()) * u128::from(self.context_threshold_pct)
            })
        {
            return Decision::None;
        }

        // A refusal never starts on its own; consent for the specific handoff lets any trigger proceed.
        match consent {
            Consent::Granted => Decision::Proceed,
            Consent::Pending | Consent::Declined => Decision::Propose,
        }
    }
}

fn deserialize_threshold<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u8, D::Error> {
    let threshold = u8::deserialize(deserializer)?;
    if (1..=100).contains(&threshold) {
        Ok(threshold)
    } else {
        Err(de::Error::custom(
            "context_threshold_pct must be between 1 and 100",
        ))
    }
}

fn next_number(current: u64) -> Option<RecordSessionHandoffV2Number> {
    RecordSessionHandoffV2Number::new(current.checked_add(1)?).ok()
}

pub fn next_session_name(
    lineage: &CommonIdV1,
    current: u64,
    timestamp: &CommonTimestampV1,
) -> Result<String, ChainError> {
    let number = next_number(current).ok_or(ChainError::Number)?;
    let date = &timestamp.as_str()[..10];
    Ok(format!("{} n{} · {date}", lineage.as_str(), number.get()))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChainError {
    RepeatedHandoff,
    RepeatedSession,
    PreviousHandoff,
    SourceSession,
    Lineage,
    Number,
    /// The consent comes from a user other than the one of the first handoff.
    User,
    /// The creation instant is earlier than the one of the previous handoff.
    CreatedAt,
    /// The creation timestamp cannot be read as an instant.
    UnreadableInstant,
    RepeatedStateDocument,
    RepeatedOpeningPrompt,
}

/// Ordering key of a UTC timestamp whose calendar the contract type already checked. Date and
/// time have a fixed width and order as text; the fraction does not, so it counts in microseconds.
fn instant_key(text: &str) -> Option<(&str, u32)> {
    let date_time = text.get(..19)?;
    let micros = match text.get(19..)?.strip_suffix('Z')? {
        "" => 0,
        fraction => {
            let digits = fraction.strip_prefix('.')?;
            if !(1..=6).contains(&digits.len()) || !digits.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            format!("{digits:0<6}").parse().ok()?
        }
    };
    Some((date_time, micros))
}

/// Validate a complete chain in order from the first handoff.
pub fn validate_chain(chain: &[RecordSessionHandoffV2]) -> Result<(), ChainError> {
    let Some(first) = chain.first() else {
        return Ok(());
    };
    let mut handoffs = HashSet::new();
    let mut sessions = HashSet::from([first.source_session_id.as_str()]);
    let mut documents = HashSet::new();
    let mut prompts = HashSet::new();
    let mut previous: Option<&RecordSessionHandoffV2> = None;
    let mut previous_instant = None;

    for record in chain {
        if !handoffs.insert(record.id.as_str()) {
            return Err(ChainError::RepeatedHandoff);
        }
        if !sessions.insert(record.target_session_id.as_str()) {
            return Err(ChainError::RepeatedSession);
        }
        if record.previous_handoff_id.as_ref() != previous.map(|record| &record.id) {
            return Err(ChainError::PreviousHandoff);
        }
        if previous.is_some_and(|previous| record.source_session_id != previous.target_session_id) {
            return Err(ChainError::SourceSession);
        }
        if record.lineage != first.lineage {
            return Err(ChainError::Lineage);
        }
        let current = previous.map_or(1, |record| record.number.get());
        if next_number(current) != Some(record.number) {
            return Err(ChainError::Number);
        }
        if record.consent.user_id != first.consent.user_id {
            return Err(ChainError::User);
        }
        let instant =
            instant_key(record.created_at.as_str()).ok_or(ChainError::UnreadableInstant)?;
        if previous_instant.is_some_and(|previous| instant < previous) {
            return Err(ChainError::CreatedAt);
        }
        if !documents.insert(record.state_document_id.as_str()) {
            return Err(ChainError::RepeatedStateDocument);
        }
        if !prompts.insert(record.opening_prompt_id.as_str()) {
            return Err(ChainError::RepeatedOpeningPrompt);
        }
        previous = Some(record);
        previous_instant = Some(instant);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_text_has_no_instant() {
        for text in [
            "",
            "2030-01-02",
            "2030-01-02T03:04:05",
            "2030-01-02T03:04:05.Z",
            "2030-01-02T03:04:05.1234567Z",
            "2030-01-02T03:04:05.+5Z",
            "2030-01-02T03:04:05+00:00",
            "2030-01-02T03:04:0é",
        ] {
            assert_eq!(instant_key(text), None, "{text}");
        }
    }
}
