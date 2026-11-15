//! Host parameters. The defaults live only in `defaults/ptyhost.toml`.

use serde::Deserialize;

use crate::types::PtyError;

const DEFAULTS_TOML: &str = include_str!("../../../defaults/ptyhost.toml");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostConfig {
    pub replay_capacity_bytes: usize,
    pub read_buffer_bytes: usize,
    pub subscriber_queue_chunks: usize,
    pub max_sessions: usize,
    pub max_frame_bytes: usize,
    pub channel_name: String,
    /// Empty means "derive from `channel_name` and the current user".
    pub channel_path: String,
}

/// Smallest `max_frame_bytes` accepted: below this not even `session.open` would fit.
const MAX_FRAME_BYTES_FLOOR: usize = 4096;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Layer {
    replay_capacity_bytes: Option<usize>,
    read_buffer_bytes: Option<usize>,
    subscriber_queue_chunks: Option<usize>,
    max_sessions: Option<usize>,
    max_frame_bytes: Option<usize>,
    channel_name: Option<String>,
    channel_path: Option<String>,
}

impl HostConfig {
    /// The values shipped in `defaults/ptyhost.toml`.
    pub fn defaults() -> Self {
        Self::from_toml_str("").expect("the shipped defaults are complete and valid")
    }

    /// The shipped defaults with every key present in `overrides` replaced.
    pub fn from_toml_str(overrides: &str) -> Result<Self, PtyError> {
        let base = parse(DEFAULTS_TOML)?;
        let over = parse(overrides)?;
        fn pick<T>(name: &str, over: Option<T>, base: Option<T>) -> Result<T, PtyError> {
            over.or(base)
                .ok_or_else(|| PtyError::Config(format!("missing key `{name}`")))
        }
        let config = Self {
            replay_capacity_bytes: pick(
                "replay_capacity_bytes",
                over.replay_capacity_bytes,
                base.replay_capacity_bytes,
            )?,
            read_buffer_bytes: pick(
                "read_buffer_bytes",
                over.read_buffer_bytes,
                base.read_buffer_bytes,
            )?,
            subscriber_queue_chunks: pick(
                "subscriber_queue_chunks",
                over.subscriber_queue_chunks,
                base.subscriber_queue_chunks,
            )?,
            max_sessions: pick("max_sessions", over.max_sessions, base.max_sessions)?,
            max_frame_bytes: pick(
                "max_frame_bytes",
                over.max_frame_bytes,
                base.max_frame_bytes,
            )?,
            channel_name: pick("channel_name", over.channel_name, base.channel_name)?,
            channel_path: pick("channel_path", over.channel_path, base.channel_path)?,
        };
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), PtyError> {
        let positive = [
            ("replay_capacity_bytes", self.replay_capacity_bytes),
            ("read_buffer_bytes", self.read_buffer_bytes),
            ("subscriber_queue_chunks", self.subscriber_queue_chunks),
            ("max_sessions", self.max_sessions),
        ];
        for (name, value) in positive {
            if value == 0 {
                return Err(PtyError::Config(format!("`{name}` must be at least 1")));
            }
        }
        if self.max_frame_bytes < MAX_FRAME_BYTES_FLOOR {
            return Err(PtyError::Config(format!(
                "`max_frame_bytes` must be at least {MAX_FRAME_BYTES_FLOOR}"
            )));
        }
        let name_ok = !self.channel_name.is_empty()
            && self.channel_name.len() <= 64
            && self
                .channel_name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'));
        if !name_ok {
            return Err(PtyError::Config(
                "`channel_name` must be 1 to 64 of letters, digits and . _ -".to_owned(),
            ));
        }
        if self.read_buffer_bytes > self.replay_capacity_bytes {
            return Err(PtyError::Config(
                "`read_buffer_bytes` must not exceed `replay_capacity_bytes`".to_owned(),
            ));
        }
        Ok(())
    }
}

fn parse(text: &str) -> Result<Layer, PtyError> {
    toml::from_str(text).map_err(|err| PtyError::Config(err.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_defaults_are_valid() {
        let config = HostConfig::defaults();
        assert_eq!(config.replay_capacity_bytes, 1_048_576);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn override_replaces_only_the_given_key() {
        let config = HostConfig::from_toml_str("replay_capacity_bytes = 16384").unwrap();
        assert_eq!(config.replay_capacity_bytes, 16_384);
        assert_eq!(config.max_sessions, HostConfig::defaults().max_sessions);
    }

    #[test]
    fn channel_keys_have_defaults_and_can_be_replaced() {
        let config = HostConfig::defaults();
        assert_eq!(config.channel_name, "ads-ptyhost");
        assert_eq!(config.channel_path, "");
        let config = HostConfig::from_toml_str("channel_path = 'somewhere'").unwrap();
        assert_eq!(config.channel_path, "somewhere");
        assert!(HostConfig::from_toml_str("channel_name = 'a/b'").is_err());
        assert!(HostConfig::from_toml_str("channel_name = ''").is_err());
        assert!(HostConfig::from_toml_str("max_frame_bytes = 100").is_err());
    }

    #[test]
    fn unknown_key_is_refused() {
        assert!(matches!(
            HostConfig::from_toml_str("replay_bytes = 1"),
            Err(PtyError::Config(_))
        ));
    }

    #[test]
    fn zero_and_inverted_sizes_are_refused() {
        assert!(HostConfig::from_toml_str("max_sessions = 0").is_err());
        assert!(HostConfig::from_toml_str("replay_capacity_bytes = 16").is_err());
    }
}
