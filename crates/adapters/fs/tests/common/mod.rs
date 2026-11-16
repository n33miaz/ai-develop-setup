//! Shared by the tests that touch the disk.
//!
//! Every test works in a folder of its own under `CARGO_TARGET_TMPDIR`, made here and never
//! removed: `<tmp>/ads-fs/<test>-<pid>-<nanos>-<n>/{fence,outside}`. The fence of the executor
//! is `fence`; `outside` is the sibling that links point to, with a sentinel that has to
//! survive. "The user home" is the text `<fence>/fake-home`. Nothing here reads an
//! environment variable at run time.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fs;
use std::num::NonZeroU64;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use ads_adapter_fs::{Executor, Journal, JournalError, Limits};
use ads_core::gate::{
    Action, ActionPlan, Actor, ActorKind, Command, DataClass, Estimate, Event, EventMeta,
    ExecutionPermit, GatePolicy, HostFacts, Id, OperationRequest, PlanRequest, ShortText,
    TargetRequest, Timestamp, Ulid, Verdict, Via, classify,
};

pub const SENTINEL: &str = "sentinel.txt";
pub const SENTINEL_TEXT: &str = "must survive";

static COUNTER: AtomicU32 = AtomicU32::new(0);

pub struct Sandbox {
    /// The folder of this test.
    pub root: PathBuf,
    pub fence: PathBuf,
    pub outside: PathBuf,
    /// The fence as a plan writes it.
    pub fence_text: String,
}

impl Sandbox {
    pub fn new(test: &str) -> Self {
        let base = Path::new(env!("CARGO_TARGET_TMPDIR"));
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let unique = format!(
            "{test}-{}-{nanos}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let root = base.join("ads-fs").join(unique);
        assert!(root.starts_with(base) && base.is_absolute());
        assert!(!root.exists(), "the folder of a test is new");
        fs::create_dir_all(&root).unwrap();
        let fence = root.join("fence");
        let outside = root.join("outside");
        fs::create_dir(&fence).unwrap();
        fs::create_dir(fence.join("work")).unwrap();
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join(SENTINEL), SENTINEL_TEXT).unwrap();
        let resolved = fs::canonicalize(&fence).unwrap();
        let resolved = resolved.to_str().unwrap();
        let fence_text = resolved
            .strip_prefix(r"\\?\")
            .unwrap_or(resolved)
            .to_owned();
        let sandbox = Self {
            root,
            fence,
            outside,
            fence_text,
        };
        assert_eq!(sandbox.executor("").fence_text(), sandbox.fence_text);
        // The resolved fence is still under the temporary folder of the target directory.
        let resolved_base = fs::canonicalize(base).unwrap();
        assert!(
            fs::canonicalize(&sandbox.fence)
                .unwrap()
                .starts_with(&resolved_base)
        );
        sandbox
    }

    /// A path below the fence as a plan writes it; `rel` uses `/`.
    pub fn text(&self, rel: &str) -> String {
        native(&format!("{}/{rel}", self.fence_text))
    }

    /// A path below the folder of the test, outside the fence, as a plan writes it.
    pub fn outside_text(&self, rel: &str) -> String {
        let parent = self.fence_text.rsplit_once(SEPARATOR).unwrap().0;
        native(&format!("{parent}/outside/{rel}"))
    }

    pub fn work_root(&self) -> String {
        self.text("work")
    }

    /// The policy of the tests: the shipped defaults, the home being a text inside the fence.
    pub fn policy(&self, overrides: &str) -> GatePolicy {
        policy_for(&self.text("fake-home"), overrides)
    }

    /// An executor that removes for good on every system (`definitive_delete = "isolated"`),
    /// so that the tests of removal run on Linux too. The shipped default is another, and
    /// has tests of its own: see [`Self::shipped_executor`].
    pub fn executor(&self, overrides: &str) -> Executor {
        self.shipped_executor(overrides).with_limits(limits(""))
    }

    /// An executor with the shipped parameters.
    pub fn shipped_executor(&self, overrides: &str) -> Executor {
        Executor::new(&self.fence, self.policy(overrides)).unwrap()
    }

    /// The private folder of the executor, below the fence with `/`.
    pub fn private_dir(&self) -> &'static str {
        "work/.ads/tmp"
    }

    /// The path on disk of `rel` below the fence.
    pub fn path(&self, rel: &str) -> PathBuf {
        let mut path = self.fence.clone();
        path.extend(rel.split('/'));
        path
    }

    pub fn dir(&self, rel: &str) -> PathBuf {
        let path = self.path(rel);
        fs::create_dir_all(&path).unwrap();
        path
    }

    pub fn file(&self, rel: &str, content: &str) -> PathBuf {
        let path = self.path(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, content).unwrap();
        path
    }

    pub fn sentinel_is_intact(&self) -> bool {
        fs::read_to_string(self.outside.join(SENTINEL)).is_ok_and(|text| text == SENTINEL_TEXT)
    }
}

