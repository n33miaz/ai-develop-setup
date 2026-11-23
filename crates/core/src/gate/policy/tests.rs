use super::*;

const WINDOWS: HostFacts<'static> = HostFacts {
    user_home: "C:\\Users\\user-a",
};
const POSIX: HostFacts<'static> = HostFacts {
    user_home: "/home/user-a",
};

fn protection(policy: &GatePolicy, raw: &str) -> Option<Protection> {
    policy.protection(&AbsPath::parse(raw).unwrap())
}

#[test]
fn shipped_defaults_are_valid() {
    let policy = GatePolicy::defaults(&WINDOWS).unwrap();
    assert!(policy.allowed_roots().is_empty());
    assert_eq!(policy.approval_item_limit(), 500);
    assert_eq!(policy.approval_byte_limit(), 104_857_600);
    assert!(policy.quarantine_enabled());
    let work_root = AbsPath::parse("D:\\work\\proj").unwrap();
    assert_eq!(
        policy.quarantine_root(&work_root).to_string(),
        "D:\\work\\proj\\.ads\\quarantine"
    );
}

#[test]
fn override_replaces_plain_keys() {
    let policy = GatePolicy::from_toml_str(
        "approval_item_limit = 10\nquarantine_enabled = false\nallowed_roots = ['D:\\cache']",
        &WINDOWS,
    )
    .unwrap();
    assert_eq!(policy.approval_item_limit(), 10);
    assert_eq!(policy.approval_byte_limit(), 104_857_600);
    assert!(!policy.quarantine_enabled());
    assert_eq!(policy.allowed_roots()[0].to_string(), "D:\\cache");
}

#[test]
fn override_cannot_shrink_a_protected_list() {
    let policy = GatePolicy::from_toml_str(
        "windows_system_dirs = []\nposix_system_dirs = ['/data/system']\nwindows_user_parents = ['Profiles']",
        &WINDOWS,
    )
    .unwrap();
    assert_eq!(
        protection(&policy, "C:\\Windows\\System32"),
        Some(Protection::SystemDir)
    );
    assert_eq!(protection(&policy, "/etc"), Some(Protection::SystemDir));
    assert_eq!(
        protection(&policy, "/data/system/x"),
        Some(Protection::SystemDir)
    );
    assert_eq!(
        protection(&policy, "E:\\Profiles\\user-b"),
        Some(Protection::UserProfile)
    );
    assert_eq!(
        protection(&policy, "C:\\Users\\user-b"),
        Some(Protection::UserProfile)
    );
}

#[test]
fn classification_budget_accepts_zero_through_the_maximum() {
    for steps in [0, MAX_CLASSIFICATION_MATCH_STEPS] {
        let overrides = format!("classification_match_steps = {steps}");
        let policy = GatePolicy::from_toml_str(&overrides, &WINDOWS).unwrap();
        assert_eq!(policy.classification_match_steps(), steps);
    }
    for steps in [MAX_CLASSIFICATION_MATCH_STEPS + 1, i64::MAX as u64] {
        let overrides = format!("classification_match_steps = {steps}");
        assert_eq!(
            GatePolicy::from_toml_str(&overrides, &WINDOWS),
            Err(PolicyError::Invalid {
                key: "classification_match_steps",
                value: steps.to_string(),
            })
        );
    }
}

#[test]
fn bad_values_are_refused() {
    for bad in [
        "unknown_key = 1",
        "approval_item_limit = -1",
        "classification_match_steps = -1",
        "quarantine_dir = '../outside'",
        "quarantine_dir = '/abs'",
        "quarantine_dir = ''",
        "allowed_roots = ['cache']",
        "allowed_roots = ['D:\\cache\\*']",
        "windows_system_dirs = ['a\\b']",
        "windows_system_dirs = ['']",
        "posix_system_dirs = ['C:\\data']",
        "posix_user_parents = ['home']",
    ] {
        assert!(GatePolicy::from_toml_str(bad, &WINDOWS).is_err(), "{bad}");
    }
}

