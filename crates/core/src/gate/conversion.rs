//! Explicit wire conversions; deserialization never constructs an action or a permit.

use super::*;

macro_rules! enum_pair {
    ($domain:ident, $wire:ident, $($variant:ident),+ $(,)?) => {
        impl From<wire::$wire> for $domain {
            fn from(value: wire::$wire) -> Self {
                match value { $(wire::$wire::$variant => Self::$variant),+ }
            }
        }
        impl From<$domain> for wire::$wire {
            fn from(value: $domain) -> Self {
                match value { $($domain::$variant => Self::$variant),+ }
            }
        }
    };
}

enum_pair!(Risk, CommonRiskV1, Low, Medium, High);

impl From<wire::CommonTimestampV1> for Timestamp {
    fn from(value: wire::CommonTimestampV1) -> Self {
        Self(value.as_str().to_owned())
    }
}
impl From<Timestamp> for wire::CommonTimestampV1 {
    fn from(value: Timestamp) -> Self {
        Self::new(value.as_str()).expect("validated domain text")
    }
}

impl From<wire::CommonSha256V1> for PlanHash {
    fn from(value: wire::CommonSha256V1) -> Self {
        Self(value.as_str().to_owned())
    }
}
impl From<PlanHash> for wire::CommonSha256V1 {
    fn from(value: PlanHash) -> Self {
        Self::new(value.as_str()).expect("validated domain text")
    }
}

fn to_legacy_via(value: Via) -> wire::EventApprovalGrantedV1DataVia {
    match value {
        Via::LocalUi => wire::EventApprovalGrantedV1DataVia::LocalUi,
        Via::Tray => wire::EventApprovalGrantedV1DataVia::Tray,
        Via::RemoteDevice => wire::EventApprovalGrantedV1DataVia::RemoteDevice,
    }
}
fn from_legacy_via(value: wire::EventApprovalGrantedV1DataVia) -> Via {
    match value {
        wire::EventApprovalGrantedV1DataVia::LocalUi => Via::LocalUi,
        wire::EventApprovalGrantedV1DataVia::Tray => Via::Tray,
        wire::EventApprovalGrantedV1DataVia::RemoteDevice => Via::RemoteDevice,
    }
}

fn to_approval_record_via(value: Via) -> wire::RecordApprovalV1Via {
    match value {
        Via::LocalUi => wire::RecordApprovalV1Via::LocalUi,
        Via::Tray => wire::RecordApprovalV1Via::Tray,
        Via::RemoteDevice => wire::RecordApprovalV1Via::RemoteDevice,
    }
}
fn from_approval_record_via(value: wire::RecordApprovalV1Via) -> Via {
    match value {
        wire::RecordApprovalV1Via::LocalUi => Via::LocalUi,
        wire::RecordApprovalV1Via::Tray => Via::Tray,
        wire::RecordApprovalV1Via::RemoteDevice => Via::RemoteDevice,
    }
}

// Each event supplies its field maps; envelope construction and stream binding are shared.
macro_rules! event_wire {
    ($domain:ident, $wire:ident, $urn:literal, $family:literal, $entity:ident,
     $value:ident => $read:expr, $envelope:ident => $write:expr $(,)?) => {
        impl EventData for $domain {
            type Wire = wire::$wire;
            const TYPE: &'static str = $urn;
            const STREAM: &'static str = $family;
            fn stream_id(&self) -> &Id {
                &self.$entity
            }
            fn from_wire($value: Self::Wire) -> Result<Envelope<Self>, ContractError> {
                let data = $read;
                if $value.stream.as_str() != format!("{}/{}", Self::STREAM, data.stream_id()) {
                    return Err(ContractError::new("event stream", $value.stream.as_str()));
                }
                Ok(Envelope::new(
                    EventMeta {
                        id: $value.id,
                        ts: $value.ts.into(),
                        seq: $value.seq,
                        actor: $value.actor,
                        cause: $value.cause,
                        data_class: $value.data_class,
                    },
                    data,
                ))
            }
            fn to_wire($envelope: &Envelope<Self>) -> Self::Wire {
                wire::$wire {
                    id: $envelope.id.clone(),
                    r#type: Default::default(),
                    ts: $envelope.ts.clone().into(),
                    stream: format!("{}/{}", Self::STREAM, $envelope.data.stream_id())
                        .try_into()
                        .expect("bound event stream"),
                    seq: $envelope.seq,
                    actor: $envelope.actor.clone(),
                    cause: $envelope.cause.clone(),
                    data_class: $envelope.data_class,
                    data: $write,
                }
            }
        }
    };
}

