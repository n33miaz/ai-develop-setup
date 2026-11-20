//! The child environment as a real process receives it: the parameters `open_params` builds are
//! handed to the operating system, and a system program reports what it got.

use std::collections::BTreeMap;
use std::process::Command;

use super::{DiskProbe, HARNESS_ID, HostRules, ToolConfig, literal, open_params};

const SECRET: &str = "leaked-sentinel";

/// A system program that prints its environment, and the arguments for it.
fn printer() -> (String, Vec<&'static str>) {
    if cfg!(windows) {
        let root = std::env::var("SystemRoot").expect("SystemRoot");
        let cmd = std::path::Path::new(&root).join("System32").join("cmd.exe");
        (literal(&cmd), vec!["/d", "/c", "set"])
    } else {
        ("/usr/bin/env".to_owned(), vec![])
    }
}

/// Runs the program of `user_toml` the way the terminal host would: the configured program,
/// arguments and folder, with only the environment of the open parameters.
fn run(user_toml: &str, parent: &[(&str, &str)]) -> String {
    let config = ToolConfig::from_toml_str(user_toml, HostRules::host())
        .unwrap_or_else(|error| panic!("{error}"));
    let request = serde_json::from_value(serde_json::json!({
        "session_id": "sess-01",
        "harness": HARNESS_ID,
        "workdir_alias": "scratch",
        "cols": 80,
        "rows": 24,
        "_meta": {"idempotency_key": "01ARZ3NDEKTSV4RRFFQ69G5FAE", "origin": "local"}
    }))
    .unwrap();
    let params = open_params(&config, &request, parent.iter().copied(), &DiskProbe).unwrap();
    let output = Command::new(params.program.as_str())
        .args(params.args.iter().map(|arg| arg.as_str()))
        .current_dir(params.cwd.as_ref().unwrap().as_str())
        .env_clear()
        .envs(&params.env)
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn user_toml(env: &str, args: &[&str]) -> String {
    let (program, default_args) = printer();
    let args = if args.is_empty() {
        default_args
    } else {
        args.to_vec()
    };
    let folder = literal(std::env::current_exe().unwrap().parent().unwrap());
    format!(
        "{env}\n[workdirs]\nscratch = {folder:?}\n[tools.{HARNESS_ID}]\nprogram = {program:?}\nargs = {args:?}\n"
    )
}

#[test]
fn a_real_child_gets_exactly_the_allowed_names_and_the_set_values() {
    let root = std::env::var("SystemRoot").unwrap_or_default();
    let parent = [
        ("SystemRoot", root.as_str()),
        ("ADS_ALLOWED_SENTINEL", "allowed-value"),
        ("ADS_SET_SENTINEL", "parent-value"),
        ("GH_TOKEN", SECRET),
        ("OPENAI_API_KEY", SECRET),
        ("Aws_Secret_Access_Key", SECRET),
        ("UNLISTED_SENTINEL", SECRET),
        // Not allowed; `cmd.exe` sets its own value for these three when they are missing.
        ("ComSpec", SECRET),
        ("PATHEXT", SECRET),
        ("PROMPT", SECRET),
    ];
    let env = r#"[env]
allow = ["SystemRoot", "ADS_ALLOWED_SENTINEL", "ADS_MISSING_SENTINEL"]
set = { ADS_SET_SENTINEL = "set-value", TERM = "xterm-256color" }"#;
    let printed = run(&user_toml(env, &[]), &parent);
    let mut child: BTreeMap<String, String> = printed
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(name, value)| (name.to_ascii_uppercase(), value.to_owned()))
        .collect();
    // `cmd.exe` may add its own value for these; the parent's would be the sentinel.
    child.retain(|name, value| {
        !(cfg!(windows) && ["COMSPEC", "PATHEXT", "PROMPT"].contains(&name.as_str()))
            || value == SECRET
    });
    let mut expected = BTreeMap::from([
        (
            "ADS_ALLOWED_SENTINEL".to_owned(),
            "allowed-value".to_owned(),
        ),
        ("ADS_SET_SENTINEL".to_owned(), "set-value".to_owned()),
        ("TERM".to_owned(), "xterm-256color".to_owned()),
    ]);
    expected.insert("SYSTEMROOT".to_owned(), root.clone());
    // Only names are shown on failure, never values.
    assert_eq!(
        child.keys().collect::<Vec<_>>(),
        expected.keys().collect::<Vec<_>>()
    );
    assert!(child == expected, "a value differs");
    assert!(!printed.contains(SECRET));
}

/// Whether the child finds `name` among its variables, compared by the system's own rule.
#[cfg(windows)]
fn child_defines(env: &str, parent: &[(&str, &str)], name: &str) -> String {
    let test = format!("if defined {name} (echo [%{name}%]) else (echo absent)");
    let toml = user_toml(env, &["/d", "/c", &test]);
    run(&toml, parent).trim().to_owned()
}

#[cfg(windows)]
#[test]
fn windows_compares_non_ascii_names_without_case_but_the_allow_list_does_not() {
    let root = std::env::var("SystemRoot").unwrap();
    // Windows folds `Ä` and `ä`: a child given `ÄX` finds it as `äx`.
    let env = r#"[env]
allow = ["SystemRoot"]
set = { "ÄX" = "set-value" }"#;
    assert_eq!(
        child_defines(env, &[("SystemRoot", &root)], "äx"),
        "[set-value]"
    );
    // The allow list folds only ASCII: `äx` does not copy the parent's `ÄX`, so it lets less
    // through than Windows would.
    let env = r#"[env]
allow = ["SystemRoot", "äx"]
set = {}"#;
    assert_eq!(
        child_defines(env, &[("SystemRoot", &root), ("ÄX", "parent-value")], "äx"),
        "absent"
    );
}

#[cfg(windows)]
#[test]
fn a_set_value_wins_over_an_allowed_name_windows_takes_as_the_same() {
    let root = std::env::var("SystemRoot").unwrap();
    let env = r#"[env]
allow = ["SystemRoot", "äx"]
set = { "ÄX" = "set-value" }"#;
    let parent = [("SystemRoot", root.as_str()), ("äx", "parent-value")];
    assert_eq!(child_defines(env, &parent, "äx"), "[set-value]");
}
