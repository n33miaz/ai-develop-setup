//! Gate parameters. The defaults live only in `defaults/action-gate.toml`.

use std::fmt;

use serde::Deserialize;

use super::path::{AbsPath, PathError, PathStyle, parse_relative, same_name_loosely};

const DEFAULTS_TOML: &str = include_str!("../../../../defaults/action-gate.toml");
const MAX_CLASSIFICATION_MATCH_STEPS: u64 = 10_000_000;
const MAX_GATE_CLOCK_REWIND_SECONDS: u64 = 60;

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
    /// The state of the product itself (audit database, vault lock, launcher control folder),
    /// as the caller bound it with [`GatePolicy::with_product_paths`].
    ProductData,
}

impl Protection {
    pub const ALL: [Protection; 7] = [
        Self::FilesystemRoot,
        Self::UserHome,
        Self::UserHomeAncestor,
        Self::UserProfile,
        Self::SystemDir,
        Self::OtherPathStyle,
        Self::ProductData,
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
            Self::ProductData => "product_data",
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
    product_paths: Vec<AbsPath>,
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

        let gate_clock_rewind_seconds = pick(
            "gate_clock_rewind_seconds",
            over.gate_clock_rewind_seconds,
            base.gate_clock_rewind_seconds,
        )?;
        if gate_clock_rewind_seconds > MAX_GATE_CLOCK_REWIND_SECONDS {
            return Err(PolicyError::Invalid {
                key: "gate_clock_rewind_seconds",
                value: gate_clock_rewind_seconds.to_string(),
            });
        }
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
            product_paths: Vec::new(),
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
            gate_clock_rewind_seconds,
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

    /// Adds the files and folders that hold the state of the product itself. The list only
    /// grows. A target that is one of them, is inside one or holds one is denied, and so is a
    /// work root or a move destination inside one. Pass every spelling the caller knows
    /// (as configured and as resolved on disk): the rules compare text only.
    pub fn with_product_paths(mut self, paths: impl IntoIterator<Item = AbsPath>) -> Self {
        for path in paths {
            if !self.product_paths.contains(&path) {
                self.product_paths.push(path);
            }
        }
        self
    }

    pub fn product_paths(&self) -> &[AbsPath] {
        &self.product_paths
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
        self.protection_of(path, true)
    }

    /// Why nothing may be placed in `path` nor done from it (a work root, a move destination),
    /// if so: [`Self::protection`], except that a folder merely holding product state is not
    /// protected for that, since only what is inside the product paths must not change.
    pub(crate) fn place_protection(&self, path: &AbsPath) -> Option<Protection> {
        self.protection_of(path, false)
    }

    fn protection_of(&self, path: &AbsPath, holding_product: bool) -> Option<Protection> {
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
        let product = self.product_paths.iter().any(|product| {
            path.is_loosely_within(product) || (holding_product && product.is_loosely_within(path))
        });
        if product {
            return Some(Protection::ProductData);
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
mod tests;
