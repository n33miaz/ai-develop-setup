//! Property tests with a fixed-seed generator (the workspace has no property-testing library).
//! The oracle works on the pieces each path was built from, never on the gate's own parsing
//! or containment code. Paths are synthetic text; nothing is touched.

mod common;

use ads_core::gate::{
    Action, ActionPlan, Actor, ActorKind, DataClass, Decision, Estimate, Event, EventMeta,
    GatePolicy, HostFacts, Id, Operation, OperationRequest, PlanRequest, State, TargetRequest,
    Timestamp, Ulid, classify,
};
use common::{canonical_of, sha256_of};

const CASES: usize = 30_000;

/// SplitMix64: small, deterministic, good enough to spread the cases.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }

    fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

/// A folder as the generator knows it, before it is spelled as text.
#[derive(Debug, Clone, PartialEq)]
struct Loc {
    windows: bool,
    drive: char,
    names: Vec<String>,
}

fn loc(text: &str) -> Loc {
    let windows = !text.starts_with('/');
    let (drive, rest) = if windows {
        (text.chars().next().unwrap(), &text[2..])
    } else {
        ('-', text)
    };
    Loc {
        windows,
        drive,
        names: rest
            .split(['/', '\\'])
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
            .collect(),
    }
}

impl Loc {
    fn child(&self, name: &str) -> Loc {
        let mut child = self.clone();
        child.names.push(name.to_owned());
        child
    }

    /// One of the many spellings of the same folder.
    fn spell(&self, rng: &mut Rng, pattern: Option<&str>) -> String {
        let mut names: Vec<String> = self.names.clone();
        names.extend(pattern.map(str::to_owned));
        let mut text = String::new();
        if self.windows {
            let drive = if rng.chance(30) {
                self.drive.to_ascii_lowercase()
            } else {
                self.drive
            };
            text.push(drive);
            text.push(':');
            if names.is_empty() {
                text.push_str(rng.pick(&["\\", "/", "\\\\"]));
            }
            for name in &names {
                text.push_str(rng.pick(&["\\", "\\", "/", "\\\\", "\\.\\", "/./"]));
                // Windows ignores case: another case is the same folder.
                match rng.below(6) {
                    0 => text.push_str(&name.to_uppercase()),
                    1 => text.push_str(&name.to_lowercase()),
                    _ => text.push_str(name),
                }
            }
            if !names.is_empty() && pattern.is_none() && rng.chance(20) {
                text.push_str(rng.pick(&["\\", "/", "\\."]));
            }
        } else {
            if names.is_empty() {
                text.push('/');
            }
            for (index, name) in names.iter().enumerate() {
                // Two slashes at the very start would spell a network path, not this folder.
                let separators: &[&str] = if index == 0 {
                    &["/", "/./"]
                } else {
                    &["/", "/", "/", "//", "/./"]
                };
                text.push_str(rng.pick(separators));
                text.push_str(name);
            }
            if !names.is_empty() && pattern.is_none() && rng.chance(20) {
                text.push_str(rng.pick(&["/", "/."]));
            }
        }
        text
    }
}

/// The oracle: `inner` is `outer` or below it. Written over the generator's own pieces.
fn within(inner: &Loc, outer: &Loc) -> bool {
    if inner.windows != outer.windows || inner.names.len() < outer.names.len() {
        return false;
    }
    if inner.windows && !inner.drive.eq_ignore_ascii_case(&outer.drive) {
        return false;
    }
    inner.names.iter().zip(&outer.names).all(|(a, b)| {
        if inner.windows {
            a.eq_ignore_ascii_case(b)
        } else {
            a == b
        }
    })
}

fn strictly_within(inner: &Loc, outer: &Loc) -> bool {
    inner.names.len() > outer.names.len() && within(inner, outer)
}

struct Case {
    request: PlanRequest,
    policy: GatePolicy,
    work_root: Loc,
    allowed_roots: Vec<Loc>,
    /// The folder each target ranges over.
    scopes: Vec<Loc>,
    destination: Option<Loc>,
    /// The work root is written in the other style than the user home.
    other_style: bool,
}

impl Case {
    fn inside_the_roots(&self, scope: &Loc) -> bool {
        strictly_within(scope, &self.work_root)
            || self
                .allowed_roots
                .iter()
                .any(|root| strictly_within(scope, root))
    }

    fn has_target_outside(&self) -> bool {
        self.scopes
            .iter()
            .any(|scope| !self.inside_the_roots(scope))
    }

    fn has_destination_outside(&self) -> bool {
        self.destination.as_ref().is_some_and(|destination| {
            !within(destination, &self.work_root)
                && !self
                    .allowed_roots
                    .iter()
                    .any(|root| within(destination, root))
        })
    }
}

