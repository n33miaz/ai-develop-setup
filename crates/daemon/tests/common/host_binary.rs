use ads_daemon::events::new_id;
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

pub type Socket = WebSocketStream<TcpStream>;
pub const TIMEOUT: Duration = Duration::from_secs(30);

pub struct Process(Child);

pub fn isolate_host_environment(command: &mut Command) -> &mut Command {
    command
        .env_clear()
        .env("SystemRoot", std::env::var_os("SystemRoot").unwrap())
}

impl Process {
    pub fn start(command: &mut Command) -> Self {
        Self(
            command
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .spawn()
                .unwrap(),
        )
    }

    pub fn host(config: &Path) -> Self {
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
        let mut command = Command::new(binary);
        let mut host = Self::start(
            isolate_host_environment(&mut command)
                .arg("--config")
                .arg(config),
        );
        assert!(host.read_ready().contains("listening on"));
        host
    }

    pub fn read_ready(&mut self) -> String {
        let stdout = self.0.stdout.take().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut line = String::new();
            let result = BufReader::new(stdout).read_line(&mut line);
            let _ = tx.send(result.map(|_| line));
        });
        let line = rx
            .recv_timeout(TIMEOUT)
            .expect("process did not start")
            .unwrap();
        assert!(
            !line.is_empty(),
            "process exited before reporting readiness"
        );
        line
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

pub async fn authenticate(launch_url: &str) -> Socket {
    let (_, fragment) = launch_url
        .trim_end()
        .split_once("#core=")
        .expect("missing launch URL");
    let (url, token) = fragment.split_once("&token=").unwrap();
    let mut request = url.into_client_request().unwrap();
    request
        .headers_mut()
        .insert("Origin", "http://127.0.0.1:5173".parse().unwrap());
    let stream = TcpStream::connect(url.strip_prefix("ws://").unwrap())
        .await
        .unwrap();
    let (mut ws, _) = tokio_tungstenite::client_async(request, stream)
        .await
        .unwrap();
    let hello = exchange(&mut ws, "auth.hello", json!({"token": token})).await;
    assert_eq!(
        hello["result"]["accepted"], true,
        "auth.hello rejected: {hello}"
    );
    ws
}

pub async fn exchange(ws: &mut Socket, method: &str, mut params: Value) -> Value {
    let id = new_id().unwrap();
    params["_meta"] = json!({"idempotency_key": new_id().unwrap(), "origin": "local"});
    let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
    ws.send(Message::Text(request.to_string().into()))
        .await
        .unwrap();
    let reply = receive(ws).await;
    assert_eq!(reply["id"], id, "{method}: {reply}");
    reply
}

pub async fn receive(ws: &mut Socket) -> Value {
    let message = ws.next().await.unwrap().unwrap().into_text().unwrap();
    serde_json::from_str(&message).unwrap()
}
