//! The concrete effects of a plan as a rehearsal finds them: every target with the inventory
//! of what is under it and, for a restore, the manifest the pairs came from. It is what an
//! approval covers; an execution whose own rehearsal finds another one stops before the disk.
//! Its contract is `urn:ads:record:action-effects:1`.

use serde::{Deserialize, Serialize};

use crate::limits::Limits;
use crate::manifest::ItemKind;
use crate::prepare::{Grant, Job};
use crate::reason::{Reason, Stop};
use crate::report::Effect;
use crate::survey::Seen;

pub const EFFECTS_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Effects {
    pub version: u32,
    pub action_id: String,
    pub plan_hash: String,
    /// The digest of the manifest a restore reads its items from; `None` for the rest.
    pub manifest: Option<String>,
    /// One per step of the report, in its order.
    pub steps: Vec<EffectStep>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectStep {
    /// The path the step is about, as the report writes it.
    pub target: String,
    pub effect: EffectKind,
    /// Where a quarantine puts the target, or where a restore takes it from.
    pub stored: Option<String>,
    /// The object acted on and, for a folder, every object of its tree, in order. Empty for
    /// an item of a manifest that is no longer in quarantine.
    pub entries: Vec<EffectEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectKind {
    Quarantine,
    Restore,
    Delete,
    DeleteRecursive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectEntry {
    /// The names below the object acted on; empty for the object itself.
    pub path: Vec<String>,
    pub kind: ItemKind,
    /// The size of a regular file; zero for the rest.
    pub bytes: u64,
    /// The identity of the object on its volume, `<volume>:<file>` in decimal.
    pub id: String,
}

impl Effects {
    pub(crate) fn of(job: &Job, grant: &Grant<'_>) -> Self {
        let steps = job
            .report
            .steps
            .iter()
            .zip(&job.ops)
            .map(|(step, op)| {
                let (effect, stored) = match &step.effect {
                    Effect::Quarantine { stored } => (EffectKind::Quarantine, Some(stored.clone())),
                    Effect::Restore { stored } => (EffectKind::Restore, Some(stored.clone())),
                    Effect::Delete => (EffectKind::Delete, None),
                    Effect::DeleteRecursive => (EffectKind::DeleteRecursive, None),
                };
                let mut entries = Vec::new();
                if let Some(op) = op {
                    flatten(&op.seen, &mut Vec::new(), &mut entries);
                }
                EffectStep {
                    target: step.target.clone(),
                    effect,
                    stored,
                    entries,
                }
            })
            .collect();
        Self {
            version: EFFECTS_VERSION,
            action_id: grant.action_id.as_str().to_owned(),
            plan_hash: grant.plan_hash.to_string(),
            manifest: job.read_manifest.clone(),
            steps,
        }
    }

    /// The canonical form of the record, the one that is measured and hashed: compact JSON
    /// with the keys of every object in byte order (see the contract).
    pub fn to_bytes(&self) -> Vec<u8> {
        let value = serde_json::to_value(self).expect("a record of effects is plain data");
        let mut out = Vec::new();
        write_canonical(&value, &mut out);
        out
    }

    /// Larger than `max_effects_bytes` is refused: the rehearsal records nothing.
    pub(crate) fn within(self, limits: &Limits) -> Result<Self, Stop> {
        let size = self.to_bytes().len() as u64;
        if size > limits.max_effects_bytes {
            return Err(Stop::new(
                Reason::EffectsTooLarge,
                format!(
                    "the effects would take {size} bytes; the limit is {}",
                    limits.max_effects_bytes
                ),
            ));
        }
        Ok(self)
    }

    /// Every entry the record names, all steps together.
    pub fn entries(&self) -> usize {
        self.steps.iter().map(|step| step.entries.len()).sum()
    }
}

fn flatten(seen: &Seen, path: &mut Vec<String>, out: &mut Vec<EffectEntry>) {
    out.push(EffectEntry {
        path: path.clone(),
        kind: seen.kind.into(),
        bytes: seen.size,
        id: format!("{}:{}", seen.id.volume, seen.id.file),
    });
    for (name, child) in &seen.children {
        path.push(name.clone());
        flatten(child, path, out);
        path.pop();
    }
}

/// Sorts the keys itself: a `Value` keeps insertion order once any crate in the build turns on
/// `serde_json/preserve_order`, which would change every stored hash.
fn write_canonical(value: &serde_json::Value, out: &mut Vec<u8>) {
    use serde_json::Value;
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_unstable();
            out.push(b'{');
            for (at, key) in keys.into_iter().enumerate() {
                if at > 0 {
                    out.push(b',');
                }
                serde_json::to_writer(&mut *out, key).expect("a key is text");
                out.push(b':');
                write_canonical(&map[key.as_str()], out);
            }
            out.push(b'}');
        }
        Value::Array(items) => {
            out.push(b'[');
            for (at, item) in items.iter().enumerate() {
                if at > 0 {
                    out.push(b',');
                }
                write_canonical(item, out);
            }
            out.push(b']');
        }
        scalar => serde_json::to_writer(&mut *out, scalar).expect("a scalar is plain data"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_canonical_form_sorts_the_keys_and_has_no_spaces() {
        let effects = Effects {
            version: EFFECTS_VERSION,
            action_id: "act-1".to_owned(),
            plan_hash: "sha256:00".to_owned(),
            manifest: None,
            steps: vec![EffectStep {
                target: "/w/a".to_owned(),
                effect: EffectKind::Delete,
                stored: None,
                entries: vec![EffectEntry {
                    path: vec![],
                    kind: ItemKind::File,
                    bytes: 3,
                    id: "1:2".to_owned(),
                }],
            }],
        };
        assert_eq!(
            String::from_utf8(effects.to_bytes()).unwrap(),
            concat!(
                r#"{"action_id":"act-1","manifest":null,"plan_hash":"sha256:00","#,
                r#""steps":[{"effect":"delete","entries":[{"bytes":3,"id":"1:2","kind":"file","#,
                r#""path":[]}],"stored":null,"target":"/w/a"}],"version":1}"#
            )
        );
    }
}
