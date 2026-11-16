//! The only place where the disk is changed. Every mutation is one entry, reached again from
//! the fence by open handles right before it is made: strictly inside the work root, no link
//! on the way, and the entry the very object that was approved. The system call then acts on
//! what is open, not on a path, so nothing that is swapped afterwards can send it elsewhere.
//! Nothing here is recursive by itself; a tree goes away entry by entry, each one compared
//! with the inventory.

use std::io;
use std::path::PathBuf;

use crate::fence::{Fence, Kind};
use crate::limits::Limits;
use crate::prepare::{Act, Job, Lay};
use crate::reason::{Reason, Stop};
use crate::report::{Laid, Scaffold, Stage, Staged, Status};
use crate::survey::{Seen, open_listed, same_names, same_object, verify};
use crate::sys::{self, Node};

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
    /// A tree about to be removed entry by entry; each entry is told as a `Remove`.
    RemoveTree(PathBuf),
    /// The entries of a folder about to be confirmed on disk.
    Sync(PathBuf),
}

/// Told about each mutation twice. An error stops the execution there, as a failure of the
/// operating system would. This is how a test fails a batch at any step, or changes the disk
/// around a check.
pub trait Probe {
    /// Before anything of the mutation is opened or checked.
    fn before(&mut self, mutation: &Mutation) -> io::Result<()>;

    /// After the last check, with the handles of the mutation open: the next thing is the
    /// system call. Not told for a `RemoveTree` or a `Sync`.
    fn opened(&mut self, _mutation: &Mutation) -> io::Result<()> {
        Ok(())
    }
}

pub(crate) struct Disk<'a> {
    pub fence: &'a Fence,
    /// The work root: every mutation is strictly below it.
    pub floor: &'a [String],
    pub probe: Option<&'a mut (dyn Probe + 'static)>,
    pub limits: &'a Limits,
    /// The folders made on the way to a destination, as they were made.
    pub laid: Vec<Staged>,
}

fn parent_of(rel: &[String]) -> &[String] {
    &rel[..rel.len().saturating_sub(1)]
}

