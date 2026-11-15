//! The only place where the disk is changed. Every mutation is one entry, named by its names
//! below the fence, and is checked again on disk right before the system call: strictly inside
//! the work root, no link on the way, the entry still what it was. Nothing here is recursive
//! by itself; a tree goes away entry by entry, each one through the same checks.

use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use crate::fence::{Fence, Kind, same_names};
use crate::prepare::{Act, Job};
use crate::reason::{Reason, Stop};
use crate::report::Status;
use crate::survey::names;

/// A change about to be made, as told to a [`Probe`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mutation {
    CreateDir(PathBuf),
    WriteFile(PathBuf),
    Rename {
        from: PathBuf,
        to: PathBuf,
    },
    /// One entry: a file, a link or an empty folder.
    Remove(PathBuf),
}

/// Told about each mutation before its last check. An error stops the execution there, as a
/// failure of the operating system would. This is how a test fails a batch in the middle, or
/// changes the disk between the first check and the mutation.
pub trait Probe {
    fn before(&mut self, mutation: &Mutation) -> io::Result<()>;
}

pub(crate) struct Disk<'a> {
    pub fence: &'a Fence,
    /// The work root: every mutation is strictly below it.
    pub floor: &'a [String],
    pub probe: Option<&'a mut (dyn Probe + 'static)>,
    pub max_depth: u32,
}