const EXTRA_NAMES: &[&str] = &[
    "build",
    "src",
    "target",
    "a.o",
    "node_modules",
    ".ads",
    "quarantine",
    "Windows",
    "etc",
    "cache",
    "proj",
    "Users",
];

fn generate(rng: &mut Rng) -> Case {
    let windows = rng.chance(50);
    let (home, work_roots, root_pool, elsewhere): (&str, &[&str], &[&str], &[&str]) = if windows {
        (
            "C:\\Users\\user-a",
            &[
                "C:\\Users\\user-a\\work\\proj",
                "D:\\work\\proj",
                "C:\\dev\\app",
            ],
            &["D:\\cache", "E:\\scratch\\tmp", "C:\\dev\\shared"],
            &[
                "C:\\",
                "D:\\",
                "C:\\Users",
                "C:\\Users\\user-a",
                "C:\\Users\\user-b",
                "C:\\Windows",
                "C:\\Windows\\System32",
                "C:\\Program Files",
                "C:\\dev",
                "E:\\scratch",
                "F:\\other",
                "/home/user-a/work/proj",
                "/work/proj",
            ],
        )
    } else {
        (
            "/home/user-a",
            &["/home/user-a/work/proj", "/work/proj", "/mnt/data/app"],
            &["/mnt/cache", "/work/shared", "/scratch/tmp"],
            &[
                "/",
                "/home",
                "/home/user-a",
                "/home/user-b",
                "/etc",
                "/usr/lib",
                "/var/tmp",
                "/mnt",
                "/work",
                "/scratch",
                "/other",
                "C:\\Users\\user-a\\work\\proj",
                "D:\\work\\proj",
            ],
        )
    };
    // Now and then the whole plan comes in the style the user home is not written in.
    let other_style = rng.chance(8);
    let work_root = if other_style {
        let pool: &[&str] = if windows {
            &["/c/Users/user-a", "/c/Users/user-a/work/proj", "/work/proj"]
        } else {
            &[
                "C:\\home\\user-a",
                "C:\\home\\user-a\\work\\proj",
                "D:\\work\\proj",
            ]
        };
        loc(rng.pick(pool))
    } else {
        loc(rng.pick(work_roots))
    };
    let allowed_roots: Vec<Loc> = root_pool
        .iter()
        .filter(|_| rng.chance(40))
        .map(|root| loc(root))
        .collect();

    let mut overrides = format!(
        "allowed_roots = [{}]\n",
        allowed_roots
            .iter()
            .map(|root| format!("'{}'", root.spell(&mut Rng(1), None)))
            .collect::<Vec<_>>()
            .join(", ")
    );
    if rng.chance(30) {
        overrides.push_str("quarantine_enabled = false\n");
    }
    if rng.chance(30) {
        overrides.push_str(&format!("approval_item_limit = {}\n", rng.below(1000)));
    }
    // Now and then the quarantine sits somewhere else in the work root.
    if rng.chance(20) {
        let dir = *rng.pick(&["trash", "build/.trash", "proj", "cache/q"]);
        overrides.push_str(&format!("quarantine_dir = '{dir}'\n"));
    }
    let policy = GatePolicy::from_toml_str(&overrides, &HostFacts { user_home: home })
        .unwrap_or_else(|err| panic!("{overrides}: {err}"));

    // A folder to aim at: a root, a near miss of one, or somewhere else entirely.
    let aim = |rng: &mut Rng| -> Loc {
        let roots: Vec<&Loc> = std::iter::once(&work_root).chain(&allowed_roots).collect();
        let root = (*rng.pick(&roots)).clone();
        let mut base = match rng.below(10) {
            0..=4 => root,
            5 => {
                // The sibling whose name only starts the same way.
                let mut near = root;
                let last = near.names.pop().unwrap();
                near.names.push(format!("{last}2"));
                near
            }
            6 => {
                let mut parent = root;
                parent.names.pop();
                parent
            }
            7 => {
                // Another case: the same folder on Windows, another one on POSIX.
                let mut other = root;
                let last = other.names.pop().unwrap();
                other.names.push(last.to_uppercase());
                other
            }
            8 => {
                let mut other = root;
                if other.windows {
                    other.drive = 'Z';
                } else {
                    other.names.insert(0, "mnt2".to_owned());
                }
                other
            }
            _ => loc(rng.pick(elsewhere)),
        };
        for _ in 0..rng.below(4) {
            base = base.child(rng.pick(EXTRA_NAMES));
        }
        base
    };

    let mut targets = Vec::new();
    let mut scopes = Vec::new();
    for _ in 0..1 + rng.below(4) {
        let scope = aim(rng);
        let pattern = rng
            .chance(20)
            .then(|| *rng.pick(&["*", "*.log", "?", "build-*"]));
        targets.push(TargetRequest {
            path: scope.spell(rng, pattern),
            repo_root: rng.chance(5),
        });
        scopes.push(scope);
    }

    let mut destination = None;
    let operation = match rng.below(5) {
        0 => OperationRequest::Delete,
        1 => OperationRequest::DeleteRecursive,
        2 => OperationRequest::Overwrite,
        3 => OperationRequest::CleanUntracked,
        _ => {
            let to = aim(rng);
            let text = to.spell(rng, None);
            destination = Some(to);
            OperationRequest::Move { destination: text }
        }
    };
    let estimate = match rng.below(3) {
        0 => None,
        1 => Some(Estimate {
            items: 3,
            bytes: 4096,
        }),
        _ => Some(Estimate {
            items: 1_000_000,
            bytes: 1 << 40,
        }),
    };
    let work_root_text = work_root.spell(rng, None);
    Case {
        request: PlanRequest {
            operation,
            targets,
            work_root: work_root_text,
            estimate,
        },
        policy,
        work_root,
        allowed_roots,
        scopes,
        destination,
        other_style,
    }
}

