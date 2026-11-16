//! The `ads-ptyhost` binary as a separate process, with a real terminal (ConPTY on Windows)
//! running a harmless system shell: the client is dropped twenty times in the middle of the
//! session and still ends with exactly the bytes the host numbered.

mod common;

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::time::Instant;

use ads_ptyhost::wire::{
    Base64, Call, NoParams, Notification, OpenParams, ResultBody, SessionId, WriteParams,
};
use common::allowed_env;
use common::client::{ChannelStream, Client, Incoming, Transcript, unique_endpoint};

const DROPS: u64 = 20;

/// The host process; killed when the test ends, whichever way it ends.
struct HostProcess {
    child: Child,
    endpoint: String,
}

impl HostProcess {
    fn start(label: &str) -> Self {
        let endpoint = unique_endpoint(label);
        // The user layer of the configuration, written inside the Cargo build folder.
        let config = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("ads-pty-{}-{label}.toml", std::process::id()));
        std::fs::write(&config, format!("channel_path = '{endpoint}'\n")).unwrap();

        let mut child = Command::new(env!("CARGO_BIN_EXE_ads-ptyhost"))
            .arg("--config")
            .arg(&config)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut ready = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut ready)
            .unwrap();
        let host = Self { child, endpoint };
        assert_eq!(
            ready.trim_end(),
            format!(
                "ads-ptyhost {} listening on {}",
                env!("CARGO_PKG_VERSION"),
                host.endpoint
            )
        );
        std::fs::remove_file(&config).unwrap();
        host
    }
}

impl Drop for HostProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// An interactive system shell: it prints only when the test types something.
fn open_shell(session_id: &SessionId) -> Call {
    #[cfg(windows)]
    let (program, args) = ("cmd.exe", vec!["/d".to_owned().try_into().unwrap()]);
    #[cfg(not(windows))]
    let (program, args) = ("/bin/sh", Vec::new());
    Call::Open(OpenParams {
        session_id: session_id.clone(),
        program: program.to_owned().try_into().unwrap(),
        args,
        cwd: None,
        env: allowed_env().into_iter().collect::<BTreeMap<_, _>>(),
        cols: 120.try_into().unwrap(),
        rows: 30.try_into().unwrap(),
    })
}

async fn type_line(client: &mut Client<ChannelStream>, session_id: &SessionId, line: &str) {
    let enter = if cfg!(windows) { "\r" } else { "\n" };
    let write = Call::Write(WriteParams {
        session_id: session_id.clone(),
        data: Base64(format!("{line}{enter}").into_bytes()).into(),
    });
    client.call(write).await.unwrap();
}

fn accept(transcript: &mut Transcript, session_id: &SessionId, incoming: Incoming) {
    match incoming {
        Incoming::Output { header, bytes } => transcript.accept(session_id, &header, &bytes),
        Incoming::Notification(other) => panic!("unexpected notification: {other:?}"),
    }
}

/// Follows the session from `transcript` on until the host says the process exited.
async fn follow_to_exit(
    client: &mut Client<ChannelStream>,
    session_id: &SessionId,
    transcript: &mut Transcript,
) -> Option<i64> {
    loop {
        match client.next().await.expect("channel closed") {
            Incoming::Notification(Notification::SessionExited(exited)) => {
                assert_eq!(exited.last_seq, transcript.last_seq);
                return exited.exit_code;
            }
            incoming => accept(transcript, session_id, incoming),
        }
    }
}

#[tokio::test]
async fn real_terminal_survives_twenty_client_drops_without_losing_a_byte() {
    let started = Instant::now();
    let host = HostProcess::start("real");
    let session_id = SessionId::new("sess-real").unwrap();

    let mut client = Client::connect(&host.endpoint).await;
    assert_eq!(
        client.call(Call::List(NoParams {})).await,
        Ok(ResultBody::SessionList(ads_ptyhost::wire::SessionList {
            sessions: Vec::new()
        }))
    );
    client.call(open_shell(&session_id)).await.unwrap();

    let mut transcript = Transcript::default();
    for drop_number in 0..DROPS {
        // Typed right before dying: the answer is printed while nobody is connected.
        type_line(
            &mut client,
            &session_id,
            &format!("echo away-{drop_number}"),
        )
        .await;
        drop(client);

        client = Client::connect(&host.endpoint).await;
        client
            .subscribe(&session_id, Some(transcript.last_seq + 1))
            .await
            .unwrap();
        // Typed while connected: at least the first piece of the answer arrives live.
        type_line(
            &mut client,
            &session_id,
            &format!("echo here-{drop_number}"),
        )
        .await;
        let incoming = client.next().await.expect("channel closed");
        accept(&mut transcript, &session_id, incoming);
    }
    type_line(&mut client, &session_id, "exit").await;
    drop(client);

    let mut client = Client::connect(&host.endpoint).await;
    client
        .subscribe(&session_id, Some(transcript.last_seq + 1))
        .await
        .unwrap();
    let exit_code = follow_to_exit(&mut client, &session_id, &mut transcript).await;
    assert_eq!(exit_code, Some(0));

    // The reference: one client that never dropped, reading everything the host numbered.
    let mut reference = Transcript::default();
    let mut reader = Client::connect(&host.endpoint).await;
    let bounds = reader.subscribe(&session_id, None).await.unwrap();
    assert_eq!(
        bounds.oldest_seq.get(),
        1,
        "the ring must hold the whole test"
    );
    follow_to_exit(&mut reader, &session_id, &mut reference).await;

    assert_eq!(transcript.last_seq, reference.last_seq);
    assert!(
        transcript.bytes == reference.bytes,
        "bytes seen across the drops differ from the bytes the host numbered"
    );
    let text = String::from_utf8_lossy(&transcript.bytes);
    for drop_number in 0..DROPS {
        for marker in [format!("away-{drop_number}"), format!("here-{drop_number}")] {
            assert!(text.contains(&marker), "{marker} missing from the output");
        }
    }
    println!(
        "{DROPS} drops on a real terminal: {} bytes in {} chunks, identical, in {:?}",
        transcript.bytes.len(),
        transcript.last_seq,
        started.elapsed()
    );
}
