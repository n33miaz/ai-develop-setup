//! Composition of the file executor with the anchored gate journal.

use std::{
    ffi::OsString,
    fs,
    io::ErrorKind,
    path::{Component, Path, PathBuf},
};

use ads_adapter_fs::{Executor, Failure, Report, Stop};
use ads_core::gate::{
    AbsPath, ActionPlan, ActionSnapshot, Command, EventMeta, GatePolicy, Id, JournalError,
    JournalGate, PlanError, PlanRequest,
};
use ads_storage::EventLog;

use crate::audit::ProductLog;

/// Suffixes SQLite gives the files beside the database; losing one loses committed history.
const DATABASE_SIDECARS: [&str; 3] = ["-wal", "-shm", "-journal"];

/// Links followed while resolving one product path before it is refused.
const MAX_LINKS: usize = 32;

/// The files and folders that hold the state of the product itself: no action of a session
/// may touch them, whatever the user policy says. Built only with the log it protects
/// ([`crate::audit::open_product`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductState {
    paths: Vec<AbsPath>,
}

impl ProductState {
    /// The audit database as configured, with its side files, the vault folder that holds the
    /// binding lock and, when the daemon was started by the launcher, the folder of the
    /// instance lock it holds. Refuses what it cannot resolve.
    pub(crate) fn resolve(
        database: &Path,
        vault_dir: &Path,
        control_dir: Option<&Path>,
    ) -> Result<Self, String> {
        let mut sources = vec![database.to_path_buf()];
        for suffix in DATABASE_SIDECARS {
            let mut name = OsString::from(database.as_os_str());
            name.push(suffix);
            sources.push(PathBuf::from(name));
        }
        sources.push(vault_dir.to_path_buf());
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

/// Every spelling under which `path` is protected: as written, as the system resolves it, and
/// each link met on the way, at its real place, since removing one breaks the path. A missing
/// entry is accepted (a file not created yet) and ends the walk; a link whose target is
/// missing or written with `..` is refused, as is a chain longer than [`MAX_LINKS`].
pub(crate) fn spellings(path: &Path) -> Result<Vec<AbsPath>, String> {
    let failed = |at: &Path, error: std::io::Error| format!("{}: {error}", at.display());
    let mut found = Vec::new();
    let mut push = |path: AbsPath| {
        if !found.contains(&path) {
            found.push(path);
        }
    };
    push(spelling(path)?);
    let mut current = path.to_path_buf();
    for _ in 0..=MAX_LINKS {
        if !current.is_absolute() {
            return Err(format!(
                "product path is not absolute: {}",
                current.display()
            ));
        }
        let mut walked = PathBuf::new();
        let mut components = current.components();
        let mut existing = current.clone();
        let mut next = None;
        while let Some(component) = components.next() {
            walked.push(component);
            match component {
                Component::Prefix(_) | Component::RootDir => continue,
                Component::Normal(_) => {}
                _ => {
                    return Err(format!(
                        "product path is not normalized: {}",
                        current.display()
                    ));
                }
            }
            let meta = match fs::symlink_metadata(&walked) {
                Ok(meta) => meta,
                Err(error) if error.kind() == ErrorKind::NotFound => {
                    existing = walked.parent().unwrap_or(&walked).to_path_buf();
                    break;
                }
                Err(error) => return Err(failed(&walked, error)),
            };
            // Junctions and the other name surrogates count as links here.
            if !meta.file_type().is_symlink() {
                continue;
            }
            let parent = walked.parent().unwrap_or(&walked);
            let real_parent = fs::canonicalize(parent).map_err(|error| failed(parent, error))?;
            if let Some(name) = walked.file_name() {
                push(spelling(&real_parent.join(name))?);
            }
            let target = fs::read_link(&walked).map_err(|error| failed(&walked, error))?;
            if target
                .components()
                .any(|part| matches!(part, Component::ParentDir))
            {
                return Err(format!("link target uses `..`: {}", walked.display()));
            }
            let target = real_parent.join(target);
            match fs::symlink_metadata(&target) {
                Ok(_) => {}
                Err(error) if error.kind() == ErrorKind::NotFound => {
                    return Err(format!("link target is missing: {}", walked.display()));
                }
                Err(error) => return Err(failed(&target, error)),
            }
            next = Some(target.join(components.as_path()));
            break;
        }
        match next {
            Some(target) => current = target,
            None => {
                let rest = current
                    .strip_prefix(&existing)
                    .map_err(|error| error.to_string())?;
                let resolved =
                    fs::canonicalize(&existing).map_err(|error| failed(&existing, error))?;
                push(spelling(&resolved.join(rest))?);
                return Ok(found);
            }
        }
    }
    Err(format!("too many links: {}", path.display()))
}

/// The text of `path` as the gate compares it. The resolved spelling of a Windows path
/// carries the `\\?\` prefix; a network share is refused.
fn spelling(path: &Path) -> Result<AbsPath, String> {
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
}

#[derive(Debug)]
pub enum PlanActionError {
    Invalid(PlanError),
    Inspect(Stop),
    Journal(JournalError),
}

/// Binds actions to one session folder and to the audit log the gate was built for: a log
/// that carries another product state is refused with [`JournalError::PolicyChanged`].
pub struct SessionGate {
    root: AbsPath,
    policy: GatePolicy,
    product: ProductState,
    executor: Executor,
}

impl SessionGate {
    pub(crate) fn new(
        root: AbsPath,
        roots: Vec<AbsPath>,
        policy: GatePolicy,
        product: &ProductLog,
        fence: &Path,
    ) -> Result<Self, String> {
        let product = product.state().clone();
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
            product,
            executor,
        })
    }

    /// The executor with the session policy, to check what it refuses on its own.
    #[cfg(test)]
    pub(crate) fn executor_mut(&mut self) -> &mut Executor {
        &mut self.executor
    }

    fn bound<'a>(&self, log: &'a mut ProductLog) -> Result<&'a mut EventLog, JournalError> {
        if log.state() != &self.product {
            return Err(JournalError::PolicyChanged);
        }
        Ok(log.log())
    }

    pub fn plan(
        &self,
        log: &mut ProductLog,
        action_id: Id,
        mut request: PlanRequest,
        meta: EventMeta,
    ) -> Result<ActionSnapshot, PlanActionError> {
        let log = self.bound(log).map_err(PlanActionError::Journal)?;
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
        log: &mut ProductLog,
        action_id: &Id,
    ) -> Result<ActionSnapshot, JournalError> {
        let log = self.bound(log)?;
        self.journal(log).inspect(action_id)
    }

    pub fn apply(
        &self,
        log: &mut ProductLog,
        action_id: &Id,
        command: Command,
        meta: EventMeta,
    ) -> Result<ActionSnapshot, JournalError> {
        let log = self.bound(log)?;
        self.journal(log).apply(action_id, command, meta)
    }

    fn journal<'a>(&'a self, log: &'a mut EventLog) -> JournalGate<'a> {
        JournalGate::new(&self.policy, log).for_work_root(&self.root)
    }

    /// Records the start before touching disk and uses it as the earliest outcome stamp.
    /// A failed outcome write preserves the operation report and leaves the start spent.
    pub fn execute(
        &mut self,
        log: &mut ProductLog,
        action_id: &Id,
        start: EventMeta,
        mut finish: EventMeta,
    ) -> Result<ExecutionOutcome, JournalError> {
        let log = self.bound(log)?;
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