event_wire!(
    ActionPlanned,
    EventActionPlannedV1,
    "urn:ads:event:action.planned:1",
    "action",
    action_id,
    value => Self {
        action_id: value.data.action_id,
        verb: value.data.verb,
        risk: value.data.risk.into(),
        plan_hash: value.data.plan_hash.into(),
        needs_elevation: value.data.needs_elevation,
    },
    envelope => wire::EventActionPlannedV1Data {
        action_id: envelope.data.action_id.clone(),
        verb: envelope.data.verb.clone(),
        risk: envelope.data.risk.into(),
        plan_hash: envelope.data.plan_hash.clone().into(),
        needs_elevation: envelope.data.needs_elevation,
    },
);

event_wire!(
    ActionDenied,
    EventActionDeniedV1,
    "urn:ads:event:action.denied:1",
    "action",
    action_id,
    value => Self {
        action_id: value.data.action_id,
        verb: Verb::new(value.data.verb.as_str())?,
        plan_hash: value.data.plan_hash.into(),
        rules: value.data.rules,
    },
    envelope => wire::EventActionDeniedV1Data {
        action_id: envelope.data.action_id.clone(),
        verb: wire::EventActionDeniedV1DataVerb::new(envelope.data.verb.clone().as_str())
            .expect("validated verb"),
        plan_hash: envelope.data.plan_hash.clone().into(),
        rules: envelope.data.rules.clone(),
    },
);

event_wire!(
    ActionStarted,
    EventActionStartedV1,
    "urn:ads:event:action.started:1",
    "action",
    action_id,
    value => Self {
        action_id: value.data.action_id,
        plan_hash: value.data.plan_hash.into(),
        approval_id: value.data.approval_id,
    },
    envelope => wire::EventActionStartedV1Data {
        action_id: envelope.data.action_id.clone(),
        plan_hash: envelope.data.plan_hash.clone().into(),
        approval_id: envelope.data.approval_id.clone(),
    },
);

event_wire!(
    ActionExecuted,
    EventActionExecutedV1,
    "urn:ads:event:action.executed:1",
    "action",
    action_id,
    value => Self {
        action_id: value.data.action_id,
        plan_hash: value.data.plan_hash.into(),
        approval_id: value.data.approval_id,
    },
    envelope => wire::EventActionExecutedV1Data {
        action_id: envelope.data.action_id.clone(),
        plan_hash: envelope.data.plan_hash.clone().into(),
        approval_id: envelope.data.approval_id.clone(),
    },
);

event_wire!(
    ActionFailed,
    EventActionFailedV1,
    "urn:ads:event:action.failed:1",
    "action",
    action_id,
    value => Self {
        action_id: value.data.action_id,
        plan_hash: value.data.plan_hash.into(),
        error_code: value.data.error_code,
    },
    envelope => wire::EventActionFailedV1Data {
        action_id: envelope.data.action_id.clone(),
        plan_hash: envelope.data.plan_hash.clone().into(),
        error_code: envelope.data.error_code.clone(),
    },
);

event_wire!(
    ApprovalRequested,
    EventApprovalRequestedV2,
    "urn:ads:event:approval.requested:2",
    "approval",
    approval_id,
    value => Self {
        approval_id: value.data.approval_id,
        action_id: value.data.action_id,
        plan_hash: value.data.plan_hash.into(),
        risk: value.data.risk.into(),
        summary: ShortText::new(value.data.summary.as_str())?,
        expires_at: value.data.expires_at.into(),
        needs_elevation: value.data.needs_elevation,
    },
    envelope => wire::EventApprovalRequestedV2Data {
        approval_id: envelope.data.approval_id.clone(),
        action_id: envelope.data.action_id.clone(),
        plan_hash: envelope.data.plan_hash.clone().into(),
        risk: envelope.data.risk.into(),
        summary: wire::EventApprovalRequestedV2DataSummary::new(
            envelope.data.summary.clone().as_str(),
        )
        .expect("validated domain text"),
        expires_at: envelope.data.expires_at.clone().into(),
        needs_elevation: envelope.data.needs_elevation,
    },
);

