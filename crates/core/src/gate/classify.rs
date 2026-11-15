//! The rules: `plan + policy -> verdict`. Pure; the answer depends on nothing else.

use super::event::Risk;
use super::path::AbsPath;
use super::plan::{ActionPlan, Operation};
use super::policy::{GatePolicy, Protection};

/// The three levels of a verdict, from the mildest to the strictest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Decision {
    Allowed,
    RequiresApproval,
    Denied,
}

impl Decision {
    /// One level of `urn:ads:common:risk:1` for each level of the verdict.
    pub fn risk(self) -> Risk {
        match self {
            Self::Allowed => Risk::Low,
            Self::RequiresApproval => Risk::Medium,
            Self::Denied => Risk::High,
        }
    }
}

/// The rule behind a finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Rule {
    // Denied, always.
    /// The target is a protected location (root, user home or an ancestor, profile, system).
    Protected(Protection),
    /// The declared work root is itself a protected location: nothing in the plan is trusted.
    WorkRootProtected(Protection),
    /// The target is the work root or contains it.
    WorkRootItself,
    /// The target is not strictly inside the work root or an allowed root.
    OutsideAllowedRoots,
    RepositoryRoot,
    /// The target is the quarantine folder or contains it.
    QuarantineItself,
    DestinationProtected(Protection),
    DestinationOutsideAllowedRoots,
    /// A move into one of its own targets.
    DestinationInsideTarget,

    // Requires approval.
    /// Inside an allowed root, outside the work root. Covers overwriting outside the work root.
    OutsideWorkRoot,
    DestinationOutsideWorkRoot,
    /// Recursive delete or clean of untracked files with no estimate of what goes away.
    EstimateUnknown,
    ItemLimitExceeded,
    ByteLimitExceeded,
    /// Deleting what is already in quarantine cannot be undone.
    QuarantinePurge,
}

impl Rule {
    pub fn decision(self) -> Decision {
        match self {
            Self::Protected(_)
            | Self::WorkRootProtected(_)
            | Self::WorkRootItself
            | Self::OutsideAllowedRoots
            | Self::RepositoryRoot
            | Self::QuarantineItself
            | Self::DestinationProtected(_)
            | Self::DestinationOutsideAllowedRoots
            | Self::DestinationInsideTarget => Decision::Denied,
            Self::OutsideWorkRoot
            | Self::DestinationOutsideWorkRoot
            | Self::EstimateUnknown
            | Self::ItemLimitExceeded
            | Self::ByteLimitExceeded
            | Self::QuarantinePurge => Decision::RequiresApproval,
        }
    }
}

/// One rule that fired, and the path it fired on.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Finding {
    pub rule: Rule,
    pub subject: String,
}

/// The answer of the gate. Only [`classify`] builds one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    decision: Decision,
    findings: Vec<Finding>,
    plan: ActionPlan,
    quarantined: bool,
}

impl Verdict {
    pub fn decision(&self) -> Decision {
        self.decision
    }

    pub fn risk(&self) -> Risk {
        self.decision.risk()
    }

    /// Every rule that fired, not only the strictest.
    pub fn findings(&self) -> &[Finding] {
        &self.findings
    }

    pub fn has(&self, rule: Rule) -> bool {
        self.findings.iter().any(|finding| finding.rule == rule)
    }

    /// The plan to hash, approve and execute: the classified plan, or its rewrite as a move
    /// to quarantine.
    pub fn plan(&self) -> &ActionPlan {
        &self.plan
    }

    /// Whether [`Self::plan`] is a delete rewritten as a move to quarantine.
    pub fn is_quarantined(&self) -> bool {
        self.quarantined
    }
}

/// Judges every target of the plan. One denied target denies the whole plan.
pub fn classify(plan: &ActionPlan, policy: &GatePolicy) -> Verdict {
    let mut findings = Vec::new();
    let mut find = |rule: Rule, subject: &dyn std::fmt::Display| {
        findings.push(Finding {
            rule,
            subject: subject.to_string(),
        });
    };

    let work_root = plan.work_root();
    if let Some(protection) = policy.protection(work_root) {
        find(Rule::WorkRootProtected(protection), work_root);
    }
    let strictly_allowed = |path: &AbsPath| {
        path.is_strictly_within(work_root)
            || policy
                .allowed_roots()
                .iter()
                .any(|root| path.is_strictly_within(root))
    };
    let quarantine = policy
        .quarantine_enabled()
        .then(|| policy.quarantine_root(work_root));
    let deletes = matches!(
        plan.operation(),
        Operation::Delete | Operation::DeleteRecursive
    );
    let mut purge = false;

    for target in plan.targets() {
        let scope = target.path().scope();
        let subject = target.path();
        if let Some(protection) = policy.protection(scope) {
            find(Rule::Protected(protection), subject);
        }
        if work_root.is_loosely_within(scope) {
            find(Rule::WorkRootItself, subject);
        }
        if !strictly_allowed(scope) {
            find(Rule::OutsideAllowedRoots, subject);
        }
        if target.is_repo_root() {
            find(Rule::RepositoryRoot, subject);
        }
        if !scope.is_within(work_root) {
            find(Rule::OutsideWorkRoot, subject);
        }
        if let Some(quarantine) = &quarantine {
            if quarantine.is_loosely_within(scope) {
                find(Rule::QuarantineItself, subject);
            } else if deletes && scope.is_loosely_within(quarantine) {
                purge = true;
                find(Rule::QuarantinePurge, subject);
            }
        }
    }

    if let Operation::Move { destination } = plan.operation() {
        if let Some(protection) = policy.protection(destination) {
            find(Rule::DestinationProtected(protection), destination);
        }
        let allowed = destination.is_within(work_root)
            || policy
                .allowed_roots()
                .iter()
                .any(|root| destination.is_within(root));
        if !allowed {
            find(Rule::DestinationOutsideAllowedRoots, destination);
        }
        let into_itself = plan
            .targets()
            .iter()
            .any(|target| destination.is_loosely_within(target.path().scope()));
        if into_itself {
            find(Rule::DestinationInsideTarget, destination);
        }
        if !destination.is_within(work_root) {
            find(Rule::DestinationOutsideWorkRoot, destination);
        }
    }

    if matches!(
        plan.operation(),
        Operation::DeleteRecursive | Operation::CleanUntracked
    ) {
        match plan.estimate() {
            None => find(Rule::EstimateUnknown, work_root),
            Some(estimate) => {
                if estimate.items > policy.approval_item_limit() {
                    find(Rule::ItemLimitExceeded, work_root);
                }
                if estimate.bytes > policy.approval_byte_limit() {
                    find(Rule::ByteLimitExceeded, work_root);
                }
            }
        }
    }

    let decision = findings
        .iter()
        .map(|finding| finding.rule.decision())
        .max()
        .unwrap_or(Decision::Allowed);

    // The folder is named after the hash of the plan as asked, so two plans never share one.
    let rewrite = match &quarantine {
        Some(quarantine) if deletes && !purge && decision != Decision::Denied => {
            let hash = plan.hash();
            let name = hash.as_str()["sha256:".len()..][..16].to_owned();
            Some(plan.as_move_to(quarantine.join(&[name])))
        }
        _ => None,
    };
    let quarantined = rewrite.is_some();
    Verdict {
        decision,
        findings,
        plan: rewrite.unwrap_or_else(|| plan.clone()),
        quarantined,
    }
}
