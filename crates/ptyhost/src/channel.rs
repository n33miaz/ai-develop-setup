//! The local channel the core connects to: a named pipe on Windows, a Unix socket elsewhere.
//! Only the user running this process can open it.

use std::io;

use crate::config::HostConfig;

/// The stream of one accepted client.
#[cfg(windows)]
pub type Stream = tokio::net::windows::named_pipe::NamedPipeServer;
#[cfg(unix)]
pub type Stream = tokio::net::UnixStream;

/// Where the channel lives: `channel_path` when set, otherwise a per-user place built from
/// `channel_name`.
pub fn endpoint(config: &HostConfig) -> io::Result<String> {
    if !config.channel_path.is_empty() {
        return Ok(config.channel_path.clone());
    }
    default_endpoint(&config.channel_name)
}

#[cfg(windows)]
fn default_endpoint(name: &str) -> io::Result<String> {
    // Pipe names are shared by every user of the machine: the identifier keeps them apart.
    Ok(format!(
        r"\\.\pipe\{name}.{}",
        crate::win::current_user_sid()?
    ))
}

#[cfg(unix)]
fn default_endpoint(name: &str) -> io::Result<String> {
    match std::env::var("XDG_RUNTIME_DIR") {
        Ok(dir) if !dir.is_empty() => Ok(format!("{dir}/{name}.sock")),
        _ => Err(io::Error::new(
            io::ErrorKind::NotFound,
            "XDG_RUNTIME_DIR is not set; set `channel_path`",
        )),
    }
}

/// The stream of a client connected to the channel.
#[cfg(windows)]
pub type ClientStream = tokio::net::windows::named_pipe::NamedPipeClient;
#[cfg(unix)]
pub type ClientStream = tokio::net::UnixStream;

/// Why a client could not use the channel.
#[derive(Debug)]
pub enum ConnectError {
    /// Nothing answers at the endpoint, or it could not be opened.
    Unavailable(io::Error),
    /// Something answers, but it is not served by the current user; nothing was sent to it.
    NotOwnedByUser,
}

impl std::fmt::Display for ConnectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Unavailable(_) => "the channel is not available",
            Self::NotOwnedByUser => "the channel is not served by the current user",
        })
    }
}

impl std::error::Error for ConnectError {}

/// Connects to the channel at `endpoint` as a client. On Windows the stream is handed over only
/// after confirming that the current user serves the pipe (`win::check_pipe_server`): anyone can
/// create a pipe with this name before the host starts.
/// Must be called inside a Tokio runtime with I/O enabled.
#[cfg(windows)]
pub async fn connect(endpoint: &str) -> Result<ClientStream, ConnectError> {
    connect_checked(endpoint, crate::win::check_pipe_server)
}

/// Connects to the channel at `endpoint` as a client.
/// Must be called inside a Tokio runtime with I/O enabled.
#[cfg(unix)]
pub async fn connect(endpoint: &str) -> Result<ClientStream, ConnectError> {
    tokio::net::UnixStream::connect(endpoint)
        .await
        .map_err(ConnectError::Unavailable)
}

/// `connect` with the check of who serves the pipe given by the caller, so tests can refuse a
/// pipe. Passing a check that always succeeds removes the protection: use `connect`.
#[cfg(windows)]
#[doc(hidden)]
pub fn connect_checked(
    endpoint: &str,
    check: impl FnOnce(std::os::windows::io::RawHandle) -> io::Result<()>,
) -> Result<ClientStream, ConnectError> {
    use std::os::windows::io::AsRawHandle;

    let stream = tokio::net::windows::named_pipe::ClientOptions::new()
        .open(endpoint)
        .map_err(ConnectError::Unavailable)?;
    // Dropping the stream on refusal closes the pipe before a single byte goes through it.
    check(stream.as_raw_handle()).map_err(|_| ConnectError::NotOwnedByUser)?;
    Ok(stream)
}

#[cfg(windows)]
pub struct Listener {
    endpoint: String,
    user_sid: String,
    /// The instance waiting for the next client.
    next: Stream,
}

#[cfg(windows)]
impl Listener {
    /// Must be called inside a Tokio runtime with I/O enabled.
    pub fn bind(endpoint: &str) -> io::Result<Self> {
        let user_sid = crate::win::current_user_sid()?;
        let next = crate::win::create_user_pipe(endpoint, &user_sid, true)?;
        Ok(Self {
            endpoint: endpoint.to_owned(),
            user_sid,
            next,
        })
    }

