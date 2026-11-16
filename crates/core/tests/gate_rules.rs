//! The rules of the gate, one named case each. Paths are synthetic text; nothing is touched.

mod common;

use ads_core::gate::{
    ActionPlan, Decision, Estimate, GatePolicy, HostFacts, Operation, OperationRequest as Op,
    PathError, PlanError, PolicyError, Protection, Risk, Rule, Verdict, classify,
};
use common::{POSIX_HOME, POSIX_ROOT, WIN_HOME, WIN_ROOT, plan, policy, posix, request, win};

fn denied_by(verdict: &Verdict, rule: Rule) {
    assert_eq!(
        verdict.decision(),
        Decision::Denied,
        "{:?}",
        verdict.findings()
    );
    assert!(
        verdict.has(rule),
        "{rule:?} not in {:?}",
        verdict.findings()
    );
    assert!(!verdict.is_quarantined());
}

fn needs_approval_by(verdict: &Verdict, rule: Rule) {
    assert_eq!(
        verdict.decision(),
        Decision::RequiresApproval,
        "{:?}",
        verdict.findings()
    );
    assert!(
        verdict.has(rule),
        "{rule:?} not in {:?}",
        verdict.findings()
    );
}

fn move_to(destination: &str) -> Op {
    Op::Move {
        destination: destination.to_owned(),
    }
}

// ---- Denied, always ---------------------------------------------------------------------------

#[test]
fn target_that_is_the_user_home_is_denied() {
    for op in [
        Op::Delete,
        Op::DeleteRecursive,
        Op::Overwrite,
        Op::CleanUntracked,
    ] {
        denied_by(
            &win(op.clone(), &[WIN_HOME]),
            Rule::Protected(Protection::UserHome),
        );
        denied_by(
            &posix(op, &[POSIX_HOME]),
            Rule::Protected(Protection::UserHome),
        );
    }
    denied_by(
        &win(move_to("C:\\Users\\user-a\\work\\proj\\out"), &[WIN_HOME]),
        Rule::Protected(Protection::UserHome),
    );
    // Other spellings of the same folder.
    for target in [
        "c:/users/USER-A/",
        "C:\\Users\\.\\user-a",
        "C:\\Users\\user-a\\*",
    ] {
        denied_by(
            &win(Op::DeleteRecursive, &[target]),
            Rule::Protected(Protection::UserHome),
        );
    }
    denied_by(
        &posix(Op::DeleteRecursive, &["/home/user-a/*"]),
        Rule::Protected(Protection::UserHome),
    );
}

#[test]
fn user_home_is_denied_even_as_work_root_or_allowed_root_content() {
    // The session was started in the home folder: the work root itself is protected.
    let verdict = classify(
        &plan(
            Op::DeleteRecursive,
            WIN_HOME,
            &["C:\\Users\\user-a\\Documents"],
        ),
        &policy(WIN_HOME, ""),
    );
    denied_by(&verdict, Rule::WorkRootProtected(Protection::UserHome));
}

#[test]
fn user_home_spelled_in_the_other_path_style_is_denied() {
    // `/c/Users/user-a` is how some shells write `C:\Users\user-a`. As text it is a POSIX
    // path that no other rule recognizes, so a whole plan in that style would have passed.
    let policy = policy(WIN_HOME, "quarantine_enabled = false");
    for op in [
        Op::Delete,
        Op::DeleteRecursive,
        Op::Overwrite,
        Op::CleanUntracked,
    ] {
        let verdict = classify(
            &plan(op, "/c/Users/user-a", &["/c/Users/user-a/Documents"]),
            &policy,
        );
        denied_by(
            &verdict,
            Rule::WorkRootProtected(Protection::OtherPathStyle),
        );
        assert!(verdict.has(Rule::Protected(Protection::OtherPathStyle)));
    }
    // The same for a target or a destination alone, and the other way around.
    denied_by(
        &win(Op::DeleteRecursive, &["/c/Users/user-a"]),
        Rule::Protected(Protection::OtherPathStyle),
    );
    denied_by(
        &win(move_to("/c/tmp"), &["C:\\Users\\user-a\\work\\proj\\build"]),
        Rule::DestinationProtected(Protection::OtherPathStyle),
    );
    denied_by(
        &posix(Op::DeleteRecursive, &["C:\\work\\proj\\build"]),
        Rule::Protected(Protection::OtherPathStyle),
    );
}

#[test]
fn allowed_root_in_the_other_path_style_is_refused() {
    let refused = GatePolicy::from_toml_str(
        "allowed_roots = ['/c/Users/user-a']",
        &HostFacts {
            user_home: WIN_HOME,
        },
    );
    assert_eq!(
        refused,
        Err(PolicyError::ProtectedAllowedRoot(
            "/c/Users/user-a".to_owned()
        ))
    );
}

