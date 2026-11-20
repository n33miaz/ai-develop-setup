//! Keeps the control folder and its files reachable only by the current user.

#[cfg(windows)]
#[allow(unsafe_code)]
mod win;

use std::{
    fs::{self, OpenOptions},
    io,
    path::Path,
};

/// Who runs the launcher. On Windows, security identifiers in text form; on Unix, the
/// effective user id in decimal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Identity {
    pub(crate) user: String,
    /// The owner this user's new objects get (Administrators in an elevated Windows session).
    pub(crate) default_owner: String,
}

impl Identity {
    pub(crate) fn current() -> io::Result<Self> {
        #[cfg(windows)]
        {
            win::current_identity()
        }
        #[cfg(unix)]
        {
            let uid = rustix::process::geteuid().as_raw().to_string();
            Ok(Self {
                user: uid.clone(),
                default_owner: uid,
            })
        }
    }

    pub(crate) fn owns(&self, owner: &str) -> bool {
        !self.user.is_empty() && (owner == self.user || owner == self.default_owner)
    }
}

/// One entry of a Windows access list, reduced to what the decision needs.
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Clone, Debug)]
pub(crate) enum Entry {
    Allow(String),
    Deny,
    /// Any other entry type: refused, since its effect is not inspected.
    Other,
}

/// Owner and access list of a Windows object; `dacl: None` is a null list, open to everyone.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) struct Security {
    pub(crate) owner: String,
    pub(crate) dacl: Option<Vec<Entry>>,
}

/// Accounts that can already take over any file of the machine: local system and the
/// Administrators group.
#[cfg_attr(not(windows), allow(dead_code))]
const MACHINE_TRUSTED: [&str; 2] = ["S-1-5-18", "S-1-5-32-544"];

/// Accepts a Windows folder only if the user owns it and no other account has any access.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn windows_private(local: &Identity, security: &Security) -> Result<(), &'static str> {
    if !local.owns(&security.owner) {
        return Err("is owned by another account");
    }
    let Some(entries) = &security.dacl else {
        return Err("has no access list, so every account can use it");
    };
    for entry in entries {
        match entry {
            Entry::Deny => {}
            Entry::Allow(sid) if local.owns(sid) || MACHINE_TRUSTED.contains(&sid.as_str()) => {}
            Entry::Allow(_) => return Err("grants access to another account"),
            Entry::Other => return Err("has an access entry of an unsupported type"),
        }
    }
    Ok(())
}

/// Accepts a Unix folder only if the user owns it and group and others have no access.
#[cfg_attr(not(unix), allow(dead_code))]
pub(crate) fn unix_private(local: &Identity, owner: u32, mode: u32) -> Result<(), &'static str> {
    if !local.owns(&owner.to_string()) {
        return Err("is owned by another account");
    }
    if mode & 0o077 != 0 {
        return Err("grants access to another account");
    }
    Ok(())
}

/// Creates the folder `path` (not its parents) readable and writable only by `local`.
pub(crate) fn create_dir(path: &Path, local: &Identity) -> io::Result<()> {
    #[cfg(windows)]
    {
        win::create_dir(path, local)
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        let _ = local;
        fs::DirBuilder::new().mode(0o700).create(path)
    }
}

/// Creates the control folder privately when missing, then refuses it unless only `local`
/// can reach it.
pub(crate) fn ensure_dir(path: &Path, local: &Identity) -> io::Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    match create_dir(path, local) {
        Err(error) if error.kind() != io::ErrorKind::AlreadyExists => return Err(error),
        _ => {}
    }
    check_dir(path, local)
}

fn check_dir(path: &Path, local: &Identity) -> io::Result<()> {
    let refuse = |reason: &str| {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "control folder {} {reason}; remove it so the launcher recreates it, or \
                 restrict it to your account only",
                path.display()
            ),
        )
    };
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() {
        return Err(refuse("is not a plain folder"));
    }
    #[cfg(windows)]
    let decision = windows_private(local, &win::security(path)?);
    #[cfg(unix)]
    let decision = {
        use std::os::unix::fs::MetadataExt;
        unix_private(local, metadata.uid(), metadata.mode())
    };
    decision.map_err(refuse)
}

/// Whether `local` owns the file `path` (the link itself, never its target).
pub(crate) fn owned_by(path: &Path, local: &Identity) -> io::Result<bool> {
    #[cfg(windows)]
    {
        Ok(local.owns(&win::security(path)?.owner))
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(local.owns(&fs::symlink_metadata(path)?.uid().to_string()))
    }
}

#[cfg(all(windows, test))]
pub(crate) fn windows_security(path: &Path) -> io::Result<Security> {
    win::security(path)
}

/// Options for files the launcher creates in the control folder. On Windows they inherit the
/// folder's single entry; on Unix they are created with mode 0600.
pub(crate) fn file_options() -> OpenOptions {
    #[allow(unused_mut)]
    let mut options = OpenOptions::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local(user: &str, default_owner: &str) -> Identity {
        Identity {
            user: user.into(),
            default_owner: default_owner.into(),
        }
    }

    const USER: &str = "S-1-5-21-1-2-3-1001";
    const OTHER: &str = "S-1-5-21-1-2-3-1002";

    fn security(owner: &str, entries: &[Entry]) -> Security {
        Security {
            owner: owner.into(),
            dacl: Some(entries.to_vec()),
        }
    }

    #[test]
    fn windows_decision_accepts_only_the_user_and_machine_accounts() {
        let user = local(USER, USER);
        let private = [
            Entry::Allow(USER.into()),
            Entry::Allow("S-1-5-18".into()),
            Entry::Allow("S-1-5-32-544".into()),
            Entry::Deny,
        ];
        assert!(windows_private(&user, &security(USER, &private)).is_ok());
        for open in ["S-1-1-0", "S-1-5-11", "S-1-5-32-545", OTHER] {
            let entries = [Entry::Allow(USER.into()), Entry::Allow(open.into())];
            assert!(
                windows_private(&user, &security(USER, &entries)).is_err(),
                "{open}"
            );
        }
        let unsupported = [Entry::Allow(USER.into()), Entry::Other];
        assert!(windows_private(&user, &security(USER, &unsupported)).is_err());
        let null = Security {
            owner: USER.into(),
            dacl: None,
        };
        assert!(windows_private(&user, &null).is_err());
    }

    #[test]
    fn windows_decision_checks_the_owner() {
        let entries = [Entry::Allow(USER.into())];
        assert!(windows_private(&local(USER, USER), &security(OTHER, &entries)).is_err());
        // An elevated session gives its new objects the Administrators group as owner.
        let elevated = local(USER, "S-1-5-32-544");
        assert!(windows_private(&elevated, &security("S-1-5-32-544", &entries)).is_ok());
        assert!(windows_private(&local("", ""), &security("", &entries)).is_err());
    }

    #[test]
    fn unix_decision_requires_owner_and_mode_0700() {
        let user = local("1000", "1000");
        assert!(unix_private(&user, 1000, 0o40700).is_ok());
        assert!(unix_private(&user, 1001, 0o40700).is_err());
        for mode in [0o40777, 0o40755, 0o40720, 0o40702] {
            assert!(unix_private(&user, 1000, mode).is_err(), "{mode:o}");
        }
    }
}
