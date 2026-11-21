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
const RETRY: &str = "desktop connection retry";
const PROOF_MISSING: &str = "desktop identity proof missing";
const RELAUNCH: &str = "desktop relaunch required";
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
    Blocked(&'static str),
}

impl ConnectionState {
    fn sender_for(&self, id: &str) -> Result<mpsc::Sender<String>, &'static str> {
        match self {
            Self::Connected(connection) if connection.id == id => Ok(connection.requests.clone()),
            _ => Err(UNAVAILABLE),
        }
    }
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
    let address = bootstrap.url.strip_prefix("ws://").ok_or(RETRY)?;
    let stream = TcpStream::connect(address).await.map_err(|_| RETRY)?;
    let client = stream.local_addr().map_err(|_| RETRY)?;
    let server = stream.peer_addr().map_err(|_| RETRY)?;
    let request = bootstrap
        .url
        .as_str()
        .into_client_request()
        .map_err(|_| RETRY)?;
    let nonce = request.headers()["sec-websocket-key"].clone();
    let (mut socket, response) = client_async(request, stream).await.map_err(|_| RETRY)?;
    let proof = response
        .headers()
        .get(desktop_proof::HEADER)
        .ok_or(PROOF_MISSING)?;
    if !desktop_proof::verify(
        &bootstrap.token,
        nonce.as_bytes(),
        client,
        server,
        proof.as_bytes(),
    ) {
        return Err(RELAUNCH);
    }
    let hello = Zeroizing::new(format!(
        r#"{{"jsonrpc":"2.0","id":"{HELLO_ID}","method":"auth.hello","params":{{"token":"{}","_meta":{{"origin":"local","idempotency_key":"{HELLO_ID}"}}}}}}"#,
        bootstrap.token
    ));
    socket
        .send(Message::Text(hello.as_str().into()))
        .await
        .map_err(|_| RETRY)?;
    drop(hello);
    let Some(Ok(Message::Text(reply))) = socket.next().await else {
        return Err(RETRY);
    };
    let reply: Value = serde_json::from_str(&reply).map_err(|_| RETRY)?;
    if reply["jsonrpc"] != "2.0" || reply["id"] != HELLO_ID || reply["result"]["accepted"] != true {
        return Err(RETRY);
    }
    Ok(socket)
}