/// The parameters of the tests: the shipped ones, removing for good on every system, with
/// the keys of `overrides` on top.
pub fn limits(overrides: &str) -> Limits {
    let base = match overrides.contains("definitive_delete") {
        true => "",
        false => "definitive_delete = \"isolated\"\n",
    };
    Limits::from_toml_str(&format!("{base}{overrides}")).unwrap()
}

pub const SEPARATOR: char = if cfg!(windows) { '\\' } else { '/' };

fn native(text: &str) -> String {
    if cfg!(windows) {
        text.replace('/', "\\")
    } else {
        text.to_owned()
    }
}

pub fn policy_for(home: &str, overrides: &str) -> GatePolicy {
    GatePolicy::from_toml_str(overrides, &HostFacts { user_home: home }).unwrap()
}

/// A junction `link -> target`. Windows only; needs no privilege.
#[cfg(windows)]
pub fn junction(link: &Path, target: &Path) {
    let status = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .stdout(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "mklink /J failed");
}

/// A symbolic link `link -> target`. `false` when the system refuses to make one (Windows
/// without the privilege): the caller skips, saying so.
pub fn symlink(link: &Path, target: &Path) -> bool {
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(target, link);
    #[cfg(windows)]
    let made = if target.is_dir() {
        std::os::windows::fs::symlink_dir(target, link)
    } else {
        std::os::windows::fs::symlink_file(target, link)
    };
    match made {
        Ok(()) => true,
        Err(error) => {
            eprintln!("SKIPPED: this system refused to make a symbolic link ({error})");
            false
        }
    }
}

/// Every entry below `path` with its content, links not followed: `rel -> "dir" | "file:<text>"
/// | "link"`. For comparing a tree before and after.
pub fn tree(path: &Path) -> BTreeMap<String, String> {
    fn walk(base: &Path, path: &Path, into: &mut BTreeMap<String, String>) {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let child = entry.path();
            let rel = child
                .strip_prefix(base)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            let metadata = fs::symlink_metadata(&child).unwrap();
            if is_link(&metadata) {
                into.insert(rel, "link".to_owned());
            } else if metadata.is_dir() {
                into.insert(rel, "dir".to_owned());
                walk(base, &child, into);
            } else {
                into.insert(rel, format!("file:{}", fs::read_to_string(&child).unwrap()));
            }
        }
    }
    let mut into = BTreeMap::new();
    walk(path, path, &mut into);
    into
}

pub fn is_link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

/// A journal that keeps the events in memory and refuses a second `action.started` of an
/// action. It forgets everything when it is dropped: good for a test, never for production,
/// which is why it lives here and not in the crate.
#[derive(Debug, Default)]
pub struct MemoryJournal {
    events: Vec<Event>,
    started: Vec<Id>,
}

impl MemoryJournal {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn events(&self) -> &[Event] {
        &self.events
    }
}

impl Journal for MemoryJournal {
    fn consume(&mut self, started: &Event) -> Result<(), JournalError> {
        let Event::ActionStarted(envelope) = started else {
            return Err(JournalError(format!(
                "{} is not action.started",
                started.kind()
            )));
        };
        let action_id = &envelope.data().action_id;
        if self.started.contains(action_id) {
            return Err(JournalError(format!("{action_id} already started")));
        }
        self.started.push(action_id.clone());
        self.events.push(started.clone());
        Ok(())
    }
}

pub fn id(text: &str) -> Id {
    Id::new(text).unwrap()
}

pub fn meta(ts: &str, seq: u64) -> EventMeta {
    EventMeta {
        id: Ulid::new("01ARZ3NDEKTSV4RRFFQ69G5FAB").unwrap(),
        ts: Timestamp::new(ts).unwrap(),
        seq: NonZeroU64::new(seq).unwrap(),
        actor: Actor {
            kind: ActorKind::Daemon,
            id: id("core-01"),
        },
        cause: None,
        data_class: DataClass::Internal,
    }
}

pub const STARTED_AT: &str = "2026-11-15T12:00:05Z";

pub struct Request<'a> {
    pub action_id: &'a str,
    pub operation: OperationRequest,
    pub work_root: String,
    pub targets: Vec<String>,
    pub estimate: Option<Estimate>,
}

/// An action taken through the gate up to the permit.
pub struct Flow {
    pub action: Action,
    pub permit: ExecutionPermit,
    pub started: Event,
    pub verdict: Verdict,
}

