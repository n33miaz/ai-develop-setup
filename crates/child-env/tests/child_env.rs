//! The child environment policy against a poisoned parent environment and a careless user file.

use std::collections::{BTreeMap, BTreeSet};

use ads_child_env::{EnvLayer, EnvList, EnvPolicy, EnvPolicyError, HARNESSES_TOML, NameCase};

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
        deny_exceptions: None,
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
        deny_exceptions: None,
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

/// The shipped `allow`, in file order. Written here so that any name added to or removed from
/// the defaults fails this test, even one no parent in these tests carries.
const SHIPPED_ALLOW: &[&str] = &[
    "SystemRoot",
    "windir",
    "ComSpec",
    "PATH",
    "PATHEXT",
    "USERPROFILE",
    "APPDATA",
    "LOCALAPPDATA",
    "HOMEDRIVE",
    "HOMEPATH",
    "TEMP",
    "TMP",
];

/// The shipped `set`, name and value.
const SHIPPED_SET: &[(&str, &str)] = &[("COLORTERM", "truecolor"), ("TERM", "xterm-256color")];

/// The shipped `deny`, in file order: removing any pattern fails this test.
const SHIPPED_DENY: &[&str] = &[
    "*TOKEN*",
    "*SECRET*",
    "*PASSWORD*",
    "*PASSWD*",
    "*CREDENTIAL*",
    "*API_KEY*",
    "*APIKEY*",
    "*PRIVATE_KEY*",
    "*_KEY",
    "*_KEY_FILE",
    "*_KEYS",
    "*_PAT",
    "*_PAT_*",
    "*_AUTH*",
    "*_PASS*",
    "*PASSPHRASE*",
    "*ASKPASS*",
    "*_SESSION*",
    "*_COOKIE*",
    "*_JWT*",
    "*_PWD",
    "MYSQL_PWD",
    "*_DSN",
    "*_KEY_ID",
    "AUTHORIZATION",
    "DATABASE_URL",
    "KUBECONFIG",
    "*NETRC*",
    "PGPASSFILE",
    "DOCKER_CONFIG",
    "NPM_CONFIG_USERCONFIG",
    "NPM_CONFIG_GLOBALCONFIG",
    "NPM_CONFIG_PREFIX",
    "NPM_CONFIG_REGISTRY",
    "NPM_CONFIG_USERNAME",
    "NPM_CONFIG_OTP",
    "NPM_CONFIG_PROXY",
    "NPM_CONFIG_HTTPS_PROXY",
    "NPM_CONFIG_CERT",
    "NPM_CONFIG_CERTFILE",
    "NPM_CONFIG_KEYFILE",
    "ANTHROPIC_*",
    "CLAUDE_*",
    "OPENAI_*",
    "CODEX_*",
    "GEMINI_*",
    "GOOGLE_*",
    "GCLOUD_*",
    "CLOUDSDK_*",
    "VERTEX_*",
    "AWS_*",
    "AZURE_*",
    "GH_*",
    "GITHUB_*",
    "COPILOT_*",
    "MISTRAL_*",
    "COHERE_*",
    "GROQ_*",
    "OPENROUTER_*",
    "HF_*",
    "HUGGING_FACE_*",
];

fn strings(list: Option<&Vec<String>>) -> Vec<&str> {
    list.unwrap().iter().map(String::as_str).collect()
}

#[test]
fn the_shipped_allow_set_and_deny_are_exactly_the_fixed_lists() {
    let layer = shipped();
    assert_eq!(strings(layer.allow.as_ref()), SHIPPED_ALLOW);
    assert_eq!(strings(layer.deny.as_ref()), SHIPPED_DENY);
    let set: Vec<(&str, &str)> = layer
        .set
        .as_ref()
        .unwrap()
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect();
    assert_eq!(set, SHIPPED_SET);
}

/// The shipped deny list without `pattern`, which must be in it.
fn shipped_without(pattern: &str) -> EnvLayer {
    let mut layer = shipped();
    let deny = layer.deny.as_mut().unwrap();
    let before = deny.len();
    deny.retain(|have| have != pattern);
    assert_eq!(deny.len() + 1, before, "{pattern} is shipped once");
    layer
}

