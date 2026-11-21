use std::time::Duration;

use ads_contracts::desktop_proof;
use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Manager, WebviewWindow, ipc::Channel};
use tokio::{
    net::TcpStream,
    sync::{Mutex, mpsc},
    time::timeout,
};
use tokio_tungstenite::{
    WebSocketStream, client_async,
    tungstenite::{Message, client::IntoClientRequest},
};
use zeroize::Zeroizing;

use super::{Bootstrap, trusted_window};

const UNAVAILABLE: &str = "desktop connection unavailable";
const DEADLINE: Duration = Duration::from_secs(5);
const HELLO_ID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAB";

pub(super) struct Bridge {
    bootstrap: Bootstrap,
    smoke: bool,
    connection: Mutex<ConnectionState>,
}

enum ConnectionState {
    Idle,
    Connected(Connection),
    RelaunchRequired,
}

struct Connection {
    id: String,
    requests: mpsc::Sender<String>,
    task: tauri::async_runtime::JoinHandle<()>,
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Bridge {
    pub(super) fn new(bootstrap: Bootstrap, smoke: bool) -> Self {
        Self {
            bootstrap,
            smoke,
            connection: Mutex::new(ConnectionState::Idle),
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(tag = "kind", content = "text", rename_all = "snake_case")]
pub(super) enum Frame {
    Message(String),
    Closed,
}

async fn authenticate(bootstrap: &Bootstrap) -> Result<WebSocketStream<TcpStream>, &'static str> {
    let address = bootstrap.url.strip_prefix("ws://").ok_or(UNAVAILABLE)?;
    let stream = TcpStream::connect(address).await.map_err(|_| UNAVAILABLE)?;
    let client = stream.local_addr().map_err(|_| UNAVAILABLE)?;
    let server = stream.peer_addr().map_err(|_| UNAVAILABLE)?;
    let request = bootstrap
        .url
        .as_str()
        .into_client_request()
        .map_err(|_| UNAVAILABLE)?;
    let nonce = request.headers()["sec-websocket-key"].clone();
    let (mut socket, response) = client_async(request, stream)
        .await
        .map_err(|_| UNAVAILABLE)?;
    let proof = response
        .headers()
        .get(desktop_proof::HEADER)
        .ok_or(UNAVAILABLE)?;
    if !desktop_proof::verify(
        &bootstrap.token,
        nonce.as_bytes(),
        client,
        server,
        proof.as_bytes(),
    ) {
        return Err(UNAVAILABLE);
    }
    let hello = Zeroizing::new(format!(
        r#"{{"jsonrpc":"2.0","id":"{HELLO_ID}","method":"auth.hello","params":{{"token":"{}","_meta":{{"origin":"local","idempotency_key":"{HELLO_ID}"}}}}}}"#,
        bootstrap.token
    ));
    socket
        .send(Message::Text(hello.as_str().into()))
        .await
        .map_err(|_| UNAVAILABLE)?;
    drop(hello);
    let Some(Ok(Message::Text(reply))) = socket.next().await else {
        return Err(UNAVAILABLE);
    };
    let reply: Value = serde_json::from_str(&reply).map_err(|_| UNAVAILABLE)?;
    if reply["jsonrpc"] != "2.0" || reply["id"] != HELLO_ID || reply["result"]["accepted"] != true {
        return Err(UNAVAILABLE);
    }
    Ok(socket)
}

async fn relay(
    mut socket: WebSocketStream<TcpStream>,
    mut requests: mpsc::Receiver<String>,
    frames: &Channel<Frame>,
    smoke: Option<AppHandle>,
) -> Result<(), ()> {
    loop {
        tokio::select! {
            request = requests.recv() => {
                let Some(request) = request else { return Ok(()) };
                timeout(DEADLINE, socket.send(Message::Text(request.into())))
                    .await.map_err(|_| ())?.map_err(|_| ())?;
            }
            message = socket.next() => match message {
                Some(Ok(Message::Text(text))) => {
                    frames.send(Frame::Message(text.to_string())).map_err(|_| ())?;
                    if let Some(app) = smoke {
                        app.exit(0);
                        return Ok(());
                    }
                }
                Some(Ok(Message::Ping(_))) => {
                    timeout(DEADLINE, socket.flush()).await.map_err(|_| ())?.map_err(|_| ())?;
                }
                Some(Ok(Message::Pong(_))) => {}
                _ => return Ok(()),
            }
        }
    }
}

#[tauri::command]
pub(super) async fn desktop_connect(
    window: WebviewWindow,
    bridge: tauri::State<'_, Bridge>,
    id: String,
    frames: Channel<Frame>,
) -> Result<(), &'static str> {
    if !trusted_window(&window) {
        return Err(UNAVAILABLE);
    }
    let mut connection = bridge.connection.lock().await;
    if matches!(*connection, ConnectionState::RelaunchRequired) {
        return Err(UNAVAILABLE);
    }
    *connection = ConnectionState::RelaunchRequired;
    let socket = timeout(DEADLINE, authenticate(&bridge.bootstrap))
        .await
        .map_err(|_| UNAVAILABLE)??;
    let smoke = bridge.smoke.then(|| window.app_handle().clone());
    let (requests, incoming) = mpsc::channel(1);
    let task = tauri::async_runtime::spawn(async move {
        let _ = relay(socket, incoming, &frames, smoke).await;
        let _ = frames.send(Frame::Closed);
    });
    *connection = ConnectionState::Connected(Connection { id, requests, task });
    Ok(())
}

#[tauri::command]
pub(super) async fn desktop_send(
    window: WebviewWindow,
    bridge: tauri::State<'_, Bridge>,
    id: String,
    text: String,
) -> Result<(), &'static str> {
    if !trusted_window(&window) {
        return Err(UNAVAILABLE);
    }
    let request: Value = serde_json::from_str(&text).map_err(|_| UNAVAILABLE)?;
    if request["method"]
        .as_str()
        .is_none_or(|method| method == "auth.hello")
    {
        return Err(UNAVAILABLE);
    }
    let sender = match &*bridge.connection.lock().await {
        ConnectionState::Connected(connection) if connection.id == id => {
            connection.requests.clone()
        }
        _ => return Err(UNAVAILABLE),
    };
    timeout(DEADLINE, sender.send(text))
        .await
        .map_err(|_| UNAVAILABLE)?
        .map_err(|_| UNAVAILABLE)
}