impl Disk<'_> {
    fn guard(&self, rel: &[String]) -> Result<(), Stop> {
        let floor = self.floor;
        let below = rel.len() > floor.len() && same_names(&rel[..floor.len()], floor);
        if !below {
            return Err(Stop::new(
                Reason::OutsideWorkRoot,
                format!(
                    "{} is not strictly inside the work root",
                    self.fence.text_of(rel)
                ),
            ));
        }
        Ok(())
    }

    fn announce(&mut self, mutation: Mutation) -> Result<(), Stop> {
        match &mut self.probe {
            Some(probe) => probe.before(&mutation).map_err(|error| {
                Stop::new(Reason::Io, format!("stopped before {mutation:?}: {error}"))
            }),
            None => Ok(()),
        }
    }

    /// Makes the folder `rel`, which must not exist.
    fn create_dir(&mut self, rel: &[String]) -> Result<(), Stop> {
        self.guard(rel)?;
        self.announce(Mutation::CreateDir(self.fence.path_of(rel)))?;
        let (path, found) = self.fence.inspect(rel)?;
        if found.is_some() {
            return Err(Stop::new(
                Reason::DestinationTaken,
                format!("{} already exists", path.display()),
            ));
        }
        fs::create_dir(&path).map_err(|error| match error.kind() {
            io::ErrorKind::AlreadyExists => Stop::new(
                Reason::DestinationTaken,
                format!("{} already exists", path.display()),
            ),
            _ => Stop::io("creating", &path, &error),
        })
    }

    /// Makes the folder `rel` when it is missing; anything there that is not a plain folder
    /// refuses.
    fn ensure_dir(&mut self, rel: &[String]) -> Result<(), Stop> {
        self.guard(rel)?;
        match self.fence.inspect(rel)? {
            (_, Some(Kind::Dir)) => Ok(()),
            (_, None) => self.create_dir(rel),
            (path, Some(Kind::File)) => Err(Stop::new(
                Reason::NotADirectory,
                format!("{} is not a folder", path.display()),
            )),
            (path, Some(Kind::Link { .. })) => Err(Stop::new(
                Reason::LinkInPath,
                format!("{} is a link", path.display()),
            )),
        }
    }

    /// Writes the new file `rel`; an existing entry is never opened.
    fn write_new(&mut self, rel: &[String], bytes: &[u8]) -> Result<(), Stop> {
        self.guard(rel)?;
        self.announce(Mutation::WriteFile(self.fence.path_of(rel)))?;
        let (path, found) = self.fence.inspect(rel)?;
        if found.is_some() {
            return Err(Stop::new(
                Reason::DestinationTaken,
                format!("{} already exists", path.display()),
            ));
        }
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| Stop::io("creating", &path, &error))?;
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|error| Stop::io("writing", &path, &error))
    }

    /// Moves the entry `from`, still of `kind`, to the vacant place `to`.
    fn rename(
        &mut self,
        from: &[String],
        kind: Kind,
        to: &[String],
        taken: Reason,
    ) -> Result<(), Stop> {
        self.guard(from)?;
        self.guard(to)?;
        self.announce(Mutation::Rename {
            from: self.fence.path_of(from),
            to: self.fence.path_of(to),
        })?;
        let (from_path, found) = self.fence.inspect(from)?;
        if found != Some(kind) {
            return Err(Stop::new(
                Reason::Changed,
                format!("{} is not what was checked", from_path.display()),
            ));
        }
        let (to_path, found) = self.fence.inspect(to)?;
        if found.is_some() {
            return Err(Stop::new(taken, format!("{} exists", to_path.display())));
        }
        fs::rename(&from_path, &to_path).map_err(|error| Stop::io("moving", &from_path, &error))
    }

    /// Removes the one entry `rel`, still of `kind`. A link goes away as a link: what it
    /// points to is never opened. A folder has to be empty.
    fn remove_entry(&mut self, rel: &[String], kind: Kind) -> Result<(), Stop> {
        self.guard(rel)?;
        self.announce(Mutation::Remove(self.fence.path_of(rel)))?;
        let (path, found) = self.fence.inspect(rel)?;
        if found != Some(kind) {
            return Err(Stop::new(
                Reason::Changed,
                format!("{} is not what was checked", path.display()),
            ));
        }
        let removed = match kind {
            Kind::File | Kind::Link { dir: false } => fs::remove_file(&path),
            Kind::Dir | Kind::Link { dir: true } => fs::remove_dir(&path),
        };
        removed.map_err(|error| Stop::io("removing", &path, &error))
    }

    /// Removes the tree at `rel`, the deepest entries first. Every entry is looked at again
    /// from the fence before it goes; a link is removed and never entered.
    fn remove_tree(&mut self, rel: &[String], depth: u32, removed: &mut u64) -> Result<(), Stop> {
        self.guard(rel)?;
        let (path, kind) = self.fence.existing(rel)?;
        if kind == Kind::Dir {
            if depth >= self.max_depth {
                return Err(Stop::new(
                    Reason::TooDeep,
                    format!(
                        "{} is deeper than {} levels",
                        path.display(),
                        self.max_depth
                    ),
                ));
            }
            for name in names(&path)? {
                let mut child = rel.to_vec();
                child.push(name);
                self.remove_tree(&child, depth + 1, removed)?;
            }
        }
        self.remove_entry(rel, kind)?;
        *removed += 1;
        Ok(())
    }

    /// Runs the job, stopping at the first refusal. The steps of the job say what was done.
    pub(crate) fn run(&mut self, job: &mut Job) -> Result<(), Stop> {
        for rel in &job.ensure {
            self.ensure_dir(rel)?;
        }
        for rel in &job.reserve {
            self.create_dir(rel)?;
        }
        if let Some((rel, manifest)) = &job.manifest {
            self.write_new(rel, &manifest.to_bytes())?;
        }
        for (step, op) in job.steps.iter_mut().zip(&job.ops) {
            let Some(op) = op else { continue };
            let mut removed = 0;
            let outcome = match &op.act {
                Act::MoveTo { to, floor, taken } => (floor + 1..to.len())
                    .try_for_each(|depth| self.ensure_dir(&to[..depth]))
                    .and_then(|()| self.rename(&op.rel, op.kind, to, *taken)),
                Act::Remove { recursive: false } => self.remove_entry(&op.rel, op.kind),
                Act::Remove { recursive: true } => match self.fence.existing(&op.rel) {
                    Ok((_, kind)) if kind == op.kind => self.remove_tree(&op.rel, 0, &mut removed),
                    Ok((path, _)) => Err(Stop::new(
                        Reason::Changed,
                        format!("{} is not what was checked", path.display()),
                    )),
                    Err(stop) => Err(stop),
                },
            };
            match outcome {
                Ok(()) => step.status = Status::Done,
                Err(stop) => {
                    step.status = Status::Failed {
                        reason: stop.reason,
                        detail: stop.detail.clone(),
                        removed,
                    };
                    return Err(stop);
                }
            }
        }
        Ok(())
    }
}
