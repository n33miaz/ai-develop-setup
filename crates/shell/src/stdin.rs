#![allow(unsafe_code)]

use std::{
    fs::File,
    io,
    os::windows::io::{AsHandle, AsRawHandle, FromRawHandle, OwnedHandle},
};

use windows::Win32::{
    Foundation::HANDLE,
    System::Console::{STD_INPUT_HANDLE, SetStdHandle},
};

pub(super) fn private_input() -> io::Result<File> {
    let stdin = io::stdin();
    let input = stdin.as_handle();
    let reader = File::from(input.try_clone_to_owned()?);
    let raw = input.as_raw_handle();
    // SAFETY: a null handle detaches the process stdin; the reader owns its separate handle.
    unsafe { SetStdHandle(STD_INPUT_HANDLE, HANDLE::default()) }.map_err(io::Error::other)?;
    // SAFETY: the detached handle came from process startup and is no longer used by stdin.
    drop(unsafe { OwnedHandle::from_raw_handle(raw) });
    Ok(reader)
}
