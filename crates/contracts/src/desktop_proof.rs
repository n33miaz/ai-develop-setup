//! Server proof for the desktop WebSocket upgrade.

use std::net::SocketAddr;

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

pub const HEADER: &str = "ads-desktop-proof";

fn mac(token: &str, nonce: &[u8], client: SocketAddr, server: SocketAddr) -> Hmac<Sha256> {
    let mut mac = Hmac::<Sha256>::new_from_slice(token.as_bytes()).expect("HMAC accepts any key");
    // Binding the proof to both endpoints prevents forwarding a challenge over another socket.
    mac.update(format!("urn:ads:handshake:desktop-proof:1\n{client}\n{server}\n").as_bytes());
    mac.update(nonce);
    mac
}

pub fn sign(token: &str, nonce: &[u8], client: SocketAddr, server: SocketAddr) -> String {
    URL_SAFE_NO_PAD.encode(mac(token, nonce, client, server).finalize().into_bytes())
}

pub fn verify(
    token: &str,
    nonce: &[u8],
    client: SocketAddr,
    server: SocketAddr,
    proof: &[u8],
) -> bool {
    let mut tag = [0; 32];
    proof.len() == 43
        && URL_SAFE_NO_PAD.decode_slice(proof, &mut tag) == Ok(tag.len())
        && mac(token, nonce, client, server).verify_slice(&tag).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmac_sha256_matches_rfc_4231_case_two() {
        let mut mac = Hmac::<Sha256>::new_from_slice(b"Jefe").unwrap();
        mac.update(b"what do ya want for nothing?");
        let tag = mac.finalize().into_bytes();
        assert_eq!(
            tag[..],
            [
                0x5b, 0xdc, 0xc1, 0x46, 0xbf, 0x60, 0x75, 0x4e, 0x6a, 0x04, 0x24, 0x26, 0x08, 0x95,
                0x75, 0xc7, 0x5a, 0x00, 0x3f, 0x08, 0x9d, 0x27, 0x39, 0x83, 0x9d, 0xec, 0x58, 0xb9,
                0x64, 0xec, 0x38, 0x43,
            ]
        );
    }

    #[test]
    fn proof_matches_only_the_session_and_connection_that_requested_it() {
        let client = "127.0.0.1:12345".parse().unwrap();
        let server = "127.0.0.1:12346".parse().unwrap();
        let proof = sign("session", b"nonce", client, server);
        let proof = proof.as_bytes();
        assert!(verify("session", b"nonce", client, server, proof));
        assert!(!verify("new session", b"nonce", client, server, proof));
        assert!(!verify("session", b"new nonce", client, server, proof));
        assert!(!verify("session", b"nonce", server, client, proof));
        assert!(!verify("session", b"nonce", client, server, b"invalid"));
    }
}
