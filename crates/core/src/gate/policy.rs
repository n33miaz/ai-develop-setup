//! Gate parameters. The defaults live only in `defaults/action-gate.toml`.

use std::fmt;

use serde::Deserialize;

use super::path::{AbsPath, PathError, PathStyle, parse_relative, same_name_loosely};

const DEFAULTS_TOML: &str = include_str!("../../../../defaults/action-gate.toml");

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
    Toml(String),
    MissingKey(&'static str),
    Path {
        key: &'static str,
        value: String,
        error: PathError,
    },
    /// A folder name, the quarantine sub-path or the user home in a shape that is not accepted.
    Invalid {
        key: &'static str,
        value: String,
    },
    /// An allowed root that is itself a protected location.
    ProtectedAllowedRoot(String),
}

impl fmt::Display for PolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Toml(message) => f.write_str(message),
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
    windows_system_dirs: Vec<String>,
    posix_system_dirs: Vec<AbsPath>,
    windows_user_parents: Vec<String>,
    posix_user_parents: Vec<AbsPath>,
    approval_item_limit: u64,
    approval_byte_limit: u64,
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
    approval_byte_limit: Option<u64>,
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

        let policy = Self {
            user_home,
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
            approval_item_limit: pick(
                "approval_item_limit",
                over.approval_item_limit,
                base.approval_item_limit,
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
        match policy
            .allowed_roots
            .iter()
            .find(|root| policy.protection(root).is_some())
        {
            Some(root) => Err(PolicyError::ProtectedAllowedRoot(root.to_string())),
            None => Ok(policy),
        }
    }

    pub fn user_home(&self) -> &AbsPath {
        &self.user_home
    }

    pub fn allowed_roots(&self) -> &[AbsPath] {
        &self.allowed_roots
    }

    pub fn approval_item_limit(&self) -> u64 {
        self.approval_item_limit
    }

    pub fn approval_byte_limit(&self) -> u64 {
        self.approval_byte_limit
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

fn parse(text: &str) -> Result<Layer, PolicyError> {
    toml::from_str(text).map_err(|err| PolicyError::Toml(err.to_string()))
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
    fn bad_values_are_refused() {
        for bad in [
            "unknown_key = 1",
            "approval_item_limit = -1",
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
