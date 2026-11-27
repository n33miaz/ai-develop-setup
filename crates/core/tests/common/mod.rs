//! Shared by the gate tests. Every path is synthetic text: nothing here touches the file system.
#![allow(dead_code)]

use std::num::NonZeroU64;

use ads_core::gate::{
    AbsPath, ActionPlan, Actor, ActorKind, DataClass, Estimate, EventMeta, GatePolicy, HostFacts,
    Id, OperationRequest, PlanRequest, TargetRequest, Timestamp, Ulid, Verdict, classify,
};

pub const WIN_HOME: &str = "C:\\Users\\user-a";
pub const WIN_ROOT: &str = "C:\\Users\\user-a\\work\\proj";
pub const POSIX_HOME: &str = "/home/user-a";
pub const POSIX_ROOT: &str = "/home/user-a/work/proj";

pub const SMALL: Option<Estimate> = Some(Estimate {
    items: 3,
    bytes: 4096,
});

pub fn policy(home: &str, overrides: &str) -> GatePolicy {
    GatePolicy::from_toml_str(overrides, &HostFacts { user_home: home })
        .unwrap()
        .with_work_roots(
            [WIN_ROOT, POSIX_ROOT, "/work/proj", r"C:\work\proj"]
                .map(|path| AbsPath::parse(path).unwrap())
                .to_vec(),
        )
}

pub fn request(operation: OperationRequest, work_root: &str, targets: &[&str]) -> PlanRequest {
    PlanRequest {
        operation,
        targets: targets
            .iter()
            .map(|path| TargetRequest {
                path: (*path).to_owned(),
                repo_root: false,
            })
            .collect(),
        work_root: work_root.to_owned(),
        estimate: SMALL,
    }
}

pub fn plan(operation: OperationRequest, work_root: &str, targets: &[&str]) -> ActionPlan {
    ActionPlan::normalize(&request(operation, work_root, targets)).unwrap()
}

/// The verdict for a Windows plan under the shipped defaults.
pub fn win(operation: OperationRequest, targets: &[&str]) -> Verdict {
    classify(&plan(operation, WIN_ROOT, targets), &policy(WIN_HOME, "")).unwrap()
}

/// The verdict for a POSIX plan under the shipped defaults.
pub fn posix(operation: OperationRequest, targets: &[&str]) -> Verdict {
    classify(
        &plan(operation, POSIX_ROOT, targets),
        &policy(POSIX_HOME, ""),
    )
    .unwrap()
}

pub fn id(text: &str) -> Id {
    Id::new(text).unwrap()
}

pub fn at(text: &str) -> Timestamp {
    Timestamp::new(text).unwrap()
}

pub fn meta(ts: &str, seq: u64) -> EventMeta {
    EventMeta {
        id: Ulid::new("01ARZ3NDEKTSV4RRFFQ69G5FAB").unwrap(),
        ts: at(ts),
        seq: NonZeroU64::new(seq).unwrap(),
        actor: Actor {
            kind: ActorKind::Daemon,
            id: id("core-01"),
        },
        cause: None,
        data_class: DataClass::Internal,
    }
}

/// The canonical serialization of `contracts/README.md`, written again here from the JSON of
/// a `record.action-plan`, so the tests do not check the core against its own code.
pub fn canonical_of(record: &serde_json::Value) -> String {
    let text = |value: &serde_json::Value| value.as_str().unwrap().to_owned();
    let field = |text: &str| format!("{}:{text}", text.len());
    let operation = if record["quarantined"] == true {
        "fs.move".to_owned()
    } else {
        text(&record["operation"])
    };
    let mut lines = vec![
        "ads.action-plan/1".to_owned(),
        format!("op:{operation}"),
        format!("work_root:{}", field(&text(&record["work_root"]))),
    ];
    lines.push(match record["destination"].as_str() {
        Some(destination) => format!("destination:{}", field(destination)),
        None => "destination:-".to_owned(),
    });
    lines.push(match &record["estimate"] {
        serde_json::Value::Null => "estimate:unknown".to_owned(),
        estimate => format!("estimate:{}:{}", estimate["items"], estimate["bytes"]),
    });
    let targets = record["targets"].as_array().unwrap();
    lines.push(format!("targets:{}", targets.len()));
    for target in targets {
        let path = text(&target["path"]);
        let whole = match target["pattern"].as_str() {
            None => path,
            Some(pattern) => {
                let windows = !path.starts_with('/');
                let root = path == "/" || (windows && path.len() == 3);
                let separator = match (root, windows) {
                    (true, _) => "",
                    (false, true) => "\\",
                    (false, false) => "/",
                };
                format!("{path}{separator}{pattern}")
            }
        };
        let repo_root = u8::from(target["repo_root"] == true);
        lines.push(format!("target:{repo_root}:{}", field(&whole)));
    }
    lines.join("\n") + "\n"
}

/// `sha256:` and the digest of `text`.
pub fn sha256_of(text: &str) -> String {
    ads_core::gate::sha256_text(text.as_bytes())
}