#[test]
fn target_that_is_an_ancestor_of_the_user_home_is_denied() {
    denied_by(
        &win(Op::DeleteRecursive, &["C:\\Users"]),
        Rule::Protected(Protection::UserHomeAncestor),
    );
    denied_by(
        &win(Op::Delete, &["C:\\Users\\*"]),
        Rule::Protected(Protection::UserHomeAncestor),
    );
    denied_by(
        &posix(Op::DeleteRecursive, &["/home"]),
        Rule::Protected(Protection::UserHomeAncestor),
    );
    let home = "/data/people/user-a";
    for target in ["/data/people", "/data", "/data/*"] {
        let verdict = classify(
            &plan(Op::DeleteRecursive, "/data/people/user-a/proj", &[target]),
            &policy(home, ""),
        );
        denied_by(&verdict, Rule::Protected(Protection::UserHomeAncestor));
    }
}

#[test]
fn work_root_parent_written_with_dot_dot_is_refused() {
    for target in [
        "C:\\Users\\user-a\\work\\proj\\..",
        "C:/Users/user-a/work/proj/../",
        "C:\\Users\\user-a\\work\\proj\\src\\..\\..\\..",
    ] {
        let refused = ActionPlan::normalize(&request(Op::DeleteRecursive, WIN_ROOT, &[target]));
        assert_eq!(
            refused,
            Err(PlanError::Target {
                index: 0,
                error: PathError::ParentComponent
            }),
            "{target}"
        );
    }
    let refused = ActionPlan::normalize(&request(
        Op::DeleteRecursive,
        POSIX_ROOT,
        &["/home/user-a/work/proj/.."],
    ));
    assert!(matches!(refused, Err(PlanError::Target { index: 0, .. })));
    // A work root declared with `..` is refused as well.
    let refused = ActionPlan::normalize(&request(
        Op::Delete,
        "/home/user-a/work/proj/..",
        &["/home/user-a/work/x"],
    ));
    assert_eq!(
        refused,
        Err(PlanError::WorkRoot(PathError::ParentComponent))
    );
}

#[test]
fn work_root_parent_written_out_is_denied() {
    let verdict = win(Op::DeleteRecursive, &["C:\\Users\\user-a\\work"]);
    denied_by(&verdict, Rule::WorkRootItself);
    assert!(verdict.has(Rule::OutsideAllowedRoots));
    denied_by(
        &posix(Op::DeleteRecursive, &["/home/user-a/work"]),
        Rule::WorkRootItself,
    );
}

#[test]
fn target_with_unexpanded_variable_is_refused() {
    for target in [
        "$HOME/work/proj/build",
        "${HOME}/work/proj/build",
        "%USERPROFILE%\\work\\proj\\build",
        "C:\\Users\\%USERNAME%\\work\\proj\\build",
        "/home/$USER/work/proj/build",
        "/home/user-a/work/proj/$BUILD_DIR",
    ] {
        let refused = ActionPlan::normalize(&request(Op::DeleteRecursive, POSIX_ROOT, &[target]));
        assert_eq!(
            refused,
            Err(PlanError::Target {
                index: 0,
                error: PathError::UnexpandedVariable
            }),
            "{target}"
        );
    }
    for target in ["~", "~/work/proj/build", "/home/user-a/work/proj/~/x"] {
        let refused = ActionPlan::normalize(&request(Op::DeleteRecursive, POSIX_ROOT, &[target]));
        assert_eq!(
            refused,
            Err(PlanError::Target {
                index: 0,
                error: PathError::UnexpandedTilde
            }),
            "{target}"
        );
    }
}

#[test]
fn one_good_and_one_bad_target_denies_the_whole_plan() {
    let good = "C:\\Users\\user-a\\work\\proj\\build";
    assert_eq!(win(Op::Delete, &[good]).decision(), Decision::Allowed);

    // Bad for the rules: the plan is denied and nothing of it is rewritten or kept.
    for bad in [
        WIN_HOME,
        "C:\\",
        "C:\\Windows\\Temp\\x",
        "D:\\elsewhere\\x",
        "C:\\Users\\user-a\\Documents",
    ] {
        let verdict = win(Op::Delete, &[good, bad]);
        assert_eq!(verdict.decision(), Decision::Denied, "{bad}");
        assert!(!verdict.is_quarantined());
        let verdict = win(Op::Delete, &[bad, good]);
        assert_eq!(verdict.decision(), Decision::Denied, "{bad}");
    }

    // Bad for the normalization: there is no plan at all.
    for bad in ["build", "..\\proj2", "%TEMP%\\x", "\\\\server\\share\\x"] {
        let refused = ActionPlan::normalize(&request(Op::Delete, WIN_ROOT, &[good, bad]));
        assert!(
            matches!(refused, Err(PlanError::Target { index: 1, .. })),
            "{bad}"
        );
    }
}