event_wire!(
    ApprovalGranted,
    EventApprovalGrantedV2,
    "urn:ads:event:approval.granted:2",
    "approval",
    approval_id,
    value => Self {
        approval_id: value.data.approval_id,
        action_id: value.data.action_id,
        plan_hash: value.data.plan_hash.into(),
        via: value.data.via,
    },
    envelope => wire::EventApprovalGrantedV2Data {
        approval_id: envelope.data.approval_id.clone(),
        action_id: envelope.data.action_id.clone(),
        plan_hash: envelope.data.plan_hash.clone().into(),
        via: envelope.data.via,
    },
);

event_wire!(
    ApprovalDenied,
    EventApprovalDeniedV2,
    "urn:ads:event:approval.denied:2",
    "approval",
    approval_id,
    value => Self {
        approval_id: value.data.approval_id,
        action_id: value.data.action_id,
        plan_hash: value.data.plan_hash.into(),
        reason: value
            .data
            .reason
            .map(|text| ShortText::new(text.as_str()))
            .transpose()?,
    },
    envelope => wire::EventApprovalDeniedV2Data {
        approval_id: envelope.data.approval_id.clone(),
        action_id: envelope.data.action_id.clone(),
        plan_hash: envelope.data.plan_hash.clone().into(),
        reason: envelope.data.reason.clone().map(|text| {
            wire::EventApprovalDeniedV2DataReason::new(text.as_str())
                .expect("validated domain text")
        }),
    },
);

event_wire!(
    ApprovalExpired,
    EventApprovalExpiredV2,
    "urn:ads:event:approval.expired:2",
    "approval",
    approval_id,
    value => Self {
        approval_id: value.data.approval_id,
        action_id: value.data.action_id,
        plan_hash: value.data.plan_hash.into(),
    },
    envelope => wire::EventApprovalExpiredV2Data {
        approval_id: envelope.data.approval_id.clone(),
        action_id: envelope.data.action_id.clone(),
        plan_hash: envelope.data.plan_hash.clone().into(),
    },
);

event_wire!(
    ApprovalRequestedV1,
    EventApprovalRequestedV1,
    "urn:ads:event:approval.requested:1",
    "approval",
    approval_id,
    value => Self {
        approval_id: value.data.approval_id,
        plan_hash: value.data.plan_hash.into(),
        risk: value.data.risk.into(),
        summary: ShortText::new(value.data.summary.as_str())?,
        expires_at: value.data.expires_at.into(),
        needs_elevation: value.data.needs_elevation,
    },
    envelope => wire::EventApprovalRequestedV1Data {
        approval_id: envelope.data.approval_id.clone(),
        plan_hash: envelope.data.plan_hash.clone().into(),
        risk: envelope.data.risk.into(),
        summary: wire::EventApprovalRequestedV1DataSummary::new(
            envelope.data.summary.clone().as_str(),
        )
        .expect("validated domain text"),
        expires_at: envelope.data.expires_at.clone().into(),
        needs_elevation: envelope.data.needs_elevation,
    },
);

event_wire!(
    ApprovalGrantedV1,
    EventApprovalGrantedV1,
    "urn:ads:event:approval.granted:1",
    "approval",
    approval_id,
    value => Self {
        approval_id: value.data.approval_id,
        plan_hash: value.data.plan_hash.into(),
        via: from_legacy_via(value.data.via),
    },
    envelope => wire::EventApprovalGrantedV1Data {
        approval_id: envelope.data.approval_id.clone(),
        plan_hash: envelope.data.plan_hash.clone().into(),
        via: to_legacy_via(envelope.data.via),
    },
);

