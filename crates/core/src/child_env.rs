//! Environment of a child process started for a tool: only allow-listed names reach it, and
//! names that look like credentials never do, even when someone allow-lists them. Pure: the
//! caller passes the parent environment and the parameters as data.

use std::collections::BTreeMap;
use std::fmt;

use serde::Deserialize;

/// How variable names are compared against the allow list. Deny patterns ignore case always.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NameCase {
    /// Windows: `Path` and `PATH` are the same variable.
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvPolicyError {
    MissingKey(&'static str),
    /// Empty, or with `=`, `*` or a control character.
    BadName(String),
    /// Not `NAME`, `PREFIX*` or `*SUFFIX`.
    BadPattern(String),
    /// A value the product sets itself matches a deny pattern.
    SetDenied(String),
}

impl fmt::Display for EnvPolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingKey(key) => write!(f, "missing key `env.{key}`"),
            Self::BadName(name) => write!(f, "invalid variable name `{name}`"),
            Self::BadPattern(pattern) => write!(f, "invalid deny pattern `{pattern}`"),
            Self::SetDenied(name) => write!(f, "`env.set` names a denied variable `{name}`"),
        }
    }
}

impl std::error::Error for EnvPolicyError {}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Pattern {
    Exact(String),
    Prefix(String),
    Suffix(String),
}

impl Pattern {
    /// Stored in ASCII upper case; matched against the upper-cased name.
    fn parse(raw: &str) -> Result<Self, EnvPolicyError> {
        let bad = || EnvPolicyError::BadPattern(raw.to_owned());
        let upper = raw.to_ascii_uppercase();
        let pattern = match (upper.strip_prefix('*'), upper.strip_suffix('*')) {
            (Some(_), Some(_)) => return Err(bad()),
            (Some(suffix), None) => Self::Suffix(suffix.to_owned()),
            (None, Some(prefix)) => Self::Prefix(prefix.to_owned()),
            (None, None) => Self::Exact(upper.clone()),
        };
        let core = match &pattern {
            Self::Exact(text) | Self::Prefix(text) | Self::Suffix(text) => text,
        };
        if valid_name(core) {
            Ok(pattern)
        } else {
            Err(bad())
        }
    }

    fn matches(&self, upper_name: &str) -> bool {
        match self {
            Self::Exact(text) => upper_name == text,
            Self::Prefix(text) => upper_name.starts_with(text.as_str()),
            Self::Suffix(text) => upper_name.ends_with(text.as_str()),
        }
    }
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
            .map(|raw| Pattern::parse(raw))
            .collect::<Result<Vec<_>, _>>()?;
        for name in allow.iter().chain(set.keys()) {
            if !valid_name(name) {
                return Err(EnvPolicyError::BadName(name.clone()));
            }
        }
        let policy = Self {
            allow,
            deny,
            set,
            case,
        };
        if let Some(name) = policy.set.keys().find(|name| policy.is_denied(name)) {
            return Err(EnvPolicyError::SetDenied(name.clone()));
        }
        Ok(policy)
    }

    /// Whether `name` matches a deny pattern, ignoring case.
    pub fn is_denied(&self, name: &str) -> bool {
        let upper = name.to_ascii_uppercase();
        self.deny.iter().any(|pattern| pattern.matches(&upper))
    }

    /// The child's whole environment: each allowed, non-denied name the parent has, written as
    /// the allow list spells it, then the values the product sets. Nothing else.
    pub fn child_env<I, K, V>(&self, parent: I) -> BTreeMap<String, String>
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        let parent: Vec<(String, String)> = parent
            .into_iter()
            .map(|(name, value)| (name.into(), value.into()))
            .collect();
        let mut child: BTreeMap<String, String> = BTreeMap::new();
        for wanted in &self.allow {
            if self.is_denied(wanted) || child.keys().any(|have| self.case.same(have, wanted)) {
                continue;
            }
            // Two spellings of one name in the parent: the exact one, else the smallest.
            let found = parent
                .iter()
                .filter(|(name, _)| self.case.same(name, wanted) && !self.is_denied(name))
                .min_by_key(|(name, _)| (name != wanted, name.clone()));
            if let Some((_, value)) = found {
                child.insert(wanted.clone(), value.clone());
            }
        }
        for (name, value) in &self.set {
            child.retain(|have, _| !self.case.same(have, name));
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
            Pattern::parse("*_token").unwrap(),
            Pattern::Suffix("_TOKEN".into())
        );
        assert_eq!(
            Pattern::parse("aws_*").unwrap(),
            Pattern::Prefix("AWS_".into())
        );
        assert_eq!(
            Pattern::parse("GH_TOKEN").unwrap(),
            Pattern::Exact("GH_TOKEN".into())
        );
        for bad in ["*", "**", "*A*", "A*B", "", "A=B*"] {
            assert!(Pattern::parse(bad).is_err(), "{bad}");
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
    fn a_set_value_that_is_denied_is_refused() {
        let base = layer(&[], &["*_TOKEN"], &[("MY_TOKEN", "x")]);
        assert_eq!(
            EnvPolicy::from_layers(&base, None, NameCase::Sensitive),
            Err(EnvPolicyError::SetDenied("MY_TOKEN".into()))
        );
    }

    #[test]
    fn bad_allow_names_are_refused() {
        for bad in ["", "A=B", "A*"] {
            let base = layer(&[bad], &[], &[]);
            assert!(EnvPolicy::from_layers(&base, None, NameCase::Sensitive).is_err());
        }
    }
}