#[test]
fn each_less_used_deny_pattern_refuses_its_names_on_its_own() {
    // Names that only the pattern under test catches, so that removing it lets them through.
    let cases: &[(&str, &[&str])] = &[
        (
            "*_AUTH*",
            &[
                "CONSUL_HTTP_AUTH",
                "SSH_AUTH_SOCK",
                "APP_AUTHDATA",
                "NPM_CONFIG__AUTH",
            ],
        ),
        ("*PASSPHRASE*", &["PASSPHRASE", "GpgPassphrase"]),
        ("*ASKPASS*", &["GIT_ASKPASS", "SSH_ASKPASS", "sudo_askpass"]),
        (
            "*_SESSION*",
            &[
                "BW_SESSION",
                "OP_SESSION_myaccount",
                "APP_SESSION_ID",
                "web_sessionid_store",
                "TG_SESSION_STRING",
            ],
        ),
        ("*NETRC*", &["NETRC", "CURL_NETRC_FILE"]),
        ("PGPASSFILE", &["PGPASSFILE"]),
        ("DOCKER_CONFIG", &["DOCKER_CONFIG"]),
        ("NPM_CONFIG_USERCONFIG", &["NPM_CONFIG_USERCONFIG"]),
        ("NPM_CONFIG_GLOBALCONFIG", &["NPM_CONFIG_GLOBALCONFIG"]),
        ("NPM_CONFIG_PREFIX", &["NPM_CONFIG_PREFIX"]),
        ("NPM_CONFIG_REGISTRY", &["NPM_CONFIG_REGISTRY"]),
        ("NPM_CONFIG_USERNAME", &["NPM_CONFIG_USERNAME"]),
        ("NPM_CONFIG_CERT", &["NPM_CONFIG_CERT"]),
        ("NPM_CONFIG_CERTFILE", &["NPM_CONFIG_CERTFILE"]),
        ("NPM_CONFIG_KEYFILE", &["NPM_CONFIG_KEYFILE"]),
        (
            "*_COOKIE*",
            &["BROWSER_COOKIE_JAR", "site_cookie", "App_Cookies"],
        ),
        ("*PASSWD*", &["PASSWD", "smbpasswd", "LdapPasswdFile"]),
        ("*_JWT*", &["CI_JOB_JWT", "app_jwt_signer"]),
        ("*_PWD", &["SA_PWD", "redis_pwd"]),
        ("*_DSN", &["SENTRY_DSN", "app_dsn"]),
        ("*_KEY_ID", &["R2_ACCESS_KEY_ID", "signing_key_id"]),
        ("AUTHORIZATION", &["AUTHORIZATION", "Authorization"]),
        ("DATABASE_URL", &["DATABASE_URL", "database_url"]),
        ("KUBECONFIG", &["KUBECONFIG", "KubeConfig"]),
    ];
    for (pattern, names) in cases {
        for case in [NameCase::Insensitive, NameCase::Sensitive] {
            let without = shipped_without(pattern);
            for name in *names {
                let over = allow_only(&["PATH", name]);
                assert_eq!(
                    policy(Some(&over), case),
                    Err(EnvPolicyError::DeniedName {
                        list: EnvList::Allow,
                        index: 1
                    }),
                    "{pattern} {name} {case:?}"
                );
                assert_eq!(
                    policy(Some(&set_only(&[(name, "x")])), case),
                    Err(EnvPolicyError::DeniedName {
                        list: EnvList::Set,
                        index: 0
                    }),
                    "{pattern} {name} {case:?}"
                );
                // Without the pattern the same name passes: no other pattern hides a gap.
                let child = EnvPolicy::from_layers(&without, Some(&over), case)
                    .unwrap_or_else(|error| panic!("{pattern} {name}: {error}"))
                    .child_env([(*name, "v")]);
                assert_eq!(child.get(*name).map(String::as_str), Some("v"), "{name}");
            }
        }
    }
    // Also caught by `*_PWD`; listed on its own so that narrowing the suffix keeps it denied.
    let policy = policy(None, NameCase::Sensitive).unwrap();
    assert!(policy.is_denied("MYSQL_PWD"));
    assert!(
        EnvPolicy::from_layers(&shipped_without("*_PWD"), None, NameCase::Sensitive)
            .unwrap()
            .is_denied("MYSQL_PWD")
    );
}

