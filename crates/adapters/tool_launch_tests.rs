//! Terminal launch contract exercised through each adapter.

use super::{
    ConfigError, DENIED_EXTRA, DiskProbe, HARNESS_ID, LaunchError, OTHER_HARNESS, POISON_EXTRA,
    PROGRAM, Probe, ToolConfig, WRONG_HARNESSES, open_params,
};
use std::collections::BTreeSet;

use ads_contracts::CommandSessionOpenV1Params;
use ads_core::child_env::{EnvList, EnvPolicyError, NameCase};
use ads_core::gate::PathError;

const FOLDER: &str = r"C:\work\scratch";

struct FakeProbe {
    files: Vec<&'static str>,
    dirs: Vec<&'static str>,
}

impl FakeProbe {
    fn all() -> Self {
        Self {
            files: vec![PROGRAM],
            dirs: vec![FOLDER],
        }
    }
}

impl Probe for FakeProbe {
    fn is_file(&self, path: &str) -> bool {
        self.files.contains(&path)
    }
    fn is_dir(&self, path: &str) -> bool {
        self.dirs.contains(&path)
    }
}

fn user_toml(program: &str) -> String {
    format!(
        r#"
[workdirs]
scratch = '{FOLDER}'

[tools.{HARNESS_ID}]
program = '{program}'
args = ["--fixed-flag", "value"]
"#
    )
}

fn config() -> ToolConfig {
    ToolConfig::from_toml_str(&user_toml(PROGRAM), NameCase::Insensitive).unwrap()
}

fn request(harness: &str, alias: &str) -> CommandSessionOpenV1Params {
    serde_json::from_value(serde_json::json!({
        "session_id": "sess-01",
        "harness": harness,
        "workdir_alias": alias,
        "cols": 120,
        "rows": 30,
        "_meta": {"idempotency_key": "01ARZ3NDEKTSV4RRFFQ69G5FAE", "origin": "local"}
    }))
    .unwrap()
}

const POISON: &[&str] = &[
    "ANTHROPIC_API_KEY",
    "anthropic_api_key",
    "OPENAI_API_KEY",
    "Openai_Api_Key",
    "GH_TOKEN",
    "gh_token",
    "GITHUB_TOKEN",
    "FOO_TOKEN",
    "Foo_Token",
    "BAR_SECRET",
    "bar_secret",
    "AWS_SECRET_ACCESS_KEY",
    "aws_secret_access_key",
];

fn poisoned_parent() -> Vec<(String, String)> {
    let mut env = vec![
        ("Path".to_string(), r"C:\bin".to_string()),
        ("SystemRoot".into(), r"C:\Windows".into()),
        ("USERPROFILE".into(), r"C:\Users\someone".into()),
        ("UNRELATED".into(), "x".into()),
    ];
    env.extend(
        POISON
            .iter()
            .chain(POISON_EXTRA)
            .map(|n| (n.to_string(), "s3cr3t".to_string())),
    );
    env
}

#[test]
fn a_valid_request_gets_the_configured_program_args_folder_and_size() {
    let params = open_params(
        &config(),
        &request(HARNESS_ID, "scratch"),
        poisoned_parent(),
        &FakeProbe::all(),
    )
    .unwrap();
    assert_eq!(params.session_id.as_str(), "sess-01");
    assert_eq!(params.program.as_str(), PROGRAM);
    let args: Vec<&str> = params.args.iter().map(|a| a.as_str()).collect();
    assert_eq!(args, ["--fixed-flag", "value"]);
    assert_eq!(params.cwd.as_ref().map(|c| c.as_str()), Some(FOLDER));
    assert_eq!((params.cols.get(), params.rows.get()), (120, 30));
}

#[test]
fn the_child_environment_is_exactly_the_allowed_names_the_parent_has() {
    let params = open_params(
        &config(),
        &request(HARNESS_ID, "scratch"),
        poisoned_parent(),
        &FakeProbe::all(),
    )
    .unwrap();
    let names: BTreeSet<&str> = params.env.keys().map(String::as_str).collect();
    assert_eq!(
        names,
        BTreeSet::from(["COLORTERM", "PATH", "SystemRoot", "TERM", "USERPROFILE"])
    );
    assert_eq!(params.env["PATH"], r"C:\bin");
    assert!(!params.env.values().any(|v| v == "s3cr3t"));
}

