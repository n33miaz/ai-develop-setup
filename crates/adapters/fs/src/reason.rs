//! Why the executor refused or stopped, with the stable code that goes into `action.failed`.

use std::fmt;

use ads_core::gate::ErrorCode;

/// One entry per cause. A code never changes its meaning; a new cause gets a new code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Reason {
    /// `action.started` given to the executor is not the one of the permit.
    StartedMismatch,
    /// The journal did not confirm that `action.started` was written.
    JournalRefused,
    /// A path written in the style of another operating system.
    PathStyle,
    /// A path that is not inside the fence.
    OutsideFence,
    /// A target, a destination or an entry that is not strictly inside the work root.
    OutsideWorkRoot,
    /// A symbolic link, a junction or another reparse point on the way to the path.
    LinkInPath,
    /// The path on disk is not the path that was written (another spelling of it).
    NotPhysical,
    /// The path crosses into another volume.
    OtherVolume,
    /// The target, or a folder on the way to it, does not exist.
    Missing,
    /// Something that is not a folder where a folder had to be.
    NotADirectory,
    /// A device, a socket or a pipe: neither file, folder nor link.
    SpecialFile,
    /// The operation, or this form of it, is not executed by this adapter.
    Unsupported,
    /// One target of the plan is inside another.
    OverlappingTargets,
    /// The trusted policy denies the plan of the permit.
    PolicyDenies,
    /// The trusted policy asks for an approval the permit does not carry.
    ApprovalMissing,
    /// A delete the policy sends to quarantine came as a real delete.
    QuarantineBypassed,
    /// What is on disk contradicts what the plan declared, in a way that changes the verdict.
    FactsContradictPlan,
    /// The id of the action cannot be the name of a folder.
    ActionIdUnusable,
    /// The quarantine folder of this action already exists.
    DestinationTaken,
    /// A plain delete of a folder that is not empty.
    NotEmpty,
    /// The manifest of a quarantine batch is missing, malformed or does not match its folder.
    ManifestInvalid,
    /// The place an item came from is taken.
    RestorePlaceTaken,
    /// No item of the manifest is in quarantine any more.
    NothingToRestore,
    /// The entry changed between the check and the mutation.
    Changed,
    /// The tree is deeper than the limit.
    TooDeep,
    /// The operating system refused the operation.
    Io,
}

impl Reason {
    pub const ALL: [Reason; 26] = [
        Self::StartedMismatch,
        Self::JournalRefused,
        Self::PathStyle,
        Self::OutsideFence,
        Self::OutsideWorkRoot,
        Self::LinkInPath,
        Self::NotPhysical,
        Self::OtherVolume,
        Self::Missing,
        Self::NotADirectory,
        Self::SpecialFile,
        Self::Unsupported,
        Self::OverlappingTargets,
        Self::PolicyDenies,
        Self::ApprovalMissing,
        Self::QuarantineBypassed,
        Self::FactsContradictPlan,
        Self::ActionIdUnusable,
        Self::DestinationTaken,
        Self::NotEmpty,
        Self::ManifestInvalid,
        Self::RestorePlaceTaken,
        Self::NothingToRestore,
        Self::Changed,
        Self::TooDeep,
        Self::Io,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::StartedMismatch => "E_FS_STARTED_MISMATCH",
            Self::JournalRefused => "E_FS_JOURNAL_REFUSED",
            Self::PathStyle => "E_FS_PATH_STYLE",
            Self::OutsideFence => "E_FS_OUTSIDE_FENCE",
            Self::OutsideWorkRoot => "E_FS_OUTSIDE_WORK_ROOT",
            Self::LinkInPath => "E_FS_LINK_IN_PATH",
            Self::NotPhysical => "E_FS_NOT_PHYSICAL",
            Self::OtherVolume => "E_FS_OTHER_VOLUME",
            Self::Missing => "E_FS_MISSING",
            Self::NotADirectory => "E_FS_NOT_A_DIRECTORY",
            Self::SpecialFile => "E_FS_SPECIAL_FILE",
            Self::Unsupported => "E_FS_UNSUPPORTED",
            Self::OverlappingTargets => "E_FS_OVERLAPPING_TARGETS",
            Self::PolicyDenies => "E_FS_POLICY_DENIES",
            Self::ApprovalMissing => "E_FS_APPROVAL_MISSING",
            Self::QuarantineBypassed => "E_FS_QUARANTINE_BYPASSED",
            Self::FactsContradictPlan => "E_FS_FACTS_CONTRADICT_PLAN",
            Self::ActionIdUnusable => "E_FS_ACTION_ID_UNUSABLE",
            Self::DestinationTaken => "E_FS_DESTINATION_TAKEN",
            Self::NotEmpty => "E_FS_NOT_EMPTY",
            Self::ManifestInvalid => "E_FS_MANIFEST_INVALID",
            Self::RestorePlaceTaken => "E_FS_RESTORE_PLACE_TAKEN",
            Self::NothingToRestore => "E_FS_NOTHING_TO_RESTORE",
            Self::Changed => "E_FS_CHANGED",
            Self::TooDeep => "E_FS_TOO_DEEP",
            Self::Io => "E_FS_IO",
        }
    }

    /// The `error_code` of `action.failed`.
    pub fn error_code(self) -> ErrorCode {
        ErrorCode::new(self.as_str()).expect("the codes above follow the contract pattern")
    }
}

/// A refusal or a stop, before it is tied to a report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stop {
    pub reason: Reason,
    /// For people: which path, which system error. Never parsed.
    pub detail: String,
}

impl Stop {
    pub(crate) fn new(reason: Reason, detail: impl fmt::Display) -> Self {
        Self {
            reason,
            detail: detail.to_string(),
        }
    }

    pub(crate) fn io(what: &str, path: &std::path::Path, error: &std::io::Error) -> Self {
        Self::new(Reason::Io, format!("{what} {}: {error}", path.display()))
    }
}

impl fmt::Display for Stop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.reason.as_str(), self.detail)
    }
}
