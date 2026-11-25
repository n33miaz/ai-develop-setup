//! Spend guard: off by default, refuses without a monthly cap, stops at the cap.
//!
//! Pure state: the caller supplies the month and the amount already spent (from the ledger) and
//! persists nothing through this type.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

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

/// A reservation refused because another guard issued it. It carries the reservation back so
/// the caller can finish it on the issuing guard; dropping it leaves that guard's hold in place.
#[derive(Debug)]
#[must_use = "settle or release the returned reservation on the guard that issued it"]
pub struct ForeignReservation(Reservation);

impl ForeignReservation {
    pub fn into_reservation(self) -> Reservation {
        self.0
    }
}

impl fmt::Display for ForeignReservation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the reservation was issued by another spend guard")
    }
}

impl std::error::Error for ForeignReservation {}

/// Amount held for one call, in the month it was reserved, until it settles or is released.
/// Not `Clone`: settled once.
#[derive(Debug)]
#[must_use = "settle or release the reservation"]
pub struct Reservation {
    guard_id: u64,
    month: Month,
    micro_usd: u64,
}

impl Reservation {
    pub fn micro_usd(&self) -> u64 {
        self.micro_usd
    }
}

#[derive(Debug)]
pub struct SpendGuard {
    id: u64,
    enabled: bool,
    cap: Option<u64>,
    month: Month,
    spent: u64,
    reserved: u64,
}

impl SpendGuard {
    pub fn new(config: &ApiConfig, month: Month, spent_micro_usd: u64) -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        Self {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            enabled: config.enabled,
            cap: config.monthly_cap_micro_usd,
            month,
            spent: spent_micro_usd,
            reserved: 0,
        }
    }

    /// The latest month the guard was built with or has authorized in.
    pub fn month(&self) -> Month {
        self.month
    }

    /// Spend of the guard's month.
    pub fn spent_micro_usd(&self) -> u64 {
        self.spent
    }

    /// Hold `worst_case_micro_usd` for a call, or refuse. A later month starts with nothing
    /// spent or held; an earlier one (clock set back) counts in the guard's month.
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
            self.reserved = 0;
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
            guard_id: self.id,
            month: self.month,
            micro_usd: worst_case_micro_usd,
        })
    }

    /// Replace the reservation with what the call actually cost, in the month that reserved it:
    /// once the guard is past that month, nothing changes here. A reservation from another guard
    /// is returned in the error before any counter changes.
    pub fn settle(
        &mut self,
        reservation: Reservation,
        actual_micro_usd: u64,
    ) -> Result<(), ForeignReservation> {
        let reservation = self.check_owner(reservation)?;
        if reservation.month == self.month {
            self.reserved = self.reserved.saturating_sub(reservation.micro_usd);
            self.spent = self.spent.saturating_add(actual_micro_usd);
        }
        Ok(())
    }

    /// The call never reached the provider: nothing is charged.
    pub fn release(&mut self, reservation: Reservation) -> Result<(), ForeignReservation> {
        let reservation = self.check_owner(reservation)?;
        if reservation.month == self.month {
            self.reserved = self.reserved.saturating_sub(reservation.micro_usd);
        }
        Ok(())
    }

    fn check_owner(&self, reservation: Reservation) -> Result<Reservation, ForeignReservation> {
        if reservation.guard_id == self.id {
            Ok(reservation)
        } else {
            Err(ForeignReservation(reservation))
        }
    }
}
