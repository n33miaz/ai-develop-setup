use super::{authenticated, call, fixture, open_params};
use ads_core::tool_launch::HostRules;
use ads_daemon::tool_launcher::ToolLauncher;
use serde_json::json;
use std::sync::Arc;

struct FakeProbe;

impl ads_core::tool_launch::Probe for FakeProbe {
    fn is_file(&self, path: &str) -> bool {
        matches!(path, r"C:\tools\claude.exe" | r"C:\tools\codex.exe")
    }

    fn is_dir(&self, path: &str) -> bool {
        path == r"C:\workspace"
    }
}

#[tokio::test]
async fn configured_adapters_supply_the_host_with_filtered_parameters() {
    let overrides = r#"
[accounts.claude.primary]
profile_dir = 'C:/profiles/claude'
[accounts.codex.primary]
profile_dir = 'C:/profiles/codex'
[workdirs]
workspace = 'C:/workspace'
[tools.claude]
program = 'C:/tools/claude.exe'
args = ['claude-arg']
[tools.codex]
program = 'C:/tools/codex.exe'
args = ['codex-arg']
"#;
    let parent_env = [
        ("PATH", "C:/tools"),
        ("ANTHROPIC_API_KEY", "sentinel"),
        ("OPENAI_API_KEY", "sentinel"),
        ("GH_TOKEN", "sentinel"),
        ("CLAUDE_CONFIG_DIR", "sentinel"),
        ("CODEX_HOME", "sentinel"),
        ("UNLISTED", "sentinel"),
    ]
    .map(|(name, value)| (name.to_owned(), value.to_owned()))
    .to_vec();
    let launcher = ToolLauncher::from_toml_str(
        overrides,
        HostRules::WINDOWS,
        parent_env,
        FakeProbe,
        FakeProbe,
    )
    .unwrap();
    let f = fixture("", Arc::new(launcher)).await;
    let mut ws = authenticated(&f).await;
    for (index, harness) in ["claude", "codex"].into_iter().enumerate() {
        let mut params = open_params(harness);
        params["harness"] = json!(harness);
        let reply = call(&mut ws, "session.open", params).await;
        assert_eq!(reply["result"]["accepted"], true, "{reply}");
        let terminals = f.backend.terminals();
        assert_eq!(terminals.len(), index + 1);
        let spec = terminals[index].spec();
        assert_eq!(spec.program, format!(r"C:\tools\{harness}.exe"));
        assert_eq!(spec.args, [format!("{harness}-arg")]);
        assert_eq!(
            spec.cwd.as_deref(),
            Some(std::path::Path::new(r"C:\workspace"))
        );
        assert_eq!(
            spec.env
                .into_iter()
                .collect::<std::collections::BTreeMap<_, _>>(),
            [
                ("PATH", "C:/tools"),
                ("TERM", "xterm-256color"),
                ("COLORTERM", "truecolor"),
            ]
            .map(|(name, value)| (name.to_owned(), value.to_owned()))
            .into()
        );
    }
}

#[tokio::test]
async fn unconfigured_and_unknown_tools_are_refused_before_spawning() {
    let launcher = ToolLauncher::from_toml_str(
        "",
        HostRules::WINDOWS,
        Vec::<(String, String)>::new(),
        FakeProbe,
        FakeProbe,
    )
    .unwrap();
    let f = fixture("", Arc::new(launcher)).await;
    let mut ws = authenticated(&f).await;
    for (harness, code) in [("claude", -32011), ("codex", -32011), ("unknown", -32602)] {
        let mut params = open_params(harness);
        params["harness"] = json!(harness);
        assert_eq!(
            call(&mut ws, "session.open", params).await["error"]["code"],
            code
        );
    }
    assert!(f.backend.terminals().is_empty());
}