#[test]
fn the_essential_names_stay_allowed_and_are_copied() {
    // The shipped list, the Linux names documented in the defaults and names that only look
    // like a denied one.
    let essential: Vec<&str> = SHIPPED_ALLOW
        .iter()
        .copied()
        .chain([
            "HOME",
            "TMPDIR",
            "LANG",
            "USER",
            "LOGNAME",
            "SHELL",
            "PWD",
            "OLDPWD",
            "USERNAME",
            "SESSIONNAME",
            "COOKIEJAR",
            "PASSPORT_DIR",
            "KEYBOARD_LAYOUT",
            "DSN_LIST_DIR",
            "AUTHORIZED_KEYS_DIR",
            // `_AUTHOR` and harmless `_SESSION` names.
            "GIT_AUTHOR_NAME",
            "GIT_AUTHOR_EMAIL",
            "GIT_AUTHOR_DATE",
            "WT_SESSION",
            "XDG_SESSION_ID",
            "XDG_SESSION_TYPE",
            "XDG_SESSION_CLASS",
            "XDG_SESSION_DESKTOP",
            "DBUS_SESSION_BUS_ADDRESS",
            "DESKTOP_SESSION",
        ])
        .collect();
    for case in [NameCase::Insensitive, NameCase::Sensitive] {
        let built = policy(Some(&allow_only(&essential)), case)
            .unwrap_or_else(|error| panic!("{case:?}: {error}"));
        let child = built.child_env(essential.iter().map(|name| (*name, "v")));
        for name in &essential {
            assert!(!built.is_denied(name), "{name} {case:?}");
            assert_eq!(
                child.get(*name).map(String::as_str),
                Some("v"),
                "{name} {case:?}"
            );
        }
        for (name, _) in SHIPPED_SET {
            assert!(!built.is_denied(name), "{name} {case:?}");
        }
    }
}

#[test]
fn on_windows_only_ascii_case_is_ignored_in_allowed_names() {
    // Windows folds more than ASCII when it compares variable names; the allow list does not.
    // When copying, a name that differs from a parent name beyond ASCII case is a different
    // name, which can only let less through.
    let with_allow = |names: &[&str]| policy(Some(&allow_only(names)), NameCase::Insensitive);

    // Repeated all the same: Windows takes `ÄX` and `äx` as one variable.
    assert_eq!(
        with_allow(&["PATH", "\u{c4}X", "\u{e4}x"]),
        Err(EnvPolicyError::DuplicateName {
            list: EnvList::Allow,
            index: 2
        })
    );
    assert_eq!(
        with_allow(&["\u{c4}X", "\u{c4}x"]),
        Err(EnvPolicyError::DuplicateName {
            list: EnvList::Allow,
            index: 1
        })
    );

    let child = with_allow(&["\u{c4}X"])
        .unwrap()
        .child_env([("\u{e4}x", "v")]);
    assert!(child.keys().all(|name| name != "\u{c4}X"), "{child:?}");
    let child = with_allow(&["\u{c4}X"])
        .unwrap()
        .child_env([("\u{c4}x", "v")]);
    assert_eq!(child["\u{c4}X"], "v");
}

#[test]
fn deny_patterns_match_every_folded_spelling() {
    // Long s, dotless i, Kelvin sign, sharp s and fullwidth letters spell a denied name once
    // folded: denied in `allow` and in `set`, in both case rules.
    let spellings = [
        "MY_\u{17f}ECRET",
        "OPENA\u{131}_ORG",
        "\u{212a}UBECONFIG",
        "DB_PA\u{df}WORD",
        "\u{ff27}\u{ff28}_HOST",
        "GH\u{ff3f}HOST",
        "my_\u{ff53}ecret",
    ];
    for case in [NameCase::Insensitive, NameCase::Sensitive] {
        for name in spellings {
            assert_eq!(
                policy(Some(&allow_only(&["PATH", name])), case),
                Err(EnvPolicyError::DeniedName {
                    list: EnvList::Allow,
                    index: 1
                }),
                "{name} {case:?}"
            );
            assert_eq!(
                policy(Some(&set_only(&[(name, "x")])), case),
                Err(EnvPolicyError::DeniedName {
                    list: EnvList::Set,
                    index: 0
                }),
                "{name} {case:?}"
            );
        }
    }
    // Non-ASCII names that fold to no pattern stay allowed.
    for name in ["\u{c4}X", "CAF\u{c9}_DIR", "\u{ff2c}ANG"] {
        assert!(
            policy(Some(&allow_only(&[name])), NameCase::Insensitive).is_ok(),
            "{name}"
        );
    }
}

