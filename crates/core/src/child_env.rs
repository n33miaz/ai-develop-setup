//! Environment of a child process started for a tool: only allow-listed names reach it, and
//! names known to carry or point to credentials never do. A name in `allow` or `set` that
//! matches a deny pattern is refused when the policy is built. Pure: the caller passes the
//! parent environment and the parameters as data.

use std::collections::BTreeMap;
use std::fmt;

use serde::Deserialize;

/// How variable names are compared against the allow list. Deny patterns ignore case always,
/// with the widest comparison (see [`EnvPolicy::is_denied`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NameCase {
    /// Windows: `Path` and `PATH` are the same variable. Copying from the parent folds only
    /// ASCII letters, while Windows also folds other letters (`Ä` and `ä`): an allowed name
    /// copies only a parent name equal to it apart from ASCII case, so it can only let less
    /// through. Repeated names and the names `set` replaces use the wider fold, so two entries
    /// Windows takes as one variable are refused, and a `set` value always wins.
    Insensitive,
    Sensitive,
}

impl NameCase {
    /// The rule of the system this code runs on.
    pub const fn host() -> Self {
        if cfg!(windows) {
            Self::Insensitive
        } else {
            Self::Sensitive
        }
    }

    fn same(self, a: &str, b: &str) -> bool {
        match self {
            Self::Insensitive => a.eq_ignore_ascii_case(b),
            Self::Sensitive => a == b,
        }
    }

    /// Whether the system may take `a` and `b` as one variable: on Windows, equal once every
    /// letter with a one-letter upper case is upper-cased. At least as wide as Windows' rule.
    fn one_variable(self, a: &str, b: &str) -> bool {
        match self {
            Self::Insensitive => a.chars().map(simple_upper).eq(b.chars().map(simple_upper)),
            Self::Sensitive => a == b,
        }
    }
}

fn simple_upper(c: char) -> char {
    let mut upper = c.to_uppercase();
    match (upper.next(), upper.next()) {
        (Some(one), None) => one,
        _ => c,
    }
}

/// One layer of the `[env]` table: the shipped defaults or the user's file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvLayer {
    /// Names copied from the parent. A later layer replaces the list.
    pub allow: Option<Vec<String>>,
    /// Patterns never passed on. A later layer adds to the list, never removes.
    pub deny: Option<Vec<String>>,
    /// Values the product sets itself, not read from the parent. A later layer replaces the table.
    pub set: Option<BTreeMap<String, String>>,
}

/// Which list of `[env]` an error is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnvList {
    Allow,
    Deny,
    /// Entries in name order.
    Set,
}

impl EnvList {
    fn key(self) -> &'static str {
        match self {
            Self::Allow => "env.allow",
            Self::Deny => "env.deny",
            Self::Set => "env.set",
        }
    }
}

/// Why a policy was refused. Carries a position, never the rejected name or value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvPolicyError {
    MissingKey(&'static str),
    /// Empty, or with `=`, `*` or a control character.
    BadName {
        list: EnvList,
        index: usize,
    },
    /// Not `NAME`, `PREFIX*`, `*SUFFIX` or `*INFIX*`. `index` counts the shipped patterns first.
    BadPattern {
        index: usize,
    },
    /// A name in `allow` or `set` matches a deny pattern.
    DeniedName {
        list: EnvList,
        index: usize,
    },
    /// The same name twice in one list, as the system would compare them.
    DuplicateName {
        list: EnvList,
        index: usize,
    },
    /// A value in `set` with NUL or another control character.
    BadValue {
        index: usize,
    },
}

impl fmt::Display for EnvPolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingKey(key) => write!(f, "missing key `env.{key}`"),
            Self::BadName { list, index } => {
                write!(f, "invalid variable name at `{}` entry {index}", list.key())
            }
            Self::BadPattern { index } => write!(f, "invalid pattern at `env.deny` entry {index}"),
            Self::DeniedName { list, index } => {
                write!(f, "denied variable name at `{}` entry {index}", list.key())
            }
            Self::DuplicateName { list, index } => {
                write!(
                    f,
                    "repeated variable name at `{}` entry {index}",
                    list.key()
                )
            }
            Self::BadValue { index } => write!(f, "invalid value at `env.set` entry {index}"),
        }
    }
}

impl std::error::Error for EnvPolicyError {}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Pattern {
    Exact(String),
    Prefix(String),
    Suffix(String),
    Infix(String),
}

