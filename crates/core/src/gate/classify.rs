//! The rules: `plan + policy -> verdict`. Pure; the answer depends on nothing else.

use super::event::{
    ActionPlanFields, ActionPlanRecord, ContractError, Decision, Id, PathText, PatternText,
    PlanEstimate, PlanReason, PlanTarget, PlanVerdict, Risk, RuleCode, RuleList, SubjectText,
};
use super::path::AbsPath;
use super::plan::{ActionPlan, Operation};
use super::policy::{GatePolicy, Protection};

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
    /// Every rule of the gate, one entry per protection where the rule has that detail.
    pub fn all() -> Vec<Rule> {
        let detailed = [
            Self::Protected,
            Self::WorkRootProtected,
            Self::DestinationProtected,
        ];
        let mut all: Vec<Rule> = detailed
            .iter()
            .flat_map(|rule| Protection::ALL.map(rule))
            .collect();
        all.extend([
            Self::WorkRootItself,
            Self::OutsideAllowedRoots,
            Self::RepositoryRoot,
            Self::QuarantineItself,
            Self::DestinationOutsideAllowedRoots,
            Self::DestinationInsideTarget,
            Self::OutsideWorkRoot,
            Self::DestinationOutsideWorkRoot,
            Self::EstimateUnknown,
            Self::ItemLimitExceeded,
            Self::ByteLimitExceeded,
            Self::QuarantinePurge,
        ]);
        all
    }

    /// The stable code of the rule (`urn:ads:common:action-rule:1`). A code never changes its
    /// meaning: logs and records written with it stay readable.
    pub fn code(self) -> RuleCode {
        let detailed = |name: &str, protection: Protection| format!("{name}.{}", protection.code());
        let code = match self {
            Self::Protected(protection) => detailed("protected", protection),
            Self::WorkRootProtected(protection) => detailed("work_root_protected", protection),
            Self::DestinationProtected(protection) => detailed("destination_protected", protection),
            Self::WorkRootItself => "work_root_itself".to_owned(),
            Self::OutsideAllowedRoots => "outside_allowed_roots".to_owned(),
            Self::RepositoryRoot => "repository_root".to_owned(),
            Self::QuarantineItself => "quarantine_itself".to_owned(),
            Self::DestinationOutsideAllowedRoots => "destination_outside_allowed_roots".to_owned(),
            Self::DestinationInsideTarget => "destination_inside_target".to_owned(),
            Self::OutsideWorkRoot => "outside_work_root".to_owned(),
            Self::DestinationOutsideWorkRoot => "destination_outside_work_root".to_owned(),
            Self::EstimateUnknown => "estimate_unknown".to_owned(),
            Self::ItemLimitExceeded => "item_limit_exceeded".to_owned(),
            Self::ByteLimitExceeded => "byte_limit_exceeded".to_owned(),
            Self::QuarantinePurge => "quarantine_purge".to_owned(),
        };
        RuleCode::new(code).expect("the codes above follow the contract pattern")
    }

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
    /// The operation as asked, which the quarantine rewrite replaces in `plan`.
    requested: Operation,
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

    /// The operation of the plan as it was asked, before any rewrite.
    pub fn requested_operation(&self) -> &Operation {
        &self.requested
    }

    /// The codes of the rules that denied the plan, each one once, in the order they fired.
    /// Rules that only ask for approval are left out. `None` when the plan was not denied.
    pub fn denying_rules(&self) -> Option<RuleList> {
        let mut codes: Vec<RuleCode> = Vec::new();
        for finding in &self.findings {
            let code = finding.rule.code();
            if finding.rule.decision() == Decision::Denied && !codes.contains(&code) {
                codes.push(code);
            }
        }
        RuleList::new(codes).ok()
    }

    /// The `record.action-plan` of this verdict for the action `action_id`: the operation as
    /// asked, the plan in force and every rule that fired. Refused when the plan does not fit
    /// the limits of the contract (path length, number of targets or of reasons).
    pub fn record(&self, action_id: Id) -> Result<ActionPlanRecord, ContractError> {
        let path = |path: &AbsPath| PathText::new(path.to_string());
        let targets = self.plan.targets().iter().map(|target| {
            Ok(PlanTarget {
                path: path(target.path().scope())?,
                pattern: target.path().pattern().map(PatternText::new).transpose()?,
                repo_root: target.is_repo_root(),
            })
        });
        let reasons = self.findings.iter().map(|finding| {
            Ok(PlanReason {
                rule: finding.rule.code(),
                subject: SubjectText::new(finding.subject.as_str())?,
            })
        });
        ActionPlanRecord::try_from(ActionPlanFields {
            version: 1,
            action_id,
            plan_hash: self.plan.hash(),
            operation: self.requested.name(),
            work_root: path(self.plan.work_root())?,
            destination: self.plan.destination().map(path).transpose()?,
            targets: targets.collect::<Result<_, ContractError>>()?,
            estimate: self.plan.estimate().map(|estimate| PlanEstimate {
                items: estimate.items,
                bytes: estimate.bytes,
            }),
            verdict: PlanVerdict {
                decision: self.decision,
                risk: self.risk(),
                reasons: reasons.collect::<Result<_, ContractError>>()?,
            },
            quarantined: self.quarantined,
        })
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
        requested: plan.operation().clone(),
        quarantined,
    }
}