#[test]
fn drive_root_and_slash_are_denied() {
    for target in ["C:\\", "c:/", "D:\\", "C:\\*", "C:\\?"] {
        denied_by(
            &win(Op::DeleteRecursive, &[target]),
            Rule::Protected(Protection::FilesystemRoot),
        );
    }
    for target in ["/", "/*", "//./"] {
        match ActionPlan::normalize(&request(Op::DeleteRecursive, POSIX_ROOT, &[target])) {
            Ok(plan) => denied_by(
                &classify(&plan, &policy(POSIX_HOME, "")),
                Rule::Protected(Protection::FilesystemRoot),
            ),
            Err(error) => assert_eq!(
                error,
                PlanError::Target {
                    index: 0,
                    error: PathError::DevicePrefix
                }
            ),
        }
    }
}

#[test]
fn system_folders_are_denied() {
    for target in [
        "C:\\Windows",
        "C:\\Windows\\System32\\drivers\\etc",
        "c:\\WINDOWS\\Temp\\x",
        "D:\\Program Files\\tool",
        "C:\\Program Files (x86)",
        "C:\\ProgramData\\tool\\cache",
    ] {
        denied_by(
            &win(Op::Delete, &[target]),
            Rule::Protected(Protection::SystemDir),
        );
    }
    for target in [
        "/etc",
        "/etc/hosts",
        "/usr/local/bin/tool",
        "/var/lib",
        "/bin/*",
    ] {
        denied_by(
            &posix(Op::Overwrite, &[target]),
            Rule::Protected(Protection::SystemDir),
        );
    }
}

#[test]
fn system_folder_is_denied_even_inside_an_allowed_root() {
    // Not reachable through the policy file (a protected allowed root is refused), so the
    // work root is the one that sits in a system folder.
    let verdict = classify(
        &plan(Op::Delete, "/var/work/proj", &["/var/work/proj/build"]),
        &policy(POSIX_HOME, ""),
    );
    denied_by(&verdict, Rule::WorkRootProtected(Protection::SystemDir));
    assert!(verdict.has(Rule::Protected(Protection::SystemDir)));
}

#[test]
fn profile_of_another_user_is_denied() {
    denied_by(
        &win(Op::DeleteRecursive, &["C:\\Users\\user-b"]),
        Rule::Protected(Protection::UserProfile),
    );
    denied_by(
        &posix(Op::DeleteRecursive, &["/home/user-b"]),
        Rule::Protected(Protection::UserProfile),
    );
}

#[test]
fn work_root_itself_is_denied() {
    for target in [
        WIN_ROOT,
        "c:\\users\\user-a\\WORK\\PROJ\\",
        "C:\\Users\\user-a\\work\\proj\\*",
        "C:\\Users\\user-a\\work\\proj\\*.*",
    ] {
        for op in [
            Op::Delete,
            Op::DeleteRecursive,
            Op::CleanUntracked,
            Op::Overwrite,
        ] {
            denied_by(&win(op, &[target]), Rule::WorkRootItself);
        }
    }
    denied_by(
        &posix(Op::DeleteRecursive, &[POSIX_ROOT]),
        Rule::WorkRootItself,
    );
    denied_by(
        &posix(Op::DeleteRecursive, &["/home/user-a/work/proj/*"]),
        Rule::WorkRootItself,
    );
}

#[test]
fn target_outside_the_allowed_roots_is_denied() {
    for target in [
        "D:\\elsewhere\\x",
        "C:\\Users\\user-a\\work\\proj2\\build",
        "C:\\Users\\user-a\\work\\other",
        "C:\\Users\\user-a\\Documents\\x",
        "C:\\data",
        "/home/user-a/work/proj/build",
    ] {
        for op in [
            Op::Delete,
            Op::DeleteRecursive,
            Op::Overwrite,
            Op::CleanUntracked,
        ] {
            denied_by(&win(op, &[target]), Rule::OutsideAllowedRoots);
        }
    }
    // POSIX compares case: another spelling is another folder, and it is outside.
    denied_by(
        &posix(Op::Delete, &["/home/user-a/work/PROJ/build"]),
        Rule::OutsideAllowedRoots,
    );
}

#[test]
fn allowed_root_itself_is_denied() {
    let policy = policy(WIN_HOME, "allowed_roots = ['D:\\cache']");
    let verdict = classify(
        &plan(Op::DeleteRecursive, WIN_ROOT, &["D:\\cache"]),
        &policy,
    );
    denied_by(&verdict, Rule::OutsideAllowedRoots);
    let verdict = classify(
        &plan(Op::DeleteRecursive, WIN_ROOT, &["D:\\cache\\*"]),
        &policy,
    );
    denied_by(&verdict, Rule::OutsideAllowedRoots);
}