/// The `allow = [...]` example in the comment over `allow` in the defaults.
fn documented_linux_allow() -> Vec<String> {
    let line = HARNESSES_TOML
        .lines()
        .find_map(|line| line.strip_prefix("#   allow = "))
        .expect("the defaults document a Linux allow list");
    let table: toml::Table = toml::from_str(&format!("allow = {line}")).unwrap();
    table["allow"].clone().try_into().unwrap()
}

#[test]
fn the_documented_linux_allow_list_passes_home_and_replaces_the_shipped_one() {
    let names = documented_linux_allow();
    for name in ["HOME", "PATH", "TMPDIR"] {
        assert!(names.iter().any(|have| have == name), "{name}");
    }
    let over = EnvLayer {
        allow: Some(names.clone()),
        ..EnvLayer::default()
    };
    let parent = [
        ("HOME", "/home/someone"),
        ("PATH", "/usr/bin"),
        ("TMPDIR", "/tmp/someone"),
        ("LANG", "C.UTF-8"),
        ("USER", "someone"),
        ("TEMP", "/tmp"),
        ("GH_TOKEN", "s3cr3t"),
    ];
    let child = policy(Some(&over), NameCase::Sensitive)
        .unwrap()
        .child_env(parent);
    for name in &names {
        assert!(child.contains_key(name), "{name}");
    }
    assert_eq!(child["HOME"], "/home/someone");
    // A shipped name the example does not repeat stops passing.
    assert!(!child.contains_key("TEMP"));
    assert!(!child.values().any(|v| v == "s3cr3t"));
}

#[test]
fn session_credentials_are_denied_in_allow_and_set() {
    for name in ["APP_SESSION_ID", "APP_SESSION_SECRET"] {
        for (over, list) in [
            (allow_only(&[name]), EnvList::Allow),
            (set_only(&[(name, "fixture")]), EnvList::Set),
        ] {
            assert_eq!(
                policy(Some(&over), NameCase::host()),
                Err(EnvPolicyError::DeniedName { list, index: 0 }),
                "{name}"
            );
        }
    }
}

#[test]
fn npm_cache_is_allowed_and_userconfig_is_denied() {
    let over = allow_only(&["NPM_CONFIG_CACHE"]);
    let built = policy(Some(&over), NameCase::host()).unwrap();
    assert_eq!(
        built.child_env([("NPM_CONFIG_CACHE", "fixture")])["NPM_CONFIG_CACHE"],
        "fixture"
    );
    for (over, list) in [
        (allow_only(&["NPM_CONFIG_USERCONFIG"]), EnvList::Allow),
        (
            set_only(&[("NPM_CONFIG_USERCONFIG", "fixture")]),
            EnvList::Set,
        ),
    ] {
        assert_eq!(
            policy(Some(&over), NameCase::host()),
            Err(EnvPolicyError::DeniedName { list, index: 0 })
        );
    }
}

#[test]
fn shipped_exceptions_are_nominal_and_user_denies_still_win() {
    for name in ["WT_SESSION", "GIT_AUTHOR_NAME"] {
        let over = set_only(&[(name, "fixture")]);
        let built = policy(Some(&over), NameCase::host()).unwrap();
        assert_eq!(
            built.child_env(Vec::<(String, String)>::new())[name],
            "fixture"
        );
    }
    let over = EnvLayer {
        allow: Some(vec!["WT_SESSION".into()]),
        deny: Some(vec!["*_SESSION*".into()]),
        ..EnvLayer::default()
    };
    assert_eq!(
        policy(Some(&over), NameCase::host()),
        Err(EnvPolicyError::DeniedName {
            list: EnvList::Allow,
            index: 0
        })
    );
    assert!(
        policy(None, NameCase::host())
            .unwrap()
            .is_denied("WT_SESSION_EXTRA")
    );
}

#[test]
fn user_configuration_cannot_add_deny_exceptions() {
    let over = EnvLayer {
        deny_exceptions: Some(BTreeMap::from([(
            "*_SESSION*".into(),
            vec!["APP_SESSION_ID".into()],
        )])),
        ..EnvLayer::default()
    };
    assert_eq!(
        policy(Some(&over), NameCase::host()),
        Err(EnvPolicyError::BadDenyExceptions)
    );
}
