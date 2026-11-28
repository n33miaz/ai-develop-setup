use sha2::{Digest, Sha256};

/// `sha256:` and the SHA-256 of `bytes`, in lower-case hexadecimal.
pub fn sha256_text(bytes: &[u8]) -> String {
    let hex: String = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("sha256:{hex}")
}
