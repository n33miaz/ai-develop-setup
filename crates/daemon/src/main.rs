//! Inicialização do daemon local.

use ads_daemon::api::{ApiConfig, ApiError, LocalApi};
use ads_daemon::pty_client::{ClientConfig, PtyClient};
use ads_daemon::tool_launcher::ToolLauncher;
use ads_ptyhost::HostConfig;
use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;

fn load_config(
    mut args: impl Iterator<Item = String>,
) -> Result<(ApiConfig, HostConfig, String), String> {
    let mut host = HostConfig::defaults();
    let mut overrides = String::new();
    while let Some(arg) = args.next() {
        match (arg.as_str(), args.next()) {
            ("--config", Some(path)) => {
                if !Path::new(&path).is_absolute() {
                    return Err("--config requires an absolute path".into());
                }
                overrides =
                    std::fs::read_to_string(path).map_err(|_| "cannot read user configuration")?;
            }
            ("--host-config", Some(path)) => {
                let text =
                    std::fs::read_to_string(path).map_err(|_| "cannot read host configuration")?;
                host =
                    HostConfig::from_toml_str(&text).map_err(|_| "invalid host configuration")?;
            }
            _ => return Err("invalid daemon arguments".into()),
        }
    }
    let mut api: toml::Table =
        toml::from_str(&overrides).map_err(|_| "invalid user configuration TOML")?;
    let mut tools = toml::Table::new();
    for name in ["env", "workdirs", "tools"] {
        if let Some(value) = api.remove(name) {
            tools.insert(name.to_owned(), value);
        }
    }
    if let Some(toml::Value::Table(entries)) = tools.get("tools")
        && entries.keys().any(|name| {
            !matches!(
                name.as_str(),
                ads_adapter_claude::HARNESS_ID | ads_adapter_codex::HARNESS_ID
            )
        })
    {
        return Err("unknown tool in user configuration".into());
    }
    let api =
        ApiConfig::from_toml_str(&api.to_string()).map_err(|_| "invalid daemon configuration")?;
    Ok((api, host, tools.to_string()))
}
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
    fn arguments_are_closed() {
        assert!(load_config(std::iter::empty()).is_ok());
        for arg in ["--unknown", "--config"] {
            assert!(load_config([arg.to_owned()].into_iter()).is_err());
        }
        assert_eq!(
            load_config(["--config".to_owned(), "relative.toml".to_owned()].into_iter())
                .unwrap_err(),
            "--config requires an absolute path"
        );
    }
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
