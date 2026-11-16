//! The child environment policy against a poisoned parent environment and a careless user file.

use std::collections::{BTreeMap, BTreeSet};

use ads_core::child_env::{EnvLayer, EnvList, EnvPolicy, EnvPolicyError, NameCase};

const HARNESSES_TOML: &str = include_str!("../../../defaults/harnesses.toml");

/// What the shipped defaults must give the child. Written here, not read from the defaults, so a
/// wider allow list in the defaults fails this test.
const EXPECTED: &[&str] = &[
    "APPDATA",
    "COLORTERM",
    "ComSpec",
    "HOMEDRIVE",
    "HOMEPATH",
    "LOCALAPPDATA",
    "PATH",
    "PATHEXT",
    "SystemRoot",
    "TEMP",
    "TERM",
    "TMP",
    "USERPROFILE",
    "windir",
];

/// Names that carry, point to or configure a credential, in several spellings.
const SENSITIVE: &[&str] = &[
    "ANTHROPIC_API_KEY",
    "anthropic_api_key",
    "Anthropic_Auth_Token",
    "OPENAI_API_KEY",
    "openai_api_key",
    "GH_TOKEN",
    "gh_token",
    "GITHUB_TOKEN",
    "GITHUB_PAT",
    "github_pat",
    "MY_PAT",
    "GH_TOKEN_FILE",
    "gh_token_file",
    "CLAUDE_CODE_API_KEY_FILE",
    "claude_code_api_key_file",
    "CLAUDE_CONFIG_DIR",
    "Claude_Config_Dir",
    "CLAUDE_CODE_OAUTH_TOKEN",
    "CODEX_HOME",
    "GOOGLE_APPLICATION_CREDENTIALS",
    "google_application_credentials",
    "GEMINI_API_KEY",
    "FOO_TOKEN",
    "foo_token",
    "BAR_SECRET",
    "Bar_Secret",
    "BAR_SECRET_FILE",
    "SIGNING_KEY",
    "SIGNING_KEY_FILE",
    "DB_PASSWORD",
    "DB_PASS",
    "NPM_CONFIG__AUTH",
    "SSH_AUTH_SOCK",
    "AWS_SECRET_ACCESS_KEY",
    "aws_access_key_id",
    "AZURE_CLIENT_SECRET",
    "AZURE_TENANT_ID",
    "SERVICE_CREDENTIALS",
    "HF_HOME",
];

fn shipped() -> EnvLayer {
    let file: toml::Table = toml::from_str(HARNESSES_TOML).unwrap();
    file["env"].clone().try_into().unwrap()
}

fn policy(over: Option<&EnvLayer>, case: NameCase) -> Result<EnvPolicy, EnvPolicyError> {
    EnvPolicy::from_layers(&shipped(), over, case)
}

fn allow_only(names: &[&str]) -> EnvLayer {
    EnvLayer {
        allow: Some(names.iter().map(|n| n.to_string()).collect()),
        ..EnvLayer::default()
    }
}

fn set_only(pairs: &[(&str, &str)]) -> EnvLayer {
    EnvLayer {
        set: Some(
            pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        ),
        ..EnvLayer::default()
    }
}

/// Every expected name, every sensitive one and some noise.
fn parent() -> Vec<(String, String)> {
    let mut env: Vec<(String, String)> = EXPECTED
        .iter()
        .map(|name| (name.to_string(), format!("value-of-{name}")))
        .collect();
    env.extend(
        SENSITIVE
            .iter()
            .map(|n| (n.to_string(), "s3cr3t".to_string())),
    );
    // Common variables outside the list: a wider allow list in the defaults lets one through.
    for name in [
        "HOME",
        "USER",
        "USERNAME",
        "USERDOMAIN",
        "COMPUTERNAME",
        "LOGONSERVER",
        "OS",
        "LANG",
        "SHELL",
        "PWD",
        "PROMPT",
        "PSModulePath",
        "ProgramFiles",
        "ProgramData",
        "PUBLIC",
        "OneDrive",
        "NUMBER_OF_PROCESSORS",
        "PROCESSOR_ARCHITECTURE",
        "SESSIONNAME",
        "XDG_CONFIG_HOME",
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "NODE_OPTIONS",
        "RANDOM_VAR",
    ] {
        env.push((name.into(), "x".into()));
    }
    env
}

#[test]
fn the_shipped_defaults_give_exactly_the_fixed_list() {
    for case in [NameCase::Insensitive, NameCase::Sensitive] {
        let child = policy(None, case).unwrap().child_env(parent());
        let names: BTreeSet<&str> = child.keys().map(String::as_str).collect();
        assert_eq!(names, EXPECTED.iter().copied().collect(), "{case:?}");
        assert_eq!(child["PATH"], "value-of-PATH");
        assert_eq!(
            child["TERM"], "xterm-256color",
            "the parent's TERM is not used"
        );
        assert_eq!(child["COLORTERM"], "truecolor");
        assert!(!child.values().any(|v| v == "s3cr3t"));
    }
}

#[test]
fn a_sensitive_name_in_allow_is_refused_in_any_case() {
    for case in [NameCase::Insensitive, NameCase::Sensitive] {
        for name in SENSITIVE {
            let over = allow_only(&["PATH", name]);
            assert_eq!(
                policy(Some(&over), case),
                Err(EnvPolicyError::DeniedName {
                    list: EnvList::Allow,
                    index: 1
                }),
                "{name} {case:?}"
            );
        }
    }
}

