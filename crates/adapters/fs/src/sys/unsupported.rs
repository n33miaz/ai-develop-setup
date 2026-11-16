//! A system without the calls this crate needs (open relative to a folder without following
//! links, move that refuses an existing destination, mount identification): nothing opens,
//! so no executor can be built.

use std::io;
use std::path::Path;

use super::{Error, Identity, Meta};

#[derive(Debug)]
pub struct Node {
    never: std::convert::Infallible,
}

#[derive(Debug)]
pub struct Claim<'a> {
    parent: &'a Node,
}

impl Node {
    pub fn root(_path: &Path) -> io::Result<Self> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "this system is not supported",
        ))
    }

    pub fn again(&self) -> io::Result<Self> {
        match self.never {}
    }

    pub fn meta(&self) -> io::Result<Meta> {
        match self.never {}
    }

    pub fn child(&self, _name: &str) -> Result<Self, Error> {
        match self.never {}
    }

    pub fn listed(&self, _name: &str) -> Result<Self, Error> {
        match self.never {}
    }

    pub fn names(&self) -> io::Result<Vec<String>> {
        match self.never {}
    }

    pub fn make_dir(&self, _name: &str) -> Result<Self, Error> {
        match self.never {}
    }

    pub fn write_new(&self, _name: &str, _bytes: &[u8]) -> Result<(), Error> {
        match self.never {}
    }

    pub fn read_file(&self, _name: &str, _limit: u64) -> Result<Vec<u8>, Error> {
        match self.never {}
    }

    pub fn sync(&self) -> io::Result<()> {
        match self.never {}
    }

    pub fn claim(&self, _name: &str, _expected: Identity) -> Result<Claim<'_>, Error> {
        match self.never {}
    }
}

impl Claim<'_> {
    pub fn remove(self) -> Result<(), Error> {
        match self.parent.never {}
    }

    pub fn move_to(self, _to: &Node, _name: &str) -> Result<(), Error> {
        match self.parent.never {}
    }
}
