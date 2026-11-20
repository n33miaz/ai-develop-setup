//! Terminal launch contract, run by the core with its test tool and by each adapter.

use super::{
    ConfigError, DENIED_EXTRA, DiskProbe, EnvList, EnvPolicyError, HARNESS_ID, LaunchError,
    NameCase, OTHER_HARNESS, POISON_EXTRA, PROGRAM, PathError, Probe, ToolConfig, WRONG_HARNESSES,
    open_params,
};
use std::collections::BTreeSet;

use ads_contracts::CommandSessionOpenV1Params;

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
    // The test binary and its folder exist on every system and need nothing written.
    let program = std::env::current_exe().unwrap();
    let folder = program.parent().unwrap();
    let text = format!(
        "[workdirs]
scratch = {}
[tools.{HARNESS_ID}]
program = {}
",
        toml_string(&folder.display().to_string()),
        toml_string(&program.display().to_string())
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

    let missing = folder.join("missing-tool.exe");
    assert!(!missing.exists());
    let gone = format!(
        "[workdirs]
scratch = {}
[tools.{HARNESS_ID}]
program = {}
",
        toml_string(&folder.display().to_string()),
        toml_string(&missing.display().to_string())
    );
    let config = ToolConfig::from_toml_str(&gone, NameCase::host()).unwrap();
    let result = open_params(
        &config,
        &request(HARNESS_ID, "scratch"),
        poisoned_parent(),
        &DiskProbe,
    );
    assert_eq!(result, Err(LaunchError::ProgramMissing));
}

/// `text` as a TOML basic string, so that control characters reach the path check.
fn toml_string(text: &str) -> String {
    let mut quoted = String::from('"');
    for c in text.chars() {
        match c {
            '\\' => quoted.push_str("\\\\"),
            '"' => quoted.push_str("\\\""),
            c if c.is_control() => quoted.push_str(&format!("\\u{:04X}", u32::from(c))),
            c => quoted.push(c),
        }
    }
    quoted.push('"');
    quoted
}

/// Path forms that name something other than a literal absolute file or folder, with the
/// reason they are refused.
const DANGEROUS_PATHS: &[(&str, PathError)] = &[
    // Special prefixes and network paths.
    (r"\\?\C:\Tools\tool.exe", PathError::DevicePrefix),
    (r"\\.\C:\Tools\tool.exe", PathError::DevicePrefix),
    (r"\\.\pipe\tool", PathError::DevicePrefix),
    (r"\\?\UNC\server\share\tool.exe", PathError::DevicePrefix),
    ("//?/C:/Tools/tool.exe", PathError::DevicePrefix),
    (r"\\server\share\tool.exe", PathError::Network),
    ("//server/share/tool.exe", PathError::Network),
    (r"\??\C:\Tools\tool.exe", PathError::Relative),
    // Alternate data streams; `$` is refused first.
    (r"C:\Tools\tool.exe:hidden", PathError::AlternateStream),
    (r"C:\Tools:hidden\tool.exe", PathError::AlternateStream),
    (r"C:\Tools\tool.exe::$DATA", PathError::UnexpandedVariable),
    // Short names.
    (r"C:\PROGRA~1\Tools\tool.exe", PathError::ShortName),
    (r"C:\Tools\TOOL~1.EXE", PathError::ShortName),
    (r"C:\Tools\TOOL~12", PathError::ShortName),
    // Device names, with or without an extension.
    (r"C:\Tools\NUL", PathError::DeviceName),
    (r"C:\Tools\con.exe", PathError::DeviceName),
    (r"C:\Tools\COM1\tool.exe", PathError::DeviceName),
    ("C:\\Tools\\LPT\u{b9}.exe", PathError::DeviceName),
    ("C:\\Tools\\aux\u{a0}.exe", PathError::DeviceName),
    (r"C:\Tools\CONIN$", PathError::UnexpandedVariable),
    // Control characters.
    ("C:\\Tools\\tool\0.exe", PathError::ControlCharacter),
    ("C:\\Tools\\tool\n.exe", PathError::ControlCharacter),
    ("C:\\Tools\\tool\u{1b}[0m.exe", PathError::ControlCharacter),
    ("C:\\Tools\\tool\u{7f}.exe", PathError::ControlCharacter),
    ("C:\\Tools\\tool\u{85}.exe", PathError::ControlCharacter),
    ("/usr/bin/tool\t", PathError::ControlCharacter),
    // Spaces that Windows drops or that hide the root.
    ("   ", PathError::Empty),
    (r" C:\Tools\tool.exe", PathError::Relative),
    (r"C:\Tools \tool.exe", PathError::TrailingDotOrSpace),
    (r"C:\Tools\tool.exe ", PathError::TrailingDotOrSpace),
    (r"C:\Tools\tool.exe.", PathError::TrailingDotOrSpace),
    // Reserved characters and wildcards.
    (r"C:\Tools\a|b.exe", PathError::ReservedCharacter),
    (r"C:\Tools\a<b.exe", PathError::ReservedCharacter),
    ("C:\\Tools\\a\"b.exe", PathError::ReservedCharacter),
    (r"C:\Tools\*.exe", PathError::WildcardNotAllowed),
    (r"C:\Tools\tool?.exe", PathError::WildcardNotAllowed),
    // POSIX paths.
    ("/usr/local/bin/../tool", PathError::ParentComponent),
    (r"/usr/local/bin/a\b", PathError::ReservedCharacter),
    ("~/bin/tool", PathError::UnexpandedTilde),
    ("/home/~someone/tool", PathError::UnexpandedTilde),
    ("/home/$USER/tool", PathError::UnexpandedVariable),
    ("usr/local/bin/tool", PathError::Relative),
    // Relative forms, also as a folder.
    (r"C:\work\..\tool", PathError::ParentComponent),
    ("C:/work/tool/..", PathError::ParentComponent),
    ("C:work", PathError::DriveRelative),
    (r"\work", PathError::Relative),
    (r".\work", PathError::Relative),
    ("tool.exe", PathError::Relative),
];

