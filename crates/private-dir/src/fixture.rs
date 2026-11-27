//! Directory handles for explicit, non-recursive test fixture cleanup.

use std::{io, path::Path, sync::Arc};

use super::Kind;
#[cfg(unix)]
use super::unix as platform;
#[cfg(windows)]
use super::win as platform;

#[cfg(unix)]
type Handle = std::os::fd::OwnedFd;
#[cfg(windows)]
type Handle = std::fs::File;

/// A test directory whose children are opened and removed relative to its handle.
pub struct FixtureDir(Arc<OpenDir>);

struct OpenDir {
    handle: Handle,
    parent: Option<(Arc<OpenDir>, String)>,
}

impl FixtureDir {
    /// Opens a directory for comparison or as the parent of removable directories.
    pub fn open(path: &Path) -> io::Result<Self> {
        #[cfg(unix)]
        let handle = platform::open_dir(path)?;
        #[cfg(windows)]
        let handle = platform::open_fixture_dir(path)?;
        Self::from_handle(handle, None)
    }

    fn from_handle(handle: Handle, parent: Option<(Arc<OpenDir>, String)>) -> io::Result<Self> {
        #[cfg(unix)]
        let kind = platform::kind(&platform::stat(&handle)?);
        #[cfg(windows)]
        let kind = platform::kind(&handle)?;
        if kind != Kind::Dir {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "expected a directory without a link or reparse point",
            ));
        }
        Ok(Self(Arc::new(OpenDir { handle, parent })))
    }

    /// Opens one child by name, without following a link.
    pub fn child(&self, name: &str) -> io::Result<Self> {
        check_name(name)?;
        let handle = platform::open_child_dir(&self.0.handle, name)?;
        Self::from_handle(handle, Some((Arc::clone(&self.0), name.to_owned())))
    }

    pub fn same_directory(&self, other: &Self) -> io::Result<bool> {
        let (left, right) = (&self.0.handle, &other.0.handle);
        #[cfg(unix)]
        return Ok(platform::same(
            &platform::stat(left)?,
            &platform::stat(right)?,
        ));
        #[cfg(windows)]
        Ok(platform::file_id(left)? == platform::file_id(right)?)
    }

    /// Removes one named file from this directory, without following a link.
    pub fn remove_file(&self, name: &str) -> io::Result<()> {
        check_name(name)?;
        #[cfg(unix)]
        {
            self.verify_location()?;
            platform::remove(&self.0.handle, name)
        }
        #[cfg(windows)]
        platform::remove_fixture_file(&self.0.handle, name)
    }

    /// Removes this empty child directory. The starting directory cannot be removed.
    pub fn remove(self) -> io::Result<()> {
        let (parent, name) = self.0.parent.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "cannot remove the starting directory",
            )
        })?;
        #[cfg(unix)]
        {
            self.verify_location()?;
            platform::remove_child_dir(&parent.handle, name, &self.0.handle)
        }
        #[cfg(windows)]
        {
            let _ = (parent, name);
            platform::remove_fixture_entry(&self.0.handle)
        }
    }

    #[cfg(unix)]
    fn verify_location(&self) -> io::Result<()> {
        let mut current = self.0.as_ref();
        while let Some((parent, name)) = &current.parent {
            let opened = platform::stat(&current.handle)?;
            let linked = platform::stat_at(&parent.handle, name);
            match linked {
                Ok(stat) if platform::same(&opened, &stat) => {}
                Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
                _ => return Err(io::Error::other("scratch folder identity changed")),
            }
            current = parent;
        }
        Ok(())
    }
}

fn check_name(name: &str) -> io::Result<()> {
    if name.is_empty() || matches!(name, "." | "..") || name.contains(['/', '\\', ':', '\0']) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid fixture name",
        ));
    }
    Ok(())
}
