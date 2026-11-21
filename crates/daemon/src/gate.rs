//! Composition of the file executor with the anchored gate journal.

use std::{
    ffi::OsString,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use ads_adapter_fs::{Executor, Failure, Report, Stop};
use ads_core::gate::{
    AbsPath, ActionPlan, ActionSnapshot, Command, EventMeta, GatePolicy, Id, JournalError,
    JournalGate, PlanError, PlanRequest,
};
use ads_storage::{EventLog, StorageConfig, StorageError};

/// Suffixes SQLite gives the files beside the database; losing one loses committed history.
const DATABASE_SIDECARS: [&str; 3] = ["-wal", "-shm", "-journal"];

/// The files and folders that hold the state of the product itself, taken from its
/// configuration: no action of a session may touch them, whatever the user policy says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductState {
    paths: Vec<AbsPath>,
}

impl ProductState {
    /// The audit database (`audit_database_path`, or its place in the vault folder) with its
    /// side files, the vault folder that holds the binding lock and, when the daemon was
    /// started by the launcher, its control folder (the folder of `--instance-lock`). Each
    /// path is kept as configured and as resolved on disk. Refuses what it cannot resolve.
    pub fn from_config(
        storage: &StorageConfig,
        control_dir: Option<&Path>,
    ) -> Result<Self, String> {
        let database = crate::audit::canonical_path(storage).map_err(|error| error.to_string())?;
        let mut sources = vec![database.clone()];
        for suffix in DATABASE_SIDECARS {
            let mut name = OsString::from(database.as_os_str());
            name.push(suffix);
            sources.push(PathBuf::from(name));
        }
        match crate::audit::vault_directory() {
            Ok(vault) => sources.push(vault),
            // Only where the vault does not exist; the audit log does not open there either.
            Err(StorageError::AnchorUnavailable) if !cfg!(windows) => {}
            Err(error) => return Err(error.to_string()),
        }
        sources.extend(control_dir.map(Path::to_path_buf));
        let mut paths = Vec::new();
        for source in &sources {
            for path in spellings(source)? {
                if !paths.contains(&path) {
                    paths.push(path);
                }
            }
        }
        Ok(Self { paths })
    }

    pub fn paths(&self) -> &[AbsPath] {
        &self.paths
    }
}

/// `path` as written and as the system resolves it: the deepest folder that exists is
/// resolved (links, short names, case) and the rest is appended.
fn spellings(path: &Path) -> Result<Vec<AbsPath>, String> {
    if !path.is_absolute() {
        return Err(format!("product path is not absolute: {}", path.display()));
    }
    // The resolved spelling of a Windows path carries the `\\?\` prefix; a share is refused.
    let parse = |path: &Path| {
        let text = path
            .to_str()
            .ok_or_else(|| format!("product path is not valid text: {}", path.display()))?;
        let text = match text.strip_prefix(r"\\?\") {
            Some(rest) if rest.starts_with(r"UNC\") => {
                return Err(format!("product path is on a network share: {text}"));
            }
            Some(rest) => rest,
            None => text,
        };
        AbsPath::parse(text).map_err(|error| format!("{text}: {error}"))
    };
    let mut found = vec![parse(path)?];
    for existing in path.ancestors() {
        let resolved = match std::fs::canonicalize(existing) {
            Ok(resolved) => resolved,
            Err(error) if error.kind() == ErrorKind::NotFound => continue,
            Err(error) => return Err(format!("{}: {error}", existing.display())),
        };
        let rest = path
            .strip_prefix(existing)
            .map_err(|error| error.to_string())?;
        let resolved = parse(&resolved.join(rest))?;
        if !found.contains(&resolved) {
            found.push(resolved);
        }
        break;
    }
    Ok(found)
}

#[derive(Debug)]
pub enum PlanActionError {
    Invalid(PlanError),
    Inspect(Stop),
    Journal(JournalError),
}

pub struct SessionGate {
    root: AbsPath,
    policy: GatePolicy,
    executor: Executor,
}

impl SessionGate {
    pub(crate) fn new(
        root: AbsPath,
        roots: Vec<AbsPath>,
        policy: GatePolicy,
        product: &ProductState,
        fence: &Path,
    ) -> Result<Self, String> {
        if !roots.contains(&root) {
            return Err("session working folder is no longer configured".to_owned());
        }
        let policy = policy
            .with_work_roots(roots)
            .with_product_paths(product.paths().iter().cloned());
        let executor = Executor::new(fence, policy.clone()).map_err(|error| error.to_string())?;
        Ok(Self {
            root,
            policy,
            executor,
        })
    }

    pub fn plan(
        &self,
        log: &mut EventLog,
        action_id: Id,
        mut request: PlanRequest,
        meta: EventMeta,
    ) -> Result<ActionSnapshot, PlanActionError> {
        request.work_root = self.root.to_string();
        let plan = ActionPlan::normalize(&request).map_err(PlanActionError::Invalid)?;
        let observed = self
            .executor
            .observe_plan(&plan)
            .map_err(PlanActionError::Inspect)?;
        self.journal(log)
            .plan(action_id, &observed, meta)
            .map_err(PlanActionError::Journal)
    }

    pub fn inspect(
        &self,
        log: &mut EventLog,
        action_id: &Id,
    ) -> Result<ActionSnapshot, JournalError> {
        self.journal(log).inspect(action_id)
    }

    pub fn apply(
        &self,
        log: &mut EventLog,
        action_id: &Id,
        command: Command,
        meta: EventMeta,
    ) -> Result<ActionSnapshot, JournalError> {
        self.journal(log).apply(action_id, command, meta)
    }

    fn journal<'a>(&'a self, log: &'a mut EventLog) -> JournalGate<'a> {
        JournalGate::new(&self.policy, log).for_work_root(&self.root)
    }

    /// Records the start before touching disk and uses it as the earliest outcome stamp.
    /// A failed outcome write preserves the operation report and leaves the start spent.
    pub fn execute(
        &mut self,
        log: &mut EventLog,
        action_id: &Id,
        start: EventMeta,
        mut finish: EventMeta,
    ) -> Result<ExecutionOutcome, JournalError> {
        let mut gate = JournalGate::new(&self.policy, log).for_work_root(&self.root);
        if finish.ts.is_before(&start.ts) {
            finish.ts = start.ts.clone();
        }
        let started = gate.authorize(action_id, start)?;
        match self.executor.execute_recorded(started) {
            Ok(completed) => {
                let (recorded, report) = completed.record(&mut gate, action_id, finish);
                Ok(ExecutionOutcome {
                    operation: Ok(report),
                    recorded,
                })
            }
            Err(failure) => {
                let command = Command::Fail {
                    error_code: failure.reason.error_code(),
                };
                Ok(ExecutionOutcome {
                    operation: Err(failure),
                    recorded: gate.apply(action_id, command, finish),
                })
            }
        }
    }
}

pub struct ExecutionOutcome {
    pub operation: Result<Report, Box<Failure>>,
    pub recorded: Result<ActionSnapshot, JournalError>,
}