    pub async fn accept(&mut self) -> io::Result<Stream> {
        // A client that opens the pipe and leaves before being accepted makes `connect` fail;
        // that instance is thrown away and the wait goes on with a fresh one.
        while self.next.connect().await.is_err() {
            self.next = crate::win::create_user_pipe(&self.endpoint, &self.user_sid, false)?;
        }
        let next = crate::win::create_user_pipe(&self.endpoint, &self.user_sid, false)?;
        Ok(std::mem::replace(&mut self.next, next))
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }
}

#[cfg(unix)]
pub struct Listener {
    endpoint: String,
    inner: tokio::net::UnixListener,
}

#[cfg(unix)]
impl Listener {
    /// Must be called inside a Tokio runtime with I/O enabled.
    pub fn bind(endpoint: &str) -> io::Result<Self> {
        use std::os::unix::fs::{FileTypeExt, PermissionsExt};

        // A socket file nobody answers on is what a previous run left behind; anything else
        // at that path is not ours to remove.
        if let Ok(meta) = std::fs::symlink_metadata(endpoint) {
            let stale = meta.file_type().is_socket()
                && std::os::unix::net::UnixStream::connect(endpoint).is_err();
            if !stale {
                return Err(io::Error::new(
                    io::ErrorKind::AddrInUse,
                    "the channel path is already in use",
                ));
            }
            std::fs::remove_file(endpoint)?;
        }
        let inner = tokio::net::UnixListener::bind(endpoint)?;
        std::fs::set_permissions(endpoint, std::fs::Permissions::from_mode(0o600))?;
        Ok(Self {
            endpoint: endpoint.to_owned(),
            inner,
        })
    }

    pub async fn accept(&mut self) -> io::Result<Stream> {
        let (stream, _) = self.inner.accept().await?;
        Ok(stream)
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }
}

