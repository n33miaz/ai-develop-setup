//! The only place where the disk is changed. Every mutation is one entry, reached again from
//! the fence by open handles right before it is made: strictly inside the work root, no link
//! on the way, and the entry the very object that was approved, with the size it had. The
//! system call then acts on what is open, not on a path: on Windows on the handle of the entry
//! itself; on Linux on a name inside the open folder, where an entry swapped under the name
//! right before the call is moved instead and the check after the move tells it. Nothing here
//! is recursive by itself; a tree goes away entry by entry, each one compared with the
//! inventory.

use std::io::{self, Write};
use std::path::PathBuf;

use crate::fence::{Fence, Kind};
use crate::limits::Limits;
use crate::prepare::{Act, Job, Lay};
use crate::reason::{Reason, Stop};
use crate::report::{Laid, Scaffold, Stage, Staged, Status};
use crate::survey::{Seen, open_listed, same_names, same_object, verify};
use crate::sys::{self, Identity, Node};

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
    /// A tree about to be removed for good: first moved to its holding folder (a `Rename`),
    /// then removed there entry by entry (each one a `Remove`).
    RemoveTree(PathBuf),
    /// The entries of a folder about to be confirmed on disk.
    Sync(PathBuf),
}

/// A point inside a mutation that is more than one system call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Midway {
    /// The new entry exists: a folder, not confirmed on disk yet, or a file, empty, its
    /// content not written yet.
    Created,
    /// The content was written: the file is not confirmed on disk yet.
    Written,
    /// The system made the move: what arrived is not checked yet.
    Moved,
}

/// Told about each mutation as it goes. An error stops the execution there, as a failure of
/// the operating system would. This is how a test fails a batch at any step, or changes the
/// disk around a check.
pub trait Probe {
    /// Before anything of the mutation is opened or checked.
    fn before(&mut self, mutation: &Mutation) -> io::Result<()>;

    /// After the last check, with the handles of the mutation open: the next thing is the
    /// system call. Not told for a `RemoveTree` or a `Sync`.
    fn opened(&mut self, _mutation: &Mutation) -> io::Result<()> {
        Ok(())
    }

    /// Inside a `CreateDir`, a `WriteFile` or a `Rename`, after a system call that already
    /// changed the disk.
    fn midway(&mut self, _mutation: &Mutation, _at: Midway) -> io::Result<()> {
        Ok(())
    }
}

/// The entries of a tree that went away: seen to go, and confirmed on disk afterwards.
#[derive(Debug, Clone, Copy, Default)]
struct Tally {
    removed: u64,
    confirmed: u64,
}

/// One step of the way from the holding folder to the folder being emptied: `name` of the
/// open `parent` has to be the object `id`.
struct Link {
    parent: Node,
    name: String,
    id: Identity,
}

/// How far a step went when it stopped.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Reached {
    /// The target was not changed.
    Nothing,
    /// The system call on the target itself was made.
    Target,
    /// The target is in its holding folder.
    Holding,
}

pub(crate) struct Disk<'a> {
    pub fence: &'a Fence,
    /// The work root: every mutation is strictly below it.
    pub floor: &'a [String],
    /// The holding folder of this execution, when it has one: mutations go there too, and
    /// the folders on the way to it, below the work root (`work` names deep), may be made.
    pub private: Option<&'a [String]>,
    pub work: usize,
    pub probe: Option<&'a mut (dyn Probe + 'static)>,
    pub limits: &'a Limits,
    /// The folders made on the way to a destination, as they were made.
    pub laid: Vec<Staged>,
    /// The holding folder of this execution, as it was made: only that object is removed
    /// at the end.
    pub holding: Option<Identity>,
}

fn parent_of(rel: &[String]) -> &[String] {
    &rel[..rel.len().saturating_sub(1)]
}

fn below(rel: &[String], base: &[String]) -> bool {
    rel.len() > base.len() && rel.starts_with(base)
}

