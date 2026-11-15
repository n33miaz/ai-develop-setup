//! Stable process that holds terminal sessions and the replay ring.

use std::error::Error;
use std::process::ExitCode;
use std::sync::Arc;

use ads_ptyhost::channel::{self, Listener};
use ads_ptyhost::{HostConfig, PortablePtyBackend, PtyHost, server};

fn banner() -> String {
    format!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
}

/// The shipped defaults, with the keys of the file given by `--config <file>` on top.
fn load_config(mut args: impl Iterator<Item = String>) -> Result<HostConfig, Box<dyn Error>> {
    let mut overrides = String::new();
    while let Some(arg) = args.next() {
        match (arg.as_str(), args.next()) {
            ("--config", Some(path)) => overrides = std::fs::read_to_string(path)?,
            _ => return Err("usage: ads-ptyhost [--config <file>]".into()),
        }
    }
    Ok(HostConfig::from_toml_str(&overrides)?)
}

fn run() -> Result<(), Box<dyn Error>> {
    let config = load_config(std::env::args().skip(1))?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()?;
    runtime.block_on(async {
        let listener = Listener::bind(&channel::endpoint(&config)?)?;
        let host = Arc::new(PtyHost::new(Arc::new(PortablePtyBackend), config)?);
        // One line, so whoever started the process knows where to connect and that it is ready.
        println!("{} listening on {}", banner(), listener.endpoint());
        server::serve(host, listener).await?;
        Ok(())
    })
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{}: {err}", env!("CARGO_PKG_NAME"));
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> impl Iterator<Item = String> {
        list.iter()
            .map(|arg| (*arg).to_owned())
            .collect::<Vec<_>>()
            .into_iter()
    }

    #[test]
    fn banner_names_the_package() {
        assert!(banner().starts_with("ads-ptyhost "));
    }

    #[test]
    fn no_arguments_means_the_shipped_defaults() {
        assert_eq!(load_config(args(&[])).unwrap(), HostConfig::defaults());
    }

    #[test]
    fn unknown_or_incomplete_arguments_are_refused() {
        assert!(load_config(args(&["--verbose"])).is_err());
        assert!(load_config(args(&["--config"])).is_err());
    }
}