async fn connect_socket(
    bootstrap: &Bootstrap,
    connection: &mut ConnectionState,
) -> Result<WebSocketStream<TcpStream>, &'static str> {
    if let ConnectionState::Blocked(reason) = connection {
        return Err(reason);
    }
    *connection = ConnectionState::Idle;
    let result = timeout(DEADLINE, authenticate(bootstrap))
        .await
        .unwrap_or(Err(RETRY));
    if let Err(reason @ (PROOF_MISSING | RELAUNCH)) = result {
        *connection = ConnectionState::Blocked(reason);
    }
    result
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
    let socket = connect_socket(&bridge.bootstrap, &mut connection).await?;
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
    let sender = bridge.connection.lock().await.sender_for(&id)?;
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

    use tokio::net::TcpListener;
    use tokio_tungstenite::{
        accept_hdr_async,
        tungstenite::handshake::server::{Request, Response},
    };

    fn bootstrap(listener: &TcpListener) -> Bootstrap {
        Bootstrap {
            url: format!("ws://{}", listener.local_addr().unwrap()),
            token: "a".repeat(43),
        }
    }

    #[allow(clippy::result_large_err)] // A assinatura do callback é definida pela biblioteca WebSocket.
    async fn accept(
        stream: TcpStream,
        proof_token: Option<&str>,
        wrong_connection: bool,
    ) -> WebSocketStream<TcpStream> {
        let client = stream.peer_addr().unwrap();
        let server = stream.local_addr().unwrap();
        accept_hdr_async(stream, |request: &Request, mut response: Response| {
            if let Some(token) = proof_token {
                let (client, server) = if wrong_connection {
                    (server, client)
                } else {
                    (client, server)
                };
                let proof = desktop_proof::sign(
                    token,
                    request.headers()["sec-websocket-key"].as_bytes(),
                    client,
                    server,
                );
                response
                    .headers_mut()
                    .insert(desktop_proof::HEADER, proof.parse().unwrap());
            }
            Ok(response)
        })
        .await
        .unwrap()
    }

    fn accepted() -> Message {
        Message::Text(
            format!(r#"{{"jsonrpc":"2.0","id":"{HELLO_ID}","result":{{"accepted":true}}}}"#).into(),
        )
    }

    async fn accept_authenticated(listener: &TcpListener, token: &str) {
        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = accept(stream, Some(token), false).await;
        let hello = socket.next().await.unwrap().unwrap().into_text().unwrap();
        let hello: Value = serde_json::from_str(hello.as_str()).unwrap();
        assert_eq!(hello["method"], "auth.hello");
        assert_eq!(hello["params"]["token"], token);
        socket.send(accepted()).await.unwrap();
    }

    #[tokio::test]
    async fn reports_retry_for_refused_upgrade_between_authenticated_connections() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let bootstrap = bootstrap(&listener);
        let token = bootstrap.token.clone();
        let daemon = tokio::spawn(async move {
            accept_authenticated(&listener, &token).await;
            drop(listener.accept().await.unwrap());
            accept_authenticated(&listener, &token).await;
        });

        let mut state = ConnectionState::Idle;
        let mut socket = connect_socket(&bootstrap, &mut state).await.unwrap();
        assert!(matches!(
            timeout(DEADLINE, socket.next()).await,
            Ok(None | Some(Err(_)))
        ));
        assert!(matches!(
            connect_socket(&bootstrap, &mut state).await,
            Err(RETRY)
        ));
        assert!(connect_socket(&bootstrap, &mut state).await.is_ok());
        timeout(DEADLINE, daemon).await.unwrap().unwrap();
    }

    async fn listener_without_valid_proof_receives_no_frames(wrong_connection: bool) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let bootstrap = bootstrap(&listener);
        let token = bootstrap.token.clone();
        let impostor = tokio::spawn(async move {
            accept_authenticated(&listener, &token).await;
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = accept(stream, wrong_connection.then_some(token.as_str()), true).await;
            let _ = socket.send(accepted()).await;
            assert!(!matches!(socket.next().await, Some(Ok(_))));
        });

        let mut state = ConnectionState::Idle;
        let mut socket = connect_socket(&bootstrap, &mut state).await.unwrap();
        assert!(matches!(
            timeout(DEADLINE, socket.next()).await,
            Ok(None | Some(Err(_)))
        ));
        let result = connect_socket(&bootstrap, &mut state).await;
        let expected = if wrong_connection {
            RELAUNCH
        } else {
            PROOF_MISSING
        };
        assert_eq!(result.err(), Some(expected));
        timeout(DEADLINE, impostor).await.unwrap().unwrap();
        assert_eq!(
            connect_socket(&bootstrap, &mut state).await.err(),
            Some(expected)
        );
    }

    #[tokio::test]
    async fn missing_proof_sends_no_frames_and_blocks_reconnection() {
        listener_without_valid_proof_receives_no_frames(false).await;
    }

    #[tokio::test]
    async fn proof_for_another_connection_sends_no_frames_and_requires_relaunch() {
        listener_without_valid_proof_receives_no_frames(true).await;
    }

    #[tokio::test]
    async fn desktop_send_rejects_the_previous_connection_id() {
        let (requests, mut incoming) = mpsc::channel(1);
        let state = ConnectionState::Connected(Connection {
            id: "current".into(),
            requests,
            task: tauri::async_runtime::spawn(std::future::pending()),
        });
        assert!(matches!(state.sender_for("previous"), Err(UNAVAILABLE)));
        state
            .sender_for("current")
            .unwrap()
            .try_send("command".into())
            .unwrap();
        assert_eq!(incoming.try_recv().unwrap(), "command");
    }
}
