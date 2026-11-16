#![cfg(windows)]

use ads_ptyhost::{HostConfig, channel::Listener, wire};
use std::io::{BufRead, BufReader, Read};
use std::process::{Command, Stdio};
use std::time::Duration;
use tokio::io::AsyncWriteExt;

struct Child(std::process::Child);
impl Drop for Child {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[tokio::test]
async fn binary_stdout_contains_only_the_launch_url() {
    let endpoint = format!(r"\\.\pipe\ads-daemon-stdout-{}", std::process::id());
    let overrides = format!("channel_path = '{endpoint}'\n");
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("daemon-stdout-{}.toml", std::process::id()));
    std::fs::write(&path, &overrides).unwrap();
    let host = HostConfig::from_toml_str(&overrides).unwrap();
    let mut listener = Listener::bind(&channel_endpoint(&host)).unwrap();
    let mut child = Child(
        Command::new(env!("CARGO_BIN_EXE_ads-daemon"))
            .args(["--host-config", path.to_str().unwrap()])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let stdout = child.0.stdout.take().unwrap();
    let (line_tx, line_rx) = tokio::sync::oneshot::channel();
    let capture = std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        line_tx.send(line.clone()).unwrap();
        reader.read_to_string(&mut line).unwrap();
        line
    });
    let mut stream = tokio::time::timeout(Duration::from_secs(5), listener.accept())
        .await
        .unwrap()
        .unwrap();
    let wire::Frame::Json(payload) = wire::read_frame(&mut stream, 131072)
        .await
        .unwrap()
        .unwrap()
    else {
        panic!()
    };
    let request = wire::Request::parse(&payload).unwrap();
    stream
        .write_all(
            &wire::encode_json(&wire::Response {
                id: Some(request.id),
                outcome: Ok(wire::ResultBody::Hello(wire::HelloResult {
                    contract: Default::default(),
                })),
            })
            .unwrap(),
        )
        .await
        .unwrap();
    let line = tokio::time::timeout(Duration::from_secs(5), line_rx)
        .await
        .unwrap()
        .unwrap();
    let (base, fragment) = line.trim_end().split_once('#').unwrap();
    assert_eq!(base, "http://localhost:5173/");
    let (core, token) = fragment.split_once("&token=").unwrap();
    assert!(core.starts_with("core=ws://127.0.0.1:"));
    assert!(
        core.trim_start_matches("core=ws://127.0.0.1:")
            .parse::<u16>()
            .unwrap()
            > 0
    );
    assert_eq!(token.len(), 43);
    drop(stream);
    tokio::time::timeout(Duration::from_secs(5), async {
        while child.0.try_wait().unwrap().is_none() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(capture.join().unwrap(), line);
    assert_eq!(line.lines().count(), 1);
}

fn channel_endpoint(config: &HostConfig) -> String {
    ads_ptyhost::channel::endpoint(config).unwrap()
}
