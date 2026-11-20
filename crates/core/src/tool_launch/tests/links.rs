//! Checks the targets of configured links when a session opens.

use std::path::{Path, PathBuf};

use super::{DiskProbe, HARNESS_ID, HostRules, LaunchError, ToolConfig, literal, open_params};

/// A scratch folder made by this test, independent of the build folder. Dropping it removes
/// each entry one by one, links with link removal; nothing is removed recursively.
struct Scratch {
    base: PathBuf,
    /// Created entries, removed in reverse order.
    made: Vec<(PathBuf, Kind)>,
}

#[derive(Clone, Copy)]
enum Kind {
    File,
    Dir,
    FileLink,
    DirLink,
}

impl Scratch {
    fn new(name: &str) -> Self {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        // Dentro de .test-tmp/, ignorada pelo git: uma junção para a raiz que sobre de um teste
        // interrompido não fica solta na árvore do repositório.
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join(".test-tmp");
        std::fs::create_dir_all(&workspace).expect("could not create .test-tmp");
        let workspace = PathBuf::from(literal(&workspace));
        let base = loop {
            let sequence = NEXT.fetch_add(1, Ordering::Relaxed);
            let base = workspace.join(format!(".{name}-{}-{sequence}", std::process::id()));
            match std::fs::create_dir(&base) {
                Ok(()) => break base,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("could not create scratch folder: {error}"),
            }
        };
        println!("link test folder: {}", base.display());
        Self {
            base,
            made: Vec::new(),
        }
    }

    fn file(&mut self, name: &str) -> PathBuf {
        let path = self.base.join(name);
        std::fs::write(&path, b"").unwrap();
        self.made.push((path.clone(), Kind::File));
        path
    }

    fn dir(&mut self, name: &str) -> PathBuf {
        let path = self.base.join(name);
        std::fs::create_dir(&path).unwrap();
        self.made.push((path.clone(), Kind::Dir));
        path
    }

    /// A link to a folder, which may not exist. A junction on Windows: it needs no privilege.
    fn dir_link(&mut self, name: &str, target: &Path) -> PathBuf {
        let path = self.base.join(name);
        #[cfg(windows)]
        {
            let status = std::process::Command::new(system_cmd())
                .args(["/d", "/c", "mklink", "/J"])
                .arg(&path)
                .arg(target)
                .stdout(std::process::Stdio::null())
                .status()
                .unwrap();
            assert!(status.success(), "mklink /J failed");
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink(target, &path).unwrap();
        self.made.push((path.clone(), Kind::DirLink));
        path
    }

    /// A symbolic link to a file, or `None` where the system refuses it to this user (Windows
    /// without the privilege or developer mode).
    fn file_link(&mut self, name: &str, target: &Path) -> Option<PathBuf> {
        let path = self.base.join(name);
        #[cfg(windows)]
        let made = std::os::windows::fs::symlink_file(target, &path);
        #[cfg(unix)]
        let made = std::os::unix::fs::symlink(target, &path);
        match made {
            Ok(()) => {
                self.made.push((path.clone(), Kind::FileLink));
                Some(path)
            }
            Err(error) if cfg!(windows) => {
                println!("file symbolic link not created ({error}); that case is skipped");
                None
            }
            Err(error) => panic!("{error}"),
        }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        for (path, kind) in self.made.drain(..).rev() {
            println!("removing {}", path.display());
            let removed = match kind {
                Kind::File | Kind::FileLink => std::fs::remove_file(&path),
                // A junction is removed as an empty folder, without touching its target.
                Kind::Dir => std::fs::remove_dir(&path),
                Kind::DirLink if cfg!(windows) => std::fs::remove_dir(&path),
                Kind::DirLink => std::fs::remove_file(&path),
            };
            if let Err(error) = removed {
                println!("could not remove {}: {error}", path.display());
            }
        }
        println!("removing {}", self.base.display());
        let _ = std::fs::remove_dir(&self.base);
    }
}

#[cfg(windows)]
fn system_cmd() -> PathBuf {
    Path::new(&std::env::var("SystemRoot").unwrap()).join("System32\\cmd.exe")
}

fn open(program: &Path, folder: &Path) -> Result<(), LaunchError> {
    // The text of the link itself, normalized like any configured path; never its target.
    let text = |path: &Path| path.display().to_string();
    let toml = format!(
        "[workdirs]\nscratch = {:?}\n[tools.{HARNESS_ID}]\nprogram = {:?}\n",
        text(folder),
        text(program)
    );
    let config = ToolConfig::from_toml_str(&toml, HostRules::host()).unwrap();
    let request = serde_json::from_value(serde_json::json!({
        "session_id": "sess-01",
        "harness": HARNESS_ID,
        "workdir_alias": "scratch",
        "cols": 80,
        "rows": 24,
        "_meta": {"idempotency_key": "01ARZ3NDEKTSV4RRFFQ69G5FAE", "origin": "local"}
    }))
    .unwrap();
    let params = open_params(
        &config,
        &request,
        Vec::<(String, String)>::new(),
        &DiskProbe,
    )?;
    assert_eq!(params.program.as_str(), literal(program));
    assert_eq!(params.cwd.unwrap().as_str(), literal(folder));
    Ok(())
}

#[test]
fn a_link_is_followed_and_its_target_must_be_of_the_right_kind() {
    let mut scratch = Scratch::new("link-probe");
    let real = scratch.dir("real");
    let tool = real.join("tool.exe");
    std::fs::write(&tool, b"").unwrap();
    scratch.made.push((tool.clone(), Kind::File));
    let linked_dir = scratch.dir_link("linked", &real);
    let missing = scratch.base.join("missing");
    let dangling = scratch.dir_link("dangling", &missing);
    let plain = scratch.file("plain.txt");

    // Through a folder link, as the program's folder and as the working folder.
    assert_eq!(open(&linked_dir.join("tool.exe"), &linked_dir), Ok(()));
    // A link whose target is missing is not found.
    assert_eq!(
        open(&dangling.join("tool.exe"), &real),
        Err(LaunchError::ProgramMissing)
    );
    assert_eq!(open(&tool, &dangling), Err(LaunchError::WorkdirMissing));
    // A folder link is not a program, and a file is not a folder.
    assert_eq!(open(&linked_dir, &real), Err(LaunchError::ProgramMissing));
    assert_eq!(open(&tool, &plain), Err(LaunchError::WorkdirMissing));

    if let Some(linked_file) = scratch.file_link("linked.exe", &tool) {
        assert_eq!(open(&linked_file, &real), Ok(()));
        assert_eq!(open(&tool, &linked_file), Err(LaunchError::WorkdirMissing));
    }
}

#[test]
fn a_workdir_link_to_the_volume_root_is_refused() {
    let mut scratch = Scratch::new("root-link-probe");
    let tool = scratch.file("tool.exe");
    let root = scratch.base.ancestors().last().unwrap().to_owned();
    let linked_root = scratch.dir_link("linked-root", &root);
    assert_eq!(open(&tool, &linked_root), Err(LaunchError::WorkdirMissing));
}

#[test]
fn scratch_is_independent_of_the_test_binary_folder() {
    let scratch = Scratch::new("scratch-location-probe");
    let executable = std::env::current_exe().unwrap();
    let build_folder = std::fs::canonicalize(executable.parent().unwrap()).unwrap();
    assert!(
        !std::fs::canonicalize(&scratch.base)
            .unwrap()
            .starts_with(build_folder)
    );
}