#[test]
fn credentials_in_the_user_allow_list_or_set_refuse_the_configuration() {
    let names = POISON
        .iter()
        .chain(POISON_EXTRA)
        .chain(&["GITHUB_PAT", "gh_token_file"])
        .chain(DENIED_EXTRA);
    for name in names {
        let tables = [
            ("allow = ['PATH', '%']", EnvList::Allow, 1),
            ("set = { '%' = 'x' }", EnvList::Set, 0),
        ];
        for (table, list, index) in tables {
            let text = format!(
                "{}\n[env]\ndeny = []\n{}\n",
                user_toml(PROGRAM),
                table.replace('%', name)
            );
            for case in [NameCase::Insensitive, NameCase::Sensitive] {
                assert_eq!(
                    ToolConfig::from_toml_str(&text, case),
                    Err(ConfigError::Env(EnvPolicyError::DeniedName { list, index })),
                    "{name} {case:?}"
                );
            }
        }
    }
}

#[test]
fn configuration_errors_never_quote_the_rejected_text() {
    let m = "ZQXMARKER";
    let cases = [
        format!("{m} = 1\n"),
        format!("[tools.{HARNESS_ID}]\nprogram = '{m}.exe'\n"),
        format!(
            r"[tools.{HARNESS_ID}]
program = 'C:\{m}\..\x.exe'"
        ),
        format!("[tools.{HARNESS_ID}]\nprogram = 1 # {m}\n"),
        format!(
            r#"[tools.{HARNESS_ID}]
args = ["{m}\u0000"]"#
        ),
        format!(
            r#"[tools.{HARNESS_ID}]
args = ["{m}\u001b[0m"]"#
        ),
        format!("[tools.{HARNESS_ID}]\nmode = '{m}'\n"),
        format!("[tools.{HARNESS_ID}]\n{m} = 1\n"),
        format!(
            r"[workdirs]
'bad {m}' = 'C:\x'"
        ),
        format!(
            r"[workdirs]
ok = '{m}\x'"
        ),
        format!("[env]\nallow = ['{m}_TOKEN']\n"),
        format!(
            r#"[env]
set = {{ TERM = "{m}\u0000" }}"#
        ),
        format!("[env]\ndeny = ['*{m}*X*']\n"),
        format!("program = '{m}\n"),
    ];
    for text in cases {
        let error = ToolConfig::from_toml_str(&text, NameCase::Insensitive).unwrap_err();
        for shown in [error.to_string(), format!("{error:?}")] {
            assert!(!shown.contains(m), "{text:?} -> {shown}");
        }
    }
}

#[test]
fn another_tool_is_refused() {
    for &harness in WRONG_HARNESSES {
        let result = open_params(
            &config(),
            &request(harness, "scratch"),
            poisoned_parent(),
            &FakeProbe::all(),
        );
        assert_eq!(result, Err(LaunchError::UnknownHarness), "{harness}");
    }
}

#[test]
fn an_empty_program_turns_the_tool_off() {
    for config in [
        ToolConfig::defaults(NameCase::Insensitive),
        ToolConfig::from_toml_str(&user_toml(""), NameCase::Insensitive).unwrap(),
    ] {
        let result = open_params(
            &config,
            &request(HARNESS_ID, "scratch"),
            poisoned_parent(),
            &FakeProbe::all(),
        );
        assert_eq!(result, Err(LaunchError::Disabled));
    }
}

#[test]
fn a_program_path_that_is_not_absolute_and_literal_is_refused() {
    let cases = [
        (format!("{HARNESS_ID}.exe"), PathError::Relative),
        (format!(r"tools\{HARNESS_ID}.exe"), PathError::Relative),
        (format!("C:{HARNESS_ID}.exe"), PathError::DriveRelative),
        (
            format!(r"%APPDATA%\npm\{HARNESS_ID}.cmd"),
            PathError::UnexpandedVariable,
        ),
        (
            format!(r"C:\Users\%USERNAME%\{HARNESS_ID}.exe"),
            PathError::UnexpandedVariable,
        ),
        (
            format!(r"C:\tools\..\{HARNESS_ID}.exe"),
            PathError::ParentComponent,
        ),
        (format!("~/bin/{HARNESS_ID}"), PathError::UnexpandedTilde),
        (
            format!(r"\\server\share\{HARNESS_ID}.exe"),
            PathError::Network,
        ),
    ];
    for (program, error) in cases {
        let result = ToolConfig::from_toml_str(&user_toml(&program), NameCase::Insensitive);
        assert_eq!(result, Err(ConfigError::Program(error)), "{program}");
    }
}

