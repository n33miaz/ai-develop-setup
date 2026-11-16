//! Inicialização do daemon local.

use ads_daemon::api::{ApiConfig, ApiError, DisabledLauncher, LocalApi};
use ads_daemon::pty_client::{ClientConfig, PtyClient};
use ads_ptyhost::HostConfig;
use std::process::ExitCode;
use std::sync::Arc;

fn load_config(mut args: impl Iterator<Item = String>) -> Result<ApiConfig, ApiError> {
    let mut overrides = String::new();
    while let Some(arg) = args.next() {
        match (arg.as_str(), args.next()) {
            ("--config", Some(path)) => {
                overrides = std::fs::read_to_string(path).map_err(|_| ApiError::Invalid)?
            }
            _ => return Err(ApiError::Invalid),
        }
    }
    ApiConfig::from_toml_str(&overrides)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let result = async {
        let config = load_config(std::env::args().skip(1))?;
        let host_config = HostConfig::defaults();
        let host = tokio::time::timeout(
            std::time::Duration::from_millis(config.host_timeout_ms),
            PtyClient::connect(&host_config, ClientConfig::recommended(&host_config)),
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .map_err(|_| ApiError::Unavailable)?;
        let api = LocalApi::bind(config, host, Arc::new(DisabledLauncher)).await?;
        println!("{}", api.launch_url()?);
        api.run().await
    }
    .await;
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("ads-daemon: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arguments_are_closed() {
        assert!(load_config(std::iter::empty()).is_ok());
        for arg in ["--unknown", "--config"] {
            assert!(load_config([arg.to_owned()].into_iter()).is_err());
        }
    }
}