impl Pattern {
    /// Stored in ASCII upper case; matched against the upper-cased name.
    fn parse(raw: &str) -> Option<Self> {
        let upper = raw.to_ascii_uppercase();
        let pattern = match (upper.strip_prefix('*'), upper.strip_suffix('*')) {
            (Some(_), Some(_)) => Self::Infix(upper.get(1..upper.len().checked_sub(1)?)?.into()),
            (Some(suffix), None) => Self::Suffix(suffix.to_owned()),
            (None, Some(prefix)) => Self::Prefix(prefix.to_owned()),
            (None, None) => Self::Exact(upper.clone()),
        };
        let core = match &pattern {
            Self::Exact(text) | Self::Prefix(text) | Self::Suffix(text) | Self::Infix(text) => text,
        };
        valid_name(core).then_some(pattern)
    }

    fn matches(&self, upper_name: &str) -> bool {
        match self {
            Self::Exact(text) => upper_name == text,
            Self::Prefix(text) => upper_name.starts_with(text.as_str()),
            Self::Suffix(text) => upper_name.ends_with(text.as_str()),
            Self::Infix(text) => upper_name.contains(text.as_str()),
        }
    }
}

/// Fullwidth ASCII forms (`Ｔ`, `＿`) as plain ASCII. Other forms that compatibility
/// normalization turns into ASCII (mathematical `𝐓`, circled `Ⓣ`, ligatures `ﬁ`) are left as
/// they are: no system reads such a name as the ASCII one, so it names another variable.
fn fold_fullwidth(name: &str) -> String {
    name.chars()
        .map(|c| match u32::from(c) {
            code @ 0xFF01..=0xFF5E => char::from_u32(code - 0xFEE0).unwrap_or(c),
            _ => c,
        })
        .collect()
}

fn valid_name(name: &str) -> bool {
    !name.is_empty() && !name.chars().any(|c| c == '=' || c == '*' || c.is_control())
}

/// The allow list, the deny patterns and the values the product sets. Only
/// [`EnvPolicy::from_layers`] builds one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvPolicy {
    allow: Vec<String>,
    deny: Vec<Pattern>,
    set: BTreeMap<String, String>,
    case: NameCase,
}

impl EnvPolicy {
    /// `base` must be complete; `over`, if any, replaces `allow` and `set` and adds to `deny`.
    pub fn from_layers(
        base: &EnvLayer,
        over: Option<&EnvLayer>,
        case: NameCase,
    ) -> Result<Self, EnvPolicyError> {
        let over = over.cloned().unwrap_or_default();
        let allow = over
            .allow
            .or_else(|| base.allow.clone())
            .ok_or(EnvPolicyError::MissingKey("allow"))?;
        let set = over
            .set
            .or_else(|| base.set.clone())
            .ok_or(EnvPolicyError::MissingKey("set"))?;
        let base_deny = base
            .deny
            .as_ref()
            .ok_or(EnvPolicyError::MissingKey("deny"))?;
        let deny = base_deny
            .iter()
            .chain(over.deny.iter().flatten())
            .enumerate()
            .map(|(index, raw)| Pattern::parse(raw).ok_or(EnvPolicyError::BadPattern { index }))
            .collect::<Result<Vec<_>, _>>()?;
        let policy = Self {
            allow,
            deny,
            set,
            case,
        };
        policy.check_names(EnvList::Allow, policy.allow.iter())?;
        policy.check_names(EnvList::Set, policy.set.keys())?;
        if let Some(index) = policy
            .set
            .values()
            .position(|value| value.chars().any(char::is_control))
        {
            return Err(EnvPolicyError::BadValue { index });
        }
        Ok(policy)
    }