impl Disk<'_> {
    fn guard(&self, rel: &[String]) -> Result<(), Stop> {
        let floor = self.floor;
        if !(rel.len() > floor.len() && rel.starts_with(floor)) {
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

    fn announce(&mut self, mutation: &Mutation) -> Result<(), Stop> {
        match &mut self.probe {
            Some(probe) => probe.before(mutation).map_err(|error| {
                Stop::new(Reason::Io, format!("stopped before {mutation:?}: {error}"))
            }),
            None => Ok(()),
        }
    }

    fn opened(&mut self, mutation: &Mutation) -> Result<(), Stop> {
        match &mut self.probe {
            Some(probe) => probe.opened(mutation).map_err(|error| {
                Stop::new(Reason::Io, format!("stopped at {mutation:?}: {error}"))
            }),
            None => Ok(()),
        }
    }

    /// Has the names of the open folder at `rel` on disk. Until this returns, the change
    /// made in it does not count as done.
    fn sync(&mut self, node: &Node, rel: &[String]) -> Result<(), Stop> {
        let text = self.fence.text_of(rel);
        let unconfirmed = |error: io::Error| {
            Stop::new(
                Reason::Unconfirmed,
                format!("{text} is not confirmed on disk: {error}"),
            )
        };
        let mutation = Mutation::Sync(self.fence.path_of(rel));
        if let Some(probe) = &mut self.probe {
            probe.before(&mutation).map_err(unconfirmed)?;
        }
        node.sync().map_err(unconfirmed)
    }

    fn taken(&self, rel: &[String], reason: Reason) -> Stop {
        Stop::new(reason, format!("{} exists", self.fence.text_of(rel)))
    }

    /// Makes the folder `rel`, which must not exist.
    fn create_dir(&mut self, rel: &[String]) -> Result<(), Stop> {
        self.guard(rel)?;
        let mutation = Mutation::CreateDir(self.fence.path_of(rel));
        self.announce(&mutation)?;
        let spot = self.fence.locate(rel)?;
        if spot.entry.is_some() {
            return Err(self.taken(rel, Reason::DestinationTaken));
        }
        self.opened(&mutation)?;
        match spot.parent.make_dir(&spot.name) {
            Ok(_) => {}
            Err(sys::Error::Exists) => return Err(self.taken(rel, Reason::DestinationTaken)),
            Err(error) => return Err(Stop::io("creating", self.fence.text_of(rel), error)),
        }
        self.sync(&spot.parent, parent_of(rel))
    }

    /// Makes the folder `rel` when it is missing, and says whether it did; anything there
    /// that is not a plain folder refuses.
    fn ensure_dir(&mut self, rel: &[String]) -> Result<bool, Stop> {
        self.guard(rel)?;
        match self.fence.locate(rel)?.entry.map(|entry| entry.kind) {
            Some(Kind::Dir) => Ok(false),
            None => self.create_dir(rel).map(|()| true),
            Some(Kind::File) => Err(Stop::new(
                Reason::NotADirectory,
                format!("{} is not a folder", self.fence.text_of(rel)),
            )),
            Some(Kind::Link) => Err(Stop::new(
                Reason::LinkInPath,
                format!("{} is a link", self.fence.text_of(rel)),
            )),
        }
    }

    /// Writes the new file `rel`; an existing entry is never opened.
    fn write_new(&mut self, rel: &[String], bytes: &[u8]) -> Result<(), Stop> {
        self.guard(rel)?;
        let mutation = Mutation::WriteFile(self.fence.path_of(rel));
        self.announce(&mutation)?;
        let spot = self.fence.locate(rel)?;
        if spot.entry.is_some() {
            return Err(self.taken(rel, Reason::DestinationTaken));
        }
        self.opened(&mutation)?;
        match spot.parent.write_new(&spot.name, bytes) {
            Ok(()) => {}
            Err(sys::Error::Exists) => return Err(self.taken(rel, Reason::DestinationTaken)),
            Err(error) => return Err(Stop::io("writing", self.fence.text_of(rel), error)),
        }
        self.sync(&spot.parent, parent_of(rel))
    }

    fn not_approved(&self, rel: &[String]) -> Stop {
        Stop::new(
            Reason::Changed,
            format!(
                "{} is not the entry that was approved",
                self.fence.text_of(rel)
            ),
        )
    }

    /// Takes hold of the approved object under the last name of `rel`, in its open folder.
    fn claim<'n>(
        &self,
        parent: &'n Node,
        name: &str,
        seen: &Seen,
        rel: &[String],
    ) -> Result<sys::Claim<'n>, Stop> {
        match parent.claim(name, seen.id) {
            Ok(claim) => Ok(claim),
            Err(sys::Error::Missing | sys::Error::Changed) => Err(self.not_approved(rel)),
            Err(error) => Err(Stop::io("opening", self.fence.text_of(rel), error)),
        }
    }

    /// Moves the entry `from`, with the tree of the inventory `seen`, to the vacant place
    /// `to`. The system refuses an existing destination by itself: nothing is replaced.
    fn rename(
        &mut self,
        from: &[String],
        seen: &Seen,
        to: &[String],
        taken: Reason,
    ) -> Result<(), Stop> {
        self.guard(from)?;
        self.guard(to)?;
        let mutation = Mutation::Rename {
            from: self.fence.path_of(from),
            to: self.fence.path_of(to),
        };
        self.announce(&mutation)?;
        let (source, entry) = match self.fence.existing(from) {
            Err(stop) if stop.reason == Reason::Missing => return Err(self.not_approved(from)),
            other => other?,
        };
        verify(self.fence, &entry, seen, &mut from.to_vec())?;
        drop(entry);
        let dest = self.fence.locate(to)?;
        if dest.entry.is_some() {
            return Err(self.taken(to, taken));
        }
        let claim = self.claim(&source.parent, &source.name, seen, from)?;
        self.opened(&mutation)?;
        match claim.move_to(&dest.parent, &dest.name) {
            Ok(()) => {}
            Err(sys::Error::Exists) => return Err(self.taken(to, taken)),
            Err(sys::Error::Changed) => {
                return Err(Stop::new(
                    Reason::Changed,
                    format!(
                        "what arrived at {} is not the entry that was approved",
                        self.fence.text_of(to)
                    ),
                ));
            }
            Err(error) => return Err(Stop::io("moving", self.fence.text_of(from), error)),
        }
        self.sync(&dest.parent, parent_of(to))?;
        self.sync(&source.parent, parent_of(from))
    }

    /// Removes the one entry under `name` of the open folder `parent`: the object of the
    /// inventory, or nothing. A link goes away as a link; a folder has to be empty.
    fn remove_claimed(
        &mut self,
        parent: &Node,
        name: &str,
        seen: &Seen,
        rel: &[String],
    ) -> Result<(), Stop> {
        let mutation = Mutation::Remove(self.fence.path_of(rel));
        self.announce(&mutation)?;
        let claim = self.claim(parent, name, seen, rel)?;
        self.opened(&mutation)?;
        claim.remove().map_err(|error| match error {
            sys::Error::Io(error) if error.kind() == io::ErrorKind::DirectoryNotEmpty => Stop::new(
                Reason::Changed,
                format!("{} is no longer empty", self.fence.text_of(rel)),
            ),
            error => Stop::io("removing", self.fence.text_of(rel), error),
        })
    }

    /// Removes the one entry `rel`, reached again from the fence.
    fn remove_entry(&mut self, rel: &[String], seen: &Seen) -> Result<(), Stop> {
        self.guard(rel)?;
        let spot = self.fence.open_dir(parent_of(rel))?;
        let name = rel.last().expect("guarded: below the work root");
        self.remove_claimed(&spot, name, seen, rel)?;
        self.sync(&spot, parent_of(rel))
    }

    /// Removes the tree under `name` of the open folder `parent`, the deepest entries first.
    /// Each folder is opened by handle and has to hold exactly the names of the inventory;
    /// each entry has to be the object of the inventory. A link is removed, never entered.
    fn remove_below(
        &mut self,
        parent: &Node,
        name: &str,
        seen: &Seen,
        rel: &mut Vec<String>,
        depth: u32,
        removed: &mut u64,
    ) -> Result<(), Stop> {
        if seen.kind == Kind::Dir {
            if depth >= self.limits.max_depth {
                return Err(Stop::new(
                    Reason::TooDeep,
                    format!(
                        "{} is deeper than {} levels",
                        self.fence.text_of(rel),
                        self.limits.max_depth
                    ),
                ));
            }
            let entry = open_listed(self.fence, parent, rel, name)?;
            same_object(self.fence, &entry, seen, rel)?;
            same_names(self.fence, &entry.node, seen, rel)?;
            for (child, child_seen) in &seen.children {
                rel.push(child.clone());
                self.remove_below(&entry.node, child, child_seen, rel, depth + 1, removed)?;
                rel.pop();
            }
        }
        self.remove_claimed(parent, name, seen, rel)?;
        *removed += 1;
        Ok(())
    }

    /// Removes the tree at `rel`, reached again from the fence.
    fn remove_tree(&mut self, rel: &[String], seen: &Seen, removed: &mut u64) -> Result<(), Stop> {
        self.guard(rel)?;
        self.announce(&Mutation::RemoveTree(self.fence.path_of(rel)))?;
        let parent = self.fence.open_dir(parent_of(rel))?;
        let name = rel.last().expect("guarded: below the work root");
        self.remove_below(&parent, name, seen, &mut rel.to_vec(), 0, removed)?;
        self.sync(&parent, parent_of(rel))
    }

    /// Makes the missing folders between `to[..floor]` and the parent of `to`, noting each.
    fn make_way(&mut self, to: &[String], floor: usize) -> Result<(), Stop> {
        for depth in floor + 1..to.len() {
            let rel = &to[..depth];
            let path = self.fence.text_of(rel);
            let status = match self.ensure_dir(rel) {
                Ok(false) => continue,
                Ok(true) => Ok(Laid::Made),
                Err(stop) => Err(stop),
            };
            let (status, stop) = match status {
                Ok(laid) => (laid, None),
                Err(stop) => (laid_failure(&stop), Some(stop)),
            };
            self.laid.push(Staged {
                path,
                what: Scaffold::Folder,
                status,
            });
            if let Some(stop) = stop {
                return Err(stop);
            }
        }
        Ok(())
    }

    /// Runs the job, stopping at the first refusal. The report of the job says what was done.
    pub(crate) fn run(
        &mut self,
        job: &mut Job,
        manifest: Option<&[u8]>,
    ) -> Result<(), (Stage, Stop)> {
        for (stage, staged) in job.stages.iter().zip(&mut job.report.staging) {
            let outcome = match stage.lay {
                Lay::Ensure => self.ensure_dir(&stage.rel).map(|made| match made {
                    true => Laid::Made,
                    false => Laid::AlreadyThere,
                }),
                Lay::Reserve => self.create_dir(&stage.rel).map(|()| Laid::Made),
                Lay::Manifest => self
                    .write_new(&stage.rel, manifest.unwrap_or_default())
                    .map(|()| Laid::Made),
            };
            match outcome {
                Ok(laid) => staged.status = laid,
                Err(stop) => {
                    staged.status = laid_failure(&stop);
                    return Err((Stage::Staging, stop));
                }
            }
        }
        for (step, op) in job.report.steps.iter_mut().zip(&job.ops) {
            let Some(op) = op else { continue };
            let mut removed = 0;
            // The folders on the way are not the target: failing there leaves it untouched.
            let way = match &op.act {
                Act::MoveTo { to, floor, .. } => self.make_way(to, *floor),
                Act::Remove { .. } => Ok(()),
            };
            let outcome = match (&op.act, way) {
                (_, Err(stop)) => Err((stop, false)),
                (Act::MoveTo { to, taken, .. }, Ok(())) => self
                    .rename(&op.rel, &op.seen, to, *taken)
                    .map_err(|stop| (stop, true)),
                (Act::Remove { recursive: false }, Ok(())) => self
                    .remove_entry(&op.rel, &op.seen)
                    .map_err(|stop| (stop, true)),
                (Act::Remove { recursive: true }, Ok(())) => self
                    .remove_tree(&op.rel, &op.seen, &mut removed)
                    .map_err(|stop| (stop, true)),
            };
            match outcome {
                Ok(()) => step.status = Status::Done,
                Err((stop, at_target)) => {
                    step.status = match stop.reason {
                        Reason::Unconfirmed if at_target => Status::Unconfirmed {
                            detail: stop.detail.clone(),
                        },
                        reason => Status::Failed {
                            reason,
                            detail: stop.detail.clone(),
                            removed,
                        },
                    };
                    return Err((Stage::Steps, stop));
                }
            }
        }
        Ok(())
    }
}

fn laid_failure(stop: &Stop) -> Laid {
    match stop.reason {
        Reason::Unconfirmed => Laid::Unconfirmed {
            detail: stop.detail.clone(),
        },
        reason => Laid::Failed {
            reason,
            detail: stop.detail.clone(),
        },
    }
}