#[test]
fn repository_root_is_denied() {
    let mut request = request(
        Op::CleanUntracked,
        WIN_ROOT,
        &["C:\\Users\\user-a\\work\\proj\\vendor\\lib"],
    );
    let policy = policy(WIN_HOME, "");
    let verdict = classify(&ActionPlan::normalize(&request).unwrap(), &policy);
    assert_eq!(verdict.decision(), Decision::Allowed);

    request.targets[0].repo_root = true;
    for op in [
        Op::CleanUntracked,
        Op::DeleteRecursive,
        Op::Delete,
        Op::Overwrite,
    ] {
        request.operation = op;
        let verdict = classify(&ActionPlan::normalize(&request).unwrap(), &policy);
        denied_by(&verdict, Rule::RepositoryRoot);
    }
}

#[test]
fn quarantine_folder_itself_is_denied() {
    for target in [
        "C:\\Users\\user-a\\work\\proj\\.ads",
        "C:\\Users\\user-a\\work\\proj\\.ads\\quarantine",
        "C:\\Users\\user-a\\work\\proj\\.ADS\\*",
        "C:\\Users\\user-a\\work\\proj\\.ads\\quarantine\\*",
    ] {
        denied_by(&win(Op::DeleteRecursive, &[target]), Rule::QuarantineItself);
    }
    // With quarantine off the folder is a folder like any other.
    let verdict = classify(
        &plan(
            Op::DeleteRecursive,
            WIN_ROOT,
            &["C:\\Users\\user-a\\work\\proj\\.ads"],
        ),
        &policy(WIN_HOME, "quarantine_enabled = false"),
    );
    assert_eq!(verdict.decision(), Decision::Allowed);
}

#[test]
fn move_destination_is_judged_too() {
    let target = ["C:\\Users\\user-a\\work\\proj\\build"];
    denied_by(
        &win(move_to("D:\\elsewhere"), &target),
        Rule::DestinationOutsideAllowedRoots,
    );
    denied_by(
        &win(move_to("C:\\Windows\\Temp"), &target),
        Rule::DestinationProtected(Protection::SystemDir),
    );
    denied_by(
        &win(move_to(WIN_HOME), &target),
        Rule::DestinationProtected(Protection::UserHome),
    );
    denied_by(
        &win(
            move_to("C:\\Users\\user-a\\work\\proj\\build\\old"),
            &target,
        ),
        Rule::DestinationInsideTarget,
    );
    denied_by(
        &win(move_to("C:\\Users\\user-a\\work\\proj\\build"), &target),
        Rule::DestinationInsideTarget,
    );
    assert_eq!(
        win(move_to("C:\\Users\\user-a\\work\\proj\\old"), &target).decision(),
        Decision::Allowed
    );
    assert_eq!(
        win(
            move_to(WIN_ROOT),
            &["C:\\Users\\user-a\\work\\proj\\out\\build"]
        )
        .decision(),
        Decision::Allowed
    );

    let policy = policy(WIN_HOME, "allowed_roots = ['D:\\cache']");
    let verdict = classify(&plan(move_to("D:\\cache"), WIN_ROOT, &target), &policy);
    needs_approval_by(&verdict, Rule::DestinationOutsideWorkRoot);
}