#[test]
fn quarantine_that_would_land_in_a_protected_folder_is_refused() {
    let refused = |host: &HostFacts<'_>, overrides: &str, dir: &str| {
        assert_eq!(
            GatePolicy::from_toml_str(overrides, host),
            Err(PolicyError::Invalid {
                key: "quarantine_dir",
                value: dir.to_owned()
            }),
            "{overrides}"
        );
    };
    // A listed folder below the first level that ends where the sub-path begins: some
    // work root other than `/` would put the quarantine inside it.
    refused(
        &POSIX,
        "posix_system_dirs = ['/work/proj/trash']\nquarantine_dir = 'trash'",
        "trash",
    );
    refused(
        &POSIX,
        "posix_system_dirs = ['/srv/vault/keep']\nquarantine_dir = 'Vault/KEEP/q'",
        "Vault/KEEP/q",
    );
    refused(
        &POSIX,
        "posix_user_parents = ['/srv/people']\nquarantine_dir = 'PEOPLE/q'",
        "PEOPLE/q",
    );
    // Names that are not a folder on Windows, in any component, quarantine on or off.
    for dir in ["nul", "q/con", "aux.d/q", "COM1"] {
        refused(&WINDOWS, &format!("quarantine_dir = '{dir}'"), dir);
        let off = format!("quarantine_enabled = false\nquarantine_dir = '{dir}'");
        refused(&WINDOWS, &off, dir);
    }

    // Safe: a name shared with a protected folder of the first level only (the work root
    // would have to be `/`, which no plan gets past), or beside the protected one.
    for overrides in [
        "quarantine_dir = 'trash'",
        "quarantine_dir = 'etc'",
        "quarantine_dir = 'etc/q'",
        "quarantine_dir = 'home/q'",
        "quarantine_dir = 'Users'",
        "quarantine_dir = 'q/etc'",
        "quarantine_dir = 'etc.d/q'",
        "quarantine_dir = 'nul'",
        "posix_system_dirs = ['/srv/vault/keep']\nquarantine_dir = 'vault/q'",
        "posix_system_dirs = ['/srv/vault']\nquarantine_dir = 'keep/vault'",
        // With quarantine off the sub-path is only checked for its shape.
        "quarantine_enabled = false\nposix_system_dirs = ['/work/proj/trash']\nquarantine_dir = 'trash'",
    ] {
        assert!(
            GatePolicy::from_toml_str(overrides, &POSIX).is_ok(),
            "{overrides}"
        );
    }
    // Safe: on a Windows policy the POSIX lists say nothing, and the Windows lists name
    // folders of the first level of a drive only.
    for overrides in [
        "quarantine_dir = 'etc/q'",
        "quarantine_dir = 'home/q'",
        "quarantine_dir = 'Windows/q'",
        "quarantine_dir = 'users/q'",
        "posix_system_dirs = ['/work/proj/trash']\nquarantine_dir = 'trash'",
        "quarantine_enabled = false\nquarantine_dir = 'etc/q'",
    ] {
        assert!(
            GatePolicy::from_toml_str(overrides, &WINDOWS).is_ok(),
            "{overrides}"
        );
    }
    // The shape is checked either way.
    for bad in ["../q", "/abs", "", "a//b", "a/./b", "C:/q"] {
        for enabled in [true, false] {
            let overrides = format!("quarantine_enabled = {enabled}\nquarantine_dir = '{bad}'");
            assert!(
                GatePolicy::from_toml_str(&overrides, &POSIX).is_err(),
                "{bad}"
            );
            assert!(
                GatePolicy::from_toml_str(&overrides, &WINDOWS).is_err(),
                "{bad}"
            );
        }
    }
}

#[test]
fn protected_allowed_root_is_refused() {
    for root in [
        "C:\\",
        "C:\\Users",
        "C:\\Users\\user-a",
        "C:\\Windows\\Temp",
        "/",
        "/var/x",
    ] {
        let overrides = format!("allowed_roots = ['{root}']");
        assert_eq!(
            GatePolicy::from_toml_str(&overrides, &WINDOWS),
            Err(PolicyError::ProtectedAllowedRoot(
                AbsPath::parse(root).unwrap().to_string()
            )),
            "{root}"
        );
    }
}

