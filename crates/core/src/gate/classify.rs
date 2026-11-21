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
    /// The target is an allowed root or contains one, even when another root contains it.
    AllowedRootItself,
    /// The target is not strictly inside the work root or an allowed root.
    OutsideAllowedRoots,
    RepositoryRoot,
    RepositoryMetadata,
    WorkRootUnknown,
    QuarantineOfOtherRoot,
    /// The target is the quarantine folder or contains it.
    QuarantineItself,
    DestinationProtected(Protection),
    DestinationOutsideAllowedRoots,
    /// A move into one of its own targets, or onto one of them.
    DestinationInsideTarget,
    /// The entry a move would create in its destination (`destination/name of the target`) is
    /// the work root, an allowed root or the quarantine folder, or contains one.
    DestinationEntryReserved,

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
            Self::AllowedRootItself,
            Self::OutsideAllowedRoots,
            Self::RepositoryRoot,
            Self::RepositoryMetadata,
            Self::WorkRootUnknown,
            Self::QuarantineOfOtherRoot,
            Self::QuarantineItself,
            Self::DestinationOutsideAllowedRoots,
            Self::DestinationInsideTarget,
            Self::DestinationEntryReserved,
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
            Self::AllowedRootItself => "allowed_root_itself".to_owned(),
            Self::OutsideAllowedRoots => "outside_allowed_roots".to_owned(),
            Self::RepositoryRoot => "repository_root".to_owned(),
            Self::RepositoryMetadata => "repository_metadata".to_owned(),
            Self::WorkRootUnknown => "work_root_unknown".to_owned(),
            Self::QuarantineOfOtherRoot => "quarantine_of_other_root".to_owned(),
            Self::QuarantineItself => "quarantine_itself".to_owned(),
            Self::DestinationOutsideAllowedRoots => "destination_outside_allowed_roots".to_owned(),
            Self::DestinationInsideTarget => "destination_inside_target".to_owned(),
            Self::DestinationEntryReserved => "destination_entry_reserved".to_owned(),
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
            | Self::AllowedRootItself
            | Self::OutsideAllowedRoots
            | Self::RepositoryRoot
            | Self::RepositoryMetadata
            | Self::WorkRootUnknown
            | Self::QuarantineOfOtherRoot
            | Self::QuarantineItself
            | Self::DestinationProtected(_)
            | Self::DestinationOutsideAllowedRoots
            | Self::DestinationInsideTarget
            | Self::DestinationEntryReserved => Decision::Denied,
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
    pub(super) clock_rewind_seconds: u64,
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
            version: ActionPlanRecord::VERSION,
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

/// Judges the plan and, when a delete becomes a move to quarantine, the rewritten plan as well,
/// before the verdict is closed: the generated destination goes through the rules of any move.
/// The verdict keeps every finding of both and takes the strictest level, so judging
/// [`Verdict::plan`] again never finds anything the verdict does not already hold. One denied
/// target, or a denied destination, denies the whole plan, and a denied plan is not rewritten.
pub fn classify(plan: &ActionPlan, policy: &GatePolicy) -> Verdict {
    let strictest = |findings: &[Finding]| {
        findings
            .iter()
            .map(|finding| finding.rule.decision())
            .max()
            .unwrap_or(Decision::Allowed)
    };
    let mut findings = judge(plan, policy);

    let deletes = matches!(
        plan.operation(),
        Operation::Delete | Operation::DeleteRecursive
    );
    // Deleting what is already in quarantine cannot go to quarantine again.
    let purge = findings
        .iter()
        .any(|finding| finding.rule == Rule::QuarantinePurge);
    let mut rewrite = None;
    if policy.quarantine_enabled() && deletes && !purge && strictest(&findings) != Decision::Denied
    {
        // The folder is named after the hash of the plan as asked, so two plans never share one.
        let hash = plan.hash();
        let name = hash.as_str()["sha256:".len()..][..16].to_owned();
        let moved = plan.as_move_to(policy.quarantine_root(plan.work_root()).join(&[name]));
        for finding in judge(&moved, policy) {
            if !findings.contains(&finding) {
                findings.push(finding);
            }
        }
        rewrite = Some(moved);
    }

    let decision = strictest(&findings);
    let rewrite = rewrite.filter(|_| decision != Decision::Denied);
    let quarantined = rewrite.is_some();
    Verdict {
        decision,
        findings,
        plan: rewrite.unwrap_or_else(|| plan.clone()),
        requested: plan.operation().clone(),
        quarantined,
        clock_rewind_seconds: policy.clock_rewind_seconds(),
    }
}

