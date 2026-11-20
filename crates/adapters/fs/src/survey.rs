//! What is really on disk under a target. Read only, by open handles, and no link is entered.
//!
//! The survey also takes the inventory: which object is under each name of the tree. That is
//! what an approval covers; the execution compares the disk with it and refuses an entry that
//! is new or that is another object.

use std::collections::BTreeMap;

use crate::fence::{Entry, Fence, Kind};
use crate::reason::{Reason, Stop};
use crate::sys::{self, Identity, Node};

/// The facts a plan declares, as the disk has them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Facts {
    /// The target is a folder with a `.git` entry right in it.
    pub repo_root: bool,
    /// A `.git` entry (a folder, or the file of a worktree) deeper in the tree: the target
    /// holds a repository of its own.
    pub nested_repo: bool,
    /// Every entry of the tree, the target included. A link counts as one and is not entered.
    pub items: u64,
    /// The sizes of the regular files added up.
    pub bytes: u64,
}

fn overflow() -> Stop {
    Stop::new(Reason::Overflow, "the count does not fit in 64 bits")
}

impl Facts {
    /// Counts one entry of `bytes` bytes. A sum that does not fit refuses: it never wraps.
    pub(crate) fn count(&mut self, bytes: u64) -> Result<(), Stop> {
        self.items = self.items.checked_add(1).ok_or_else(overflow)?;
        self.bytes = self.bytes.checked_add(bytes).ok_or_else(overflow)?;
        Ok(())
    }
}

/// The items and bytes of every target of a plan added up.
pub(crate) fn total<'a>(facts: impl IntoIterator<Item = &'a Facts>) -> Result<(u64, u64), Stop> {
    let mut items: u64 = 0;
    let mut bytes: u64 = 0;
    for one in facts {
        items = items.checked_add(one.items).ok_or_else(overflow)?;
        bytes = bytes.checked_add(one.bytes).ok_or_else(overflow)?;
    }
    Ok((items, bytes))
}

/// One entry of the inventory: the object that was under a name, and what was in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Seen {
    pub id: Identity,
    pub kind: Kind,
    /// The bytes of a regular file when it was counted; zero for the rest. The approval was
    /// given for these bytes: a file that has another size is not what was approved.
    pub size: u64,
    pub children: BTreeMap<String, Seen>,
}

fn changed(fence: &Fence, rel: &[String], what: &str) -> Stop {
    Stop::new(Reason::Changed, format!("{} {what}", fence.text_of(rel)))
}

/// Opens the entry `name`, which the open folder `parent` listed or held when it was surveyed.
pub(crate) fn open_listed(
    fence: &Fence,
    parent: &Node,
    rel: &[String],
    name: &str,
) -> Result<Entry, Stop> {
    let node = match parent.listed(name) {
        Ok(node) => node,
        Err(error @ sys::Error::Missing(_)) => {
            return Err(changed(fence, rel, "is gone").with_cause(error));
        }
        Err(error) => return Err(Stop::io("opening", fence.text_of(rel), error)),
    };
    let meta = node
        .meta()
        .map_err(|error| Stop::io("looking at", fence.text_of(rel), error))?;
    let kind = fence.admit(rel, &meta)?;
    Ok(Entry { node, meta, kind })
}

pub(crate) fn list(fence: &Fence, node: &Node, rel: &[String]) -> Result<Vec<String>, Stop> {
    node.names().map_err(|error| match error.kind() {
        std::io::ErrorKind::InvalidData => Stop::new(
            Reason::NotPhysical,
            format!("{} holds a name that is not text", fence.text_of(rel)),
        )
        .with_cause(error),
        _ => Stop::io("listing", fence.text_of(rel), error),
    })
}

/// Counts the tree of the open `entry` at `rel` and takes its inventory.
pub(crate) fn survey(
    fence: &Fence,
    entry: &Entry,
    rel: &[String],
    max_depth: u32,
) -> Result<(Seen, Facts), Stop> {
    let mut facts = Facts::default();
    let mut rel = rel.to_vec();
    let seen = walk(fence, entry, &mut rel, 0, max_depth, &mut facts)?;
    Ok((seen, facts))
}

