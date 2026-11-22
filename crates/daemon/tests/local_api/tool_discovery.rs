use super::{
    Socket, TestLauncher, authenticated, bounded, call, closed, command, connect, fixture,
    open_params, receive, remote, transmit,
};
use ads_core::tool_launch::HostRules;
use ads_daemon::api::{DisabledLauncher, Launcher};
use ads_daemon::tool_launcher::ToolLauncher;
use futures_util::StreamExt;
use serde_json::{Value, json};
use std::sync::Arc;
use tokio_tungstenite::tungstenite::Message;

struct ListProbe;

impl ads_core::tool_launch::Probe for ListProbe {
    fn is_file(&self, _: &str) -> bool {
        panic!("listing must not probe the disk")
    }

    fn is_dir(&self, _: &str) -> bool {
        panic!("listing must not probe the disk")
    }
}

async fn raw_call(ws: &mut Socket, value: &Value) -> String {
    transmit(ws, value).await;
    let Message::Text(text) = bounded(ws.next()).await.unwrap().unwrap() else {
        panic!("expected text")
    };
    let text = text.to_string();
    let reply: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(reply["id"], value["id"]);
    for schema in [
        "urn:ads:contract:local-api.server-message:2",
        "urn:ads:command:harness.list.response:1",
    ] {
        assert!(
            ads_contracts::reencode(schema, reply.clone())
                .unwrap()
                .is_ok()
        );
    }
    text
}

#[tokio::test]
async fn harness_list_returns_only_tool_names_and_folder_aliases() {
    let overrides = r#"
[env]
allow = ["PATH", "LIST_SENTINEL_VAR"]
[workdirs]
default = 'C:/dir-sentinel/inner'
second = 'D:/other-sentinel'
[tools.claude]
program = 'C:/prog-sentinel/claude.exe'
args = ['--arg-sentinel']
"#;
    let parent_env = [
        ("PATH", "C:/path-sentinel"),
        ("LIST_SENTINEL_VAR", "value-sentinel"),
        ("ANTHROPIC_API_KEY", "key-sentinel"),
    ]
    .map(|(name, value)| (name.to_owned(), value.to_owned()))
    .to_vec();
    let launcher = ToolLauncher::from_toml_str(
        overrides,
        HostRules::WINDOWS,
        parent_env,
        ListProbe,
        ListProbe,
    );
    let launcher = launcher.unwrap();
    let f = fixture("", Arc::new(launcher)).await;
    let mut ws = authenticated(&f).await;
    let request = command("harness.list", json!({}));
    let text = raw_call(&mut ws, &request).await;
    let reply: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        reply["result"],
        json!({"harnesses": ["claude"], "workdir_aliases": ["default", "second"]})
    );
    for leak in [
        "sentinel", ":/", ":\\", "\\", "PATH", "LIST_", "program", "args", "env", "cwd",
    ] {
        assert!(!text.contains(leak), "{leak} leaked in {text}");
    }
    // Reading twice with the same key answers the list again, not a replay marker.
    let text = raw_call(&mut ws, &request).await;
    assert!(text.contains("\"workdir_aliases\""), "{text}");
    assert!(f.backend.terminals().is_empty());
}

#[tokio::test]
async fn harness_list_is_empty_without_configured_tools() {
    for launcher in [
        Arc::new(DisabledLauncher) as Arc<dyn Launcher>,
        Arc::new(
            ToolLauncher::from_toml_str(
                "",
                HostRules::WINDOWS,
                Vec::<(String, String)>::new(),
                ListProbe,
                ListProbe,
            )
            .unwrap(),
        ),
        Arc::new(TestLauncher),
    ] {
        let f = fixture("", launcher).await;
        let mut ws = authenticated(&f).await;
        let reply = call(&mut ws, "harness.list", json!({})).await;
        assert_eq!(
            reply["result"],
            json!({"harnesses": [], "workdir_aliases": []})
        );
    }
}

#[tokio::test]
async fn harness_list_refuses_parameters() {
    let f = fixture("", Arc::new(TestLauncher)).await;
    let mut ws = authenticated(&f).await;
    let reply = call(&mut ws, "harness.list", json!({"program": "x"})).await;
    assert_eq!(reply["error"]["code"], -32602);
    assert_eq!(
        call(&mut ws, "harness.list", json!({})).await["result"]["harnesses"],
        json!([])
    );
}

#[tokio::test]
async fn harness_list_needs_the_allowed_page_authentication_and_local_origin() {
    let overrides = "[workdirs]\ndefault = 'C:/dir-sentinel'\n[tools.codex]\nprogram = 'C:/prog-sentinel/codex.exe'\n";
    let launcher = ToolLauncher::from_toml_str(
        overrides,
        HostRules::WINDOWS,
        Vec::<(String, String)>::new(),
        ListProbe,
        ListProbe,
    )
    .unwrap();
    let f = fixture("", Arc::new(launcher)).await;
    let expected = json!({"harnesses": ["codex"], "workdir_aliases": ["default"]});

    match connect(&f, Some("https://untrusted.invalid"), None).await {
        Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
            assert_eq!(response.status(), 403)
        }
        _ => panic!("expected 403"),
    }

    let mut early = connect(&f, Some("http://127.0.0.1:5173"), None)
        .await
        .unwrap();
    transmit(&mut early, &command("harness.list", json!({}))).await;
    closed(&mut early).await;

    let mut ws = authenticated(&f).await;
    transmit(&mut ws, &remote(command("harness.list", json!({})))).await;
    let refused = receive(&mut ws).await;
    assert_eq!(refused["error"]["code"], -32010);
    assert!(refused.get("result").is_none());
    assert_eq!(
        call(&mut ws, "harness.list", json!({})).await["result"],
        expected
    );
}

#[tokio::test]
async fn a_client_that_never_lists_tools_only_receives_first_version_frames() {
    let f = fixture("", Arc::new(TestLauncher)).await;
    // `authenticated` and `call` check every reply against version 1 as well.
    let mut ws = authenticated(&f).await;
    let mut events = authenticated(&f).await;
    assert_eq!(
        call(
            &mut events,
            "events.subscribe",
            json!({"stream": "session/legacy", "after_seq": 0})
        )
        .await["result"]["accepted"],
        true
    );
    assert_eq!(
        call(&mut ws, "session.open", open_params("legacy")).await["result"]["accepted"],
        true
    );
    assert_eq!(
        call(&mut ws, "unknown", json!({})).await["error"]["code"],
        -32601
    );
    assert_eq!(
        call(&mut ws, "session.close", json!({"session_id": "missing"})).await["error"]["code"],
        -32602
    );
    let created = receive(&mut events).await;
    assert_eq!(created["type"], "urn:ads:event:session.created:1");
    assert_eq!(
        call(&mut ws, "session.close", json!({"session_id": "legacy"})).await["result"]["accepted"],
        true
    );
}