/// The destination is the folder that receives the targets. What the move creates or replaces
/// is `destination/name of the target`, and that entry goes through the deny rules as well.
#[test]
fn entry_a_move_creates_in_its_destination_is_judged_too() {
    let entry = |verdict: &Verdict, rule: Rule, subject: &str| {
        denied_by(verdict, rule);
        let found = verdict
            .findings()
            .iter()
            .any(|finding| finding.rule == rule && finding.subject == subject);
        assert!(found, "{rule:?} on {subject}: {:?}", verdict.findings());
    };

    // Onto the work root: `/work/other/proj` moved into `/work` lands on `/work/proj`.
    let policy = policy(POSIX_HOME, "allowed_roots = ['/work']");
    let verdict = classify(
        &plan(move_to("/work"), "/work/proj", &["/work/other/proj"]),
        &policy,
    );
    entry(&verdict, Rule::DestinationEntryReserved, "/work/proj");
    // The same with another case, and onto a folder that holds the work root.
    let verdict = classify(
        &plan(move_to("/work"), "/work/proj", &["/work/other/PROJ"]),
        &policy,
    );
    entry(&verdict, Rule::DestinationEntryReserved, "/work/PROJ");
    let nested = common::policy(POSIX_HOME, "allowed_roots = ['/data']");
    let verdict = classify(
        &plan(move_to("/data"), "/data/work/proj", &["/data/tmp/work"]),
        &nested,
    );
    entry(&verdict, Rule::DestinationEntryReserved, "/data/work");

    // Onto an allowed root, and onto the quarantine folder or a folder that holds it.
    let roots = common::policy(POSIX_HOME, "allowed_roots = ['/work/proj/old/data']");
    let verdict = classify(
        &plan(
            move_to("/work/proj/old"),
            "/work/proj",
            &["/work/proj/data"],
        ),
        &roots,
    );
    entry(
        &verdict,
        Rule::DestinationEntryReserved,
        "/work/proj/old/data",
    );
    let verdict = posix(
        move_to("/home/user-a/work/proj"),
        &["/home/user-a/work/proj/tmp/.ads"],
    );
    entry(
        &verdict,
        Rule::DestinationEntryReserved,
        "/home/user-a/work/proj/.ads",
    );
    let verdict = posix(
        move_to("/home/user-a/work/proj/.ads"),
        &["/home/user-a/work/proj/tmp/quarantine"],
    );
    entry(
        &verdict,
        Rule::DestinationEntryReserved,
        "/home/user-a/work/proj/.ads/quarantine",
    );

    // Onto one of the targets: itself (a move to where it already is) or another one.
    let verdict = posix(
        move_to("/home/user-a/work/proj"),
        &["/home/user-a/work/proj/build"],
    );
    entry(
        &verdict,
        Rule::DestinationInsideTarget,
        "/home/user-a/work/proj/build",
    );
    let verdict = posix(
        move_to("/home/user-a/work/proj/a"),
        &["/home/user-a/work/proj/a/b", "/home/user-a/work/proj/b"],
    );
    entry(
        &verdict,
        Rule::DestinationInsideTarget,
        "/home/user-a/work/proj/a/b",
    );

    // A wildcard names no entry: any child of the destination it may name counts.
    let verdict = posix(
        move_to("/home/user-a/work/proj"),
        &["/home/user-a/work/proj/tmp/*"],
    );
    entry(
        &verdict,
        Rule::DestinationEntryReserved,
        "/home/user-a/work/proj/.ads",
    );
    for may_name in ["?ADS", ".a*", "*s"] {
        let target = format!("/home/user-a/work/proj/tmp/{may_name}");
        let verdict = posix(move_to("/home/user-a/work/proj"), &[&target]);
        denied_by(&verdict, Rule::DestinationEntryReserved);
    }
    // One that cannot name it is not held back, nor is the same move with quarantine off.
    for cannot in ["*.log", "?", "ads*"] {
        let target = format!("/home/user-a/work/proj/tmp/{cannot}");
        let verdict = posix(move_to("/home/user-a/work/proj"), &[&target]);
        assert_eq!(verdict.decision(), Decision::Allowed, "{cannot}");
    }
    let off = common::policy(POSIX_HOME, "quarantine_enabled = false");
    let asked = plan(
        move_to(POSIX_ROOT),
        POSIX_ROOT,
        &["/home/user-a/work/proj/tmp/.a*"],
    );
    assert_eq!(classify(&asked, &off).decision(), Decision::Allowed);
    // With quarantine off, `tmp/*` into the parent of `tmp` is still held back: the wildcard
    // may name `tmp` itself, the very folder it ranges over.
    let asked = plan(
        move_to(POSIX_ROOT),
        POSIX_ROOT,
        &["/home/user-a/work/proj/tmp/*"],
    );
    entry(
        &classify(&asked, &off),
        Rule::DestinationInsideTarget,
        "/home/user-a/work/proj/tmp",
    );

    // An entry beside all of that is the plain move it always was.
    let verdict = posix(
        move_to("/home/user-a/work/proj/old"),
        &["/home/user-a/work/proj/build"],
    );
    assert_eq!(verdict.decision(), Decision::Allowed);
    assert!(verdict.findings().is_empty());
}

