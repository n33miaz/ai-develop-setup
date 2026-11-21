//! Gate parameters. The defaults live only in `defaults/action-gate.toml`.

use std::fmt;

use serde::Deserialize;

use super::path::{AbsPath, PathError, PathStyle, parse_relative, same_name_loosely};

const DEFAULTS_TOML: &str = include_str!("../../../../defaults/action-gate.toml");
const MAX_CLASSIFICATION_MATCH_STEPS: u64 = 10_000_000;

/// What only the caller can know about the machine. The core reads no environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostFacts<'a> {
    /// The home folder of the current user, absolute and already resolved.
    pub user_home: &'a str,
}

/// A location no action may touch, and the reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Protection {
    /// A drive root or `/`.
    FilesystemRoot,
    UserHome,
    UserHomeAncestor,
    /// The folder that holds the user profiles, or the profile of any user.
    UserProfile,
    SystemDir,
    /// A path written in the other style than the user home (`/c/...` against `C:\...`): it
    /// may be another spelling of a protected folder, and text alone cannot tell.
    OtherPathStyle,
}

impl Protection {
    pub const ALL: [Protection; 6] = [
        Self::FilesystemRoot,
        Self::UserHome,
        Self::UserHomeAncestor,
        Self::UserProfile,
        Self::SystemDir,
        Self::OtherPathStyle,
    ];

    /// The detail of a rule code: `protected.<this>`.
    pub fn code(self) -> &'static str {
        match self {
            Self::FilesystemRoot => "filesystem_root",
            Self::UserHome => "user_home",
            Self::UserHomeAncestor => "user_home_ancestor",
            Self::UserProfile => "user_profile",
            Self::SystemDir => "system_dir",
            Self::OtherPathStyle => "other_path_style",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyError {
    Toml(Box<toml::de::Error>),
    MissingKey(&'static str),
    Path {
        key: &'static str,
        value: String,
        error: PathError,
    },
    /// A policy value outside its accepted range or shape.
    /// For `quarantine_dir`, also a sub-path that, in the style of the policy, would put the
    /// quarantine of some work root in a protected folder.
    Invalid {
        key: &'static str,
        value: String,
    },
    /// An allowed root that is itself a protected location.
    ProtectedAllowedRoot(String),
}

impl std::error::Error for PolicyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Toml(error) => Some(error.as_ref()),
            Self::Path { error, .. } => Some(error),
            _ => None,
        }
    }
}