fn meta() -> EventMeta {
    EventMeta {
        id: Ulid::new("01ARZ3NDEKTSV4RRFFQ69G5FAB").unwrap(),
        ts: Timestamp::new("2030-01-02T03:04:05Z").unwrap(),
        seq: std::num::NonZeroU64::new(1).unwrap(),
        actor: Actor {
            kind: ActorKind::Daemon,
            id: Id::new("core-01").unwrap(),
        },
        cause: None,
        data_class: DataClass::Internal,
    }
}

#[test]
fn no_plan_with_a_target_outside_the_allowed_roots_gets_past_denied() {
    let mut rng = Rng(0x00AD_5C0D_E000_0001);
    let (mut outside, mut inside_passed, mut inside_denied, mut other_style) = (0, 0, 0, 0);
    let (mut rewritten_with_approval, mut rewritten_plain_delete) = (0, 0);
    for index in 0..CASES {
        let case = generate(&mut rng);
        let plan = ActionPlan::normalize(&case.request)
            .unwrap_or_else(|err| panic!("case {index}: {:?}: {err}", case.request));
        let verdict = classify(&plan, &case.policy);

        // Whatever the verdict, the plan has a record, and the hash of the plan in force is
        // the one of the canonical text rebuilt here from the record alone.
        let record = verdict
            .record(Id::new("act-01").unwrap())
            .unwrap_or_else(|err| panic!("case {index}: {:?}: {err}", case.request))
            .to_json();
        assert_eq!(record["plan_hash"], verdict.plan().hash().as_str());
        assert_eq!(
            record["plan_hash"],
            sha256_of(&canonical_of(&record)),
            "case {index}: {record}"
        );
        assert_eq!(record["quarantined"], verdict.is_quarantined());
        // Judging again the plan that came out never asks for less; the case that matters is
        // the delete that needed approval and was rewritten as a move to quarantine.
        rewritten_with_approval += usize::from(
            verdict.is_quarantined() && verdict.decision() == Decision::RequiresApproval,
        );
        let again = classify(verdict.plan(), &case.policy);
        assert!(
            again.decision() >= verdict.decision(),
            "case {index}: {:?} became {:?}: {:?}",
            verdict.decision(),
            again.decision(),
            case.request
        );
        // Nor for more: the verdict already holds everything judging its plan again finds.
        // The generated destination was judged before the verdict was closed.
        assert!(
            again.decision() <= verdict.decision(),
            "case {index}: {:?} became {:?}: {:?}",
            verdict.decision(),
            again.decision(),
            case.request
        );
        for finding in again.findings() {
            assert!(
                verdict.findings().contains(finding),
                "case {index}: {finding:?} only shows when judged again: {:?}",
                case.request
            );
        }
        assert_eq!(again.plan(), verdict.plan(), "case {index}");
        assert_eq!(again.plan().hash(), verdict.plan().hash(), "case {index}");
        let plain_delete = matches!(case.request.operation, OperationRequest::Delete);
        rewritten_plain_delete += usize::from(verdict.is_quarantined() && plain_delete);
        // No finding comes twice.
        for (at, finding) in verdict.findings().iter().enumerate() {
            assert!(
                !verdict.findings()[..at].contains(finding),
                "case {index}: {finding:?} twice"
            );
        }

        if case.other_style {
            other_style += 1;
            assert_eq!(
                verdict.decision(),
                Decision::Denied,
                "case {index}: {:?}",
                case.request
            );
        }
        if case.has_target_outside() || case.has_destination_outside() {
            outside += 1;
            assert_eq!(
                verdict.decision(),
                Decision::Denied,
                "case {index}: {:?}",
                case.request
            );
            assert!(!verdict.is_quarantined(), "case {index}");
            assert_eq!(verdict.plan(), &plan, "case {index}");
            // And a denied plan is an action born dead: `action.denied`, then nothing.
            let (action, event, record) =
                Action::plan(Id::new("act-01").unwrap(), &verdict, meta()).unwrap();
            assert_eq!(action.state(), &State::Denied, "case {index}");
            assert!(matches!(event, Event::ActionDenied(_)), "case {index}");
            assert_eq!(event.to_json()["data"]["plan_hash"], plan.hash().as_str());
            assert_eq!(record.fields().plan_hash, plan.hash(), "case {index}");
            assert!(
                action.authorize(&plan, meta()).is_err(),
                "case {index}: {:?}",
                case.request
            );
        } else if verdict.decision() == Decision::Denied {
            inside_denied += 1;
        } else {
            inside_passed += 1;
        }
    }
    // The generator must reach all three regions, or the property proves nothing.
    assert!(outside > CASES / 4, "outside: {outside}");
    assert!(
        inside_passed > CASES / 50,
        "inside, passed: {inside_passed}"
    );
    assert!(
        inside_denied > CASES / 200,
        "inside, denied: {inside_denied}"
    );
    assert!(other_style > CASES / 50, "other style: {other_style}");
    assert!(
        rewritten_with_approval > CASES / 500,
        "rewritten with approval: {rewritten_with_approval}"
    );
    assert!(
        rewritten_plain_delete > CASES / 500,
        "plain delete rewritten: {rewritten_plain_delete}"
    );
}

