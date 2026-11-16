//! The child environment policy against a poisoned parent environment.

use std::collections::{BTreeMap, BTreeSet};

use ads_core::child_env::{EnvLayer, EnvPolicy, NameCase};

const HARNESSES_TOML: &str = include_str!("../../../defaults/harnesses.toml");

const POISON: &[&str] = &[
    "ANTHROPIC_API_KEY",
    "anthropic_api_key",
    "Anthropic_Auth_Token",
    "OPENAI_API_KEY",
    "openai_api_key",
    "GH_TOKEN",
    "gh_token",
    "GITHUB_TOKEN",
    "FOO_TOKEN",
    "foo_token",
    "BAR_SECRET",
    "Bar_Secret",
    "AWS_SECRET_ACCESS_KEY",
    "aws_access_key_id",
    "AZURE_CLIENT_SECRET",
    "DB_PASSWORD",
    "SIGNING_KEY",
    "MY_API_KEY",
];

fn shipped() -> EnvLayer {
    let file: toml::Table = toml::from_str(HARNESSES_TOML).unwrap();
    file["env"].clone().try_into().unwrap()
}

fn allowed() -> Vec<String> {
    shipped().allow.unwrap()
}

/// Every allowed name plus every poisoned one, and some unrelated noise.
fn parent() -> Vec<(String, String)> {
    let mut env: Vec<(String, String)> = allowed()
        .into_iter()
        .map(|name| (name.clone(), format!("value-of-{name}")))
        .collect();
    for name in POISON {
        env.push((name.to_string(), "s3cr3t".into()));
    }
    env.push(("RANDOM_VAR".into(), "x".into()));
    env.push(("TERM".into(), "dumb".into()));
    env
}

fn policy(over: Option<&EnvLayer>, case: NameCase) -> EnvPolicy {
    EnvPolicy::from_layers(&shipped(), over, case).unwrap()
}

fn assert_no_poison(child: &BTreeMap<String, String>) {
    for (name, value) in child {
        assert!(
            !POISON.iter().any(|p| p.eq_ignore_ascii_case(name)),
            "leaked {name}"
        );
        assert_ne!(value, "s3cr3t", "leaked the value of {name}");
    }
}

#[test]
fn the_child_gets_exactly_the_allow_list_and_the_product_values() {
    for case in [NameCase::Insensitive, NameCase::Sensitive] {
        let child = policy(None, case).child_env(parent());
        let mut expected: BTreeSet<String> = allowed().into_iter().collect();
        expected.insert("TERM".into());
        expected.insert("COLORTERM".into());
        assert_eq!(child.keys().cloned().collect::<BTreeSet<_>>(), expected);
        for name in allowed() {
            assert_eq!(child[&name], format!("value-of-{name}"));
        }
        assert_eq!(
            child["TERM"], "xterm-256color",
            "the parent's TERM is not used"
        );
        assert_eq!(child["COLORTERM"], "truecolor");
        assert_no_poison(&child);
    }
}

#[test]
fn credential_names_never_leak_even_when_the_user_allows_them() {
    let mut allow = allowed();
    allow.extend(POISON.iter().map(|name| name.to_string()));
    allow.extend(["anthropic_base_url", "OpenAI_Org"].map(String::from));
    let over = EnvLayer {
        allow: Some(allow),
        ..EnvLayer::default()
    };
    for case in [NameCase::Insensitive, NameCase::Sensitive] {
        let policy = policy(Some(&over), case);
        let mut env = parent();
        env.push(("ANTHROPIC_BASE_URL".into(), "s3cr3t".into()));
        env.push(("OPENAI_ORG".into(), "s3cr3t".into()));
        let child = policy.child_env(env);
        assert_no_poison(&child);
        assert!(
            !child
                .keys()
                .any(|k| k.eq_ignore_ascii_case("ANTHROPIC_BASE_URL"))
        );
        assert!(!child.keys().any(|k| k.eq_ignore_ascii_case("OPENAI_ORG")));
    }
}

#[test]
fn a_user_layer_cannot_remove_the_shipped_deny_patterns() {
    let over = EnvLayer {
        allow: Some(vec!["GH_TOKEN".into(), "FOO_TOKEN".into(), "PATH".into()]),
        deny: Some(vec![]),
        set: None,
    };
    let policy = policy(Some(&over), NameCase::Sensitive);
    let child = policy.child_env(parent());
    assert_no_poison(&child);
    assert!(child.contains_key("PATH"));
}

#[test]
fn a_user_layer_adds_deny_patterns() {
    let over = EnvLayer {
        deny: Some(vec!["HOME*".into()]),
        ..EnvLayer::default()
    };
    let child = policy(Some(&over), NameCase::Sensitive).child_env(parent());
    assert!(!child.contains_key("HOMEDRIVE") && !child.contains_key("HOMEPATH"));
    assert!(child.contains_key("PATH"));
}

#[test]
fn on_windows_names_match_without_case_and_keep_the_list_spelling() {
    let env = [("Path", "C:\\bin"), ("systemroot", "C:\\Windows")];
    let child = policy(None, NameCase::Insensitive).child_env(env);
    assert_eq!(child["PATH"], "C:\\bin");
    assert_eq!(child["SystemRoot"], "C:\\Windows");
    assert!(!child.contains_key("Path"));

    let child = policy(None, NameCase::Sensitive).child_env(env);
    assert!(!child.contains_key("PATH") && !child.contains_key("Path"));
}

#[test]
fn two_spellings_in_the_parent_pick_the_exact_one_then_the_smallest() {
    let case = NameCase::Insensitive;
    let child = policy(None, case).child_env([("path", "a"), ("PATH", "b"), ("Path", "c")]);
    assert_eq!(child["PATH"], "b");
    let child = policy(None, case).child_env([("path", "a"), ("Path", "c")]);
    assert_eq!(child["PATH"], "c");
}

#[test]
fn a_product_value_replaces_any_spelling_from_the_parent() {
    let over = EnvLayer {
        allow: Some(vec!["term".into()]),
        ..EnvLayer::default()
    };
    let child = policy(Some(&over), NameCase::Insensitive).child_env([("term", "dumb")]);
    assert_eq!(child.len(), 2);
    assert_eq!(child["TERM"], "xterm-256color");
}