impl fmt::Display for PolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Toml(error) => error.fmt(f),
            Self::MissingKey(key) => write!(f, "missing key `{key}`"),
            Self::Path { key, value, error } => write!(f, "`{key}`: {value:?}: {error}"),
            Self::Invalid { key, value } => write!(f, "`{key}`: {value:?} is not accepted"),
            Self::ProtectedAllowedRoot(root) => {
                write!(f, "`allowed_roots`: {root:?} is a protected location")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatePolicy {
    user_home: AbsPath,
    allowed_roots: Vec<AbsPath>,
    work_roots: Option<Vec<AbsPath>>,
    windows_system_dirs: Vec<String>,
    posix_system_dirs: Vec<AbsPath>,
    windows_user_parents: Vec<String>,
    posix_user_parents: Vec<AbsPath>,
    approval_item_limit: u64,
    classification_match_steps: u64,
    approval_byte_limit: u64,
    gate_clock_rewind_seconds: u64,
    quarantine_enabled: bool,
    quarantine_dir: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Layer {
    allowed_roots: Option<Vec<String>>,
    windows_system_dirs: Option<Vec<String>>,
    posix_system_dirs: Option<Vec<String>>,
    windows_user_parents: Option<Vec<String>>,
    posix_user_parents: Option<Vec<String>>,
    approval_item_limit: Option<u64>,
    classification_match_steps: Option<u64>,
    approval_byte_limit: Option<u64>,
    #[serde(alias = "clock_rewind_seconds")]
    gate_clock_rewind_seconds: Option<u64>,
    quarantine_enabled: Option<bool>,
    quarantine_dir: Option<String>,
}

impl GatePolicy {
    /// The values shipped in `defaults/action-gate.toml`.
    pub fn defaults(host: &HostFacts<'_>) -> Result<Self, PolicyError> {
        Self::from_toml_str("", host)
    }

    /// The shipped defaults with the keys of `overrides` applied: the four protected lists
    /// only grow (an override adds entries, never removes one); the other keys are replaced.
    pub fn from_toml_str(overrides: &str, host: &HostFacts<'_>) -> Result<Self, PolicyError> {
        let base = parse(DEFAULTS_TOML)?;
        let over = parse(overrides)?;
        fn pick<T>(key: &'static str, over: Option<T>, base: Option<T>) -> Result<T, PolicyError> {
            over.or(base).ok_or(PolicyError::MissingKey(key))
        }
        fn grow(
            key: &'static str,
            over: Option<Vec<String>>,
            base: Option<Vec<String>>,
        ) -> Result<Vec<String>, PolicyError> {
            let mut all = base.ok_or(PolicyError::MissingKey(key))?;
            all.extend(over.unwrap_or_default());
            Ok(all)
        }
        fn names(key: &'static str, list: Vec<String>) -> Result<Vec<String>, PolicyError> {
            list.into_iter()
                .map(|name| {
                    // A single literal folder name, checked by parsing it under a drive.
                    match AbsPath::parse(&format!("C:\\{name}")) {
                        Ok(path) if path.depth() == 1 => Ok(path.components()[0].clone()),
                        _ => Err(PolicyError::Invalid { key, value: name }),
                    }
                })
                .collect()
        }
        fn paths(key: &'static str, list: Vec<String>) -> Result<Vec<AbsPath>, PolicyError> {
            list.into_iter()
                .map(|value| match AbsPath::parse(&value) {
                    Ok(path) => Ok(path),
                    Err(error) => Err(PolicyError::Path { key, value, error }),
                })
                .collect()
        }
        fn posix(key: &'static str, list: Vec<String>) -> Result<Vec<AbsPath>, PolicyError> {
            let list = paths(key, list)?;
            match list.iter().find(|path| path.style() != PathStyle::Posix) {
                Some(path) => Err(PolicyError::Invalid {
                    key,
                    value: path.to_string(),
                }),
                None => Ok(list),
            }
        }

        let user_home = AbsPath::parse(host.user_home).map_err(|error| PolicyError::Path {
            key: "user_home",
            value: host.user_home.to_owned(),
            error,
        })?;
        if user_home.is_root() {
            return Err(PolicyError::Invalid {
                key: "user_home",
                value: user_home.to_string(),
            });
        }
        let quarantine_text = pick("quarantine_dir", over.quarantine_dir, base.quarantine_dir)?;
        let quarantine_dir = parse_relative(&quarantine_text).ok_or(PolicyError::Invalid {
            key: "quarantine_dir",
            value: quarantine_text,
        })?;

        let classification_match_steps = pick(
            "classification_match_steps",
            over.classification_match_steps,
            base.classification_match_steps,
        )?;
        if classification_match_steps > MAX_CLASSIFICATION_MATCH_STEPS {
            return Err(PolicyError::Invalid {
                key: "classification_match_steps",
                value: classification_match_steps.to_string(),
            });
        }

        let policy = Self {
            user_home,
            work_roots: None,
            allowed_roots: paths(
                "allowed_roots",
                pick("allowed_roots", over.allowed_roots, base.allowed_roots)?,
            )?,
            windows_system_dirs: names(
                "windows_system_dirs",
                grow(
                    "windows_system_dirs",
                    over.windows_system_dirs,
                    base.windows_system_dirs,
                )?,
            )?,
            posix_system_dirs: posix(
                "posix_system_dirs",
                grow(
                    "posix_system_dirs",
                    over.posix_system_dirs,
                    base.posix_system_dirs,
                )?,
            )?,
            windows_user_parents: names(
                "windows_user_parents",
                grow(
                    "windows_user_parents",
                    over.windows_user_parents,
                    base.windows_user_parents,
                )?,
            )?,
            posix_user_parents: posix(
                "posix_user_parents",
                grow(
                    "posix_user_parents",
                    over.posix_user_parents,
                    base.posix_user_parents,
                )?,
            )?,
            classification_match_steps,
            approval_item_limit: pick(
                "approval_item_limit",
                over.approval_item_limit,
                base.approval_item_limit,
            )?,
            gate_clock_rewind_seconds: pick(
                "gate_clock_rewind_seconds",
                over.gate_clock_rewind_seconds,
                base.gate_clock_rewind_seconds,
            )?,
            approval_byte_limit: pick(
                "approval_byte_limit",
                over.approval_byte_limit,
                base.approval_byte_limit,
            )?,
            quarantine_enabled: pick(
                "quarantine_enabled",
                over.quarantine_enabled,
                base.quarantine_enabled,
            )?,
            quarantine_dir,
        };
        if let Some(root) = policy
            .allowed_roots
            .iter()
            .find(|root| policy.protection(root).is_some())
        {
            return Err(PolicyError::ProtectedAllowedRoot(root.to_string()));
        }
        if !policy.quarantine_fits() {
            return Err(PolicyError::Invalid {
                key: "quarantine_dir",
                value: policy.quarantine_dir.join("/"),
            });
        }
        Ok(policy)
    }

    /// What can be told about the quarantine sub-path with no work root at hand, in the style
    /// of this policy (the one of the user home) and nothing wider:
    ///
    /// - on Windows, a name that is not a folder there (`nul` is a device), quarantine on or
    ///   off;
    /// - on POSIX, with quarantine on, a sub-path that some work root other than `/` would
    ///   put inside a listed folder: the listed folder ends where the sub-path begins and
    ///   has something before it (`/srv/vault` and `vault/q`, for the work root `/srv`).
    ///
    /// A name shared with a protected folder of the other style, or of the first level only
    /// (`etc/q` in `/work/proj`), is a common folder. Whether the generated destination is
    /// protected is judged with each plan, in its own style and work root ([`classify`]).
    ///
    /// [`classify`]: super::classify::classify
    fn quarantine_fits(&self) -> bool {
        let dir = &self.quarantine_dir;
        match self.user_home.style() {
            PathStyle::Windows => AbsPath::parse(&format!("C:\\{}", dir.join("\\")))
                .is_ok_and(|path| path.depth() == dir.len()),
            PathStyle::Posix if !self.quarantine_enabled => true,
            PathStyle::Posix => !self
                .posix_system_dirs
                .iter()
                .chain(&self.posix_user_parents)
                .any(|protected| {
                    let names = protected.components();
                    (1..names.len().min(dir.len() + 1)).any(|shared| {
                        names[names.len() - shared..]
                            .iter()
                            .zip(&dir[..shared])
                            .all(|(theirs, mine)| same_name_loosely(theirs, mine))
                    })
                }),
        }
    }

    /// Binds the policy to the trusted roots from the session launcher configuration.
    /// An empty list denies every work root.
    pub fn with_work_roots(mut self, roots: Vec<AbsPath>) -> Self {
        self.work_roots = Some(roots);
        self
    }

    pub(crate) fn work_roots(&self) -> Option<&[AbsPath]> {
        self.work_roots.as_deref()
    }

    pub fn user_home(&self) -> &AbsPath {
        &self.user_home
    }

    pub fn allowed_roots(&self) -> &[AbsPath] {
        &self.allowed_roots
    }

    pub fn classification_match_steps(&self) -> u64 {
        self.classification_match_steps
    }

    pub fn approval_item_limit(&self) -> u64 {
        self.approval_item_limit
    }

    pub fn approval_byte_limit(&self) -> u64 {
        self.approval_byte_limit
    }

    pub fn gate_clock_rewind_seconds(&self) -> u64 {
        self.gate_clock_rewind_seconds
    }

    pub fn quarantine_enabled(&self) -> bool {
        self.quarantine_enabled
    }

    /// The quarantine folder of a work root.
    pub fn quarantine_root(&self, work_root: &AbsPath) -> AbsPath {
        work_root.join(&self.quarantine_dir)
    }

    /// Why `path` may never be the target of an action, if it may not. Case is ignored in both
    /// styles: a deny rule must not be dodged by changing case.
    pub fn protection(&self, path: &AbsPath) -> Option<Protection> {
        if path.is_root() {
            return Some(Protection::FilesystemRoot);
        }
        if self.user_home.is_loosely_within(path) {
            return Some(if path.depth() == self.user_home.depth() {
                Protection::UserHome
            } else {
                Protection::UserHomeAncestor
            });
        }
        let first = &path.components()[0];
        let listed = |names: &[String]| names.iter().any(|name| same_name_loosely(name, first));
        match path.style() {
            PathStyle::Windows => {
                if listed(&self.windows_system_dirs) {
                    return Some(Protection::SystemDir);
                }
                if listed(&self.windows_user_parents) && path.depth() <= 2 {
                    return Some(Protection::UserProfile);
                }
            }
            PathStyle::Posix => {
                let system = self
                    .posix_system_dirs
                    .iter()
                    .any(|dir| path.is_loosely_within(dir) || dir.is_loosely_within(path));
                if system {
                    return Some(Protection::SystemDir);
                }
                let profile = self.posix_user_parents.iter().any(|parent| {
                    parent.is_loosely_within(path)
                        || (path.is_loosely_within(parent) && path.depth() == parent.depth() + 1)
                });
                if profile {
                    return Some(Protection::UserProfile);
                }
            }
        }
        // Last on purpose: the rules above name the reason when they can.
        (path.style() != self.user_home.style()).then_some(Protection::OtherPathStyle)
    }
}

pub(super) fn default_gate_clock_rewind_seconds() -> u64 {
    parse(DEFAULTS_TOML)
        .expect("valid bundled gate policy")
        .gate_clock_rewind_seconds
        .expect("bundled gate clock tolerance")
}

fn parse(text: &str) -> Result<Layer, PolicyError> {
    toml::from_str(text).map_err(|err| PolicyError::Toml(Box::new(err)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const WINDOWS: HostFacts<'static> = HostFacts {
        user_home: "C:\\Users\\user-a",
    };
    const POSIX: HostFacts<'static> = HostFacts {
        user_home: "/home/user-a",
    };

    fn protection(policy: &GatePolicy, raw: &str) -> Option<Protection> {
        policy.protection(&AbsPath::parse(raw).unwrap())
    }

    #[test]
    fn shipped_defaults_are_valid() {
        let policy = GatePolicy::defaults(&WINDOWS).unwrap();
        assert!(policy.allowed_roots().is_empty());
        assert_eq!(policy.approval_item_limit(), 500);
        assert_eq!(policy.approval_byte_limit(), 104_857_600);
        assert!(policy.quarantine_enabled());
        let work_root = AbsPath::parse("D:\\work\\proj").unwrap();
        assert_eq!(
            policy.quarantine_root(&work_root).to_string(),
            "D:\\work\\proj\\.ads\\quarantine"
        );
    }

    #[test]
    fn override_replaces_plain_keys() {
        let policy = GatePolicy::from_toml_str(
            "approval_item_limit = 10\nquarantine_enabled = false\nallowed_roots = ['D:\\cache']",
            &WINDOWS,
        )
        .unwrap();
        assert_eq!(policy.approval_item_limit(), 10);
        assert_eq!(policy.approval_byte_limit(), 104_857_600);
        assert!(!policy.quarantine_enabled());
        assert_eq!(policy.allowed_roots()[0].to_string(), "D:\\cache");
    }

    #[test]
    fn override_cannot_shrink_a_protected_list() {
        let policy = GatePolicy::from_toml_str(
            "windows_system_dirs = []\nposix_system_dirs = ['/data/system']\nwindows_user_parents = ['Profiles']",
            &WINDOWS,
        )
        .unwrap();
        assert_eq!(
            protection(&policy, "C:\\Windows\\System32"),
            Some(Protection::SystemDir)
        );
        assert_eq!(protection(&policy, "/etc"), Some(Protection::SystemDir));
        assert_eq!(
            protection(&policy, "/data/system/x"),
            Some(Protection::SystemDir)
        );
        assert_eq!(
            protection(&policy, "E:\\Profiles\\user-b"),
            Some(Protection::UserProfile)
        );
        assert_eq!(
            protection(&policy, "C:\\Users\\user-b"),
            Some(Protection::UserProfile)
        );
    }

    #[test]
    fn classification_budget_accepts_zero_through_the_maximum() {
        for steps in [0, MAX_CLASSIFICATION_MATCH_STEPS] {
            let overrides = format!("classification_match_steps = {steps}");
            let policy = GatePolicy::from_toml_str(&overrides, &WINDOWS).unwrap();
            assert_eq!(policy.classification_match_steps(), steps);
        }
        for steps in [MAX_CLASSIFICATION_MATCH_STEPS + 1, i64::MAX as u64] {
            let overrides = format!("classification_match_steps = {steps}");
            assert_eq!(
                GatePolicy::from_toml_str(&overrides, &WINDOWS),
                Err(PolicyError::Invalid {
                    key: "classification_match_steps",
                    value: steps.to_string(),
                })
            );
        }
    }

    #[test]
    fn bad_values_are_refused() {
        for bad in [
            "unknown_key = 1",
            "approval_item_limit = -1",
            "classification_match_steps = -1",
            "quarantine_dir = '../outside'",
            "quarantine_dir = '/abs'",
            "quarantine_dir = ''",
            "allowed_roots = ['cache']",
            "allowed_roots = ['D:\\cache\\*']",
            "windows_system_dirs = ['a\\b']",
            "windows_system_dirs = ['']",
            "posix_system_dirs = ['C:\\data']",
            "posix_user_parents = ['home']",
        ] {
            assert!(GatePolicy::from_toml_str(bad, &WINDOWS).is_err(), "{bad}");
        }
    }

    #[test]
    fn quarantine_that_would_land_in_a_protected_folder_is_refused() {
        let refused = |host: &HostFacts<'_>, overrides: &str, dir: &str| {
            assert_eq!(
                GatePolicy::from_toml_str(overrides, host),
                Err(PolicyError::Invalid {
                    key: "quarantine_dir",
                    value: dir.to_owned()
                }),
                "{overrides}"
            );
        };
        // A listed folder below the first level that ends where the sub-path begins: some
        // work root other than `/` would put the quarantine inside it.
        refused(
            &POSIX,
            "posix_system_dirs = ['/work/proj/trash']\nquarantine_dir = 'trash'",
            "trash",
        );
        refused(
            &POSIX,
            "posix_system_dirs = ['/srv/vault/keep']\nquarantine_dir = 'Vault/KEEP/q'",
            "Vault/KEEP/q",
        );
        refused(
            &POSIX,
            "posix_user_parents = ['/srv/people']\nquarantine_dir = 'PEOPLE/q'",
            "PEOPLE/q",
        );
        // Names that are not a folder on Windows, in any component, quarantine on or off.
        for dir in ["nul", "q/con", "aux.d/q", "COM1"] {
            refused(&WINDOWS, &format!("quarantine_dir = '{dir}'"), dir);
            let off = format!("quarantine_enabled = false\nquarantine_dir = '{dir}'");
            refused(&WINDOWS, &off, dir);
        }

        // Safe: a name shared with a protected folder of the first level only (the work root
        // would have to be `/`, which no plan gets past), or beside the protected one.
        for overrides in [
            "quarantine_dir = 'trash'",
            "quarantine_dir = 'etc'",
            "quarantine_dir = 'etc/q'",
            "quarantine_dir = 'home/q'",
            "quarantine_dir = 'Users'",
            "quarantine_dir = 'q/etc'",
            "quarantine_dir = 'etc.d/q'",
            "quarantine_dir = 'nul'",
            "posix_system_dirs = ['/srv/vault/keep']\nquarantine_dir = 'vault/q'",
            "posix_system_dirs = ['/srv/vault']\nquarantine_dir = 'keep/vault'",
            // With quarantine off the sub-path is only checked for its shape.
            "quarantine_enabled = false\nposix_system_dirs = ['/work/proj/trash']\nquarantine_dir = 'trash'",
        ] {
            assert!(
                GatePolicy::from_toml_str(overrides, &POSIX).is_ok(),
                "{overrides}"
            );
        }
        // Safe: on a Windows policy the POSIX lists say nothing, and the Windows lists name
        // folders of the first level of a drive only.
        for overrides in [
            "quarantine_dir = 'etc/q'",
            "quarantine_dir = 'home/q'",
            "quarantine_dir = 'Windows/q'",
            "quarantine_dir = 'users/q'",
            "posix_system_dirs = ['/work/proj/trash']\nquarantine_dir = 'trash'",
            "quarantine_enabled = false\nquarantine_dir = 'etc/q'",
        ] {
            assert!(
                GatePolicy::from_toml_str(overrides, &WINDOWS).is_ok(),
                "{overrides}"
            );
        }
        // The shape is checked either way.
        for bad in ["../q", "/abs", "", "a//b", "a/./b", "C:/q"] {
            for enabled in [true, false] {
                let overrides = format!("quarantine_enabled = {enabled}\nquarantine_dir = '{bad}'");
                assert!(
                    GatePolicy::from_toml_str(&overrides, &POSIX).is_err(),
                    "{bad}"
                );
                assert!(
                    GatePolicy::from_toml_str(&overrides, &WINDOWS).is_err(),
                    "{bad}"
                );
            }
        }
    }

    #[test]
    fn protected_allowed_root_is_refused() {
        for root in [
            "C:\\",
            "C:\\Users",
            "C:\\Users\\user-a",
            "C:\\Windows\\Temp",
            "/",
            "/var/x",
        ] {
            let overrides = format!("allowed_roots = ['{root}']");
            assert_eq!(
                GatePolicy::from_toml_str(&overrides, &WINDOWS),
                Err(PolicyError::ProtectedAllowedRoot(
                    AbsPath::parse(root).unwrap().to_string()
                )),
                "{root}"
            );
        }
    }

    #[test]
    fn user_home_must_be_a_resolved_folder() {
        for home in [
            "",
            "~",
            "%USERPROFILE%",
            "$HOME",
            "home/user-a",
            "/",
            "C:\\",
        ] {
            let host = HostFacts { user_home: home };
            assert!(GatePolicy::defaults(&host).is_err(), "{home}");
        }
    }

    #[test]
    fn windows_protected_locations() {
        let policy = GatePolicy::defaults(&WINDOWS).unwrap();
        let cases = [
            ("C:\\", Some(Protection::FilesystemRoot)),
            ("D:\\", Some(Protection::FilesystemRoot)),
            ("C:\\Users\\user-a", Some(Protection::UserHome)),
            ("c:\\users\\USER-A", Some(Protection::UserHome)),
            ("C:\\Users", Some(Protection::UserHomeAncestor)),
            ("D:\\Users", Some(Protection::UserProfile)),
            ("C:\\Users\\user-b", Some(Protection::UserProfile)),
            ("C:\\Windows", Some(Protection::SystemDir)),
            (
                "D:\\windows\\system32\\drivers",
                Some(Protection::SystemDir),
            ),
            ("C:\\Program Files (x86)\\x", Some(Protection::SystemDir)),
            ("C:\\ProgramData", Some(Protection::SystemDir)),
            ("C:\\Users\\user-a\\work\\proj", None),
            ("C:\\work\\Windows", None),
            ("D:\\work\\proj", None),
        ];
        for (raw, expected) in cases {
            assert_eq!(protection(&policy, raw), expected, "{raw}");
        }
    }

    #[test]
    fn posix_protected_locations() {
        let policy = GatePolicy::defaults(&POSIX).unwrap();
        let cases = [
            ("/", Some(Protection::FilesystemRoot)),
            ("/home/user-a", Some(Protection::UserHome)),
            ("/Home/User-A", Some(Protection::UserHome)),
            ("/home", Some(Protection::UserHomeAncestor)),
            ("/home/user-b", Some(Protection::UserProfile)),
            ("/Users/user-b", Some(Protection::UserProfile)),
            ("/Users", Some(Protection::UserProfile)),
            ("/etc", Some(Protection::SystemDir)),
            ("/usr/lib/x", Some(Protection::SystemDir)),
            ("/ETC/x", Some(Protection::SystemDir)),
            ("/root", Some(Protection::SystemDir)),
            ("/home/user-a/work/proj", None),
            ("/home/user-b/work", None),
            ("/work/proj", None),
            ("/work/etc", None),
        ];
        for (raw, expected) in cases {
            assert_eq!(protection(&policy, raw), expected, "{raw}");
        }
    }

    #[test]
    fn path_in_another_style_than_the_user_home_is_protected() {
        let policy = GatePolicy::defaults(&WINDOWS).unwrap();
        for raw in [
            "/c/Users/user-a",
            "/c/Users/user-a/Documents",
            "/mnt/c/work",
            "/work",
        ] {
            assert_eq!(
                protection(&policy, raw),
                Some(Protection::OtherPathStyle),
                "{raw}"
            );
        }
        let policy = GatePolicy::defaults(&POSIX).unwrap();
        assert_eq!(
            protection(&policy, "C:\\work\\proj"),
            Some(Protection::OtherPathStyle)
        );
        assert_eq!(
            GatePolicy::from_toml_str("allowed_roots = ['/c/Users/user-a']", &WINDOWS),
            Err(PolicyError::ProtectedAllowedRoot(
                "/c/Users/user-a".to_owned()
            ))
        );
    }

    #[test]
    fn user_home_outside_the_usual_parents_is_protected_with_its_ancestors() {
        let host = HostFacts {
            user_home: "/data/people/user-a",
        };
        let policy = GatePolicy::defaults(&host).unwrap();
        assert_eq!(
            protection(&policy, "/data/people/user-a"),
            Some(Protection::UserHome)
        );
        assert_eq!(
            protection(&policy, "/data/people"),
            Some(Protection::UserHomeAncestor)
        );
        assert_eq!(
            protection(&policy, "/data"),
            Some(Protection::UserHomeAncestor)
        );
        assert_eq!(protection(&policy, "/data/people/user-a/proj"), None);
    }
}
