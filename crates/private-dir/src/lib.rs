//! Keeps the control folder and its files reachable only by the current user.
//!
//! The folder is opened once, checked on that open handle, and held: every later operation
//! (the lock, the state, the signals) is relative to the handle, never to the path again. How
//! the path was resolved therefore does not matter: a link or junction anywhere on it only
//! changes which folder was opened, and that folder is the one checked and used. On Windows
//! the handle also keeps the folder, and every folder above it, from being renamed while the
//! launcher runs. Each file is checked on its own handle too, and a signal is removed through
//! the handle it was checked on.

#[cfg(unix)]
#[path = "platform/unix.rs"]
mod unix;
#[cfg(windows)]
#[allow(unsafe_code)]
#[path = "platform/win.rs"]
mod win;

use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

/// Who runs the launcher. On Windows, security identifiers in text form; on Unix, the
/// effective user id in decimal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    pub user: String,
    /// The owner this user's new objects get (Administrators in an elevated Windows session).
    pub default_owner: String,
}

impl Identity {
    pub fn current() -> io::Result<Self> {
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

    pub fn owns(&self, owner: &str) -> bool {
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
    check_windows_private(local, security, false)
}

#[cfg_attr(not(windows), allow(dead_code))]
fn check_windows_private(
    local: &Identity,
    security: &Security,
    strict: bool,
) -> Result<(), &'static str> {
    if (strict && local.user != security.owner) || !local.owns(&security.owner) {
        return Err("is owned by another account");
    }
    let Some(entries) = &security.dacl else {
        return Err("has no access list, so every account can use it");
    };
    for entry in entries {
        match entry {
            Entry::Deny => {}
            Entry::Allow(sid)
                if sid == &local.user
                    || sid == MACHINE_TRUSTED[0]
                    || (!strict && (local.owns(sid) || sid == MACHINE_TRUSTED[1])) => {}
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

/// Whether this path names a Windows network share or a mapped network drive.
pub fn is_network_path(_path: &Path) -> bool {
    #[cfg(windows)]
    return win::is_network_path(_path);
    #[cfg(not(windows))]
    false
}

/// What an open entry is. A link or reparse point is `Other`, never what it points to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Dir,
    File,
    Other,
}

/// What was found under the name of a signal.
#[derive(Debug, PartialEq, Eq)]
pub enum Signal {
    Absent,
    Accepted,
    Stale,
    /// A file another account owns or can use: left in place and never honored.
    Foreign,
}

/// Creates the folder `path` (not its parents) readable and writable only by `local`.
pub fn create_dir(path: &Path, local: &Identity) -> io::Result<()> {
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

fn refusal(what: &str, path: &Path, reason: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        format!(
            "control {what} {} {reason}; remove it so the launcher recreates it, or restrict it \
             to your account only",
            path.display()
        ),
    )
}

#[cfg(unix)]
fn is_loop(error: &io::Error) -> bool {
    error.raw_os_error() == Some(rustix::io::Errno::LOOP.raw_os_error())
}

/// The control folder, open and held for as long as the value lives.
pub struct ControlDir {
    #[cfg(windows)]
    dir: File,
    #[cfg(unix)]
    dir: std::os::fd::OwnedFd,
    path: PathBuf,
    local: Identity,
}

impl ControlDir {
    /// Creates the folder privately when missing, opens it and refuses it unless only `local`
    /// can reach it.
    pub fn open(path: &Path, local: &Identity) -> io::Result<Self> {
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
        Self::checked(path, local, false)
    }

    /// Holds a directory owned by the current user, permitting only that user and SYSTEM.
    pub fn open_user_owned(path: &Path, create: bool) -> io::Result<Self> {
        let local = Identity::current()?;
        if create {
            match create_dir(path, &local) {
                Err(error) if error.kind() != io::ErrorKind::AlreadyExists => return Err(error),
                _ => {}
            }
        }
        Self::checked(path, &local, true)
    }

    fn checked(path: &Path, local: &Identity, _strict: bool) -> io::Result<Self> {
        let refuse = |reason| refusal("folder", path, reason);
        #[cfg(windows)]
        let dir = {
            let dir = win::open_dir(path)?;
            if win::kind(&dir)? != Kind::Dir {
                return Err(refuse("is not a plain folder"));
            }
            check_windows_private(local, &win::security(&dir)?, _strict).map_err(refuse)?;
            dir
        };
        #[cfg(unix)]
        let dir = {
            // With `O_DIRECTORY`, Linux reports a link as `ENOTDIR`, not `ELOOP`.
            let dir = unix::open_dir(path).map_err(|error| {
                if is_loop(&error)
                    || error.raw_os_error() == Some(rustix::io::Errno::NOTDIR.raw_os_error())
                {
                    refuse("is not a plain folder")
                } else {
                    error
                }
            })?;
            let stat = unix::stat(&dir)?;
            if unix::kind(&stat) != Kind::Dir {
                return Err(refuse("is not a plain folder"));
            }
            unix_private(local, stat.st_uid, stat.st_mode).map_err(refuse)?;
            dir
        };
        Ok(Self {
            dir,
            path: path.to_path_buf(),
            local: local.clone(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Requires an existing file to belong to the current user and grant access only to
    /// that user and SYSTEM. An absent entry is allowed. The name is relative to this folder.
    #[cfg(windows)]
    pub fn check_user_owned_file_if_present(&self, name: &str) -> io::Result<()> {
        let refuse = |reason| refusal("file", &self.path.join(name), reason);
        if name.is_empty() || matches!(name, "." | "..") || name.contains(['/', '\\', ':', '\0']) {
            return Err(refuse("does not have a plain file name"));
        }
        let file = match win::open_for_inspection(&self.dir, name) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        };
        if win::kind(&file)? != Kind::File {
            return Err(refuse("is not a plain file"));
        }
        check_windows_private(&self.local, &win::security(&file)?, true).map_err(refuse)
    }

    /// The same folder, with its files judged as `local`'s.
    #[cfg(feature = "test-support")]
    pub fn judged_as(self, local: Identity) -> Self {
        Self { local, ..self }
    }

    /// Opens the lock file `name`, created private when missing; an existing one is refused
    /// unless it is a plain file that only the user can reach.
    pub fn lock_file(&self, name: &str) -> io::Result<File> {
        let refuse = |reason| refusal("file", &self.path.join(name), reason);
        #[cfg(windows)]
        {
            let file = win::open_or_create(&self.dir, name, &self.local)?;
            if win::kind(&file)? != Kind::File {
                return Err(refuse("is not a plain file"));
            }
            windows_private(&self.local, &win::security(&file)?).map_err(refuse)?;
            Ok(file)
        }
        #[cfg(unix)]
        {
            let file = unix::open_or_create(&self.dir, name).map_err(|error| {
                if is_loop(&error) {
                    refuse("is not a plain file")
                } else {
                    error
                }
            })?;
            let stat = unix::stat_file(&file)?;
            if unix::kind(&stat) != Kind::File {
                return Err(refuse("is not a plain file"));
            }
            unix_private(&self.local, stat.st_uid, stat.st_mode).map_err(refuse)?;
            Ok(file)
        }
    }

    /// Replaces the file `name` with `bytes` at once: a new private file is written, then
    /// moved over it.
    pub fn publish(&self, name: &str, bytes: &[u8]) -> io::Result<()> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let temp = format!("{name}.{}-{stamp}.tmp", std::process::id());
        #[cfg(windows)]
        {
            let mut file = win::create_new(&self.dir, &temp, &self.local)?;
            let result = file
                .write_all(bytes)
                .and_then(|()| file.sync_all())
                .and_then(|()| win::rename(&file, &self.dir, name));
            if result.is_err() {
                let _ = win::remove(&file);
            }
            result
        }
        #[cfg(unix)]
        {
            let mut file = unix::create_new(&self.dir, &temp)?;
            let result = file.write_all(bytes).and_then(|()| file.sync_all());
            drop(file);
            let result = result.and_then(|()| unix::rename(&self.dir, &temp, name));
            if result.is_err() {
                let _ = unix::remove(&self.dir, &temp);
            }
            result
        }
    }

    /// Looks at the signal `name` and, when it is the user's own and `consume` is set,
    /// removes it: the entry removed is the one checked.
    pub fn signal(&self, name: &str, consume: bool) -> io::Result<Signal> {
        self.signal_with(name, consume, |_| Ok(Signal::Accepted))
    }

    /// Looks at the signal `name` against a point in time: a request dated (created or last
    /// changed) before `stale_before` is `Stale` and removed; a newer one is `Accepted` and
    /// removed, or left in place as `Absent` while `keep_recent` is set.
    pub fn dated_signal(
        &self,
        name: &str,
        stale_before: SystemTime,
        keep_recent: bool,
    ) -> io::Result<Signal> {
        self.signal_with(name, true, |file| {
            let metadata = file.metadata()?;
            let modified = metadata.modified()?;
            let last_request = modified.max(metadata.created().unwrap_or(modified));
            Ok(if last_request < stale_before {
                Signal::Stale
            } else if keep_recent {
                Signal::Absent
            } else {
                Signal::Accepted
            })
        })
    }

    fn signal_with(
        &self,
        name: &str,
        consume: bool,
        classify: impl FnOnce(&File) -> io::Result<Signal>,
    ) -> io::Result<Signal> {
        #[cfg(windows)]
        {
            let file = match win::open_to_remove(&self.dir, name) {
                Ok(file) => file,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    return Ok(Signal::Absent);
                }
                // The user cannot even read who owns it: another account's.
                Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                    return Ok(Signal::Foreign);
                }
                Err(error) => return Err(error),
            };
            if win::kind(&file)? != Kind::File {
                return Ok(Signal::Absent);
            }
            if windows_private(&self.local, &win::security(&file)?).is_err() {
                return Ok(Signal::Foreign);
            }
            let signal = classify(&file)?;
            if consume && matches!(signal, Signal::Accepted | Signal::Stale) {
                before_remove();
                win::remove(&file)?;
            }
            Ok(signal)
        }
        #[cfg(unix)]
        {
            let owns = |stat: &rustix::fs::Stat| self.local.owns(&stat.st_uid.to_string());
            let entry = match unix::open_entry(&self.dir, name) {
                Ok(entry) => entry,
                Err(error) if error.kind() == io::ErrorKind::NotFound || is_loop(&error) => {
                    return Ok(Signal::Absent);
                }
                Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                    let stat = unix::stat_at(&self.dir, name)?;
                    return if owns(&stat) {
                        Err(error)
                    } else {
                        Ok(Signal::Foreign)
                    };
                }
                Err(error) => return Err(error),
            };
            let checked = unix::stat(&entry)?;
            if unix::kind(&checked) != Kind::File {
                return Ok(Signal::Absent);
            }
            if !owns(&checked) {
                return Ok(Signal::Foreign);
            }
            let file = File::from(entry);
            let signal = classify(&file)?;
            if consume && matches!(signal, Signal::Accepted | Signal::Stale) {
                before_remove();
                // Only the user can change the entries of the private folder held open; the
                // comparison still makes sure the name removed is the file checked.
                if !unix::same(&checked, &unix::stat_at(&self.dir, name)?) {
                    return Err(io::Error::other(
                        "the signal changed while it was checked; left in place",
                    ));
                }
                unix::remove(&self.dir, name)?;
            }
            Ok(signal)
        }
    }
}

#[cfg(any(test, feature = "test-support"))]
thread_local! {
    /// Runs once between the check of a signal and its removal, to stand in for a race.
    pub static BEFORE_REMOVE: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
        const { std::cell::RefCell::new(None) };
}

fn before_remove() {
    #[cfg(any(test, feature = "test-support"))]
    if let Some(hook) = BEFORE_REMOVE.with(|hook| hook.borrow_mut().take()) {
        hook();
    }
}

/// The access list of `path` in text form, protection and inheritance flags included.
#[cfg(all(windows, any(test, feature = "test-support")))]
pub fn access_list_text(path: &Path) -> io::Result<String> {
    win::access_list_text(path)
}

/// `text` in the form `access_list_text` returns, with the SIDs Windows abbreviates replaced.
#[cfg(all(windows, any(test, feature = "test-support")))]
pub fn canonical_access_list(text: &str) -> io::Result<String> {
    win::canonical_access_list(text)
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

    /// Windows writes some SIDs back as aliases (the built-in administrator as `LA`), so the
    /// expected lists are compared in the form it writes.
    #[cfg(windows)]
    #[test]
    fn canonical_access_list_uses_the_aliases_windows_writes() {
        let text = |sid: &str| canonical_access_list(&format!("D:P(A;;FA;;;{sid})")).unwrap();
        assert_eq!(text("S-1-5-18"), "D:P(A;;FA;;;SY)");
        assert_eq!(text("S-1-5-32-544"), "D:P(A;;FA;;;BA)");
        assert_eq!(text(USER), format!("D:P(A;;FA;;;{USER})"));
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
    fn strict_windows_decision_rejects_administrators_access_and_ownership() {
        let admins = "S-1-5-32-544";
        let elevated = local(USER, admins);
        for security in [
            security(
                USER,
                &[Entry::Allow(USER.into()), Entry::Allow(admins.into())],
            ),
            security(admins, &[Entry::Allow(USER.into())]),
        ] {
            assert!(windows_private(&elevated, &security).is_ok());
            assert!(check_windows_private(&elevated, &security, true).is_err());
        }
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