#[test]
fn a_program_that_does_not_exist_is_refused() {
    let probe = FakeProbe {
        files: vec![],
        dirs: vec![FOLDER],
    };
    let result = open_params(
        &config(),
        &request(HARNESS_ID, "scratch"),
        poisoned_parent(),
        &probe,
    );
    assert_eq!(result, Err(LaunchError::ProgramMissing));
}

#[test]
fn an_unknown_folder_alias_is_refused() {
    for alias in ["other", "Scratch", "scratch2"] {
        let result = open_params(
            &config(),
            &request(HARNESS_ID, alias),
            poisoned_parent(),
            &FakeProbe::all(),
        );
        assert_eq!(result, Err(LaunchError::UnknownWorkdir), "{alias}");
    }
}

#[test]
fn a_configured_folder_that_does_not_exist_is_refused() {
    let probe = FakeProbe {
        files: vec![PROGRAM],
        dirs: vec![],
    };
    let result = open_params(
        &config(),
        &request(HARNESS_ID, "scratch"),
        poisoned_parent(),
        &probe,
    );
    assert_eq!(result, Err(LaunchError::WorkdirMissing));
}

#[test]
fn a_relative_folder_in_the_configuration_is_refused() {
    let text = user_toml(PROGRAM).replace(FOLDER, r"work\scratch");
    assert_eq!(
        ToolConfig::from_toml_str(&text, NameCase::Insensitive),
        Err(ConfigError::Workdir {
            index: 0,
            error: PathError::Relative
        })
    );
}

#[test]
fn the_request_cannot_carry_a_program_args_env_or_folder() {
    for (field, value) in [
        ("program", serde_json::json!("C:\\evil.exe")),
        ("args", serde_json::json!(["--dangerous"])),
        ("env", serde_json::json!({"GH_TOKEN": "x"})),
        ("cwd", serde_json::json!("C:\\")),
    ] {
        let mut json = serde_json::to_value(request("claude", "scratch")).unwrap();
        json[field] = value;
        assert!(
            serde_json::from_value::<CommandSessionOpenV1Params>(json).is_err(),
            "{field}"
        );
    }
}

#[test]
fn headless_mode_and_unknown_keys_are_refused() {
    let headless = format!("{}mode = 'headless'\n", user_toml(PROGRAM));
    assert!(matches!(
        ToolConfig::from_toml_str(&headless, NameCase::Insensitive),
        Err(ConfigError::Toml { .. })
    ));
    let unknown = format!("{}environment = []\n", user_toml(PROGRAM));
    assert!(ToolConfig::from_toml_str(&unknown, NameCase::Insensitive).is_err());
    let typo = format!("[tool.{HARNESS_ID}]\nprogram = 'C:\\\\x.exe'\n");
    assert!(ToolConfig::from_toml_str(&typo, NameCase::Insensitive).is_err());
}

#[test]
fn another_tool_table_is_left_to_its_adapter() {
    let text = format!(
        "{}\n[tools.{OTHER_HARNESS}]\nprogram = 'whatever'\nfuture_key = 1\n",
        user_toml(PROGRAM)
    );
    assert!(ToolConfig::from_toml_str(&text, NameCase::Insensitive).is_ok());
}

#[test]
fn a_real_program_and_folder_on_disk_open() {
    let root =
        std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{HARNESS_ID}-launch"));
    let folder = root.join("work");
    std::fs::create_dir_all(&folder).unwrap();
    let program = root.join("tool.exe");
    std::fs::write(&program, b"").unwrap();
    let text = format!(
        "[workdirs]\nscratch = '{}'\n[tools.{HARNESS_ID}]\nprogram = '{}'\n",
        folder.display(),
        program.display()
    );
    let config = ToolConfig::from_toml_str(&text, NameCase::host()).unwrap();
    let params = open_params(
        &config,
        &request(HARNESS_ID, "scratch"),
        poisoned_parent(),
        &DiskProbe,
    )
    .unwrap();
    assert!(std::path::Path::new(params.program.as_str()).is_file());

    let gone = text.replace("tool.exe", "missing.exe");
    let config = ToolConfig::from_toml_str(&gone, NameCase::host()).unwrap();
    let result = open_params(
        &config,
        &request(HARNESS_ID, "scratch"),
        poisoned_parent(),
        &DiskProbe,
    );
    assert_eq!(result, Err(LaunchError::ProgramMissing));
}
