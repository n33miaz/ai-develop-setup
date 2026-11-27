//! Session handoff rules.

use std::collections::HashSet;
use std::num::NonZeroU64;

use ads_contracts::{
    CommonIdV1, CommonTimestampV1, EventSessionHandoffV4, EventSessionHandoffV4DataStatus,
    RecordSessionHandoffV2, RecordSessionHandoffV2Number, RecordSessionHandoffV2Trigger as Trigger,
};
use serde::{Deserialize, Deserializer, de};

use crate::device::{DeviceRefusal, check_owner_device};

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

    /// Consent must already have been verified for this handoff by the caller, with
    /// [`verify_consent`] or, after a model refusal, [`crate::refusal::Offer::accept`].
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsentError {
    /// A model refusal is accepted only with its offer, which names the chosen alternative.
    ModelRefusal,
    /// An agent or the daemon cannot accept.
    NotDevice,
    /// The device was not registered to the lineage owner at the instant of the event.
    NotOwnerDevice,
    NotAccepted,
    OtherHandoff,
    OtherSession,
    OtherTrigger,
    OtherChannel,
    /// The event is not the one the consent of the record refers to.
    OtherEvent,
    /// The consent of the record names a user other than the lineage owner.
    OtherUser,
}

/// Confirms, before a handoff is appended, that `event` is the acceptance the record refers to,
/// issued for the recorded trigger and channel by a device of the lineage owner.
/// The caller supplies the owner and the event from its authenticated log.
/// `owner_at` reads the device registry at the instant of the event.
pub fn verify_consent(
    record: &RecordSessionHandoffV2,
    owner_id: &CommonIdV1,
    event: &EventSessionHandoffV4,
    owner_at: impl Fn(&CommonIdV1, &CommonTimestampV1) -> Option<CommonIdV1>,
) -> Result<(), ConsentError> {
    if record.trigger == Trigger::ModelRefusal {
        return Err(ConsentError::ModelRefusal);
    }
    check_owner_device(&event.actor, &event.ts, owner_id, owner_at).map_err(
        |refusal| match refusal {
            DeviceRefusal::NotDevice => ConsentError::NotDevice,
            DeviceRefusal::NotOwnerDevice => ConsentError::NotOwnerDevice,
        },
    )?;
    if event.data.status != EventSessionHandoffV4DataStatus::Accepted {
        return Err(ConsentError::NotAccepted);
    }
    check_consent_record(record, owner_id, event)
}

pub(crate) fn check_consent_record(
    record: &RecordSessionHandoffV2,
    owner_id: &CommonIdV1,
    event: &EventSessionHandoffV4,
) -> Result<(), ConsentError> {
    if event.data.handoff_id != record.id {
        return Err(ConsentError::OtherHandoff);
    }
    if event.data.source_session_id != record.source_session_id {
        return Err(ConsentError::OtherSession);
    }
    if event.id != record.consent.event_id {
        return Err(ConsentError::OtherEvent);
    }
    if record.consent.user_id != *owner_id {
        return Err(ConsentError::OtherUser);
    }
    if event.data.trigger.as_str() != record.trigger.as_str() {
        return Err(ConsentError::OtherTrigger);
    }
    if event
        .data
        .channel
        .is_none_or(|channel| channel.as_str() != record.consent.channel.as_str())
    {
        return Err(ConsentError::OtherChannel);
    }
    Ok(())
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
