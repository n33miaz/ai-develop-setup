use super::*;

fn grant_access_to_others(path: &Path) {
    #[cfg(windows)]
    assert!(
        std::process::Command::new("icacls")
            .arg(path)
            .args(["/grant", "*S-1-1-0:(OI)(CI)RX"])
            .output()
            .unwrap()
            .status
            .success()
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
}

#[test]
fn audit_folder_new_has_private_permissions() {
    let root = fixture();
    grant_access_to_others(&root);
    let directory = root.join("private");
    let path = directory.join("audit.db");
    let log = open_configured(&path, &configured_for(&path), &MemoryVault::default()).unwrap();
    assert!(path.exists());
    assert!(ControlDir::open_user_owned(&directory, false).is_ok());
    #[cfg(windows)]
    {
        use ads_private_dir::{Identity, access_list_text, canonical_access_list};
        let user = Identity::current().unwrap().user;
        let expected = canonical_access_list(&format!("D:P(A;OICI;FA;;;{user})")).unwrap();
        assert_eq!(access_list_text(&directory).unwrap(), expected);
    }
    drop(log);
}

#[cfg(windows)]
#[test]
fn audit_folder_unc_is_refused_before_filesystem_access() {
    for path in [
        Path::new(r"\\localhost\ads-audit-regression-missing\audit.db"),
        Path::new(r"\\?\UNC\localhost\ads-audit-regression-missing\audit.db"),
    ] {
        assert_eq!(
            open_configured(path, &configured_for(path), &MemoryVault::default()).unwrap_err(),
            StorageError::AuditDatabaseOnNetwork
        );
    }
}

#[test]
fn session_that_gives_new_files_another_owner_is_refused_before_creating_them() {
    let root = fixture();
    let path = root.join("absent").join("audit.db");
    let user = Identity::current().unwrap().user;
    SESSION.with(|session| {
        *session.borrow_mut() = Some(Identity {
            user: user.clone(),
            default_owner: "S-1-5-32-544".into(),
        });
    });
    let refused = open_configured(&path, &configured_for(&path), &MemoryVault::default());
    SESSION.with(|session| session.borrow_mut().take());
    assert_eq!(refused.unwrap_err(), StorageError::ElevatedSession);
    assert!(!path.parent().unwrap().exists());
    assert_eq!(
        StorageError::ElevatedSession.to_string(),
        format!("storage: {}", ads_private_dir::ELEVATED_SESSION)
    );
    drop(open_configured(&path, &configured_for(&path), &MemoryVault::default()).unwrap());
}

#[test]
fn audit_folder_existing_open_to_others_is_refused() {
    let root = fixture();
    grant_access_to_others(&root);
    let path = root.join("audit.db");
    assert_eq!(
        open_configured(&path, &configured_for(&path), &MemoryVault::default()).unwrap_err(),
        StorageError::AuditDirectoryNotPrivate
    );
    assert!(!path.exists());
}

#[cfg(windows)]
fn grant_windows_access(path: &Path, ace: &str) {
    let output = std::process::Command::new("icacls")
        .arg(path)
        .args(["/grant", ace])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
}

#[cfg(windows)]
#[test]
fn audit_folder_open_to_administrators_is_refused() {
    let root = fixture();
    grant_windows_access(&root, "*S-1-5-32-544:(OI)(CI)F");
    let path = root.join("audit.db");
    assert_eq!(
        open_configured(&path, &configured_for(&path), &MemoryVault::default()).unwrap_err(),
        StorageError::AuditDirectoryNotPrivate
    );
}

#[cfg(windows)]
#[test]
fn audit_files_open_to_other_accounts_are_refused() {
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let root = fixture();
        let path = root.join("custom.db");
        let exposed = root.join(format!("custom.db{suffix}"));
        fs::write(&exposed, []).unwrap();
        grant_windows_access(&exposed, "*S-1-1-0:R");
        assert_eq!(
            open_configured(&path, &configured_for(&path), &MemoryVault::default()).unwrap_err(),
            StorageError::AuditDirectoryNotPrivate,
            "{suffix}"
        );
        assert_eq!(fs::metadata(&exposed).unwrap().len(), 0, "{suffix}");
    }
}

#[test]
fn custom_audit_path_leaves_the_default_folder_private() {
    let root = fixture();
    grant_access_to_others(&root);
    let vault_dir = root.join("ads");
    let custom = root.join("custom").join("audit.db");
    drop(
        open_product_in(
            &vault_dir,
            &custom,
            &configured_for(&custom),
            Box::new(Keys),
            MemoryVault::default(),
            None,
            &[],
        )
        .unwrap(),
    );

    let default = vault_dir.join("audit.db");
    drop(open_configured(&default, &configured_for(&default), &MemoryVault::default()).unwrap());
}

#[test]
fn audit_folder_resolving_inside_workdir_is_refused() {
    let root = fixture();
    let work = root.join("work");
    let destination = work.join("private");
    fs::create_dir_all(&destination).unwrap();
    let link = root.join("link");
    link_folder(&link, &destination);
    let path = link.join("audit.db");
    let result = canonical_anchor(
        &path,
        &configured_for(&path),
        true,
        MemoryVault::default(),
        &[work],
    );
    unlink_folder(&link);
    assert!(matches!(result, Err(StorageError::AuditDatabaseInWorkdir)));
    assert!(!destination.join("audit.db").exists());
}

#[test]
fn audit_folder_inside_workdir_is_refused_before_creation() {
    let root = fixture();
    let work = root.join("work");
    fs::create_dir(&work).unwrap();
    let path = work.join("private").join("audit.db");
    assert!(matches!(
        canonical_anchor(
            &path,
            &configured_for(&path),
            true,
            MemoryVault::default(),
            &[work]
        ),
        Err(StorageError::AuditDatabaseInWorkdir)
    ));
    assert!(!path.parent().unwrap().exists());
}