/// Every rule that fires on the plan exactly as it is written, with no rewrite. Each finding
/// comes once.
fn judge(plan: &ActionPlan, policy: &GatePolicy) -> Vec<Finding> {
    let mut findings: Vec<Finding> = Vec::new();
    let mut find = |rule: Rule, subject: &dyn std::fmt::Display| {
        let finding = Finding {
            rule,
            subject: subject.to_string(),
        };
        if !findings.contains(&finding) {
            findings.push(finding);
        }
    };

    let work_root = plan.work_root();
    if let Some(protection) = policy.protection(work_root) {
        find(Rule::WorkRootProtected(protection), work_root);
    }
    let same_root = |root: &AbsPath| root.is_within(work_root) && work_root.is_within(root);
    let roots = policy.work_roots().unwrap_or_default();
    if !roots.iter().any(same_root) {
        find(Rule::WorkRootUnknown, work_root);
    }
    let other_quarantines: Vec<_> = roots
        .iter()
        .filter(|root| !same_root(root))
        .map(|root| policy.quarantine_root(root))
        .collect();
    let repository_metadata = |path: &AbsPath| {
        path.components()
            .iter()
            .any(|name| name.eq_ignore_ascii_case(".git"))
    };
    let in_other_quarantine = |path: &AbsPath| {
        other_quarantines
            .iter()
            .any(|root| path.is_loosely_within(root))
    };
    let overlaps_other_quarantine = |path: &AbsPath| {
        in_other_quarantine(path)
            || other_quarantines
                .iter()
                .any(|root| root.is_loosely_within(path))
    };
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

    for target in plan.targets() {
        let scope = target.path().scope();
        let subject = target.path();
        if let Some(protection) = policy.protection(scope) {
            find(Rule::Protected(protection), subject);
        }
        if work_root.is_loosely_within(scope) {
            find(Rule::WorkRootItself, subject);
        }
        let holds_a_root = policy
            .allowed_roots()
            .iter()
            .any(|root| root.is_loosely_within(scope));
        if holds_a_root {
            find(Rule::AllowedRootItself, subject);
        }
        if !strictly_allowed(scope) {
            find(Rule::OutsideAllowedRoots, subject);
        }
        if repository_metadata(scope) {
            find(Rule::RepositoryMetadata, subject);
        }
        if overlaps_other_quarantine(scope) {
            find(Rule::QuarantineOfOtherRoot, subject);
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
                find(Rule::QuarantinePurge, subject);
            }
        }
    }

    if let Operation::Move { destination } = plan.operation() {
        if repository_metadata(destination) {
            find(Rule::RepositoryMetadata, destination);
        }
        if in_other_quarantine(destination) {
            find(Rule::QuarantineOfOtherRoot, destination);
        }
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

        // The destination is the folder that receives the targets: what a move creates or
        // replaces is `destination/name`, judged too. Deny rules only, so the wide comparison.
        let reserved: Vec<&AbsPath> = std::iter::once(work_root)
            .chain(policy.allowed_roots())
            .chain(&quarantine)
            .collect();
        for target in plan.targets() {
            let scope = target.path().scope();
            match target.path().pattern() {
                None => {
                    let Some(name) = scope.components().last() else {
                        continue;
                    };
                    let entry = destination.join(std::slice::from_ref(name));
                    if overlaps_other_quarantine(&entry) {
                        find(Rule::QuarantineOfOtherRoot, &entry);
                    }
                    if let Some(protection) = policy.protection(&entry) {
                        find(Rule::DestinationProtected(protection), &entry);
                    }
                    // On a target, inside one, or holding one: the tree of any target.
                    let onto_a_target = plan.targets().iter().any(|other| {
                        let other = other.path().scope();
                        entry.is_loosely_within(other) || other.is_loosely_within(&entry)
                    });
                    if onto_a_target {
                        find(Rule::DestinationInsideTarget, &entry);
                    }
                    if reserved.iter().any(|path| path.is_loosely_within(&entry)) {
                        find(Rule::DestinationEntryReserved, &entry);
                    }
                }
                // The names are not known: any child of the destination the wildcard may name.
                // Such a child that is, or holds, a reserved folder or the folder of any target
                // of the plan (this one included) denies; when in doubt, it may name it.
                Some(pattern) => {
                    let named = |path: &AbsPath| {
                        let below = path.depth() > destination.depth()
                            && path.is_loosely_within(destination);
                        let name = below.then(|| &path.components()[destination.depth()])?;
                        may_name(pattern, name)
                            .then(|| destination.join(std::slice::from_ref(name)))
                    };
                    for path in &other_quarantines {
                        if let Some(entry) = named(path) {
                            find(Rule::QuarantineOfOtherRoot, &entry);
                        }
                    }
                    for path in &reserved {
                        if let Some(entry) = named(path) {
                            find(Rule::DestinationEntryReserved, &entry);
                        }
                    }
                    for other in plan.targets() {
                        if let Some(entry) = named(other.path().scope()) {
                            find(Rule::DestinationInsideTarget, &entry);
                        }
                    }
                }
            }
        }
    }

    // A move into quarantine is a delete that was rewritten, and nothing in it says which
    // one: it is judged as the recursive one. A plain delete that is rewritten is judged that
    // way too, through its rewrite.
    let sized = match plan.operation() {
        Operation::DeleteRecursive | Operation::CleanUntracked => true,
        Operation::Move { destination } => quarantine
            .as_ref()
            .is_some_and(|quarantine| destination.is_loosely_within(quarantine)),
        Operation::Delete | Operation::Overwrite => false,
    };
    if sized {
        // A limit of zero always asks, whatever the estimate says.
        let over = |measured: u64, limit: u64| measured > limit || limit == 0;
        match plan.estimate() {
            None => find(Rule::EstimateUnknown, work_root),
            Some(estimate) => {
                if over(estimate.items, policy.approval_item_limit()) {
                    find(Rule::ItemLimitExceeded, work_root);
                }
                if over(estimate.bytes, policy.approval_byte_limit()) {
                    find(Rule::ByteLimitExceeded, work_root);
                }
            }
        }
    }

    findings
}