event_wire!(
    ApprovalDeniedV1,
    EventApprovalDeniedV1,
    "urn:ads:event:approval.denied:1",
    "approval",
    approval_id,
    value => Self {
        approval_id: value.data.approval_id,
        plan_hash: value.data.plan_hash.into(),
        reason: value
            .data
            .reason
            .map(|text| ShortText::new(text.as_str()))
            .transpose()?,
    },
    envelope => wire::EventApprovalDeniedV1Data {
        approval_id: envelope.data.approval_id.clone(),
        plan_hash: envelope.data.plan_hash.clone().into(),
        reason: envelope.data.reason.clone().map(|text| {
            wire::EventApprovalDeniedV1DataReason::new(text.as_str())
                .expect("validated domain text")
        }),
    },
);

event_wire!(
    ApprovalExpiredV1,
    EventApprovalExpiredV1,
    "urn:ads:event:approval.expired:1",
    "approval",
    approval_id,
    value => Self {
        approval_id: value.data.approval_id,
        plan_hash: value.data.plan_hash.into(),
    },
    envelope => wire::EventApprovalExpiredV1Data {
        approval_id: envelope.data.approval_id.clone(),
        plan_hash: envelope.data.plan_hash.clone().into(),
    },
);

enum_pair!(
    Decision,
    RecordActionPlanV2VerdictDecision,
    Allowed,
    RequiresApproval,
    Denied
);

impl TryFrom<wire::RecordActionPlanV2> for ActionPlanRecord {
    type Error = ContractError;
    fn try_from(value: wire::RecordActionPlanV2) -> Result<Self, Self::Error> {
        let value = value.into_fields();
        let verdict = value.verdict.into_fields();
        ActionPlanFields {
            version: 2,
            action_id: value.action_id,
            plan_hash: value.plan_hash.into(),
            operation: value.operation,
            work_root: PathText::new(value.work_root.as_str())?,
            destination: value
                .destination
                .map(|path| PathText::new(path.as_str()))
                .transpose()?,
            targets: value
                .targets
                .into_vec()
                .into_iter()
                .map(|target| {
                    Ok(PlanTarget {
                        path: PathText::new(target.path.as_str())?,
                        pattern: target
                            .pattern
                            .map(|text| PatternText::new(text.as_str()))
                            .transpose()?,
                        repo_root: target.repo_root,
                    })
                })
                .collect::<Result<_, ContractError>>()?,
            estimate: value.estimate.map(|value| PlanEstimate {
                items: value.items.get(),
                bytes: value.bytes.get(),
            }),
            verdict: PlanVerdict {
                decision: verdict.decision.into(),
                risk: verdict.risk.into(),
                reasons: verdict
                    .reasons
                    .into_vec()
                    .into_iter()
                    .map(|reason| {
                        Ok(PlanReason {
                            rule: reason.rule,
                            subject: SubjectText::new(reason.subject.as_str())?,
                        })
                    })
                    .collect::<Result<_, ContractError>>()?,
            },
            quarantined: value.quarantined,
        }
        .try_into()
    }
}
impl From<ActionPlanRecord> for wire::RecordActionPlanV2 {
    fn from(value: ActionPlanRecord) -> Self {
        let fields = value.0;
        Self::new(wire::RecordActionPlanV2Fields {
            version: Default::default(),
            action_id: fields.action_id,
            plan_hash: fields.plan_hash.into(),
            operation: fields.operation,
            work_root: wire::CommonAbspathV2::new(fields.work_root.as_str())
                .expect("normalized path"),
            destination: fields
                .destination
                .map(|path| wire::CommonAbspathV2::new(path.as_str()).expect("normalized path")),
            targets: wire::RecordActionPlanV2Targets::new(
                fields
                    .targets
                    .into_iter()
                    .map(|target| wire::RecordActionPlanV2TargetsItem {
                        path: wire::CommonAbspathV2::new(target.path.as_str())
                            .expect("normalized path"),
                        pattern: target.pattern.map(|text| {
                            wire::RecordActionPlanV2TargetsItemPattern::new(text.as_str())
                                .expect("validated pattern")
                        }),
                        repo_root: target.repo_root,
                    })
                    .collect(),
            )
            .expect("canonical targets"),
            estimate: fields
                .estimate
                .map(|value| wire::RecordActionPlanV2Estimate {
                    items: wire::RecordActionPlanV2EstimateItems::new(value.items)
                        .expect("bounded estimate"),
                    bytes: wire::RecordActionPlanV2EstimateBytes::new(value.bytes)
                        .expect("bounded estimate"),
                }),
            verdict: wire::RecordActionPlanV2Verdict::new(wire::RecordActionPlanV2VerdictFields {
                decision: fields.verdict.decision.into(),
                risk: fields.verdict.risk.into(),
                reasons: wire::RecordActionPlanV2VerdictReasons::new(
                    fields
                        .verdict
                        .reasons
                        .into_iter()
                        .map(|reason| wire::RecordActionPlanV2VerdictReasonsItem {
                            rule: reason.rule,
                            subject: wire::RecordActionPlanV2VerdictReasonsItemSubject::new(
                                reason.subject.as_str(),
                            )
                            .expect("validated subject"),
                        })
                        .collect(),
                )
                .expect("bounded reasons"),
            })
            .expect("coherent verdict"),
            quarantined: fields.quarantined,
        })
        .expect("checked action plan")
    }
}

