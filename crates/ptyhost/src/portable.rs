//! Real terminals through `portable-pty` (ConPTY on Windows). Nothing outside this file names it.

use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};

use crate::backend::{PtyBackend, PtyControl, PtyParts, PtyWaiter};
use crate::types::{ExitStatus, PtyError, SpawnSpec, TerminalSize};

/// Exit code reported when the real one cannot be read.
const UNKNOWN_EXIT_CODE: u32 = 1;

/// ConPTY opens by asking the terminal where the cursor is and holds the child until it hears
/// back. No client may be attached yet, so the backend answers "top left" itself.
#[cfg(windows)]
const CURSOR_POSITION_REPLY: &[u8] = b"[1;1R";

#[derive(Debug, Default, Clone, Copy)]
pub struct PortablePtyBackend;

impl PtyBackend for PortablePtyBackend {
    fn spawn(&self, spec: &SpawnSpec) -> Result<PtyParts, PtyError> {
        let pair = native_pty_system()
            .openpty(pty_size(spec.size))
            .map_err(spawn_error)?;

        let mut command = CommandBuilder::new(&spec.program);
        command.args(&spec.args);
        command.env_clear();
        for (name, value) in &spec.env {
            command.env(name, value);
        }
        // Without this the library picks a directory from the environment on its own.
        let cwd = match &spec.cwd {
            Some(cwd) => cwd.clone(),
            None => std::env::current_dir().map_err(spawn_error)?,
        };
        command.cwd(cwd);

        let mut child = pair.slave.spawn_command(command).map_err(spawn_error)?;
        drop(pair.slave);

        #[cfg(windows)]
        let job = match job_for(child.as_mut()) {
            Ok(job) => job,
            Err(err) => {
                let _ = child.kill();
                return Err(err);
            }
        };

        let io = pair
            .master
            .try_clone_reader()
            .and_then(|reader| Ok((reader, pair.master.take_writer()?)));
        #[cfg(windows)]
        let io = io.and_then(|(reader, mut writer)| {
            std::io::Write::write_all(&mut writer, CURSOR_POSITION_REPLY)?;
            Ok((reader, writer))
        });
        let (reader, writer) = match io {
            Ok(io) => io,
            Err(err) => {
                let _ = child.kill();
                return Err(spawn_error(err));
            }
        };

        Ok(PtyParts {
            reader,
            writer,
            control: Box::new(PortableControl {
                master: Some(pair.master),
                #[cfg(windows)]
                job,
                #[cfg(not(windows))]
                killer: child.clone_killer(),
                #[cfg(not(windows))]
                root_process_id: child.process_id(),
            }),
            waiter: Box::new(PortableWaiter(child)),
        })
    }
}

/// Puts the root process in a job that dies as a whole. A grandchild started in the instant
/// between process creation and this call would escape; see the crate README.
#[cfg(windows)]
fn job_for(child: &mut (dyn Child + Send + Sync)) -> Result<crate::win::JobObject, PtyError> {
    let job = crate::win::JobObject::kill_on_close().map_err(spawn_error)?;
    let handle = child
        .as_raw_handle()
        .ok_or_else(|| PtyError::Spawn("child has no process handle".to_owned()))?;
    match job.assign(handle) {
        Ok(()) => Ok(job),
        // A process that already exited cannot join a job, and has nothing left to kill.
        Err(_) if matches!(child.try_wait(), Ok(Some(_))) => Ok(job),
        Err(err) => Err(spawn_error(err)),
    }
}

fn pty_size(size: TerminalSize) -> PtySize {
    PtySize {
        rows: size.rows,
        cols: size.cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}

fn spawn_error(err: impl std::fmt::Display) -> PtyError {
    PtyError::Spawn(err.to_string())
}

struct PortableControl {
    master: Option<Box<dyn MasterPty + Send>>,
    #[cfg(windows)]
    job: crate::win::JobObject,
    #[cfg(not(windows))]
    killer: Box<dyn portable_pty::ChildKiller + Send + Sync>,
    #[cfg(not(windows))]
    root_process_id: Option<u32>,
}

impl PtyControl for PortableControl {
    fn resize(&mut self, size: TerminalSize) -> Result<(), PtyError> {
        self.master
            .as_ref()
            .ok_or(PtyError::SessionEnded)?
            .resize(pty_size(size))
            .map_err(|err| PtyError::Io(err.to_string()))
    }

    fn kill_tree(&mut self) {
        #[cfg(windows)]
        self.job.terminate();
        // Outside Windows only the root is signalled; closing the terminal hangs up the rest.
        // After the release the root was already reaped and its id may belong to someone else.
        #[cfg(not(windows))]
        if self.master.is_some() {
            let _ = self.killer.kill();
        }
    }

    fn release(&mut self) {
        self.master = None;
    }

    fn tree_process_ids(&self) -> Vec<u32> {
        #[cfg(windows)]
        {
            self.job.process_ids()
        }
        #[cfg(not(windows))]
        {
            self.root_process_id.into_iter().collect()
        }
    }
}

struct PortableWaiter(Box<dyn Child + Send + Sync>);

impl PtyWaiter for PortableWaiter {
    fn wait(mut self: Box<Self>) -> ExitStatus {
        ExitStatus {
            code: self
                .0
                .wait()
                .map_or(UNKNOWN_EXIT_CODE, |status| status.exit_code()),
        }
    }
}
