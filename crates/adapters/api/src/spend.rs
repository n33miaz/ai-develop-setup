//! Spend guard: off by default, refuses without a monthly cap, stops at the cap.
//!
//! Pure state: the caller supplies the month and the amount already spent (from the ledger) and
//! persists nothing through this type.

use std::fmt;

use crate::config::ApiConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Month {
    year: u16,
    month: u8,
}

impl Month {
    pub fn new(year: u16, month: u8) -> Option<Self> {
        (1..=12).contains(&month).then_some(Self { year, month })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    Disabled,
    NoCap,
    CapReached,
    WouldExceedCap,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Disabled => "the api adapter is disabled",
            Self::NoCap => "no monthly cap is configured",
            Self::CapReached => "the monthly cap is reached",
            Self::WouldExceedCap => "the worst case of this call does not fit in the monthly cap",
        })
    }
}

impl std::error::Error for Refusal {}

/// Amount held for one call until it settles or is released. Not `Clone`: settled once.
#[derive(Debug)]
#[must_use = "settle or release the reservation"]
pub struct Reservation {
    micro_usd: u64,
}

impl Reservation {
    pub fn micro_usd(&self) -> u64 {
        self.micro_usd
    }
}

#[derive(Debug)]
pub struct SpendGuard {
    enabled: bool,
    cap: Option<u64>,
    month: Month,
    spent: u64,
    reserved: u64,
}

impl SpendGuard {
    pub fn new(config: &ApiConfig, month: Month, spent_micro_usd: u64) -> Self {
        Self {
            enabled: config.enabled,
            cap: config.monthly_cap_micro_usd,
            month,
            spent: spent_micro_usd,
            reserved: 0,
        }
    }

    pub fn spent_micro_usd(&self) -> u64 {
        self.spent
    }

    /// Hold `worst_case_micro_usd` for a call, or refuse. A later month resets the spend; an
    /// earlier one (clock set back) does not.
    pub fn authorize(
        &mut self,
        now: Month,
        worst_case_micro_usd: u64,
    ) -> Result<Reservation, Refusal> {
        if !self.enabled {
            return Err(Refusal::Disabled);
        }
        let cap = self.cap.ok_or(Refusal::NoCap)?;
        if now > self.month {
            self.month = now;
            self.spent = 0;
        }
        let committed = self.spent.saturating_add(self.reserved);
        if committed >= cap {
            return Err(Refusal::CapReached);
        }
        if committed.saturating_add(worst_case_micro_usd) > cap {
            return Err(Refusal::WouldExceedCap);
        }
        self.reserved += worst_case_micro_usd;
        Ok(Reservation {
            micro_usd: worst_case_micro_usd,
        })
    }

    /// Replace the reservation with what the call actually cost.
    pub fn settle(&mut self, reservation: Reservation, actual_micro_usd: u64) {
        self.reserved = self.reserved.saturating_sub(reservation.micro_usd);
        self.spent = self.spent.saturating_add(actual_micro_usd);
    }

    /// The call never reached the provider: nothing is charged.
    pub fn release(&mut self, reservation: Reservation) {
        self.reserved = self.reserved.saturating_sub(reservation.micro_usd);
    }
}
