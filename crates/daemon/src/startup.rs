//! Leitura dos argumentos e dos arquivos de configuração na partida do daemon.

use crate::api::ApiConfig;
use crate::tool_launcher::Workdir;
use ads_core::gate::AbsPath;
use ads_ptyhost::HostConfig;
use std::fs::{File, OpenOptions, TryLockError};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

/// The lock of one installation, held while the daemon runs. Its folder is the launcher
/// control folder, product state that the session gate protects
/// ([`crate::audit::open_product`]).
#[derive(Debug)]
pub struct InstanceLock {
    _file: File,
    folder: PathBuf,
}

impl InstanceLock {
    /// Opens or creates the lock file at the absolute `path` and takes it, or fails when
    /// another daemon holds it.
    pub fn acquire(path: &Path) -> io::Result<Self> {
        // The lock is created without an explicit owner, so an elevated session would give it
        // the Administrators group as owner.
        crate::audit::check_session()
            .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error.to_string()))?;
        let folder = path
            .parent()
            .filter(|_| path.is_absolute())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "instance lock path must be absolute",
                )
            })?
            .to_path_buf();
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        // Only the user may open the lock: another account could otherwise take it and keep
        // every daemon of the installation from starting. The launcher refuses a wider one.
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        let file = options.open(path)?;
        file.try_lock().map_err(|error| match error {
            TryLockError::WouldBlock => io::Error::new(
                io::ErrorKind::AlreadyExists,
                "another daemon is already running for this installation",
            ),
            TryLockError::Error(error) => error,
        })?;
        Ok(Self {
            _file: file,
            folder,
        })
    }

    /// The folder that holds the lock file.
    pub fn folder(&self) -> &Path {
        &self.folder
    }
}

/// Largest configuration file read. Fixed because it guards the reading of the file that would
/// hold the setting; real files are a few kilobytes.
pub const MAX_CONFIG_BYTES: u64 = 1024 * 1024;

/// The API settings, the host settings and the tools' part of the user file (`[env]`,
/// `[workdirs]`, `[tools]`, `[accounts]`), still as TOML for the adapters.
pub type StartupConfig = (
    ApiConfig,
    HostConfig,
    String,
    Result<ads_storage::StorageConfig, ads_storage::StorageError>,
);

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
    let (api, tools, storage) = split_config(&overrides)?;
    Ok((api, host, tools, storage))
}

type StorageSettings = Result<ads_storage::StorageConfig, ads_storage::StorageError>;
type SplitConfig = (ApiConfig, String, StorageSettings);

fn split_config(overrides: &str) -> Result<SplitConfig, String> {
    let mut api: toml::Table =
        toml::from_str(overrides).map_err(|error: toml::de::Error| match error.span() {
            // Only the position: the parser message could quote the file.
            Some(span) => format!(
                "invalid user configuration TOML at line {}",
                overrides[..span.start].matches('\n').count() + 1
            ),
            None => "invalid user configuration TOML".to_owned(),
        })?;
    let storage = match api.remove("storage") {
        Some(toml::Value::Table(table)) => {
            ads_storage::StorageConfig::from_toml_str(&table.to_string())
        }
        Some(_) => Err(ads_storage::StorageError::Configuration),
        None => Ok(ads_storage::StorageConfig::defaults()),
    };
    let mut tools = toml::Table::new();
    for name in ["env", "workdirs", "tools", "accounts"] {
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
    if let Some(toml::Value::Table(entries)) = tools.get("accounts")
        && entries.keys().any(|name| {
            !matches!(
                name.as_str(),
                ads_adapter_claude::HARNESS_ID | ads_adapter_codex::HARNESS_ID
            )
        })
    {
        return Err("unsupported account profile tool in user configuration".into());
    }
    let api =
        ApiConfig::from_toml_str(&api.to_string()).map_err(|_| "invalid daemon configuration")?;
    Ok((api, tools.to_string(), storage))
}

/// Attaches the local persistent controls when startup succeeds.
pub fn attach_product_controls(
    api: crate::api::LocalApi,
    storage: StorageSettings,
    instance: Option<&InstanceLock>,
    workdirs: &[Workdir],
    max_list_bytes: usize,
) -> crate::api::LocalApi {
    attach_controls(api, workdirs, max_list_bytes, |paths| {
        crate::audit::open_local_product(&storage?, instance, paths)
    })
}

pub(crate) fn attach_controls(
    api: crate::api::LocalApi,
    workdirs: &[Workdir],
    max_list_bytes: usize,
    open: impl FnOnce(
        &[PathBuf],
    ) -> Result<
        (crate::audit::ProductLog, crate::product::LocalIdentities),
        crate::product::StartupError,
    >,
) -> crate::api::LocalApi {
    let controls = (|| {
        let paths: Vec<_> = workdirs.iter().map(|dir| dir.path.clone()).collect();
        let (log, ids) = open(&paths)?;
        crate::product::controls(log, ids, workdirs, max_list_bytes)
    })();
    match controls {
        Ok((queue, approvals)) => api.with_queue(queue).with_approvals(approvals),
        Err(error) => {
            eprintln!("ads-daemon: queue and approvals unavailable: {error}");
            api
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use ads_core::tool_launch::HostRules;

    #[test]
    fn account_tables_reach_the_corresponding_adapter() {
        let directory = if cfg!(windows) {
            r"C:\profiles\primary"
        } else {
            "/profiles/primary"
        };
        let (_, tools, _) = split_config(&format!(
            "[accounts.claude.primary]\nprofile_dir = '{directory}'"
        ))
        .unwrap();
        let config =
            ads_adapter_claude::ClaudeConfig::from_toml_str(&tools, HostRules::host()).unwrap();
        let alias = ads_contracts::CommonIdV1::new("primary").unwrap();
        assert_eq!(
            config.profile(&alias).unwrap().directory().as_str(),
            directory
        );
    }

    #[cfg(unix)]
    #[test]
    fn creates_the_instance_lock_for_the_user_only() {
        use std::os::unix::fs::PermissionsExt;
        let folder = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target")
            .join(format!("daemon-lock-test-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("daemon.lock");
        let _ = std::fs::remove_file(&path);
        let lock = InstanceLock::acquire(&path).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        drop(lock);
    }

    #[test]
    fn the_instance_lock_is_not_created_in_a_session_that_creates_as_administrators() {
        let folder = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target")
            .join(format!("daemon-elevated-lock-test-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("daemon.lock");
        let _ = std::fs::remove_file(&path);
        let user = ads_private_dir::Identity::current().unwrap().user;
        crate::audit::SESSION.with(|session| {
            *session.borrow_mut() = Some(ads_private_dir::Identity {
                user,
                default_owner: "S-1-5-32-544".into(),
            });
        });
        let refused = InstanceLock::acquire(&path);
        crate::audit::SESSION.with(|session| session.borrow_mut().take());
        let error = refused.unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert!(
            error
                .to_string()
                .contains(ads_private_dir::ELEVATED_SESSION)
        );
        assert!(!path.exists());
        let _ = std::fs::remove_dir(&folder);
    }

    #[test]
    fn an_unknown_tool_is_refused_in_account_and_tool_tables() {
        assert!(
            split_config("[accounts.antigravity.primary]\nprofile_dir = '/profiles/primary'")
                .is_err()
        );
        assert!(split_config("[tools.antigravity]\nswitch = 'manual'").is_err());
    }
}