enum_pair!(
    Decision,
    RecordActionPlanV1VerdictDecision,
    Allowed,
    RequiresApproval,
    Denied
);

fn operation_v1(value: wire::RecordActionPlanV1Operation) -> PlanOperation {
    match value {
        wire::RecordActionPlanV1Operation::Delete => PlanOperation::Delete,
        wire::RecordActionPlanV1Operation::DeleteRecursive => PlanOperation::DeleteRecursive,
        wire::RecordActionPlanV1Operation::Move => PlanOperation::Move,
        wire::RecordActionPlanV1Operation::Overwrite => PlanOperation::Overwrite,
        wire::RecordActionPlanV1Operation::CleanUntracked => PlanOperation::CleanUntracked,
    }
}
fn to_operation_v1(value: PlanOperation) -> wire::RecordActionPlanV1Operation {
    match value {
        PlanOperation::Delete => wire::RecordActionPlanV1Operation::Delete,
        PlanOperation::DeleteRecursive => wire::RecordActionPlanV1Operation::DeleteRecursive,
        PlanOperation::Move => wire::RecordActionPlanV1Operation::Move,
        PlanOperation::Overwrite => wire::RecordActionPlanV1Operation::Overwrite,
        PlanOperation::CleanUntracked => wire::RecordActionPlanV1Operation::CleanUntracked,
    }
}

