//! Credencial por execução e conferência da fronteira HTTP antes do upgrade.

use std::{fmt, net::SocketAddr};

use ads_contracts::desktop_proof;

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use subtle::ConstantTimeEq;
use tokio_tungstenite::tungstenite::handshake::server::Request;
use zeroize::Zeroize;

use crate::api::ApiError;

pub struct Token(String);

impl Drop for Token {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Token([redacted])")
    }
}

impl Token {
    pub fn generate() -> Result<Self, ApiError> {
        let mut bytes = [0; 32];
        getrandom::fill(&mut bytes).map_err(|_| ApiError::Unavailable)?;
        Ok(Self(URL_SAFE_NO_PAD.encode(bytes)))
    }

    pub fn matches(&self, candidate: &str) -> bool {
        self.0.as_bytes().ct_eq(candidate.as_bytes()).into()
    }

    /// The session proof of a connection whose `auth.hello` carried this token. Crate-private:
    /// any module of this crate may call it; in production only the connection loop does, after
    /// reading `auth.hello`.
    pub(crate) fn open_session(&self, candidate: &str) -> Option<LocalSession> {
        self.matches(candidate)
            .then_some(LocalSession { _sealed: () })
    }

    pub fn desktop_proof(&self, nonce: &[u8], client: SocketAddr, server: SocketAddr) -> String {
        desktop_proof::sign(&self.0, nonce, client, server)
    }

    pub fn desktop_bootstrap(&self, port: u16) -> String {
        format!("{port} {}", self.0)
    }

    pub fn launch_url(&self, base: &str, port: u16) -> String {
        format!(
            "{}/#core=ws://127.0.0.1:{port}&token={}",
            base.trim_end_matches('/'),
            self.0
        )
    }
}

/// Proof that a connection presented the run token in `auth.hello`. Only this crate can issue
/// it, and in production only its connection loop does, so the owner-only controls of the local
/// API take it instead of trusting ids:
///
/// ```compile_fail
/// let _ = ads_daemon::auth::LocalSession { _sealed: () };
/// ```
///
/// ```compile_fail
/// let token = ads_daemon::auth::Token::generate().unwrap();
/// let _ = token.open_session("any");
/// ```
#[derive(Debug)]
pub struct LocalSession {
    _sealed: (),
}

pub fn permits_upgrade(request: &Request, port: u16, browser_origins: Option<&[String]>) -> bool {
    let headers = request.headers();
    let one = |key: &str| {
        let mut values = headers.get_all(key).iter();
        let value = values.next()?.to_str().ok()?;
        if values.next().is_some() {
            None
        } else {
            Some(value)
        }
    };
    request.uri().path() == "/"
        && request.uri().query().is_none()
        && one("host").is_some_and(|host| {
            host == format!("127.0.0.1:{port}") || host == format!("localhost:{port}")
        })
        && match browser_origins {
            Some(origins) => {
                one("origin").is_some_and(|origin| origins.iter().any(|allowed| allowed == origin))
            }
            None => !headers.contains_key("origin"),
        }
}
