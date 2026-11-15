//! The plan of a file system action, written down before anything happens.

use std::fmt;

use sha2::{Digest, Sha256};

use super::event::{PlanHash, Verb};
use super::path::{AbsPath, PathError, TargetPath};

/// What the caller wants done, as raw text. Nothing here is trusted until normalized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanRequest {
    pub operation: OperationRequest,
    pub targets: Vec<TargetRequest>,
    /// The declared work root: the folder the action is supposed to stay in.
    pub work_root: String,
    /// What a dry run counted. `None` means nobody measured.
    pub estimate: Option<Estimate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationRequest {
    Delete,
    DeleteRecursive,
    /// `destination` is the folder that receives the targets.
    Move {
        destination: String,
    },
    Overwrite,
    CleanUntracked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetRequest {
    pub path: String,
    /// The caller found this path to be the root of a repository or worktree. It comes in as
    /// data because the core does no I/O.
    pub repo_root: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Estimate {
    pub items: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Operation {
    Delete,
    DeleteRecursive,
    Move { destination: AbsPath },
    Overwrite,
    CleanUntracked,
}

impl Operation {
    /// The `verb` of `action.planned`.
    pub fn verb(&self) -> Verb {
        let verb = match self {
            Self::Delete => "fs.delete",
            Self::DeleteRecursive => "fs.delete_recursive",
            Self::Move { .. } => "fs.move",
            Self::Overwrite => "fs.overwrite",
            Self::CleanUntracked => "fs.clean_untracked",
        };
        Verb::new(verb).expect("the verbs above follow the contract pattern")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Target {
    path: TargetPath,
    repo_root: bool,
}

impl Target {
    pub fn path(&self) -> &TargetPath {
        &self.path
    }

    pub fn is_repo_root(&self) -> bool {
        self.repo_root
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    NoTargets,
    WorkRoot(PathError),
    Destination(PathError),
    /// One bad target refuses the whole plan; `index` is its position in the request.
    Target {
        index: usize,
        error: PathError,
    },
}

impl fmt::Display for PlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoTargets => f.write_str("the plan has no targets"),
            Self::WorkRoot(error) => write!(f, "work root: {error}"),
            Self::Destination(error) => write!(f, "destination: {error}"),
            Self::Target { index, error } => write!(f, "target {index}: {error}"),
        }
    }
}

/// A plan whose every path is absolute and normalized. Only [`ActionPlan::normalize`] builds
/// one, so a relative or unresolved target cannot reach the rules or the hash.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ActionPlan {
    operation: Operation,
    work_root: AbsPath,
    /// Sorted and without repeats, so the order of the request does not change the hash.
    targets: Vec<Target>,
    estimate: Option<Estimate>,
}

impl ActionPlan {
    pub fn normalize(request: &PlanRequest) -> Result<Self, PlanError> {
        if request.targets.is_empty() {
            return Err(PlanError::NoTargets);
        }
        let work_root = AbsPath::parse(&request.work_root).map_err(PlanError::WorkRoot)?;
        let operation = match &request.operation {
            OperationRequest::Delete => Operation::Delete,
            OperationRequest::DeleteRecursive => Operation::DeleteRecursive,
            OperationRequest::Overwrite => Operation::Overwrite,
            OperationRequest::CleanUntracked => Operation::CleanUntracked,
            OperationRequest::Move { destination } => Operation::Move {
                destination: AbsPath::parse(destination).map_err(PlanError::Destination)?,
            },
        };
        let mut targets = Vec::with_capacity(request.targets.len());
        for (index, target) in request.targets.iter().enumerate() {
            let path = TargetPath::parse(&target.path)
                .map_err(|error| PlanError::Target { index, error })?;
            targets.push(Target {
                path,
                repo_root: target.repo_root,
            });
        }
        // Repeats of one path collapse into one target that keeps the stricter fact.
        targets.sort_by(|a, b| a.path.cmp(&b.path).then(b.repo_root.cmp(&a.repo_root)));
        targets.dedup_by(|later, first| later.path == first.path);
        Ok(Self {
            operation,
            work_root,
            targets,
            estimate: request.estimate,
        })
    }

    pub fn operation(&self) -> &Operation {
        &self.operation
    }

    pub fn work_root(&self) -> &AbsPath {
        &self.work_root
    }

    pub fn targets(&self) -> &[Target] {
        &self.targets
    }

    pub fn estimate(&self) -> Option<Estimate> {
        self.estimate
    }

    /// The same targets, moved to `destination` instead of deleted.
    pub(crate) fn as_move_to(&self, destination: AbsPath) -> ActionPlan {
        ActionPlan {
            operation: Operation::Move { destination },
            ..self.clone()
        }
    }

    /// The text the hash is taken over: one field per line, in a fixed order, every path
    /// prefixed by its length in bytes so no path can pass for two fields.
    pub fn canonical(&self) -> String {
        let field = |text: String| format!("{}:{text}", text.len());
        let mut out = String::from("ads.action-plan/1\n");
        out.push_str(&format!("op:{}\n", self.operation.verb()));
        out.push_str(&format!(
            "work_root:{}\n",
            field(self.work_root.to_string())
        ));
        match &self.operation {
            Operation::Move { destination } => {
                out.push_str(&format!("destination:{}\n", field(destination.to_string())));
            }
            _ => out.push_str("destination:-\n"),
        }
        match self.estimate {
            Some(estimate) => {
                out.push_str(&format!("estimate:{}:{}\n", estimate.items, estimate.bytes));
            }
            None => out.push_str("estimate:unknown\n"),
        }
        out.push_str(&format!("targets:{}\n", self.targets.len()));
        for target in &self.targets {
            out.push_str(&format!(
                "target:{}:{}\n",
                u8::from(target.repo_root),
                field(target.path.to_string())
            ));
        }
        out
    }

    /// SHA-256 of [`Self::canonical`]. Approval and execution are tied to this exact value.
    pub fn hash(&self) -> PlanHash {
        let digest = Sha256::digest(self.canonical().as_bytes());
        let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        PlanHash::new(format!("sha256:{hex}")).expect("a SHA-256 digest in lower-case hex")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(targets: &[&str]) -> PlanRequest {
        PlanRequest {
            operation: OperationRequest::DeleteRecursive,
            targets: targets
                .iter()
                .map(|path| TargetRequest {
                    path: (*path).to_owned(),
                    repo_root: false,
                })
                .collect(),
            work_root: "/work/proj".to_owned(),
            estimate: Some(Estimate {
                items: 3,
                bytes: 4096,
            }),
        }
    }

    #[test]
    fn empty_target_list_is_refused() {
        assert_eq!(
            ActionPlan::normalize(&request(&[])),
            Err(PlanError::NoTargets)
        );
    }

    #[test]
    fn one_bad_target_refuses_the_whole_plan() {
        let plan = ActionPlan::normalize(&request(&["/work/proj/build", "build/cache"]));
        assert_eq!(
            plan,
            Err(PlanError::Target {
                index: 1,
                error: PathError::Relative
            })
        );
    }

    #[test]
    fn work_root_and_destination_must_be_literal_absolute_paths() {
        let mut bad_root = request(&["/work/proj/build"]);
        bad_root.work_root = "proj".to_owned();
        assert_eq!(
            ActionPlan::normalize(&bad_root),
            Err(PlanError::WorkRoot(PathError::Relative))
        );

        let mut bad_destination = request(&["/work/proj/build"]);
        bad_destination.operation = OperationRequest::Move {
            destination: "/work/proj/*".to_owned(),
        };
        assert_eq!(
            ActionPlan::normalize(&bad_destination),
            Err(PlanError::Destination(PathError::WildcardNotAllowed))
        );
    }

    #[test]
    fn canonical_text_is_stable() {
        let plan = ActionPlan::normalize(&request(&["/work/proj/b", "/work/proj//a/"])).unwrap();
        assert_eq!(
            plan.canonical(),
            "ads.action-plan/1\n\
             op:fs.delete_recursive\n\
             work_root:10:/work/proj\n\
             destination:-\n\
             estimate:3:4096\n\
             targets:2\n\
             target:0:12:/work/proj/a\n\
             target:0:12:/work/proj/b\n"
        );
        // Computed outside this code, over the text above.
        assert_eq!(
            plan.hash().as_str(),
            "sha256:c7b870d89ef435eb431fcdf8e99f82f2b10c9b05a25eb86eb2ed603fa366eb94"
        );
    }

    #[test]
    fn hash_ignores_order_and_spelling_but_not_content() {
        let base = ActionPlan::normalize(&request(&["/work/proj/a", "/work/proj/b"])).unwrap();
        let same = ActionPlan::normalize(&request(&[
            "/work/proj/b/",
            "/work//proj/a",
            "/work/proj/./a",
        ]))
        .unwrap();
        assert_eq!(base.hash(), same.hash());

        let other_target =
            ActionPlan::normalize(&request(&["/work/proj/a", "/work/proj/c"])).unwrap();
        assert_ne!(base.hash(), other_target.hash());

        let mut changed = request(&["/work/proj/a", "/work/proj/b"]);
        changed.estimate = None;
        assert_ne!(base.hash(), ActionPlan::normalize(&changed).unwrap().hash());

        let mut changed = request(&["/work/proj/a", "/work/proj/b"]);
        changed.operation = OperationRequest::Delete;
        assert_ne!(base.hash(), ActionPlan::normalize(&changed).unwrap().hash());

        let mut changed = request(&["/work/proj/a", "/work/proj/b"]);
        changed.work_root = "/work".to_owned();
        assert_ne!(base.hash(), ActionPlan::normalize(&changed).unwrap().hash());

        let mut changed = request(&["/work/proj/a", "/work/proj/b"]);
        changed.targets[0].repo_root = true;
        assert_ne!(base.hash(), ActionPlan::normalize(&changed).unwrap().hash());
    }

    #[test]
    fn repeated_target_keeps_the_stricter_fact() {
        let mut repeated = request(&["/work/proj/a", "/work/proj/a/"]);
        repeated.targets[1].repo_root = true;
        let plan = ActionPlan::normalize(&repeated).unwrap();
        assert_eq!(plan.targets().len(), 1);
        assert!(plan.targets()[0].is_repo_root());
    }

    #[test]
    fn sha256_matches_a_known_vector() {
        let digest = Sha256::digest(b"abc");
        let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        assert_eq!(
            hex,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