/// Plans, asks for and grants the approval when the verdict wants one, and authorizes.
/// Panics when the gate denies: a test that wants a permit has to write a plan that passes.
pub fn through_gate(policy: &GatePolicy, request: Request<'_>) -> Flow {
    let plan = ActionPlan::normalize(&PlanRequest {
        operation: request.operation,
        targets: request
            .targets
            .iter()
            .map(|path| TargetRequest {
                path: path.clone(),
                repo_root: false,
            })
            .collect(),
        work_root: request.work_root,
        estimate: request.estimate,
    })
    .unwrap();
    let verdict = classify(&plan, policy);
    let codes: Vec<String> = verdict
        .findings()
        .iter()
        .map(|finding| finding.rule.code().to_string())
        .collect();
    let (mut action, _, _) = Action::plan(
        id(request.action_id),
        &verdict,
        meta("2026-11-15T12:00:00Z", 1),
    )
    .unwrap();
    assert!(
        !action.state().is_terminal(),
        "the gate denied the plan: {codes:?}"
    );
    if action.needs_approval() {
        let approval_id = id(&format!("appr-{}", request.action_id));
        (action, _) = action
            .apply(
                Command::RequestApproval {
                    approval_id: approval_id.clone(),
                    summary: ShortText::new("test").unwrap(),
                    expires_at: Timestamp::new("2026-11-15T13:00:00Z").unwrap(),
                },
                meta("2026-11-15T12:00:01Z", 1),
            )
            .unwrap();
        (action, _) = action
            .apply(
                Command::Grant {
                    action_id: id(request.action_id),
                    approval_id,
                    plan_hash: verdict.plan().hash(),
                    via: Via::LocalUi,
                },
                meta("2026-11-15T12:00:02Z", 2),
            )
            .unwrap();
    }
    let (action, permit, started) = action
        .authorize(verdict.plan(), meta(STARTED_AT, 2))
        .unwrap();
    Flow {
        action,
        permit,
        started,
        verdict,
    }
}

pub fn estimate(items: u64, bytes: u64) -> Option<Estimate> {
    Some(Estimate { items, bytes })
}

// ---- shared by the tests that drive the executor around its checks -----------------------------

/// When a probe is told about a mutation.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Moment {
    Before,
    Opened,
    Midway(ads_adapter_fs::Midway),
}

/// A probe made of a closure.
pub struct Hook<F>(pub F);

impl<F> ads_adapter_fs::Probe for Hook<F>
where
    F: FnMut(Moment, &ads_adapter_fs::Mutation) -> std::io::Result<()>,
{
    fn before(&mut self, mutation: &ads_adapter_fs::Mutation) -> std::io::Result<()> {
        (self.0)(Moment::Before, mutation)
    }

    fn opened(&mut self, mutation: &ads_adapter_fs::Mutation) -> std::io::Result<()> {
        (self.0)(Moment::Opened, mutation)
    }

    fn midway(
        &mut self,
        mutation: &ads_adapter_fs::Mutation,
        at: ads_adapter_fs::Midway,
    ) -> std::io::Result<()> {
        (self.0)(Moment::Midway(at), mutation)
    }
}

pub fn hooked(
    executor: Executor,
    probe: impl FnMut(Moment, &ads_adapter_fs::Mutation) -> std::io::Result<()> + 'static,
) -> Executor {
    executor.with_probe(Box::new(Hook(probe)))
}

pub fn delete_of(sandbox: &Sandbox, action_id: &'static str, targets: &[&str]) -> Request<'static> {
    Request {
        action_id,
        operation: OperationRequest::DeleteRecursive,
        work_root: sandbox.work_root(),
        targets: targets.iter().map(|rel| sandbox.text(rel)).collect(),
        estimate: estimate(50, 4096),
    }
}

pub fn restore_of(sandbox: &Sandbox, action_id: &'static str, batch: &str) -> Request<'static> {
    Request {
        action_id,
        operation: OperationRequest::Move {
            destination: sandbox.work_root(),
        },
        work_root: sandbox.work_root(),
        targets: vec![batch.to_owned()],
        estimate: estimate(50, 4096),
    }
}

/// Runs a flow with a journal of its own.
pub fn run(
    executor: &mut Executor,
    flow: Flow,
) -> Result<ads_adapter_fs::Completed, Box<ads_adapter_fs::Failure>> {
    executor.execute(flow.permit, &flow.started, &mut MemoryJournal::new())
}

/// The folder of the batch of a quarantine flow: below the fence with `/`, and as a plan
/// writes it.
pub fn batch_of(sandbox: &Sandbox, flow: &Flow, action_id: &str) -> (String, String) {
    let destination = flow.permit.plan().destination().unwrap().to_string();
    let text = format!("{destination}{SEPARATOR}{action_id}");
    (sandbox.below(&text), text)
}

impl Sandbox {
    /// A path as a plan writes it, as names below the fence with `/`.
    pub fn below(&self, text: &str) -> String {
        text.strip_prefix(&self.fence_text)
            .unwrap_or_else(|| panic!("{text} is not below the fence"))
            .trim_start_matches(SEPARATOR)
            .replace(SEPARATOR, "/")
    }

    /// The work root without what the executor keeps for itself.
    pub fn work_tree(&self) -> BTreeMap<String, String> {
        let mut entries = tree(&self.path("work"));
        entries.retain(|rel, _| !rel.starts_with(".ads"));
        entries
    }
}

/// A folder link to `target`: a junction on Windows, a symbolic link elsewhere.
pub fn folder_link(link: &Path, target: &Path) -> bool {
    #[cfg(windows)]
    {
        junction(link, target);
        true
    }
    #[cfg(not(windows))]
    {
        symlink(link, target)
    }
}
