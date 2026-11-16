#![cfg(windows)]

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use ads_daemon::pty_client::{ClientConfig, Event, PtyClient};
use ads_ptyhost::HostConfig;
use ads_ptyhost::wire::{Notification, OpenParams, SessionId, SessionParams};

struct HostProcess(Child);
impl Drop for HostProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[tokio::test]
async fn real_host_runs_a_noninteractive_echo_command() {
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
    let overrides = format!("channel_path = '{endpoint}'\n");
    let config_path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("daemon-echo-{}.toml", std::process::id()));
    std::fs::write(&config_path, &overrides).unwrap();
    let config = HostConfig::from_toml_str(&overrides).unwrap();
    let mut process = HostProcess(
        Command::new(binary)
            .arg("--config")
            .arg(config_path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let stdout = process.0.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let result = BufReader::new(stdout).read_line(&mut line);
        let _ = tx.send(result.map(|_| line));
    });
    let ready = rx
        .recv_timeout(Duration::from_secs(30))
        .expect("host did not start")
        .unwrap();
    assert!(ready.contains("listening on"));
    let mut client = PtyClient::connect(&config, ClientConfig::recommended(&config))
        .await
        .unwrap();
    let session_id = SessionId::new("echo").unwrap();
    let env = ["SystemRoot", "windir", "ComSpec", "PATH", "PATHEXT"]
        .into_iter()
        .filter_map(|key| std::env::var(key).ok().map(|value| (key.to_owned(), value)))
        .collect::<BTreeMap<_, _>>();
    client
        .open(OpenParams {
            session_id: session_id.clone(),
            program: "cmd.exe".to_owned().try_into().unwrap(),
            args: ["/d", "/c", "echo daemon-pty-client-ok"]
                .into_iter()
                .map(|s| s.to_owned().try_into().unwrap())
                .collect(),
            cwd: None,
            env,
            cols: 80.try_into().unwrap(),
            rows: 24.try_into().unwrap(),
        })
        .await
        .unwrap();
    client.subscribe(session_id.clone(), Some(0)).await.unwrap();
    let mut output = Vec::new();
    let mut last_seq = 0;
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            match client.next().await.unwrap() {
                Event::Output { header, bytes } => {
                    assert_eq!(header.session_id, session_id);
                    assert_eq!(header.seq.get(), last_seq + 1);
                    last_seq += 1;
                    output.extend(bytes);
                }
                Event::Notification(Notification::SessionExited(exited)) => {
                    assert_eq!(exited.exit_code, Some(0));
                    assert_eq!(exited.last_seq, last_seq);
                    break;
                }
                other => panic!("unexpected event: {other:?}"),
            }
        }
    })
    .await
    .expect("echo did not exit");
    assert!(String::from_utf8_lossy(&output).contains("daemon-pty-client-ok"));
    client.close(SessionParams { session_id }).await.unwrap();
    assert!(client.list().await.unwrap().sessions.is_empty());
}