impl TryFrom<wire::RecordActionPlanV1> for ActionPlanRecordV1 {
    type Error = ContractError;
    fn try_from(value: wire::RecordActionPlanV1) -> Result<Self, Self::Error> {
        let value = value.into_fields();
        let verdict = value.verdict.into_fields();
        ActionPlanFields {
            version: 1,
            action_id: value.action_id,
            plan_hash: value.plan_hash.into(),
            operation: operation_v1(value.operation),
            work_root: PathText::new(value.work_root.as_str())?,
            destination: value
                .destination
                .map(|path| PathText::new(path.as_str()))
                .transpose()?,
            targets: value
                .targets
                .into_vec()
                .into_iter()
                .map(|target| {
                    Ok(PlanTarget {
                        path: PathText::new(target.path.as_str())?,
                        pattern: target
                            .pattern
                            .map(|text| PatternText::new(text.as_str()))
                            .transpose()?,
                        repo_root: target.repo_root,
                    })
                })
                .collect::<Result<_, ContractError>>()?,
            estimate: value.estimate.map(|value| PlanEstimate {
                items: value.items,
                bytes: value.bytes,
            }),
            verdict: PlanVerdict {
                decision: verdict.decision.into(),
                risk: verdict.risk.into(),
                reasons: verdict
                    .reasons
                    .into_vec()
                    .into_iter()
                    .map(|reason| {
                        Ok(PlanReason {
                            rule: reason.rule,
                            subject: SubjectText::new(reason.subject.as_str())?,
                        })
                    })
                    .collect::<Result<_, ContractError>>()?,
            },
            quarantined: value.quarantined,
        }
        .try_into()
    }
}
impl From<ActionPlanRecordV1> for wire::RecordActionPlanV1 {
    fn from(value: ActionPlanRecordV1) -> Self {
        let fields = value.0;
        Self::new(wire::RecordActionPlanV1Fields {
            version: Default::default(),
            action_id: fields.action_id,
            plan_hash: fields.plan_hash.into(),
            operation: to_operation_v1(fields.operation),
            work_root: wire::CommonAbspathV1::new(fields.work_root.as_str())
                .expect("normalized path"),
            destination: fields
                .destination
                .map(|path| wire::CommonAbspathV1::new(path.as_str()).expect("normalized path")),
            targets: wire::RecordActionPlanV1Targets::new(
                fields
                    .targets
                    .into_iter()
                    .map(|target| wire::RecordActionPlanV1TargetsItem {
                        path: wire::CommonAbspathV1::new(target.path.as_str())
                            .expect("normalized path"),
                        pattern: target.pattern.map(|text| {
                            wire::RecordActionPlanV1TargetsItemPattern::new(text.as_str())
                                .expect("validated pattern")
                        }),
                        repo_root: target.repo_root,
                    })
                    .collect(),
            )
            .expect("canonical targets"),
            estimate: fields
                .estimate
                .map(|value| wire::RecordActionPlanV1Estimate {
                    items: value.items,
                    bytes: value.bytes,
                }),
            verdict: wire::RecordActionPlanV1Verdict::new(wire::RecordActionPlanV1VerdictFields {
                decision: fields.verdict.decision.into(),
                risk: fields.verdict.risk.into(),
                reasons: wire::RecordActionPlanV1VerdictReasons::new(
                    fields
                        .verdict
                        .reasons
                        .into_iter()
                        .map(|reason| wire::RecordActionPlanV1VerdictReasonsItem {
                            rule: reason.rule,
                            subject: wire::RecordActionPlanV1VerdictReasonsItemSubject::new(
                                reason.subject.as_str(),
                            )
                            .expect("validated subject"),
                        })
                        .collect(),
                )
                .expect("bounded reasons"),
            })
            .expect("coherent verdict"),
            quarantined: fields.quarantined,
        })
        .expect("checked action plan")
    }
}

impl TryFrom<wire::RecordApprovalV1> for ApprovalRecord {
    type Error = ContractError;
    fn try_from(value: wire::RecordApprovalV1) -> Result<Self, Self::Error> {
        let value = value.into_fields();
        ApprovalFields {
            version: 1,
            approval_id: value.approval_id,
            plan_hash: value.plan_hash.into(),
            risk: value.risk.into(),
            summary: ShortText::new(value.summary.as_str())?,
            status: value.status,
            requested_at: value.requested_at.into(),
            expires_at: value.expires_at.into(),
            decided_at: value.decided_at.map(Into::into),
            decided_by: value.decided_by,
            via: value.via.map(from_approval_record_via),
            reason: value
                .reason
                .map(|text| ShortText::new(text.as_str()))
                .transpose()?,
        }
        .try_into()
    }
}
impl From<ApprovalRecord> for wire::RecordApprovalV1 {
    fn from(value: ApprovalRecord) -> Self {
        let fields = value.0;
        Self::new(wire::RecordApprovalV1Fields {
            version: Default::default(),
            approval_id: fields.approval_id,
            plan_hash: fields.plan_hash.into(),
            risk: fields.risk.into(),
            summary: wire::RecordApprovalV1Summary::new(fields.summary.as_str())
                .expect("validated summary"),
            status: fields.status,
            requested_at: fields.requested_at.into(),
            expires_at: fields.expires_at.into(),
            decided_at: fields.decided_at.map(Into::into),
            decided_by: fields.decided_by,
            via: fields.via.map(to_approval_record_via),
            reason: fields.reason.map(|text| {
                wire::RecordApprovalV1Reason::new(text.as_str()).expect("validated reason")
            }),
        })
        .expect("coherent approval")
    }
}