#[tauri::command]
pub(super) async fn desktop_disconnect(
    window: WebviewWindow,
    bridge: tauri::State<'_, Bridge>,
    id: String,
) -> Result<(), &'static str> {
    if !trusted_window(&window) {
        return Err(UNAVAILABLE);
    }
    let mut connection = bridge.connection.lock().await;
    if matches!(&*connection, ConnectionState::Connected(active) if active.id == id) {
        *connection = ConnectionState::Idle;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn replacement_listener_receives_neither_token_nor_input() {
        let original = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = original.local_addr().unwrap();
        let bootstrap = Bootstrap {
            url: format!("ws://{address}"),
            token: "a".repeat(43),
        };
        drop(original);
        let replacement = tokio::net::TcpListener::bind(address).await.unwrap();
        let impostor = tokio::spawn(async move {
            let (stream, _) = replacement.accept().await.unwrap();
            let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
            let accepted =
                format!(r#"{{"jsonrpc":"2.0","id":"{HELLO_ID}","result":{{"accepted":true}}}}"#);
            let _ = socket.send(Message::Text(accepted.into())).await;
            let mut received = 0;
            while let Some(Ok(message)) = socket.next().await {
                if message.is_text() {
                    received += 1;
                }
            }
            received
        });

        let result = timeout(DEADLINE, authenticate(&bootstrap)).await.unwrap();
        let accepted = result.is_ok();
        if let Ok(mut socket) = result {
            socket
                .send(Message::Text("terminal input".into()))
                .await
                .unwrap();
        }
        assert_eq!(timeout(DEADLINE, impostor).await.unwrap().unwrap(), 0);
        assert!(!accepted);
    }
}