#[cfg(unix)]
impl Drop for Listener {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.endpoint);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    fn test_endpoint(label: &str) -> String {
        format!(r"\\.\pipe\ads-ptyhost-unit-{}-{label}", std::process::id())
    }

    #[test]
    fn explicit_path_wins_over_the_derived_one() {
        let config = HostConfig::from_toml_str("channel_path = 'explicit'").unwrap();
        assert_eq!(endpoint(&config).unwrap(), "explicit");
    }

    #[cfg(windows)]
    #[test]
    fn derived_pipe_name_carries_the_user_identifier() {
        let derived = endpoint(&HostConfig::defaults()).unwrap();
        let sid = crate::win::current_user_sid().unwrap();
        assert!(sid.starts_with("S-1-"), "{sid}");
        assert_eq!(derived, format!(r"\\.\pipe\ads-ptyhost.{sid}"));
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn pipe_grants_access_to_the_current_user_only() {
        use std::os::windows::io::AsRawHandle;

        let mut listener = Listener::bind(&test_endpoint("acl")).unwrap();
        let sid = crate::win::current_user_sid().unwrap();
        let check = |stream: &Stream| {
            let rules = crate::win::access_list_text(stream.as_raw_handle()).unwrap();
            // Protected list (no inherited entries) with exactly one entry, for this user.
            assert!(rules.starts_with("D:P(A;;"), "{rules}");
            // The system writes the built-in administrator and guest accounts by their alias.
            let who = match sid.rsplit('-').next() {
                Some("500") => "LA",
                Some("501") => "LG",
                _ => sid.as_str(),
            };
            assert!(
                rules.ends_with(&format!(";;;{sid})")) || rules.ends_with(&format!(";;;{who})")),
                "{rules}"
            );
            assert_eq!(rules.matches('(').count(), 1, "{rules}");
        };
        check(&listener.next);

        // Every later instance gets the same rule, not the system default.
        let client = tokio::net::windows::named_pipe::ClientOptions::new()
            .open(listener.endpoint())
            .unwrap();
        let accepted = listener.accept().await.unwrap();
        check(&accepted);
        check(&listener.next);
        drop(client);
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn client_accepts_the_pipe_of_its_own_host() {
        use std::os::windows::io::AsRawHandle;

        let listener = Listener::bind(&test_endpoint("own")).unwrap();
        let client = connect(listener.endpoint()).await.unwrap();
        let local = crate::win::local_user().unwrap();
        let server = crate::win::pipe_server(client.as_raw_handle()).unwrap();
        assert_eq!(server.process_user, local.user);
        assert!(
            server.object_owner == local.user || server.object_owner == local.default_owner,
            "{server:?} {local:?}"
        );
    }

    /// What the server of a fresh pipe sees when a client connects with `check` as the owner
    /// check and, if accepted, sends `sent` and leaves: whether the client was refused, and every
    /// byte that reached the server before the end of the stream.
    #[cfg(windows)]
    async fn serve_one_client(
        label: &str,
        check: impl FnOnce(std::os::windows::io::RawHandle) -> io::Result<()>,
        sent: &[u8],
    ) -> (bool, Vec<u8>) {
        use std::time::Duration;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let endpoint = test_endpoint(label);
        let sid = crate::win::current_user_sid().unwrap();
        let mut server = crate::win::create_user_pipe(&endpoint, &sid, true).unwrap();
        let refused = {
            let mut waiting = std::pin::pin!(server.connect());
            // The server starts waiting before the client opens: a client gone before the wait
            // began would fail the wait and hide what reached the pipe.
            tokio::select! {
                biased;
                early = &mut waiting => panic!("connected with no client: {early:?}"),
                () = std::future::ready(()) => {}
            }
            let refused = match connect_checked(&endpoint, check) {
                Ok(mut client) => {
                    client.write_all(sent).await.unwrap();
                    false
                }
                Err(ConnectError::NotOwnedByUser) => true,
                Err(other) => panic!("the pipe did not open: {other:?}"),
            };
            tokio::time::timeout(Duration::from_secs(5), waiting)
                .await
                .expect("the server never saw the client")
                .expect("the server wait failed");
            refused
        };
        let mut received = Vec::new();
        // The client end closes: the stream ends with no error.
        tokio::time::timeout(Duration::from_secs(5), server.read_to_end(&mut received))
            .await
            .expect("the client left the pipe open")
            .expect("the stream did not end cleanly");
        (refused, received)
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn accepted_check_lets_the_bytes_through() {
        let (refused, received) = serve_one_client("accepted", |_| Ok(()), b"hello").await;
        assert!(!refused);
        assert_eq!(received, b"hello");
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn refused_check_closes_the_pipe_before_any_byte() {
        let mut checked = false;
        let (refused, received) = serve_one_client(
            "refused",
            |_| {
                checked = true;
                Err(io::Error::from(io::ErrorKind::PermissionDenied))
            },
            b"hello",
        )
        .await;
        assert!(checked);
        assert!(refused);
        assert!(received.is_empty(), "{} bytes got through", received.len());
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn real_queries_refuse_a_pipe_for_another_identity() {
        // The host's own pipe, read by the real queries, checked as if the client were someone
        // else: the server is refused and sees no byte.
        let stranger = crate::win::LocalUser {
            user: "S-1-5-21-1000-2000-3000-1002".to_owned(),
            default_owner: "S-1-5-21-1000-2000-3000-1002".to_owned(),
        };
        let (refused, received) = serve_one_client(
            "stranger",
            |pipe| crate::win::check_pipe_server_as(&stranger, pipe),
            b"hello",
        )
        .await;
        assert!(refused);
        assert!(received.is_empty(), "{} bytes got through", received.len());

        let (refused, received) =
            serve_one_client("self", crate::win::check_pipe_server, b"hello").await;
        assert!(!refused);
        assert_eq!(received, b"hello");
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn something_at_the_endpoint_that_is_not_a_pipe_is_refused_untouched() {
        // The closest the test gets to a stranger's server without a second account: the
        // endpoint opens, but nobody can be shown to serve it.
        let path =
            std::env::temp_dir().join(format!("ads-ptyhost-not-a-pipe-{}", std::process::id()));
        std::fs::write(&path, b"").unwrap();
        let result = connect(path.to_str().unwrap()).await;
        let refused = matches!(result, Err(ConnectError::NotOwnedByUser));
        drop(result);
        // The runtime closes the handle once it has seen the end of its pending read.
        let mut written = std::fs::read(&path);
        for _ in 0..100 {
            if written.is_ok() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            written = std::fs::read(&path);
        }
        let written = written.unwrap();
        std::fs::remove_file(&path).unwrap();
        assert!(refused);
        assert!(written.is_empty());
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn a_second_host_cannot_take_the_same_pipe() {
        let endpoint = test_endpoint("twice");
        let _first = Listener::bind(&endpoint).unwrap();
        assert!(Listener::bind(&endpoint).is_err());
    }
}