#[test]
fn a_sensitive_name_in_set_is_refused_in_any_case() {
    for case in [NameCase::Insensitive, NameCase::Sensitive] {
        for name in SENSITIVE {
            let over = set_only(&[(name, "x")]);
            assert_eq!(
                policy(Some(&over), case),
                Err(EnvPolicyError::DeniedName {
                    list: EnvList::Set,
                    index: 0
                }),
                "{name} {case:?}"
            );
        }
    }
}

#[test]
fn a_user_layer_cannot_remove_the_shipped_deny_patterns() {
    let over = EnvLayer {
        allow: Some(vec!["GITHUB_PAT".into()]),
        deny: Some(vec![]),
        set: Some(BTreeMap::from([("GH_TOKEN_FILE".into(), "x".into())])),
    };
    assert!(matches!(
        policy(Some(&over), NameCase::Sensitive),
        Err(EnvPolicyError::DeniedName { .. })
    ));
}

#[test]
fn a_user_layer_adds_deny_patterns() {
    let over = EnvLayer {
        allow: Some(vec!["HOMEPATH".into()]),
        deny: Some(vec!["HOME*".into()]),
        set: None,
    };
    assert_eq!(
        policy(Some(&over), NameCase::Sensitive),
        Err(EnvPolicyError::DeniedName {
            list: EnvList::Allow,
            index: 0
        })
    );
}

#[test]
fn names_with_equals_nul_control_or_wildcard_are_refused() {
    for bad in ["", "A=B", "=C:", "A\0B", "A\nB", "A*"] {
        assert_eq!(
            policy(Some(&allow_only(&["PATH", bad])), NameCase::Sensitive),
            Err(EnvPolicyError::BadName {
                list: EnvList::Allow,
                index: 1
            }),
            "{bad:?}"
        );
        assert_eq!(
            policy(Some(&set_only(&[(bad, "x")])), NameCase::Sensitive),
            Err(EnvPolicyError::BadName {
                list: EnvList::Set,
                index: 0
            }),
            "{bad:?}"
        );
    }
}

#[test]
fn set_values_with_nul_or_control_are_refused() {
    for bad in ["a\0b", "a\nb", "\u{1b}[0m"] {
        assert_eq!(
            policy(Some(&set_only(&[("TERM", bad)])), NameCase::Sensitive),
            Err(EnvPolicyError::BadValue { index: 0 }),
            "{bad:?}"
        );
    }
}

#[test]
fn repeated_names_are_refused_under_the_case_rule() {
    let over = allow_only(&["PATH", "TEMP", "Path"]);
    assert_eq!(
        policy(Some(&over), NameCase::Insensitive),
        Err(EnvPolicyError::DuplicateName {
            list: EnvList::Allow,
            index: 2
        })
    );
    assert!(policy(Some(&over), NameCase::Sensitive).is_ok());
    assert_eq!(
        policy(Some(&allow_only(&["PATH", "PATH"])), NameCase::Sensitive),
        Err(EnvPolicyError::DuplicateName {
            list: EnvList::Allow,
            index: 1
        })
    );
    let set = set_only(&[("TERM", "a"), ("term", "b")]);
    assert_eq!(
        policy(Some(&set), NameCase::Insensitive),
        Err(EnvPolicyError::DuplicateName {
            list: EnvList::Set,
            index: 1
        })
    );
}

#[test]
fn errors_never_quote_the_rejected_text() {
    let marker = "ZQXMARKER";
    let cases = [
        allow_only(&[&format!("{marker}_TOKEN")]),
        allow_only(&[&format!("{marker}=1")]),
        allow_only(&["PATH", "PATH"]),
        set_only(&[(&format!("{marker}_SECRET"), "x")]),
        set_only(&[("TERM", &format!("{marker}\0"))]),
        EnvLayer {
            deny: Some(vec![format!("*{marker}*X*")]),
            ..EnvLayer::default()
        },
    ];
    for over in cases {
        let error = policy(Some(&over), NameCase::Sensitive).unwrap_err();
        for text in [error.to_string(), format!("{error:?}")] {
            assert!(!text.contains(marker), "{text}");
        }
    }
}

#[test]
fn on_windows_names_match_without_case_and_keep_the_list_spelling() {
    let env = [("Path", "C:\\bin"), ("systemroot", "C:\\Windows")];
    let child = policy(None, NameCase::Insensitive).unwrap().child_env(env);
    assert_eq!(child["PATH"], "C:\\bin");
    assert_eq!(child["SystemRoot"], "C:\\Windows");
    assert!(!child.contains_key("Path"));

    let child = policy(None, NameCase::Sensitive).unwrap().child_env(env);
    assert!(!child.contains_key("PATH") && !child.contains_key("Path"));
}

#[test]
fn two_spellings_in_the_parent_pick_the_exact_one_then_the_smallest() {
    let policy = policy(None, NameCase::Insensitive).unwrap();
    let child = policy.child_env([("path", "a"), ("PATH", "b"), ("Path", "c")]);
    assert_eq!(child["PATH"], "b");
    let child = policy.child_env([("path", "a"), ("Path", "c")]);
    assert_eq!(child["PATH"], "c");
}

#[test]
fn a_product_value_replaces_any_spelling_from_the_parent() {
    let child = policy(Some(&allow_only(&["term"])), NameCase::Insensitive)
        .unwrap()
        .child_env([("term", "dumb")]);
    assert_eq!(child.len(), 2);
    assert_eq!(child["TERM"], "xterm-256color");
}