    /// Every name valid, not denied and not repeated, in list order.
    fn check_names<'a>(
        &self,
        list: EnvList,
        names: impl Iterator<Item = &'a String>,
    ) -> Result<(), EnvPolicyError> {
        let mut seen: Vec<&str> = Vec::new();
        for (index, name) in names.enumerate() {
            if !valid_name(name) {
                return Err(EnvPolicyError::BadName { list, index });
            }
            if self.is_denied(name) {
                return Err(EnvPolicyError::DeniedName { list, index });
            }
            if seen.iter().any(|have| self.case.one_variable(have, name)) {
                return Err(EnvPolicyError::DuplicateName { list, index });
            }
            seen.push(name);
        }
        Ok(())
    }

    /// Whether `name` matches a deny pattern in any of its folded spellings. Wider than any
    /// system's comparison on purpose, so it only ever serves to deny: `MY_ſECRET` and
    /// `ＧＨ_TOKEN` are denied like `MY_SECRET` and `GH_TOKEN`.
    pub fn is_denied(&self, name: &str) -> bool {
        let fullwidth = fold_fullwidth(name);
        [name, fullwidth.as_str()]
            .into_iter()
            .flat_map(|form| {
                [
                    form.to_ascii_uppercase(),
                    form.to_uppercase(),
                    form.to_lowercase().to_uppercase(),
                ]
            })
            .any(|upper| self.deny.iter().any(|pattern| pattern.matches(&upper)))
    }

    /// The child's whole environment: each allowed name the parent has, written as the allow
    /// list spells it, then the values the product sets. Nothing else. A parent value with NUL
    /// is skipped: no process environment can hold it.
    pub fn child_env<I, K, V>(&self, parent: I) -> BTreeMap<String, String>
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        let parent: Vec<(String, String)> = parent
            .into_iter()
            .map(|(name, value)| (name.into(), value.into()))
            .filter(|(_, value)| !value.contains('\0'))
            .collect();
        let mut child: BTreeMap<String, String> = BTreeMap::new();
        for wanted in &self.allow {
            // Checked again here so the filter never depends on how the policy was built.
            if self.is_denied(wanted) {
                continue;
            }
            // Two spellings of one name in the parent: the exact one, else the smallest.
            let found = parent
                .iter()
                .filter(|(name, _)| self.case.same(name, wanted))
                .min_by_key(|(name, _)| (name != wanted, name.clone()));
            if let Some((_, value)) = found {
                child.insert(wanted.clone(), value.clone());
            }
        }
        for (name, value) in &self.set {
            if self.is_denied(name) {
                continue;
            }
            child.retain(|have, _| !self.case.one_variable(have, name));
            child.insert(name.clone(), value.clone());
        }
        child
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layer(allow: &[&str], deny: &[&str], set: &[(&str, &str)]) -> EnvLayer {
        EnvLayer {
            allow: Some(allow.iter().map(|s| s.to_string()).collect()),
            deny: Some(deny.iter().map(|s| s.to_string()).collect()),
            set: Some(
                set.iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            ),
        }
    }

    #[test]
    fn patterns_parse_and_refuse_bad_shapes() {
        assert_eq!(
            Pattern::parse("*_token"),
            Some(Pattern::Suffix("_TOKEN".into()))
        );
        assert_eq!(
            Pattern::parse("aws_*"),
            Some(Pattern::Prefix("AWS_".into()))
        );
        assert_eq!(
            Pattern::parse("GH_TOKEN"),
            Some(Pattern::Exact("GH_TOKEN".into()))
        );
        assert_eq!(
            Pattern::parse("*_auth*"),
            Some(Pattern::Infix("_AUTH".into()))
        );
        for bad in ["*", "**", "***", "*A*B*", "A*B", "", "A=B*", "*A\0*"] {
            assert_eq!(Pattern::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn missing_keys_in_the_base_are_refused() {
        let empty = EnvLayer::default();
        assert_eq!(
            EnvPolicy::from_layers(&empty, None, NameCase::Sensitive),
            Err(EnvPolicyError::MissingKey("allow"))
        );
    }

    #[test]
    fn the_filter_drops_denied_names_even_in_a_policy_built_without_checks() {
        // Built by hand, skipping `from_layers`: the filter alone must still hold.
        let policy = EnvPolicy {
            allow: vec!["GH_TOKEN".into(), "PATH".into()],
            deny: vec![Pattern::parse("*TOKEN*").unwrap()],
            set: BTreeMap::from([("MY_TOKEN".into(), "x".into())]),
            case: NameCase::Sensitive,
        };
        let child = policy.child_env([("GH_TOKEN", "s"), ("PATH", "p")]);
        assert_eq!(child, BTreeMap::from([("PATH".into(), "p".into())]));
    }

    #[test]
    fn names_windows_takes_as_one_are_one_name() {
        // `Ä` and `ä` are one letter to Windows, though copying folds only ASCII.
        let policy = EnvPolicy::from_layers(
            &layer(&["äx"], &[], &[("ÄX", "set")]),
            None,
            NameCase::Insensitive,
        )
        .unwrap();
        let child = policy.child_env([("äx", "parent")]);
        assert_eq!(child, BTreeMap::from([("ÄX".into(), "set".into())]));
        assert_eq!(
            EnvPolicy::from_layers(&layer(&["äx", "ÄX"], &[], &[]), None, NameCase::Insensitive),
            Err(EnvPolicyError::DuplicateName {
                list: EnvList::Allow,
                index: 1
            })
        );
        // Elsewhere they are two names.
        let policy = EnvPolicy::from_layers(
            &layer(&["äx", "ÄX"], &[], &[("ÄX", "set")]),
            None,
            NameCase::Sensitive,
        )
        .unwrap();
        let child = policy.child_env([("äx", "parent")]);
        assert_eq!(
            child,
            BTreeMap::from([("äx".into(), "parent".into()), ("ÄX".into(), "set".into())])
        );
    }

    #[test]
    fn a_parent_value_with_nul_is_skipped() {
        let policy = EnvPolicy::from_layers(
            &layer(&["PATH", "TEMP"], &[], &[]),
            None,
            NameCase::Sensitive,
        )
        .unwrap();
        let child = policy.child_env([("PATH", "a\0b"), ("TEMP", "t")]);
        assert_eq!(child, BTreeMap::from([("TEMP".into(), "t".into())]));
    }
}