/// Path forms that stay accepted, with the text the configuration keeps.
const SAFE_PATHS: &[(&str, &str)] = &[
    (
        r"C:\Program Files\My Tools\tool.exe",
        r"C:\Program Files\My Tools\tool.exe",
    ),
    ("c:/Program Files/tool.exe", r"C:\Program Files\tool.exe"),
    (r"C:\Tools\ leading space", r"C:\Tools\ leading space"),
    (r"C:\Tools\a~b\tool.exe", r"C:\Tools\a~b\tool.exe"),
    (r"C:\Tools\COM10\console.exe", r"C:\Tools\COM10\console.exe"),
    ("/usr/local/bin/tool", "/usr/local/bin/tool"),
    ("/opt/my tools//bin/./tool", "/opt/my tools/bin/tool"),
    ("/srv/NUL/CON~1/tool", "/srv/NUL/CON~1/tool"),
];

#[test]
fn the_path_corpus_is_refused_as_program_and_as_folder() {
    for (path, error) in DANGEROUS_PATHS {
        let program = format!("[tools.{HARNESS_ID}]\nprogram = {}\n", toml_string(path));
        assert_eq!(
            ToolConfig::from_toml_str(&program, NameCase::Insensitive),
            Err(ConfigError::Program(*error)),
            "program {path:?}"
        );
        let folder = format!("[workdirs]\nscratch = {}\n", toml_string(path));
        assert_eq!(
            ToolConfig::from_toml_str(&folder, NameCase::Insensitive),
            Err(ConfigError::Workdir {
                index: 0,
                error: *error
            }),
            "folder {path:?}"
        );
    }
    // An empty program turns the tool off (tested above); an empty folder is refused.
    assert_eq!(
        ToolConfig::from_toml_str("[workdirs]\nscratch = ''\n", NameCase::Insensitive),
        Err(ConfigError::Workdir {
            index: 0,
            error: PathError::Empty
        })
    );
}

#[test]
fn a_drive_root_is_refused_as_folder() {
    for root in [r"C:\", "c:/", r"D:\.\", "C://", "/", "//", "/./"] {
        let text = format!(
            "[workdirs]\nother = 'C:\\work'\nscratch = {}\n",
            toml_string(root)
        );
        let result = ToolConfig::from_toml_str(&text, NameCase::Insensitive);
        // `//` is a network prefix before it is a root.
        if root == "//" {
            assert_eq!(
                result,
                Err(ConfigError::Workdir {
                    index: 1,
                    error: PathError::Network
                })
            );
        } else {
            assert_eq!(result, Err(ConfigError::WorkdirIsRoot(1)), "{root:?}");
        }
    }
    // One level down is a folder like any other.
    let text = "[workdirs]\nscratch = 'C:\\work'\nposix = '/work'\n";
    assert!(ToolConfig::from_toml_str(text, NameCase::Insensitive).is_ok());
}

#[test]
fn safe_path_forms_are_kept_as_program_and_as_folder() {
    let alias = ads_contracts::CommonIdV1::new("scratch").unwrap();
    for (path, kept) in SAFE_PATHS {
        let text = format!(
            "[workdirs]\nscratch = {quoted}\n[tools.{HARNESS_ID}]\nprogram = {quoted}\n",
            quoted = toml_string(path)
        );
        let config = ToolConfig::from_toml_str(&text, NameCase::Insensitive)
            .unwrap_or_else(|error| panic!("{path:?}: {error}"));
        assert_eq!(config.program().map(|p| p.as_str()), Some(*kept));
        assert_eq!(config.workdir(&alias).map(|c| c.as_str()), Some(*kept));
    }
}
