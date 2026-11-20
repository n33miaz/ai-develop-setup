//! The manifest of one quarantine batch: where each item came from, and for which action.
//!
//! Layout of a batch, inside the destination the gate chose:
//! `<destination>/<action_id>/manifest.json` and `<destination>/<action_id>/items/<path of the
//! target below the work root>`. The folder of the action is created exclusively, so two
//! actions never share one, and the path below `items` keeps two targets with the same name
//! apart.
//!
//! The manifest is written before the first move and lists the whole batch; what is really in
//! quarantine is what exists under `items`. Read back from disk it is data, never trusted.

use serde::{Deserialize, Serialize};

use ads_core::gate::{Id, PlanHash, Timestamp};

use crate::fence::{Kind, check_name};
use crate::reason::{Reason, Stop};

pub const MANIFEST_NAME: &str = "manifest.json";
pub const ITEMS_DIR: &str = "items";
pub const MANIFEST_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub version: u32,
    pub action_id: String,
    pub plan_hash: String,
    /// The instant of `action.started`.
    pub started_at: String,
    pub work_root: String,
    pub items: Vec<ManifestItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestItem {
    /// The absolute path the item was taken from.
    pub original: String,
    /// The same place as names below the work root; also its place below `items`.
    pub path: Vec<String>,
    pub kind: ItemKind,
    pub items: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    Dir,
    File,
    Link,
}

impl From<Kind> for ItemKind {
    fn from(kind: Kind) -> Self {
        match kind {
            Kind::Dir => Self::Dir,
            Kind::File => Self::File,
            Kind::Link => Self::Link,
        }
    }
}

impl Manifest {
    pub(crate) fn to_bytes(&self) -> Vec<u8> {
        let mut text = serde_json::to_string_pretty(self).expect("a manifest is plain data");
        text.push('\n');
        text.into_bytes()
    }

    /// Parses and checks everything that does not need the disk.
    pub(crate) fn from_bytes(bytes: &[u8]) -> Result<Self, Stop> {
        let invalid = |what: &str| Stop::new(Reason::ManifestInvalid, what);
        let manifest: Manifest = serde_json::from_slice(bytes)
            .map_err(|error| Stop::caused_by(Reason::ManifestInvalid, error))?;
        if manifest.version != MANIFEST_VERSION {
            return Err(invalid("unknown version"));
        }
        if let Err(error) = Id::new(manifest.action_id.as_str()) {
            return Err(invalid("the action id is not an id").with_cause(error));
        }
        if let Err(error) = PlanHash::new(manifest.plan_hash.as_str()) {
            return Err(invalid("the plan hash is not a digest").with_cause(error));
        }
        if let Err(error) = Timestamp::new(manifest.started_at.as_str()) {
            return Err(invalid("the instant is not a timestamp").with_cause(error));
        }
        for item in &manifest.items {
            if item.path.is_empty() {
                return Err(invalid("an item with no path"));
            }
            for name in &item.path {
                check_name(name).map_err(|error| {
                    invalid("an item path with a name that is not plain").with_cause(error)
                })?;
            }
        }
        Ok(manifest)
    }
}

/// The name of the folder of an action. An id may hold `:` and end in `.`, and may be the name
/// of a device: none of that is a folder name on every system.
pub(crate) fn folder_name(action_id: &Id) -> Result<String, Stop> {
    let name = action_id.as_str();
    let stem = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
    let device = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.as_bytes()[3].is_ascii_digit());
    let plain = name
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'));
    if !plain || device || name.ends_with('.') || name.contains("..") {
        return Err(Stop::new(
            Reason::ActionIdUnusable,
            format!("`{name}` cannot name a folder"),
        ));
    }
    Ok(name.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(text: &str) -> Result<String, Stop> {
        folder_name(&Id::new(text).unwrap())
    }

    #[test]
    fn an_action_id_names_a_folder_only_when_it_is_a_plain_name() {
        assert_eq!(name("act-01").unwrap(), "act-01");
        assert_eq!(
            name("01ARZ3NDEKTSV4RRFFQ69G5FAB").unwrap(),
            "01ARZ3NDEKTSV4RRFFQ69G5FAB"
        );
        for bad in ["a:b", "act.", "a..b", "nul", "NUL.txt", "com1", "LPT9.x"] {
            assert_eq!(
                name(bad).unwrap_err().reason,
                Reason::ActionIdUnusable,
                "{bad}"
            );
        }
    }

    #[test]
    fn a_manifest_read_back_is_checked_as_data() {
        let good = Manifest {
            version: 1,
            action_id: "act-01".to_owned(),
            plan_hash: format!("sha256:{}", "a".repeat(64)),
            started_at: "2026-11-15T12:00:00Z".to_owned(),
            work_root: "/work".to_owned(),
            items: vec![ManifestItem {
                original: "/work/a/build".to_owned(),
                path: vec!["a".to_owned(), "build".to_owned()],
                kind: ItemKind::Dir,
                items: 2,
                bytes: 10,
            }],
        };
        assert_eq!(Manifest::from_bytes(&good.to_bytes()).unwrap(), good);

        let broken = |change: fn(&mut Manifest)| {
            let mut manifest = good.clone();
            change(&mut manifest);
            Manifest::from_bytes(&manifest.to_bytes())
                .unwrap_err()
                .reason
        };
        assert_eq!(broken(|m| m.version = 2), Reason::ManifestInvalid);
        assert_eq!(
            broken(|m| m.plan_hash = "x".to_owned()),
            Reason::ManifestInvalid
        );
        assert_eq!(
            broken(|m| m.action_id = "../x".to_owned()),
            Reason::ManifestInvalid
        );
        assert_eq!(broken(|m| m.items[0].path.clear()), Reason::ManifestInvalid);
        assert_eq!(
            broken(|m| m.items[0].path[0] = "..".to_owned()),
            Reason::ManifestInvalid
        );
        assert_eq!(
            broken(|m| m.items[0].path[1] = "a/b".to_owned()),
            Reason::ManifestInvalid
        );
        assert_eq!(
            Manifest::from_bytes(b"{\"version\":1,\"extra\":true}")
                .unwrap_err()
                .reason,
            Reason::ManifestInvalid
        );
    }
}
