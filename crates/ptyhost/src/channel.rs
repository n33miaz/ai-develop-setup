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
    async fn a_second_host_cannot_take_the_same_pipe() {
        let endpoint = test_endpoint("twice");
        let _first = Listener::bind(&endpoint).unwrap();
        assert!(Listener::bind(&endpoint).is_err());
    }
}
