use super::{Fixture, authenticated, bounded, call, open_params, receive};
use ads_contracts::CommandSessionOpenV1Params;
use ads_daemon::api::{ApiConfig, ApiError, Launcher, LocalApi};
use ads_daemon::pty_client::{ClientConfig, PtyClient};
use ads_ptyhost::{FakeBackend, HostConfig, PtyHost, server, wire::OpenParams};
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;

/// A real shell that starts a second interpreter in a console of its own, then exits after 3 s.
/// The second one waits 60 s: closing the shell's console does not end it, only its Job Object does.
struct LeftBehindLauncher;
impl Launcher for LeftBehindLauncher {
    fn prepare(&self, request: &CommandSessionOpenV1Params) -> Result<OpenParams, ApiError> {
        // Only what a system command needs to run; nothing else reaches the shell.
        let env: serde_json::Map<String, Value> = std::env::vars()
            .filter(|(name, _)| {
                ["SystemRoot", "windir", "ComSpec", "PATH", "PATHEXT"]
                    .iter()
                    .any(|allowed| allowed.eq_ignore_ascii_case(name))
            })
            .map(|(name, value)| (name, Value::String(value)))
            .collect();
        let system = r"%SystemRoot%\System32";
        let line = format!(
            r"start /min {system}\cmd.exe /d /c {system}\timeout.exe /t 60 /nobreak & {system}\timeout.exe /t 3 /nobreak >nul"
        );
        serde_json::from_value(json!({
            "session_id": request.session_id, "cols": request.cols, "rows": request.rows,
            "program": "cmd.exe", "args": ["/d", "/c", line], "cwd": null, "env": env
        }))
        .map_err(|_| ApiError::Invalid)
    }
}

#[tokio::test]
async fn ended_session_leaves_no_child_behind_on_windows() {
    use ads_ptyhost::{
        PortablePtyBackend,
        win::{ProcessProbe, process_table},
    };
    let host =
        Arc::new(PtyHost::new(Arc::new(PortablePtyBackend), HostConfig::defaults()).unwrap());
    let (near, far) = tokio::io::duplex(262144);
    let peer = tokio::spawn(server::connection(host.clone(), far));
    let client = bounded(PtyClient::over(
        near,
        ClientConfig::recommended(host.config()),
    ))
    .await
    .unwrap();
    let api = LocalApi::bind(
        ApiConfig::from_toml_str("max_sessions = 1").unwrap(),
        client,
        Arc::new(LeftBehindLauncher),
    )
    .await
    .unwrap();
    let token = api
        .launch_url()
        .unwrap()
        .split_once("&token=")
        .unwrap()
        .1
        .to_owned();
    let f = Fixture {
        port: api.port().unwrap(),
        token,
        backend: Arc::new(FakeBackend::new()),
        host: host.clone(),
        hooks: api.test_hooks(),
        release: Arc::default(),
        api: tokio::spawn(api.run()),
        peer,
    };
    let mut ws = authenticated(&f).await;
    call(
        &mut ws,
        "events.subscribe",
        json!({"stream": "session/tree", "after_seq": 0}),
    )
    .await;
    assert_eq!(
        call(&mut ws, "session.open", open_params("tree")).await["result"]["accepted"],
        true
    );
    let id = ads_ptyhost::wire::SessionId::new("tree").unwrap();
    // The shell is the session process whose parent is this test, which runs the host.
    let me = std::process::id();
    let children_of = |parent: u32| -> Vec<u32> {
        process_table()
            .unwrap()
            .into_iter()
            .filter(|entry| entry.parent_process_id == parent)
            .map(|entry| entry.process_id)
            .collect()
    };
    let mut shell = None;
    let mut left = Vec::new();
    bounded(async {
        while left.len() < 2 {
            let tree = host.tree_process_ids(&id).unwrap_or_default();
            shell = shell.or_else(|| {
                tree.iter()
                    .copied()
                    .find(|pid| children_of(me).contains(pid))
            });
            // Found by parent, not by the Job Object's list: the interpreter the shell started in a
            // console of its own (the shell's child that has children) and its 60 s wait.
            left = shell
                .into_iter()
                .flat_map(&children_of)
                .filter_map(|pid| {
                    let below = children_of(pid);
                    (!below.is_empty()).then(|| [vec![pid], below].concat())
                })
                .next()
                .unwrap_or_default();
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    let tree = host.tree_process_ids(&id).unwrap();
    let probes: Vec<ProcessProbe> = left
        .iter()
        .map(|pid| ProcessProbe::open(*pid).expect("open a left-behind process"))
        .collect();
    for (pid, probe) in left.iter().zip(&probes) {
        assert!(probe.is_alive().expect("query a left-behind process"));
        assert!(tree.contains(pid), "{pid} outside the job: {tree:?}");
    }
    loop {
        let event = receive(&mut ws).await;
        if event["type"] == "urn:ads:event:session.ended:1" {
            assert_eq!(event["data"]["reason"], "exit");
            break;
        }
    }
    // The daemon gives the slot back on its own, and what ran in its own console is gone too.
    let alive = || {
        probes
            .iter()
            .any(|p| p.is_alive().expect("query a left-behind process"))
    };
    bounded(async {
        while !host.sessions().is_empty() || alive() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    assert_eq!(f.hooks.automatic_closes(), 1);
}
