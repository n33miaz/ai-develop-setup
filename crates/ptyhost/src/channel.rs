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
///
/// The check cannot be swapped from outside this crate:
///
/// ```compile_fail
/// use ads_ptyhost::channel::connect_checked;
/// ```
///
/// ```compile_fail
/// use ads_ptyhost::win::check_pipe_server_as;
/// ```
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

/// `connect` with the check of who serves the pipe given by the caller; private, so only the
/// tests of this crate can pass another check.
#[cfg(windows)]
pub(crate) fn connect_checked(
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
mod tests;