/// Whether a wildcard (`*` any run of characters, `?` any one) could name `name` on some file
/// system: case is ignored either way. For deny rules only.
pub fn may_name(pattern: &str, name: &str) -> bool {
    // No recursion and no blow-up: both texts come from the request and have no size limit
    // here. On a mismatch the last `*` takes one more character and the match resumes there.
    fn matches(pattern: &[char], name: &[char]) -> bool {
        let (mut p, mut n) = (0, 0);
        let mut star: Option<(usize, usize)> = None;
        while n < name.len() {
            if p < pattern.len() && pattern[p] == '*' {
                star = Some((p, n));
                p += 1;
            } else if p < pattern.len() && (pattern[p] == '?' || pattern[p] == name[n]) {
                p += 1;
                n += 1;
            } else if let Some((at, taken)) = star {
                star = Some((at, taken + 1));
                p = at + 1;
                n = taken + 1;
            } else {
                return false;
            }
        }
        pattern[p..].iter().all(|rest| *rest == '*')
    }
    let folded = |fold: fn(&str) -> String| {
        let pattern: Vec<char> = fold(pattern).chars().collect();
        let name: Vec<char> = fold(name).chars().collect();
        matches(&pattern, &name)
    };
    folded(str::to_lowercase) || folded(str::to_uppercase)
}

#[cfg(test)]
mod tests {
    use super::may_name;

    #[test]
    fn wildcard_names_what_it_could_match_in_any_case() {
        for (pattern, name) in [
            ("*", ".ads"),
            ("?ADS", ".ads"),
            (".a*", ".ads"),
            ("*s", ".ADS"),
            ("*.*", "a.b"),
            ("a*b*c", "aXbYc"),
            ("**", ""),
            ("a?c", "ABC"),
        ] {
            assert!(may_name(pattern, name), "{pattern} against {name}");
        }
        for (pattern, name) in [
            ("*.log", ".ads"),
            ("?", ".ads"),
            ("ads*", ".ads"),
            ("a*b", "ac"),
            ("a?", "a"),
            ("", "a"),
        ] {
            assert!(!may_name(pattern, name), "{pattern} against {name}");
        }
    }

    /// The texts come from the request, with no size limit before the rules: the match must
    /// neither take exponential time nor grow the stack with them.
    #[test]
    fn wildcard_match_is_bounded_in_time_and_in_stack() {
        let greedy = format!("{}b", "*a".repeat(60));
        assert!(!may_name(&greedy, &"a".repeat(100)));
        let long = "a".repeat(200_000);
        assert!(may_name(&format!("{long}?"), &format!("{long}a")));
        assert!(!may_name(&format!("{long}?"), &long));
    }
}