impl Disk<'_> {
    fn guard(&self, rel: &[String]) -> Result<(), Stop> {
        let allowed = below(rel, self.floor)
            || self.private.is_some_and(|holding| {
                rel.len() > self.work && (below(rel, holding) || holding.starts_with(rel))
            });
        if !allowed {
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

    fn midway(&mut self, mutation: &Mutation, at: Midway) -> io::Result<()> {
        match &mut self.probe {
            Some(probe) => probe.midway(mutation, at),
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

    /// Makes the folder `rel`, which must not exist. `private` makes it for this executor
    /// alone, as far as the system has a way to say so.
    fn create_dir(&mut self, rel: &[String], private: bool) -> Result<(), Stop> {
        self.guard(rel)?;
        let mutation = Mutation::CreateDir(self.fence.path_of(rel));
        self.announce(&mutation)?;
        let spot = self.fence.locate(rel)?;
        if spot.entry.is_some() {
            return Err(self.taken(rel, Reason::DestinationTaken));
        }
        self.opened(&mutation)?;
        let made = match private {
            true => spot.parent.make_private_dir(&spot.name),
            false => spot.parent.make_dir(&spot.name),
        };
        match made {
            Ok(()) => {}
            Err(sys::Error::Exists) => return Err(self.taken(rel, Reason::DestinationTaken)),
            Err(error) => return Err(Stop::io("creating", self.fence.text_of(rel), error)),
        }
        // From here the folder exists, whatever comes next.
        match self.midway(&mutation, Midway::Created) {
            Ok(()) => self.sync(&spot.parent, parent_of(rel)),
            Err(error) => Err(self.created_but(&spot.parent, rel, "it stopped there", error)),
        }
    }

    /// The entry `rel` was created and what came next failed with `error`. Its folder is
    /// still confirmed on disk, and how that went is part of what is reported: the entry
    /// exists, and is told as created and not to be trusted.
    fn created_but(&mut self, parent: &Node, rel: &[String], what: &str, error: io::Error) -> Stop {
        let text = self.fence.text_of(rel);
        let folder = match self.sync(parent, parent_of(rel)) {
            Ok(()) => "its folder was confirmed on disk".to_owned(),
            Err(stop) => format!("its folder was not confirmed either: {}", stop.detail),
        };
        Stop::new(
            Reason::Unconfirmed,
            format!("{text} was created, but {what}: {error}; {folder}"),
        )
    }

    /// Makes the folder `rel` when it is missing, and says whether it did; anything there
    /// that is not a plain folder refuses.
    fn ensure_dir(&mut self, rel: &[String]) -> Result<bool, Stop> {
        self.guard(rel)?;
        match self.fence.locate(rel)?.entry.map(|entry| entry.kind) {
            Some(Kind::Dir) => Ok(false),
            None => self.create_dir(rel, false).map(|()| true),
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

    /// Writes the new file `rel`; an existing entry is never opened. Three steps: create it
    /// (exclusively), write it, have it on disk. A failure after the first one leaves the
    /// file there, and says so: `Unconfirmed`, never "not created".
    fn write_new(&mut self, rel: &[String], bytes: &[u8]) -> Result<(), Stop> {
        self.guard(rel)?;
        let mutation = Mutation::WriteFile(self.fence.path_of(rel));
        self.announce(&mutation)?;
        let spot = self.fence.locate(rel)?;
        if spot.entry.is_some() {
            return Err(self.taken(rel, Reason::DestinationTaken));
        }
        self.opened(&mutation)?;
        let mut file = match spot.parent.create_file(&spot.name) {
            Ok(file) => file,
            Err(sys::Error::Exists) => return Err(self.taken(rel, Reason::DestinationTaken)),
            Err(error) => return Err(Stop::io("creating", self.fence.text_of(rel), error)),
        };
        // From here the file exists, whatever comes next: every way out confirms its folder.
        let written = self
            .midway(&mutation, Midway::Created)
            .and_then(|()| file.write_all(bytes))
            .map_err(|error| ("its content was not written to the end", error))
            .and_then(|()| {
                self.midway(&mutation, Midway::Written)
                    .and_then(|()| file.sync_all())
                    .map_err(|error| ("it is not confirmed on disk", error))
            });
        drop(file);
        match written {
            Ok(()) => self.sync(&spot.parent, parent_of(rel)),
            Err((what, error)) => Err(self.created_but(&spot.parent, rel, what, error)),
        }
    }

    fn not_approved(&self, rel: &[String], what: &str) -> Stop {
        Stop::new(
            Reason::Changed,
            format!("{} {what}", self.fence.text_of(rel)),
        )
    }

    /// Takes hold of the approved object under the last name of `rel`, in its open folder:
    /// the same object, and for a file the same size it had when it was approved. A claim
    /// `to_remove` keeps the entry under its name until it is gone, where the system can.
    fn claim<'n>(
        &self,
        parent: &'n Node,
        name: &str,
        seen: &Seen,
        rel: &[String],
        to_remove: bool,
    ) -> Result<sys::Claim<'n>, Stop> {
        let claimed = match to_remove {
            true => parent.claim_to_remove(name, seen.id),
            false => parent.claim(name, seen.id),
        };
        let claim = match claimed {
            Ok(claim) => claim,
            Err(sys::Error::Missing | sys::Error::Changed) => {
                return Err(self.not_approved(rel, "is not the entry that was approved"));
            }
            Err(error) => return Err(Stop::io("opening", self.fence.text_of(rel), error)),
        };
        if seen.kind == Kind::File && claim.meta().size != seen.size {
            return Err(self.not_approved(
                rel,
                &format!(
                    "had {} bytes when it was approved and has {} now",
                    seen.size,
                    claim.meta().size
                ),
            ));
        }
        Ok(claim)
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
            Err(stop) if stop.reason == Reason::Missing => {
                return Err(self.not_approved(from, "is not the entry that was approved"));
            }
            other => other?,
        };
        verify(self.fence, &entry, seen, &mut from.to_vec())?;
        drop(entry);
        let dest = self.fence.locate(to)?;
        if dest.entry.is_some() {
            return Err(self.taken(to, taken));
        }
        let claim = self.claim(&source.parent, &source.name, seen, from, false)?;
        self.opened(&mutation)?;
        match claim.move_to(&dest.parent, &dest.name) {
            Ok(()) => {}
            Err(sys::Error::Exists) => return Err(self.taken(to, taken)),
            Err(error) => return Err(Stop::io("moving", self.fence.text_of(from), error)),
        }

        // From here the system made the move, whatever comes next. What arrived is checked,
        // the two folders are confirmed either way, and a check that fails is told as what
        // it is: a move that happened and was not verified.
        let verified = self
            .midway(&mutation, Midway::Moved)
            .map_err(|error| error.to_string())
            .and_then(|()| match dest.parent.arrived(&dest.name, seen.id) {
                Ok(true) => Ok(()),
                Ok(false) => Err("the approved entry is not under the name".to_owned()),
                Err(error) => Err(error.to_string()),
            });
        let synced = self.sync(&dest.parent, parent_of(to));
        let synced = synced.and(self.sync(&source.parent, parent_of(from)));
        if let Err(why) = verified {
            return Err(Stop::new(
                Reason::MoveUnverified,
                format!(
                    "the system moved {} to {}, but the check after the move failed: {why}",
                    self.fence.text_of(from),
                    self.fence.text_of(to)
                ),
            ));
        }
        synced
    }

    /// Every folder from the holding folder down to the one being emptied is still where it
    /// was, by identity. Only where the system cannot hold a folder in place: there this
    /// narrows the time another process has to take one away, and does not close it.
    fn still_there(&self, chain: &[Link], rel: &[String]) -> Result<(), Stop> {
        if sys::HOLDS {
            return Ok(());
        }
        for link in chain {
            match link.parent.holds(&link.name, link.id) {
                Ok(true) => {}
                Ok(false) => {
                    return Err(Stop::new(
                        Reason::Displaced,
                        format!(
                            "`{}`, on the way to {}, is no longer where it was",
                            link.name,
                            self.fence.text_of(rel)
                        ),
                    ));
                }
                Err(error) => return Err(Stop::io("looking at", self.fence.text_of(rel), error)),
            }
        }
        Ok(())
    }

    /// Removes the one entry under `name` of the open folder `parent`: the object of the
    /// inventory, or nothing. A link goes away as a link; a folder has to be empty.
    fn remove_claimed(
        &mut self,
        parent: &Node,
        name: &str,
        seen: &Seen,
        rel: &[String],
        chain: &[Link],
    ) -> Result<(), Stop> {
        let mutation = Mutation::Remove(self.fence.path_of(rel));
        self.announce(&mutation)?;
        // Claimed to be removed: where the system can, nobody can rename it from here on. It
        // is then looked for again under its name in the open folder, so that what is
        // removed is the entry of that folder, not one that left it before the claim held.
        let claim = self.claim(parent, name, seen, rel, true)?;
        self.opened(&mutation)?;
        self.still_there(chain, rel)?;
        match parent.holds(name, seen.id) {
            Ok(true) => {}
            Ok(false) => {
                return Err(Stop::new(
                    Reason::Displaced,
                    format!(
                        "{} is no longer under its name in its folder",
                        self.fence.text_of(rel)
                    ),
                ));
            }
            Err(error) => return Err(Stop::io("looking at", self.fence.text_of(rel), error)),
        }
        claim.remove().map_err(|error| match error {
            sys::Error::Io(error) if error.kind() == io::ErrorKind::DirectoryNotEmpty => {
                self.not_approved(rel, "is no longer empty")
            }
            error => Stop::io("removing", self.fence.text_of(rel), error),
        })
    }

    /// Removes the one entry `rel`, reached again from the fence, its folder held in place.
    fn remove_entry(&mut self, rel: &[String], seen: &Seen) -> Result<(), Stop> {
        self.guard(rel)?;
        let folder = self.fence.hold_dir(parent_of(rel))?;
        let name = rel.last().expect("guarded: below the work root");
        self.remove_claimed(&folder.node, name, seen, rel, &[])?;
        self.sync(&folder.node, parent_of(rel))
    }

    /// Removes the tree under `name` of the open folder `parent`, the deepest entries first.
    /// Each folder is opened by handle, held in place where the system can, and has to hold
    /// exactly the names of the inventory; each entry has to be the object of the inventory.
    /// A link is removed, never entered. A folder that lost entries is confirmed on disk
    /// before it is left, also when the removal stops in the middle.
    #[allow(clippy::too_many_arguments)]
    fn remove_below(
        &mut self,
        parent: &Node,
        name: &str,
        seen: &Seen,
        rel: &mut Vec<String>,
        depth: u32,
        tally: &mut Tally,
        chain: &mut Vec<Link>,
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
            let held = entry
                .node
                .hold()
                .map_err(|error| Stop::io("holding", self.fence.text_of(rel), error))?;
            // Opened and then held: in between it may have been taken away. Its folder is
            // held, so from now on the name says where it is.
            if sys::HOLDS {
                match parent.holds(name, seen.id) {
                    Ok(true) => {}
                    Ok(false) => return Err(self.not_approved(rel, "was taken away")),
                    Err(error) => {
                        return Err(Stop::io("looking at", self.fence.text_of(rel), error));
                    }
                }
            }
            same_names(self.fence, &held, seen, rel)?;
            let again = parent
                .again()
                .map_err(|error| Stop::io("opening", self.fence.text_of(rel), error))?;
            chain.push(Link {
                parent: again,
                name: name.to_owned(),
                id: seen.id,
            });
            let mut here = 0;
            let mut failure = None;
            for (child, child_seen) in &seen.children {
                rel.push(child.clone());
                let outcome =
                    self.remove_below(&held, child, child_seen, rel, depth + 1, tally, chain);
                rel.pop();
                match outcome {
                    Ok(()) => here += 1,
                    Err(stop) => {
                        failure = Some(stop);
                        break;
                    }
                }
            }
            chain.pop();
            if here > 0 {
                match self.sync(&held, rel) {
                    Ok(()) => tally.confirmed += here,
                    Err(stop) => failure = failure.or(Some(stop)),
                }
            }
            if let Some(stop) = failure {
                return Err(stop);
            }
        }
        self.remove_claimed(parent, name, seen, rel, chain)?;
        tally.removed += 1;
        Ok(())
    }

    /// Removes the tree at `rel` for good. The tree is first moved, whole and without
    /// replacing anything, to `place`, in the holding folder of this execution; there it is
    /// compared with the inventory again and removed entry by entry. Paths that other
    /// processes know stop reaching it at the move.
    fn remove_isolated(
        &mut self,
        rel: &[String],
        seen: &Seen,
        place: &[String],
        tally: &mut Tally,
    ) -> Result<(), (Stop, Reached)> {
        let nothing = |stop| (stop, Reached::Nothing);
        self.guard(rel).map_err(nothing)?;
        self.announce(&Mutation::RemoveTree(self.fence.path_of(rel)))
            .map_err(nothing)?;
        // A move that was not verified is not a place: the step says the target left, not
        // where it is. A move verified and not confirmed on disk is held where it arrived.
        self.rename(rel, seen, place, Reason::DestinationTaken)
            .map_err(|stop| match stop.reason {
                Reason::MoveUnverified => (stop, Reached::Target),
                Reason::Unconfirmed => (stop, Reached::Holding),
                _ => (stop, Reached::Nothing),
            })?;
        self.empty_holding(seen, place, tally)
            .map_err(|stop| (stop, Reached::Holding))
    }

    fn empty_holding(
        &mut self,
        seen: &Seen,
        place: &[String],
        tally: &mut Tally,
    ) -> Result<(), Stop> {
        let home = parent_of(place);
        let folder = self.fence.hold_dir(home)?;
        let name = place.last().expect("a place has a name");
        let entry = open_listed(self.fence, &folder.node, place, name)?;
        verify(self.fence, &entry, seen, &mut place.to_vec())?;
        drop(entry);
        let io = |error: io::Error| Stop::io("looking at", self.fence.text_of(home), error);
        let mut chain = Vec::new();
        if let Some(own) = home.last() {
            chain.push(Link {
                parent: self.fence.open_dir(parent_of(home))?,
                name: own.clone(),
                id: folder.node.meta().map_err(io)?.id,
            });
        }
        self.remove_below(
            &folder.node,
            name,
            seen,
            &mut place.to_vec(),
            0,
            tally,
            &mut chain,
        )?;
        self.sync(&folder.node, home)?;
        tally.confirmed += 1;
        Ok(())
    }

    /// Makes the missing folders between `to[..floor]` and the parent of `to`, noting each.
    fn make_way(&mut self, to: &[String], floor: usize) -> Result<(), Stop> {
        for depth in floor + 1..to.len() {
            let rel = &to[..depth];
            let path = self.fence.text_of(rel);
            let (status, stop) = match self.ensure_dir(rel) {
                Ok(false) => continue,
                Ok(true) => (Laid::Made, None),
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

    /// Takes the holding folder of this execution away, when everything in it is gone, with
    /// the protections of every other removal: its folder held, the folder itself claimed
    /// to be removed and found again under its name. A failure before the removal leaves the
    /// folder there and the report saying it was made, which is true. A removal the system
    /// made and did not confirm on disk is a failure, and is reported as such.
    fn clear_holding(&mut self, job: &mut Job) -> Result<(), Stop> {
        let (Some(private), Some(id)) = (self.private, self.holding) else {
            return Ok(());
        };
        let Some(name) = private.last() else {
            return Ok(());
        };
        let home = parent_of(private);
        let seen = Seen {
            id,
            kind: Kind::Dir,
            size: 0,
            children: Default::default(),
        };
        let removed = (|| {
            let folder = self.fence.hold_dir(home)?;
            let mut chain = Vec::new();
            if let Some(own) = home.last() {
                let io = |error| Stop::io("looking at", self.fence.text_of(home), error);
                chain.push(Link {
                    parent: self.fence.open_dir(parent_of(home))?,
                    name: own.clone(),
                    id: folder.node.meta().map_err(io)?.id,
                });
            }
            self.remove_claimed(&folder.node, name, &seen, private, &chain)?;
            Ok::<_, Stop>(folder)
        })();
        // Not removed: the folder stays, and `Made` says so.
        let Ok(folder) = removed else { return Ok(()) };
        let synced = self.sync(&folder.node, home);
        let status = match &synced {
            Ok(()) => Laid::Cleared,
            Err(stop) => Laid::ClearedUnconfirmed {
                detail: stop.detail.clone(),
            },
        };
        for (stage, staged) in job.stages.iter().zip(&mut job.report.staging) {
            if stage.rel == private {
                staged.status = status.clone();
            }
        }
        synced
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
                Lay::Reserve => self.create_dir(&stage.rel, false).map(|()| Laid::Made),
                Lay::ReservePrivate => self.create_dir(&stage.rel, true).map(|()| {
                    // What was made, to remove that and nothing else at the end. Not found
                    // again: it is not removed, and the report keeps saying it was made.
                    self.holding = self
                        .fence
                        .locate(&stage.rel)
                        .ok()
                        .and_then(|spot| spot.entry)
                        .filter(|entry| entry.kind == Kind::Dir)
                        .map(|entry| entry.meta.id);
                    Laid::Made
                }),
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
            let mut tally = Tally::default();
            let outcome = match &op.act {
                // The folders on the way are not the target: failing there leaves it alone.
                Act::MoveTo { to, floor, taken } => self
                    .make_way(to, *floor)
                    .map_err(|stop| (stop, Reached::Nothing))
                    .and_then(|()| {
                        self.rename(&op.rel, &op.seen, to, *taken)
                            .map_err(|stop| (stop, Reached::Target))
                    }),
                Act::Remove { place: None } => self
                    .remove_entry(&op.rel, &op.seen)
                    .map_err(|stop| (stop, Reached::Target)),
                Act::Remove { place: Some(place) } => {
                    self.remove_isolated(&op.rel, &op.seen, place, &mut tally)
                }
            };
            match outcome {
                Ok(()) => step.status = Status::Done,
                Err((stop, reached)) => {
                    let detail = stop.detail.clone();
                    step.status = match (stop.reason, reached) {
                        (reason, Reached::Holding) => Status::Held {
                            reason,
                            detail,
                            removed: tally.removed,
                            confirmed: tally.confirmed,
                        },
                        (Reason::MoveUnverified, Reached::Target) => Status::Unverified { detail },
                        (Reason::Unconfirmed, Reached::Target) => Status::Unconfirmed { detail },
                        (reason, _) => Status::Failed { reason, detail },
                    };
                    return Err((Stage::Steps, stop));
                }
            }
        }
        self.clear_holding(job)
            .map_err(|stop| (Stage::Clearing, stop))
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