#[test]
fn what_gets_past_the_gate_stays_inside_the_allowed_roots() {
    let mut rng = Rng(0x00AD_5C0D_E000_0002);
    let (mut passed, mut quarantined) = (0, 0);
    for index in 0..CASES {
        let case = generate(&mut rng);
        let plan = ActionPlan::normalize(&case.request).unwrap();
        let verdict = classify(&plan, &case.policy);
        if verdict.decision() == Decision::Denied {
            continue;
        }
        passed += 1;
        for scope in &case.scopes {
            assert!(
                case.inside_the_roots(scope),
                "case {index}: {scope:?} got past the gate: {:?}",
                case.request
            );
        }
        // The plan to execute keeps the targets that were judged.
        assert_eq!(verdict.plan().targets(), plan.targets(), "case {index}");
        if verdict.is_quarantined() {
            quarantined += 1;
            let Operation::Move { destination } = verdict.plan().operation() else {
                panic!("case {index}: quarantined but not a move");
            };
            assert!(
                destination.is_strictly_within(plan.work_root()),
                "case {index}: quarantine outside the work root"
            );
            assert!(matches!(
                plan.operation(),
                Operation::Delete | Operation::DeleteRecursive
            ));
        }
        // The rewritten plan is still not a way around the gate.
        let again = classify(verdict.plan(), &case.policy);
        assert_ne!(again.decision(), Decision::Denied, "case {index}");
        assert!(!again.is_quarantined(), "case {index}");
    }
    assert!(passed > CASES / 50, "passed: {passed}");
    assert!(quarantined > CASES / 500, "quarantined: {quarantined}");
}

#[test]
fn adding_a_target_outside_denies_a_plan_that_passed() {
    let mut rng = Rng(0x00AD_5C0D_E000_0003);
    let mut flipped = 0;
    for index in 0..CASES {
        let case = generate(&mut rng);
        let plan = ActionPlan::normalize(&case.request).unwrap();
        if classify(&plan, &case.policy).decision() == Decision::Denied {
            continue;
        }
        let outsider = if case.work_root.windows {
            loc("F:\\other\\thing")
        } else {
            loc("/other/thing")
        };
        assert!(!case.inside_the_roots(&outsider));
        let mut request = case.request.clone();
        let at = rng.below(request.targets.len() + 1);
        request.targets.insert(
            at,
            TargetRequest {
                path: outsider.spell(&mut rng, None),
                repo_root: false,
            },
        );
        let widened = ActionPlan::normalize(&request).unwrap();
        assert_ne!(widened.hash(), plan.hash(), "case {index}");
        assert_eq!(
            classify(&widened, &case.policy).decision(),
            Decision::Denied,
            "case {index}: {request:?}"
        );
        flipped += 1;
    }
    assert!(flipped > CASES / 50, "flipped: {flipped}");
}
