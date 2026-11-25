//! Whether an actor is a device of a given owner.

use ads_contracts::{CommonActorV1, CommonActorV1Kind, CommonIdV1, CommonTimestampV1};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeviceRefusal {
    NotDevice,
    NotOwnerDevice,
}

/// `owner_at` reads the device registry at the instant `at`; it is not asked about an agent or
/// the daemon.
pub(crate) fn check_owner_device(
    actor: &CommonActorV1,
    at: &CommonTimestampV1,
    owner: &CommonIdV1,
    owner_at: impl Fn(&CommonIdV1, &CommonTimestampV1) -> Option<CommonIdV1>,
) -> Result<(), DeviceRefusal> {
    if actor.kind != CommonActorV1Kind::Device {
        return Err(DeviceRefusal::NotDevice);
    }
    if owner_at(&actor.id, at).as_ref() != Some(owner) {
        return Err(DeviceRefusal::NotOwnerDevice);
    }
    Ok(())
}
