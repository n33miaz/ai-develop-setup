#![cfg(windows)]

#[path = "common/host_binary.rs"]
mod host_binary;
#[path = "common/temp.rs"]
mod temp;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use host_binary::{
    Process, Socket, TIMEOUT, authenticate, exchange, isolate_host_environment, receive,
};
use serde_json::{Value, json};
use std::path::Path;
use std::process::Command;

#[test]
fn host_command_keeps_only_the_system_root() {
    let mut command = Command::new("unused");
    command
        .env("OPENAI_API_KEY", "synthetic")
        .env("ANTHROPIC_API_KEY", "synthetic");
    isolate_host_environment(&mut command);
    let vars: Vec<_> = command.get_envs().collect();
    let root = std::env::var_os("SystemRoot").unwrap();
    assert_eq!(
        vars,
        vec![(std::ffi::OsStr::new("SystemRoot"), Some(root.as_os_str()))]
    );
}

fn test_config(name: &str, contents: &str) -> std::path::PathBuf {
    let path = temp::directory().join(format!("{name}-{}.toml", std::process::id()));
    std::fs::write(&path, contents).unwrap();
    path
}

async fn send(ws: &mut Socket, method: &str, params: Value) {
    let reply = exchange(ws, method, params).await;
    assert_eq!(reply["result"]["accepted"], true, "{reply}");
}

#[tokio::test]
async fn real_host_runs_configured_echo_through_the_api() {
    let endpoint = format!(r"\\.\pipe\ads-daemon-echo-{}", std::process::id());
    let host_config = test_config(
        "daemon-echo-host",
        &format!("channel_path = '{endpoint}'\n"),
    );
    let _host = Process::host(&host_config);

    let cmd = Path::new(&std::env::var_os("SystemRoot").unwrap()).join("System32/cmd.exe");
    let cwd = std::env::current_dir().unwrap();
    // `%NAME%` stays literal when the variable is absent from the child.
    let echo = |tool: &str, key: &str| json!({"program": cmd, "args": ["/d", "/c", format!("echo {tool}-ok [%{key}%] [%SystemRoot%]")]});
    let config = json!({
        "workdirs": {"workspace": cwd},
        "tools": {"claude": echo("claude", "ANTHROPIC_API_KEY"), "codex": echo("codex", "OPENAI_API_KEY")}
    });
    let user_config = test_config("daemon-echo-user", &toml::to_string(&config).unwrap());
    let mut daemon = Process::start(
        Command::new(env!("CARGO_BIN_EXE_ads-daemon"))
            .arg("--config")
            .arg(&user_config)
            .arg("--host-config")
            .arg(host_config)
            .env(
                "LOCALAPPDATA",
                user_config.with_extension("missing-profile"),
            )
            // Reach the daemon but must not reach the tools.
            .env("ANTHROPIC_API_KEY", "leaked-sentinel")
            .env("OPENAI_API_KEY", "leaked-sentinel"),
    );
    let line = daemon.read_ready();
    tokio::time::timeout(TIMEOUT, async {
        let mut ws = authenticate(&line).await;
        let system_root = std::env::var("SystemRoot").unwrap();
        for (tool, key) in [("claude", "ANTHROPIC_API_KEY"), ("codex", "OPENAI_API_KEY")] {
            let output = run_tool(&mut ws, tool).await;
            assert!(output.contains(&format!("{tool}-ok")), "{output}");
            assert!(output.contains(&format!("[%{key}%]")), "{output}");
            assert!(!output.contains("leaked-sentinel"), "{output}");
            // An allowed variable does arrive.
            assert!(output.contains(&format!("[{system_root}]")), "{output}");
        }
    })
    .await
    .expect("echo did not exit");
}

async fn run_tool(ws: &mut Socket, tool: &str) -> String {
    let stream = format!("session/{tool}");
    send(
        ws,
        "events.subscribe",
        json!({"stream": stream, "after_seq": 0}),
    )
    .await;
    send(
        ws,
        "session.open",
        json!({
            "session_id": tool, "harness": tool, "workdir_alias": "workspace",
            "cols": 120, "rows": 24
        }),
    )
    .await;
    let mut output = Vec::new();
    loop {
        let event = receive(ws).await;
        match event["type"].as_str() {
            Some("urn:ads:event:session.output:1") => {
                output.extend(
                    STANDARD
                        .decode(event["data"]["payload"].as_str().unwrap())
                        .unwrap(),
                );
            }
            Some("urn:ads:event:session.ended:1") => {
                assert_eq!(event["data"]["exit_code"], 0);
                break;
            }
            Some("urn:ads:event:session.created:1") => {}
            _ => panic!("unexpected event"),
        }
    }
    String::from_utf8_lossy(&output).into_owned()
}
