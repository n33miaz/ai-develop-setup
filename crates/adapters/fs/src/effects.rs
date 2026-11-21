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

    /// The record as it is stored and hashed.
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("a record of effects is plain data")
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
