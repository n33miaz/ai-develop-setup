#![cfg(windows)]

use ads_daemon::events::new_id;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    WebSocketStream,
    tungstenite::{Message, client::IntoClientRequest},
};

type Socket = WebSocketStream<TcpStream>;

struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn read_ready(process: &mut Process) -> String {
    let stdout = process.0.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let result = BufReader::new(stdout).read_line(&mut line);
        let _ = tx.send(result.map(|_| line));
    });
    rx.recv_timeout(Duration::from_secs(30))
        .expect("process did not start")
        .unwrap()
}

fn test_config(name: &str, contents: &str) -> std::path::PathBuf {
    let path =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}-{}.toml", std::process::id()));
    std::fs::write(&path, contents).unwrap();
    path
}

async fn send(ws: &mut Socket, method: &str, mut params: Value) {
    let id = new_id().unwrap();
    params["_meta"] = json!({"idempotency_key": new_id().unwrap(), "origin": "local"});
    let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
    ws.send(Message::Text(request.to_string().into()))
        .await
        .unwrap();
    let reply = receive(ws).await;
    assert_eq!(reply["id"], id);
    assert_eq!(reply["result"]["accepted"], true, "{reply}");
}

async fn receive(ws: &mut Socket) -> Value {
    let message = ws.next().await.unwrap().unwrap().into_text().unwrap();
    serde_json::from_str(&message).unwrap()
}

#[tokio::test]
async fn real_host_runs_configured_echo_through_the_api() {
    let binary = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("ads-ptyhost.exe");
    assert!(
        binary.is_file(),
        "build the host first: cargo build -p ads-ptyhost -j 2"
    );
    let endpoint = format!(r"\\.\pipe\ads-daemon-echo-{}", std::process::id());
    let host_config = test_config(
        "daemon-echo-host",
        &format!("channel_path = '{endpoint}'\n"),
    );
    let mut host = Process(
        Command::new(binary)
            .arg("--config")
            .arg(&host_config)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    assert!(read_ready(&mut host).contains("listening on"));

    let cmd = Path::new(&std::env::var_os("SystemRoot").unwrap()).join("System32/cmd.exe");
    let cwd = std::env::current_dir().unwrap();
    let config = json!({
        "workdirs": {"workspace": cwd},
        "tools": {"claude": {"program": cmd, "args": ["/d", "/c", "echo daemon-pty-client-ok [%ANTHROPIC_API_KEY%]"]}}
    });
    let user_config = test_config("daemon-echo-user", &toml::to_string(&config).unwrap());
    let mut daemon = Process(
        Command::new(env!("CARGO_BIN_EXE_ads-daemon"))
            .arg("--config")
            .arg(user_config)
            .arg("--host-config")
            .arg(host_config)
            // Reaches the daemon but must not reach the tool.
            .env("ANTHROPIC_API_KEY", "leaked-sentinel")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let line = read_ready(&mut daemon);
    let (_, fragment) = line
        .trim_end()
        .split_once("#core=")
        .expect("missing launch URL");
    let (url, token) = fragment.split_once("&token=").unwrap();
    let mut request = url.into_client_request().unwrap();
    request
        .headers_mut()
        .insert("Origin", "http://localhost:5173".parse().unwrap());
    tokio::time::timeout(Duration::from_secs(30), async {
        let stream = TcpStream::connect(url.strip_prefix("ws://").unwrap())
            .await
            .unwrap();
        let (mut ws, _) = tokio_tungstenite::client_async(request, stream)
            .await
            .unwrap();
        send(&mut ws, "auth.hello", json!({"token": token})).await;
        send(
            &mut ws,
            "events.subscribe",
            json!({"stream": "session/echo", "after_seq": 0}),
        )
        .await;
        send(
            &mut ws,
            "session.open",
            json!({
                "session_id": "echo", "harness": "claude", "workdir_alias": "workspace",
                "cols": 80, "rows": 24
            }),
        )
        .await;

        let mut output = Vec::new();
        loop {
            let event = receive(&mut ws).await;
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
        let output = String::from_utf8_lossy(&output);
        assert!(output.contains("daemon-pty-client-ok"));
        assert!(output.contains("[%ANTHROPIC_API_KEY%]"));
        assert!(!output.contains("leaked-sentinel"));
    })
    .await
    .expect("echo did not exit");
}
