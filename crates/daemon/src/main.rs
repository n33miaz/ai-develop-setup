//! Inicialização do daemon local.

use ads_daemon::api::{ApiError, LocalApi};
use ads_daemon::pty_client::{ClientConfig, PtyClient};
use ads_daemon::startup::load_config;
use ads_daemon::tool_launcher::ToolLauncher;
use std::process::ExitCode;
use std::sync::Arc;

fn print_launch_url(url: &mut String, out: &mut impl std::io::Write) -> std::io::Result<()> {
    use zeroize::Zeroize;
    let result = writeln!(out, "{url}").and_then(|()| out.flush());
    url.zeroize();
    result
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let result: Result<(), Box<dyn std::error::Error>> = async {
        let (config, host_config, tools) = load_config(std::env::args().skip(1))?;
        let launcher = ToolLauncher::from_toml_str(
            &tools,
            ads_adapter_claude::process_env(),
            ads_adapter_claude::DiskProbe,
            ads_adapter_codex::DiskProbe,
        )?;
        let host = tokio::time::timeout(
            std::time::Duration::from_millis(config.host_timeout_ms),
            PtyClient::connect(&host_config, ClientConfig::recommended(&host_config)),
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .map_err(|_| ApiError::Unavailable)?;
        let api = LocalApi::bind(config, host, Arc::new(launcher)).await?;
        let mut url = zeroize::Zeroizing::new(api.launch_url()?);
        print_launch_url(&mut url, &mut std::io::stdout()).map_err(|_| ApiError::Unavailable)?;
        Ok(api.run().await?)
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
    fn printed_url_is_one_line_and_erased_even_when_output_fails() {
        let original = "http://localhost:5173/#core=ws://127.0.0.1:1234&token=credential-sentinel";
        let mut url = original.to_owned();
        let mut bytes = Vec::new();
        print_launch_url(&mut url, &mut bytes).unwrap();
        assert_eq!(bytes, format!("{original}\n").as_bytes());
        assert!(url.is_empty());
        struct Broken;
        impl std::io::Write for Broken {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut url = original.to_owned();
        assert!(print_launch_url(&mut url, &mut Broken).is_err());
        assert!(url.is_empty());
    }
}