#[test]
fn user_home_must_be_a_resolved_folder() {
    for home in [
        "",
        "~",
        "%USERPROFILE%",
        "$HOME",
        "home/user-a",
        "/",
        "C:\\",
    ] {
        let host = HostFacts { user_home: home };
        assert!(GatePolicy::defaults(&host).is_err(), "{home}");
    }
}

#[test]
fn windows_protected_locations() {
    let policy = GatePolicy::defaults(&WINDOWS).unwrap();
    let cases = [
        ("C:\\", Some(Protection::FilesystemRoot)),
        ("D:\\", Some(Protection::FilesystemRoot)),
        ("C:\\Users\\user-a", Some(Protection::UserHome)),
        ("c:\\users\\USER-A", Some(Protection::UserHome)),
        ("C:\\Users", Some(Protection::UserHomeAncestor)),
        ("D:\\Users", Some(Protection::UserProfile)),
        ("C:\\Users\\user-b", Some(Protection::UserProfile)),
        ("C:\\Windows", Some(Protection::SystemDir)),
        (
            "D:\\windows\\system32\\drivers",
            Some(Protection::SystemDir),
        ),
        ("C:\\Program Files (x86)\\x", Some(Protection::SystemDir)),
        ("C:\\ProgramData", Some(Protection::SystemDir)),
        ("C:\\Users\\user-a\\work\\proj", None),
        ("C:\\work\\Windows", None),
        ("D:\\work\\proj", None),
    ];
    for (raw, expected) in cases {
        assert_eq!(protection(&policy, raw), expected, "{raw}");
    }
}

#[test]
fn posix_protected_locations() {
    let policy = GatePolicy::defaults(&POSIX).unwrap();
    let cases = [
        ("/", Some(Protection::FilesystemRoot)),
        ("/home/user-a", Some(Protection::UserHome)),
        ("/Home/User-A", Some(Protection::UserHome)),
        ("/home", Some(Protection::UserHomeAncestor)),
        ("/home/user-b", Some(Protection::UserProfile)),
        ("/Users/user-b", Some(Protection::UserProfile)),
        ("/Users", Some(Protection::UserProfile)),
        ("/etc", Some(Protection::SystemDir)),
        ("/usr/lib/x", Some(Protection::SystemDir)),
        ("/ETC/x", Some(Protection::SystemDir)),
        ("/root", Some(Protection::SystemDir)),
        ("/home/user-a/work/proj", None),
        ("/home/user-b/work", None),
        ("/work/proj", None),
        ("/work/etc", None),
    ];
    for (raw, expected) in cases {
        assert_eq!(protection(&policy, raw), expected, "{raw}");
    }
}

#[test]
fn path_in_another_style_than_the_user_home_is_protected() {
    let policy = GatePolicy::defaults(&WINDOWS).unwrap();
    for raw in [
        "/c/Users/user-a",
        "/c/Users/user-a/Documents",
        "/mnt/c/work",
        "/work",
    ] {
        assert_eq!(
            protection(&policy, raw),
            Some(Protection::OtherPathStyle),
            "{raw}"
        );
    }
    let policy = GatePolicy::defaults(&POSIX).unwrap();
    assert_eq!(
        protection(&policy, "C:\\work\\proj"),
        Some(Protection::OtherPathStyle)
    );
    assert_eq!(
        GatePolicy::from_toml_str("allowed_roots = ['/c/Users/user-a']", &WINDOWS),
        Err(PolicyError::ProtectedAllowedRoot(
            "/c/Users/user-a".to_owned()
        ))
    );
}

#[test]
fn user_home_outside_the_usual_parents_is_protected_with_its_ancestors() {
    let host = HostFacts {
        user_home: "/data/people/user-a",
    };
    let policy = GatePolicy::defaults(&host).unwrap();
    assert_eq!(
        protection(&policy, "/data/people/user-a"),
        Some(Protection::UserHome)
    );
    assert_eq!(
        protection(&policy, "/data/people"),
        Some(Protection::UserHomeAncestor)
    );
    assert_eq!(
        protection(&policy, "/data"),
        Some(Protection::UserHomeAncestor)
    );
    assert_eq!(protection(&policy, "/data/people/user-a/proj"), None);
}
