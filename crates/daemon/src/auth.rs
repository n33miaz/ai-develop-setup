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
