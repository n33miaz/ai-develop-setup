//! Leitura dos argumentos e dos arquivos de configuração na partida do daemon.

use crate::api::ApiConfig;
use ads_core::gate::AbsPath;
use ads_ptyhost::HostConfig;
use std::io::Read;

/// Largest configuration file read. Fixed because it guards the reading of the file that would
/// hold the setting; real files are a few kilobytes.
pub const MAX_CONFIG_BYTES: u64 = 1024 * 1024;

/// The API settings, the host settings and the tools' part of the user file (`[env]`,
/// `[workdirs]`, `[tools]`), still as TOML for the adapters.
pub type StartupConfig = (ApiConfig, HostConfig, String);

/// Reads `--config` and `--host-config`. Errors name the flag or the line, never the content.
pub fn load_config(mut args: impl Iterator<Item = String>) -> Result<StartupConfig, String> {
    let mut host = HostConfig::defaults();
    let mut overrides = String::new();
    while let Some(arg) = args.next() {
        match (arg.as_str(), args.next()) {
            ("--config", Some(path)) => overrides = read_config_file("--config", &path)?,
            ("--host-config", Some(path)) => {
                let text = read_config_file("--host-config", &path)?;
                host =
                    HostConfig::from_toml_str(&text).map_err(|_| "invalid host configuration")?;
            }
            _ => return Err("invalid daemon arguments".into()),
        }
    }
    let mut api: toml::Table =
        toml::from_str(&overrides).map_err(|error: toml::de::Error| match error.span() {
            // Only the position: the parser message could quote the file.
            Some(span) => format!(
                "invalid user configuration TOML at line {}",
                overrides[..span.start].matches('\n').count() + 1
            ),
            None => "invalid user configuration TOML".to_owned(),
        })?;
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

/// A literal absolute path (no network share, device prefix, variable, `..` or short name) to
/// a regular file of at most [`MAX_CONFIG_BYTES`].
fn read_config_file(flag: &str, path: &str) -> Result<String, String> {
    AbsPath::parse(path).map_err(|error| format!("{flag} path refused: {error}"))?;
    let unreadable = || format!("cannot read {flag} file");
    // Checked before opening too: opening a folder fails on Windows with a vaguer error.
    if !std::fs::metadata(path).map_err(|_| unreadable())?.is_file() {
        return Err(format!("{flag} must name a regular file"));
    }
    let file = std::fs::File::open(path).map_err(|_| unreadable())?;
    if !file.metadata().map_err(|_| unreadable())?.is_file() {
        return Err(format!("{flag} must name a regular file"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_CONFIG_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| unreadable())?;
    if bytes.len() as u64 > MAX_CONFIG_BYTES {
        return Err(format!("{flag} file exceeds {MAX_CONFIG_BYTES} bytes"));
    }
    String::from_utf8(bytes).map_err(|_| format!("{flag} file is not UTF-8"))
}