fn walk(
    fence: &Fence,
    entry: &Entry,
    rel: &mut Vec<String>,
    depth: u32,
    max_depth: u32,
    facts: &mut Facts,
) -> Result<Seen, Stop> {
    let mut children = BTreeMap::new();
    match entry.kind {
        Kind::File => facts.count(entry.meta.size)?,
        Kind::Link => facts.count(0)?,
        Kind::Dir => {
            facts.count(0)?;
            if depth >= max_depth {
                return Err(Stop::new(
                    Reason::TooDeep,
                    format!("{} is deeper than {max_depth} levels", fence.text_of(rel)),
                ));
            }
            for name in list(fence, &entry.node, rel)? {
                // In any case: a file system that ignores case reads `.GIT` as `.git`.
                if name.eq_ignore_ascii_case(".git") {
                    if depth == 0 {
                        facts.repo_root = true;
                    } else {
                        facts.nested_repo = true;
                    }
                }
                rel.push(name);
                let child = open_listed(fence, &entry.node, rel, rel.last().expect("pushed"))?;
                let seen = walk(fence, &child, rel, depth + 1, max_depth, facts)?;
                let name = rel.pop().expect("pushed");
                children.insert(name, seen);
            }
        }
    }
    Ok(Seen {
        id: entry.meta.id,
        kind: entry.kind,
        size: match entry.kind {
            Kind::File => entry.meta.size,
            Kind::Dir | Kind::Link => 0,
        },
        children,
    })
}

/// The open `entry` has to be the object of the inventory.
pub(crate) fn same_object(
    fence: &Fence,
    entry: &Entry,
    seen: &Seen,
    rel: &[String],
) -> Result<(), Stop> {
    if entry.meta.id != seen.id || entry.kind != seen.kind {
        return Err(changed(fence, rel, "is not the entry that was approved"));
    }
    if seen.kind == Kind::File && entry.meta.size != seen.size {
        let what = format!(
            "had {} bytes when it was approved and has {} now",
            seen.size, entry.meta.size
        );
        return Err(changed(fence, rel, &what));
    }
    Ok(())
}

/// The names in the open folder at `rel` have to be the ones of the inventory, no more and
/// no less.
pub(crate) fn same_names(
    fence: &Fence,
    node: &Node,
    seen: &Seen,
    rel: &[String],
) -> Result<(), Stop> {
    let names = list(fence, node, rel)?;
    if let Some(new) = names.iter().find(|name| !seen.children.contains_key(*name)) {
        return Err(changed(
            fence,
            rel,
            &format!("holds `{new}`, which was not there when it was approved"),
        ));
    }
    if let Some(gone) = seen.children.keys().find(|name| !names.contains(name)) {
        return Err(changed(fence, rel, &format!("no longer holds `{gone}`")));
    }
    Ok(())
}

/// The tree of the open `entry` at `rel` has to be the inventory, object by object.
pub(crate) fn verify(
    fence: &Fence,
    entry: &Entry,
    seen: &Seen,
    rel: &mut Vec<String>,
) -> Result<(), Stop> {
    same_object(fence, entry, seen, rel)?;
    if seen.kind != Kind::Dir {
        return Ok(());
    }
    same_names(fence, &entry.node, seen, rel)?;
    for (name, child_seen) in &seen.children {
        rel.push(name.clone());
        let child = open_listed(fence, &entry.node, rel, name)?;
        verify(fence, &child, child_seen, rel)?;
        rel.pop();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Pure: no file is large enough to overflow a count, so the sums are fed by hand.

    #[test]
    fn a_count_that_does_not_fit_refuses_instead_of_wrapping() {
        let mut facts = Facts {
            items: 7,
            bytes: u64::MAX - 10,
            ..Facts::default()
        };
        facts.count(10).unwrap();
        assert_eq!((facts.items, facts.bytes), (8, u64::MAX));
        assert_eq!(facts.count(1).unwrap_err().reason, Reason::Overflow);

        let mut facts = Facts {
            items: u64::MAX,
            ..Facts::default()
        };
        assert_eq!(facts.count(0).unwrap_err().reason, Reason::Overflow);
    }

    #[test]
    fn the_sum_over_the_targets_refuses_an_overflow_too() {
        let half = Facts {
            items: 1,
            bytes: u64::MAX / 2 + 1,
            ..Facts::default()
        };
        let small = Facts {
            items: 2,
            bytes: 5,
            ..Facts::default()
        };
        assert_eq!(
            total([&small, &small, &half]).unwrap(),
            (5, u64::MAX / 2 + 11)
        );
        // Two sparse files of half the range each would wrap to a tiny total.
        assert_eq!(total([&half, &half]).unwrap_err().reason, Reason::Overflow);
        let many = Facts {
            items: u64::MAX,
            ..Facts::default()
        };
        assert_eq!(total([&many, &small]).unwrap_err().reason, Reason::Overflow);
    }
}