/// A wildcard target names no entry, so the literal check "the entry lands on a target" did
/// not run for it: one wildcard could create, in the destination, the very folder another
/// target of the same move ranges over. Any child of the destination the wildcard may name
/// is checked against the folder of every target; when it may name it, the plan is denied.
#[test]
fn wildcard_that_may_create_the_folder_of_another_target_is_denied() {
    let on = |verdict: &Verdict, subject: &str| {
        denied_by(verdict, Rule::DestinationInsideTarget);
        let found = verdict.findings().iter().any(|finding| {
            finding.rule == Rule::DestinationInsideTarget && finding.subject == subject
        });
        assert!(found, "{subject}: {:?}", verdict.findings());
    };
    let judge = |destination: &str, targets: &[&str]| {
        classify(
            &plan(move_to(destination), "/work/proj", targets),
            &policy(POSIX_HOME, ""),
        )
    };

    // The example of the review: `tmp/*` may create `out/a`, which `out/a/*` ranges over.
    let verdict = judge(
        "/work/proj/out",
        &["/work/proj/tmp/*", "/work/proj/out/a/*"],
    );
    on(&verdict, "/work/proj/out/a");
    // The other target may be a literal one, deeper, or spelled in another case, and the
    // wildcard may be any that can name the child of the destination on the way to it.
    for (wildcard, other) in [
        ("/work/proj/tmp/*", "/work/proj/out/a"),
        ("/work/proj/tmp/*", "/work/proj/out/a/b/c.txt"),
        ("/work/proj/tmp/?", "/work/proj/out/a/*"),
        ("/work/proj/tmp/A*", "/work/proj/out/a/*"),
    ] {
        on(
            &judge("/work/proj/out", &[wildcard, other]),
            "/work/proj/out/a",
        );
    }
    on(
        &judge(
            "/work/proj/out",
            &["/work/proj/tmp/a*", "/work/proj/out/A/x"],
        ),
        "/work/proj/out/A",
    );
    // The folder the wildcard itself ranges over counts: `out/a/*` into `out` may hold an
    // `a`, which would land on `out/a`.
    on(
        &judge("/work/proj/out", &["/work/proj/out/a/*"]),
        "/work/proj/out/a",
    );
    // A literal target is checked both ways: its entry on a target, inside one or holding one.
    on(
        &judge(
            "/work/proj/out",
            &["/work/proj/tmp/a", "/work/proj/out/a/b/c.txt"],
        ),
        "/work/proj/out/a",
    );

    // A wildcard that cannot name that child is the plain move, and so is one whose
    // destination holds no folder of a target.
    for (wildcard, other) in [
        ("/work/proj/tmp/*.log", "/work/proj/out/a/*.txt"),
        ("/work/proj/tmp/b*", "/work/proj/out/a/*.txt"),
        ("/work/proj/tmp/??", "/work/proj/out/a/*.txt"),
        ("/work/proj/tmp/*", "/work/proj/src/a/*"),
    ] {
        let verdict = judge("/work/proj/out", &[wildcard, other]);
        assert_eq!(
            verdict.decision(),
            Decision::Allowed,
            "{wildcard} and {other}: {:?}",
            verdict.findings()
        );
    }
    // Judged again, a denied plan stays denied for the same reason.
    let again = classify(verdict.plan(), &policy(POSIX_HOME, ""));
    assert_eq!(again.findings(), verdict.findings());
}

// ---- Requires approval ------------------------------------------------------------------------

#[test]
fn recursive_delete_above_a_limit_requires_approval() {
    let target = ["C:\\Users\\user-a\\work\\proj\\build"];
    let policy = policy(WIN_HOME, "");
    let with = |operation: Op, estimate: Option<Estimate>| {
        let mut request = request(operation, WIN_ROOT, &target);
        request.estimate = estimate;
        classify(&ActionPlan::normalize(&request).unwrap(), &policy)
    };
    let estimate = |items, bytes| Some(Estimate { items, bytes });

    for op in [Op::DeleteRecursive, Op::CleanUntracked] {
        // Exactly at the limits: still allowed.
        let verdict = with(op.clone(), estimate(500, 104_857_600));
        assert_eq!(verdict.decision(), Decision::Allowed);
        needs_approval_by(&with(op.clone(), estimate(501, 0)), Rule::ItemLimitExceeded);
        needs_approval_by(
            &with(op.clone(), estimate(1, 104_857_601)),
            Rule::ByteLimitExceeded,
        );
        needs_approval_by(&with(op, None), Rule::EstimateUnknown);
    }
    // An overwrite is not measured.
    assert_eq!(
        with(Op::Overwrite, estimate(u64::MAX, u64::MAX)).decision(),
        Decision::Allowed
    );
    // A plain delete is measured through its rewrite: it becomes a move into quarantine, and
    // that move is measured. With quarantine off it is the plain delete, not measured.
    let verdict = with(Op::Delete, estimate(u64::MAX, u64::MAX));
    needs_approval_by(&verdict, Rule::ItemLimitExceeded);
    assert!(verdict.has(Rule::ByteLimitExceeded));
    assert!(verdict.is_quarantined());
    needs_approval_by(&with(Op::Delete, None), Rule::EstimateUnknown);
    assert_eq!(
        with(Op::Delete, estimate(500, 104_857_600)).decision(),
        Decision::Allowed
    );
    let off = common::policy(WIN_HOME, "quarantine_enabled = false");
    let mut request = request(Op::Delete, WIN_ROOT, &target);
    request.estimate = None;
    let verdict = classify(&ActionPlan::normalize(&request).unwrap(), &off);
    assert_eq!(verdict.decision(), Decision::Allowed);
}

#[test]
fn limits_come_from_the_policy() {
    let target = ["/home/user-a/work/proj/build"];
    let plan = plan(Op::DeleteRecursive, POSIX_ROOT, &target);
    let strict = policy(POSIX_HOME, "approval_item_limit = 2");
    needs_approval_by(&classify(&plan, &strict), Rule::ItemLimitExceeded);
    let strict = policy(POSIX_HOME, "approval_byte_limit = 0");
    needs_approval_by(&classify(&plan, &strict), Rule::ByteLimitExceeded);
}

