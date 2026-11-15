//! What is really on disk under a target. Read only, and no link is followed.

use std::fs::{self, Metadata};
use std::path::Path;

use crate::fence::{Fence, Kind, kind_of};
use crate::reason::{Reason, Stop};

/// The facts a plan declares, as the disk has them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Facts {
    /// The target is a folder with a `.git` entry right in it.
    pub repo_root: bool,
    /// Every entry of the tree, the target included. A link counts as one and is not entered.
    pub items: u64,
    /// The sizes of the regular files added up.
    pub bytes: u64,
}

/// The names in the folder `path`, sorted.
pub(crate) fn names(path: &Path) -> Result<Vec<String>, Stop> {
    let mut names = Vec::new();
    for entry in fs::read_dir(path).map_err(|error| Stop::io("listing", path, &error))? {
        let entry = entry.map_err(|error| Stop::io("listing", path, &error))?;
        let name = entry.file_name().into_string().map_err(|_| {
            Stop::new(
                Reason::NotPhysical,
                format!("{} holds a name that is not text", path.display()),
            )
        })?;
        names.push(name);
    }
    names.sort();
    Ok(names)
}

pub(crate) fn survey(fence: &Fence, rel: &[String], max_depth: u32) -> Result<(Kind, Facts), Stop> {
    let (path, kind) = fence.existing(rel)?;
    let mut facts = Facts::default();
    if kind == Kind::Dir {
        let mut git = rel.to_vec();
        git.push(".git".to_owned());
        facts.repo_root = fence.inspect(&git)?.1.is_some();
    }
    let metadata =
        fs::symlink_metadata(&path).map_err(|error| Stop::io("looking at", &path, &error))?;
    count(&path, &metadata, 0, max_depth, &mut facts)?;
    Ok((kind, facts))
}

fn count(
    path: &Path,
    metadata: &Metadata,
    depth: u32,
    max_depth: u32,
    facts: &mut Facts,
) -> Result<(), Stop> {
    facts.items += 1;
    match kind_of(metadata) {
        Some(Kind::File) => facts.bytes += metadata.len(),
        Some(Kind::Link { .. }) => {}
        Some(Kind::Dir) => {
            if depth >= max_depth {
                return Err(Stop::new(
                    Reason::TooDeep,
                    format!("{} is deeper than {max_depth} levels", path.display()),
                ));
            }
            for name in names(path)? {
                let child = path.join(&name);
                let metadata = fs::symlink_metadata(&child)
                    .map_err(|error| Stop::io("looking at", &child, &error))?;
                count(&child, &metadata, depth + 1, max_depth, facts)?;
            }
        }
        None => {
            return Err(Stop::new(
                Reason::SpecialFile,
                format!("{} is not a file, a folder or a link", path.display()),
            ));
        }
    }
    Ok(())
}