#[test]
fn anything_in_an_allowed_root_outside_the_work_root_requires_approval() {
    let policy = policy(WIN_HOME, "allowed_roots = ['D:\\cache', 'E:\\scratch']");
    for op in [
        Op::Overwrite,
        Op::Delete,
        Op::DeleteRecursive,
        Op::CleanUntracked,
    ] {
        let verdict = classify(&plan(op, WIN_ROOT, &["D:\\cache\\tool\\index"]), &policy);
        needs_approval_by(&verdict, Rule::OutsideWorkRoot);
    }
    // Overwriting inside the work root needs nothing; outside every root it is denied.
    let inside = plan(
        Op::Overwrite,
        WIN_ROOT,
        &["C:\\Users\\user-a\\work\\proj\\out.txt"],
    );
    assert_eq!(classify(&inside, &policy).decision(), Decision::Allowed);
    let outside = plan(Op::Overwrite, WIN_ROOT, &["F:\\other\\out.txt"]);
    denied_by(&classify(&outside, &policy), Rule::OutsideAllowedRoots);
}

#[test]
fn deleting_from_quarantine_requires_approval_and_is_not_rewritten() {
    let inside = "C:\\Users\\user-a\\work\\proj\\.ads\\quarantine\\0123456789abcdef";
    let verdict = win(Op::DeleteRecursive, &[inside]);
    needs_approval_by(&verdict, Rule::QuarantinePurge);
    assert!(!verdict.is_quarantined());
    assert_eq!(verdict.plan().operation(), &Operation::DeleteRecursive);

    // Mixed with a common target, the whole plan stays a real delete that needs approval.
    let verdict = win(
        Op::Delete,
        &[inside, "C:\\Users\\user-a\\work\\proj\\build\\a.o"],
    );
    needs_approval_by(&verdict, Rule::QuarantinePurge);
    assert!(!verdict.is_quarantined());
}

// ---- Allowed, and quarantine by default -------------------------------------------------------

#[test]
fn the_rest_inside_the_work_root_is_allowed() {
    let targets = [
        "C:\\Users\\user-a\\work\\proj\\build",
        "c:/users/user-a/work/proj/target/debug/*.pdb",
    ];
    for op in [
        Op::Delete,
        Op::DeleteRecursive,
        Op::Overwrite,
        Op::CleanUntracked,
        move_to("C:\\Users\\user-a\\work\\proj\\old"),
    ] {
        let verdict = win(op, &targets);
        assert_eq!(
            verdict.decision(),
            Decision::Allowed,
            "{:?}",
            verdict.findings()
        );
        assert!(verdict.findings().is_empty());
        assert_eq!(verdict.risk(), Risk::Low);
    }
    let verdict = posix(Op::DeleteRecursive, &["/home/user-a/work/proj/build"]);
    assert_eq!(verdict.decision(), Decision::Allowed);
}

#[test]
fn verdict_levels_map_onto_the_contract_risk() {
    assert_eq!(Decision::Allowed.risk(), Risk::Low);
    assert_eq!(Decision::RequiresApproval.risk(), Risk::Medium);
    assert_eq!(Decision::Denied.risk(), Risk::High);
    assert!(Decision::Allowed < Decision::RequiresApproval);
    assert!(Decision::RequiresApproval < Decision::Denied);
}

#[test]
fn delete_is_rewritten_as_a_move_to_quarantine() {
    let targets = [
        "C:\\Users\\user-a\\work\\proj\\build",
        "C:\\Users\\user-a\\work\\proj\\tmp\\*.log",
    ];
    for op in [Op::Delete, Op::DeleteRecursive] {
        let asked = plan(op, WIN_ROOT, &targets);
        let verdict = classify(&asked, &policy(WIN_HOME, ""));
        assert_eq!(verdict.decision(), Decision::Allowed);
        assert!(verdict.is_quarantined());

        let Operation::Move { destination } = verdict.plan().operation() else {
            panic!("not a move: {:?}", verdict.plan().operation());
        };
        let asked_hash = asked.hash();
        let folder = &asked_hash.as_str()["sha256:".len()..][..16];
        assert_eq!(
            destination.to_string(),
            format!("C:\\Users\\user-a\\work\\proj\\.ads\\quarantine\\{folder}")
        );
        assert!(destination.is_strictly_within(asked.work_root()));
        assert_eq!(verdict.plan().targets(), asked.targets());
        assert_eq!(verdict.plan().estimate(), asked.estimate());
        // The plan to approve and execute is the rewritten one.
        assert_ne!(verdict.plan().hash(), asked.hash());
        // And it passes the gate as it is: a move inside the work root.
        let again = classify(verdict.plan(), &policy(WIN_HOME, ""));
        assert_eq!(again.decision(), Decision::Allowed);
        assert!(!again.is_quarantined());
    }
}

#[test]
fn rewrite_keeps_the_approval_requirement() {
    let mut request = request(
        Op::DeleteRecursive,
        POSIX_ROOT,
        &["/home/user-a/work/proj/node_modules"],
    );
    request.estimate = Some(Estimate {
        items: 40_000,
        bytes: 1 << 30,
    });
    let verdict = classify(
        &ActionPlan::normalize(&request).unwrap(),
        &policy(POSIX_HOME, ""),
    );
    needs_approval_by(&verdict, Rule::ItemLimitExceeded);
    assert!(verdict.has(Rule::ByteLimitExceeded));
    assert!(verdict.is_quarantined());
    assert!(matches!(verdict.plan().operation(), Operation::Move { .. }));
}

#[test]
fn quarantine_can_be_turned_off_or_moved_by_the_policy() {
    let asked = plan(
        Op::DeleteRecursive,
        POSIX_ROOT,
        &["/home/user-a/work/proj/build"],
    );
    let off = classify(&asked, &policy(POSIX_HOME, "quarantine_enabled = false"));
    assert_eq!(off.decision(), Decision::Allowed);
    assert!(!off.is_quarantined());
    assert_eq!(off.plan(), &asked);

    let moved = classify(&asked, &policy(POSIX_HOME, "quarantine_dir = 'trash'"));
    let Operation::Move { destination } = moved.plan().operation() else {
        panic!("not a move");
    };
    assert!(
        destination
            .to_string()
            .starts_with("/home/user-a/work/proj/trash/")
    );
}

#[test]
fn only_deletes_are_rewritten() {
    let target = ["/home/user-a/work/proj/build"];
    for op in [
        Op::Overwrite,
        Op::CleanUntracked,
        move_to("/home/user-a/work/proj/old"),
    ] {
        let asked = plan(op, POSIX_ROOT, &target);
        let verdict = classify(&asked, &policy(POSIX_HOME, ""));
        assert!(!verdict.is_quarantined());
        assert_eq!(verdict.plan(), &asked);
    }
}

#[test]
fn classification_is_deterministic() {
    let asked = plan(
        Op::DeleteRecursive,
        WIN_ROOT,
        &["C:\\Users\\user-a\\work\\proj\\build", "D:\\elsewhere"],
    );
    let policy = policy(WIN_HOME, "");
    assert_eq!(classify(&asked, &policy), classify(&asked, &policy));
}

// ---- Rule codes -------------------------------------------------------------------------------

/// The codes are part of the contract: they go into `action.denied` and into the action plan
/// record, so a code never changes. Renaming one here fails this test.
#[test]
fn every_rule_has_its_stable_code() {
    let protections = [
        "filesystem_root",
        "user_home",
        "user_home_ancestor",
        "user_profile",
        "system_dir",
        "other_path_style",
    ];
    let mut denied: Vec<String> = Vec::new();
    for rule in ["protected", "work_root_protected", "destination_protected"] {
        denied.extend(protections.iter().map(|detail| format!("{rule}.{detail}")));
    }
    for rule in [
        "work_root_itself",
        "allowed_root_itself",
        "outside_allowed_roots",
        "repository_root",
        "quarantine_itself",
        "destination_outside_allowed_roots",
        "destination_inside_target",
        "destination_entry_reserved",
    ] {
        denied.push(rule.to_owned());
    }
    let approval = [
        "outside_work_root",
        "destination_outside_work_root",
        "estimate_unknown",
        "item_limit_exceeded",
        "byte_limit_exceeded",
        "quarantine_purge",
    ];

    let codes_of = |decision: Decision| -> Vec<String> {
        Rule::all()
            .into_iter()
            .filter(|rule| rule.decision() == decision)
            .map(|rule| rule.code().to_string())
            .collect()
    };
    assert_eq!(codes_of(Decision::Denied), denied);
    assert_eq!(codes_of(Decision::RequiresApproval), approval);
    assert!(codes_of(Decision::Allowed).is_empty());
    assert_eq!(Rule::all().len(), 32);

    assert_eq!(
        Rule::Protected(Protection::UserHome).code().as_str(),
        "protected.user_home"
    );
    assert_eq!(
        Rule::DestinationProtected(Protection::SystemDir)
            .code()
            .as_str(),
        "destination_protected.system_dir"
    );
}

/// The two places that list the codes for readers (the contracts and this crate) name every
/// code the gate can write.
#[test]
fn every_rule_code_is_documented() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for readme in ["../../contracts/README.md", "README.md"] {
        let text = std::fs::read_to_string(root.join(readme)).unwrap();
        for rule in Rule::all() {
            let code = rule.code().to_string();
            let listed = match code.split_once('.') {
                Some((name, detail)) => {
                    text.contains(&format!("`{name}.<proteção>`"))
                        && text.contains(&format!("`{detail}`"))
                }
                None => text.contains(&format!("`{code}`")),
            };
            assert!(listed, "{code} is not in {readme}");
        }
    }
}
