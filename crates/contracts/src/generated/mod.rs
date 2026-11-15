// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas. Não edite à mão.

use serde_json::Value;

use crate::support;

pub mod command_request_v1;
pub mod command_response_v1;
pub mod command_response_v2;
pub mod common_abspath_v1;
pub mod common_action_rule_v1;
pub mod common_actor_v1;
pub mod common_base64_v1;
pub mod common_data_class_v1;
pub mod common_id_v1;
pub mod common_relpath_v1;
pub mod common_risk_v1;
pub mod common_sha256_v1;
pub mod common_timestamp_v1;
pub mod common_ulid_v1;
pub mod contract_harness_adapter_v1;
pub mod contract_pty_host_frame_v1;
pub mod contract_pty_host_notification_v1;
pub mod contract_pty_host_request_v1;
pub mod contract_pty_host_response_v1;
pub mod event_action_denied_v1;
pub mod event_action_executed_v1;
pub mod event_action_failed_v1;
pub mod event_action_planned_v1;
pub mod event_action_started_v1;
pub mod event_approval_denied_v1;
pub mod event_approval_expired_v1;
pub mod event_approval_granted_v1;
pub mod event_approval_requested_v1;
pub mod event_approval_requested_v2;
pub mod event_envelope_v1;
pub mod event_host_alert_acked_v1;
pub mod event_host_alert_raised_v1;
pub mod event_host_sample_recorded_v1;
pub mod event_review_completed_v1;
pub mod event_review_failed_v1;
pub mod event_review_requested_v1;
pub mod event_routing_decided_v1;
pub mod event_routing_fallback_v1;
pub mod event_routing_proposed_v1;
pub mod event_session_activity_changed_v1;
pub mod event_session_created_v1;
pub mod event_session_ended_v1;
pub mod event_session_output_v1;
pub mod event_session_permission_asked_v1;
pub mod event_session_resized_v1;
pub mod event_task_cancelled_v1;
pub mod event_task_completed_v1;
pub mod event_task_created_v1;
pub mod event_task_failed_v1;
pub mod event_task_started_v1;
pub mod event_usage_reconciled_v1;
pub mod event_usage_recorded_v1;
pub mod record_action_plan_v1;
pub mod record_approval_v1;
pub mod record_host_event_v1;
pub mod record_host_sample_v1;
pub mod record_parameter_v1;
pub mod record_task_packet_v1;
pub mod record_update_manifest_v1;
pub mod record_update_status_v1;
pub mod record_usage_event_v1;
pub mod relay_device_v1;
pub mod relay_envelope_v1;

pub use command_request_v1::*;
pub use command_response_v1::*;
pub use command_response_v2::*;
pub use common_abspath_v1::*;
pub use common_action_rule_v1::*;
pub use common_actor_v1::*;
pub use common_base64_v1::*;
pub use common_data_class_v1::*;
pub use common_id_v1::*;
pub use common_relpath_v1::*;
pub use common_risk_v1::*;
pub use common_sha256_v1::*;
pub use common_timestamp_v1::*;
pub use common_ulid_v1::*;
pub use contract_harness_adapter_v1::*;
pub use contract_pty_host_frame_v1::*;
pub use contract_pty_host_notification_v1::*;
pub use contract_pty_host_request_v1::*;
pub use contract_pty_host_response_v1::*;
pub use event_action_denied_v1::*;
pub use event_action_executed_v1::*;
pub use event_action_failed_v1::*;
pub use event_action_planned_v1::*;
pub use event_action_started_v1::*;
pub use event_approval_denied_v1::*;
pub use event_approval_expired_v1::*;
pub use event_approval_granted_v1::*;
pub use event_approval_requested_v1::*;
pub use event_approval_requested_v2::*;
pub use event_envelope_v1::*;
pub use event_host_alert_acked_v1::*;
pub use event_host_alert_raised_v1::*;
pub use event_host_sample_recorded_v1::*;
pub use event_review_completed_v1::*;
pub use event_review_failed_v1::*;
pub use event_review_requested_v1::*;
pub use event_routing_decided_v1::*;
pub use event_routing_fallback_v1::*;
pub use event_routing_proposed_v1::*;
pub use event_session_activity_changed_v1::*;
pub use event_session_created_v1::*;
pub use event_session_ended_v1::*;
pub use event_session_output_v1::*;
pub use event_session_permission_asked_v1::*;
pub use event_session_resized_v1::*;
pub use event_task_cancelled_v1::*;
pub use event_task_completed_v1::*;
pub use event_task_created_v1::*;
pub use event_task_failed_v1::*;
pub use event_task_started_v1::*;
pub use event_usage_reconciled_v1::*;
pub use event_usage_recorded_v1::*;
pub use record_action_plan_v1::*;
pub use record_approval_v1::*;
pub use record_host_event_v1::*;
pub use record_host_sample_v1::*;
pub use record_parameter_v1::*;
pub use record_task_packet_v1::*;
pub use record_update_manifest_v1::*;
pub use record_update_status_v1::*;
pub use record_usage_event_v1::*;
pub use relay_device_v1::*;
pub use relay_envelope_v1::*;

/// Every Schema a type was generated from: its file key and its `$id`.
pub const SCHEMAS: &[(&str, &str)] = &[
    ("command.request.v1", "urn:ads:command:request:1"),
    ("command.response.v1", "urn:ads:command:response:1"),
    ("command.response.v2", "urn:ads:command:response:2"),
    ("common.abspath.v1", "urn:ads:common:abspath:1"),
    ("common.action-rule.v1", "urn:ads:common:action-rule:1"),
    ("common.actor.v1", "urn:ads:common:actor:1"),
    ("common.base64.v1", "urn:ads:common:base64:1"),
    ("common.data-class.v1", "urn:ads:common:data-class:1"),
    ("common.id.v1", "urn:ads:common:id:1"),
    ("common.relpath.v1", "urn:ads:common:relpath:1"),
    ("common.risk.v1", "urn:ads:common:risk:1"),
    ("common.sha256.v1", "urn:ads:common:sha256:1"),
    ("common.timestamp.v1", "urn:ads:common:timestamp:1"),
    ("common.ulid.v1", "urn:ads:common:ulid:1"),
    (
        "contract.harness-adapter.v1",
        "urn:ads:contract:harness-adapter:1",
    ),
    (
        "contract.pty-host.frame.v1",
        "urn:ads:contract:pty-host.frame:1",
    ),
    (
        "contract.pty-host.notification.v1",
        "urn:ads:contract:pty-host.notification:1",
    ),
    (
        "contract.pty-host.request.v1",
        "urn:ads:contract:pty-host.request:1",
    ),
    (
        "contract.pty-host.response.v1",
        "urn:ads:contract:pty-host.response:1",
    ),
    ("event.action.denied.v1", "urn:ads:event:action.denied:1"),
    (
        "event.action.executed.v1",
        "urn:ads:event:action.executed:1",
    ),
    ("event.action.failed.v1", "urn:ads:event:action.failed:1"),
    ("event.action.planned.v1", "urn:ads:event:action.planned:1"),
    ("event.action.started.v1", "urn:ads:event:action.started:1"),
    (
        "event.approval.denied.v1",
        "urn:ads:event:approval.denied:1",
    ),
    (
        "event.approval.expired.v1",
        "urn:ads:event:approval.expired:1",
    ),
    (
        "event.approval.granted.v1",
        "urn:ads:event:approval.granted:1",
    ),
    (
        "event.approval.requested.v1",
        "urn:ads:event:approval.requested:1",
    ),
    (
        "event.approval.requested.v2",
        "urn:ads:event:approval.requested:2",
    ),
    ("event.envelope.v1", "urn:ads:event:envelope:1"),
    (
        "event.host.alert_acked.v1",
        "urn:ads:event:host.alert_acked:1",
    ),
    (
        "event.host.alert_raised.v1",
        "urn:ads:event:host.alert_raised:1",
    ),
    (
        "event.host.sample_recorded.v1",
        "urn:ads:event:host.sample_recorded:1",
    ),
    (
        "event.review.completed.v1",
        "urn:ads:event:review.completed:1",
    ),
    ("event.review.failed.v1", "urn:ads:event:review.failed:1"),
    (
        "event.review.requested.v1",
        "urn:ads:event:review.requested:1",
    ),
    (
        "event.routing.decided.v1",
        "urn:ads:event:routing.decided:1",
    ),
    (
        "event.routing.fallback.v1",
        "urn:ads:event:routing.fallback:1",
    ),
    (
        "event.routing.proposed.v1",
        "urn:ads:event:routing.proposed:1",
    ),
    (
        "event.session.activity_changed.v1",
        "urn:ads:event:session.activity_changed:1",
    ),
    (
        "event.session.created.v1",
        "urn:ads:event:session.created:1",
    ),
    ("event.session.ended.v1", "urn:ads:event:session.ended:1"),
    ("event.session.output.v1", "urn:ads:event:session.output:1"),
    (
        "event.session.permission_asked.v1",
        "urn:ads:event:session.permission_asked:1",
    ),
    (
        "event.session.resized.v1",
        "urn:ads:event:session.resized:1",
    ),
    ("event.task.cancelled.v1", "urn:ads:event:task.cancelled:1"),
    ("event.task.completed.v1", "urn:ads:event:task.completed:1"),
    ("event.task.created.v1", "urn:ads:event:task.created:1"),
    ("event.task.failed.v1", "urn:ads:event:task.failed:1"),
    ("event.task.started.v1", "urn:ads:event:task.started:1"),
    (
        "event.usage.reconciled.v1",
        "urn:ads:event:usage.reconciled:1",
    ),
    ("event.usage.recorded.v1", "urn:ads:event:usage.recorded:1"),
    ("record.action-plan.v1", "urn:ads:record:action-plan:1"),
    ("record.approval.v1", "urn:ads:record:approval:1"),
    ("record.host_event.v1", "urn:ads:record:host_event:1"),
    ("record.host_sample.v1", "urn:ads:record:host_sample:1"),
    ("record.parameter.v1", "urn:ads:record:parameter:1"),
    ("record.task-packet.v1", "urn:ads:record:task-packet:1"),
    (
        "record.update-manifest.v1",
        "urn:ads:record:update-manifest:1",
    ),
    ("record.update-status.v1", "urn:ads:record:update-status:1"),
    ("record.usage_event.v1", "urn:ads:record:usage_event:1"),
    ("relay.device.v1", "urn:ads:relay:device:1"),
    ("relay.envelope.v1", "urn:ads:relay:envelope:1"),
];

/// Every text pattern the generated types use, as handed to the regular expression engine.
pub const PATTERNS: &[&str] = &[
    r"(^|/)\.\.(/|$)",
    r"(^|[\\/])\.\.?([\\/]|$)",
    r"(^|[\\/])~",
    r"[$%]",
    r"[*?]",
    r"[\x{0000}-\x{001f}\x{007f}-\x{009f}]",
    r"^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$",
    r#"^([A-Z]:\\([^\\/:"<>|]*[^\\/:"<>|. ](\\[^\\/:"<>|]*[^\\/:"<>|. ])*)?|/([^/\\]+(/[^/\\]+)*)?)$"#,
    r"^E_[A-Z0-9_]{1,60}$",
    r"^T[0-3]$",
    r"^[0-7][0-9A-HJKMNP-TV-Z]{25}$",
    r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(\.[0-9]{1,6})?Z$",
    r"^[0-9a-f]{40}$",
    r"^[A-Za-z0-9+/]{43}=$",
    r"^[A-Za-z0-9._-][A-Za-z0-9._/-]*$",
    r"^[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$",
    r"^[^\\/$%]*[*?][^\\/$%]*$",
    r"^[^\x{0000}-\x{001f}\x{007f}-\x{009f}]*$",
    r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$",
    r"^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)*$",
    r"^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)+$",
    r"^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)?$",
    r"^[a-z][a-z0-9_]{0,31}$",
    r"^action/",
    r"^approval/",
    r"^dev-[0-9]{8}-[0-9a-f]{7,12}$",
    r"^host/",
    r"^host\.disk\.free_(gb|pct)$",
    r"^review/",
    r"^routing/",
    r"^session/",
    r"^sha256:[0-9a-f]{64}$",
    r"^task/",
    r"^urn:ads:event:[a-z][a-z0-9_-]*(\.[a-z][a-z0-9_-]*)+:[1-9][0-9]*$",
    r"^usage/",
];

/// Reads `value` as the type generated from the Schema `id` names and writes it back.
/// `None` when no Schema has that `$id`.
pub fn reencode(id: &str, value: Value) -> Option<Result<Value, serde_json::Error>> {
    let written = match id {
        "urn:ads:command:request:1" => support::reencode::<CommandRequestV1>(value),
        "urn:ads:command:response:1" => support::reencode::<CommandResponseV1>(value),
        "urn:ads:command:response:2" => support::reencode::<CommandResponseV2>(value),
        "urn:ads:common:abspath:1" => support::reencode::<CommonAbspathV1>(value),
        "urn:ads:common:action-rule:1" => support::reencode::<CommonActionRuleV1>(value),
        "urn:ads:common:actor:1" => support::reencode::<CommonActorV1>(value),
        "urn:ads:common:base64:1" => support::reencode::<CommonBase64V1>(value),
        "urn:ads:common:data-class:1" => support::reencode::<CommonDataClassV1>(value),
        "urn:ads:common:id:1" => support::reencode::<CommonIdV1>(value),
        "urn:ads:common:relpath:1" => support::reencode::<CommonRelpathV1>(value),
        "urn:ads:common:risk:1" => support::reencode::<CommonRiskV1>(value),
        "urn:ads:common:sha256:1" => support::reencode::<CommonSha256V1>(value),
        "urn:ads:common:timestamp:1" => support::reencode::<CommonTimestampV1>(value),
        "urn:ads:common:ulid:1" => support::reencode::<CommonUlidV1>(value),
        "urn:ads:contract:harness-adapter:1" => {
            support::reencode::<ContractHarnessAdapterV1>(value)
        }
        "urn:ads:contract:pty-host.frame:1" => support::reencode::<ContractPtyHostFrameV1>(value),
        "urn:ads:contract:pty-host.notification:1" => {
            support::reencode::<ContractPtyHostNotificationV1>(value)
        }
        "urn:ads:contract:pty-host.request:1" => {
            support::reencode::<ContractPtyHostRequestV1>(value)
        }
        "urn:ads:contract:pty-host.response:1" => {
            support::reencode::<ContractPtyHostResponseV1>(value)
        }
        "urn:ads:event:action.denied:1" => support::reencode::<EventActionDeniedV1>(value),
        "urn:ads:event:action.executed:1" => support::reencode::<EventActionExecutedV1>(value),
        "urn:ads:event:action.failed:1" => support::reencode::<EventActionFailedV1>(value),
        "urn:ads:event:action.planned:1" => support::reencode::<EventActionPlannedV1>(value),
        "urn:ads:event:action.started:1" => support::reencode::<EventActionStartedV1>(value),
        "urn:ads:event:approval.denied:1" => support::reencode::<EventApprovalDeniedV1>(value),
        "urn:ads:event:approval.expired:1" => support::reencode::<EventApprovalExpiredV1>(value),
        "urn:ads:event:approval.granted:1" => support::reencode::<EventApprovalGrantedV1>(value),
        "urn:ads:event:approval.requested:1" => {
            support::reencode::<EventApprovalRequestedV1>(value)
        }
        "urn:ads:event:approval.requested:2" => {
            support::reencode::<EventApprovalRequestedV2>(value)
        }
        "urn:ads:event:envelope:1" => support::reencode::<EventEnvelopeV1>(value),
        "urn:ads:event:host.alert_acked:1" => support::reencode::<EventHostAlertAckedV1>(value),
        "urn:ads:event:host.alert_raised:1" => support::reencode::<EventHostAlertRaisedV1>(value),
        "urn:ads:event:host.sample_recorded:1" => {
            support::reencode::<EventHostSampleRecordedV1>(value)
        }
        "urn:ads:event:review.completed:1" => support::reencode::<EventReviewCompletedV1>(value),
        "urn:ads:event:review.failed:1" => support::reencode::<EventReviewFailedV1>(value),
        "urn:ads:event:review.requested:1" => support::reencode::<EventReviewRequestedV1>(value),
        "urn:ads:event:routing.decided:1" => support::reencode::<EventRoutingDecidedV1>(value),
        "urn:ads:event:routing.fallback:1" => support::reencode::<EventRoutingFallbackV1>(value),
        "urn:ads:event:routing.proposed:1" => support::reencode::<EventRoutingProposedV1>(value),
        "urn:ads:event:session.activity_changed:1" => {
            support::reencode::<EventSessionActivityChangedV1>(value)
        }
        "urn:ads:event:session.created:1" => support::reencode::<EventSessionCreatedV1>(value),
        "urn:ads:event:session.ended:1" => support::reencode::<EventSessionEndedV1>(value),
        "urn:ads:event:session.output:1" => support::reencode::<EventSessionOutputV1>(value),
        "urn:ads:event:session.permission_asked:1" => {
            support::reencode::<EventSessionPermissionAskedV1>(value)
        }
        "urn:ads:event:session.resized:1" => support::reencode::<EventSessionResizedV1>(value),
        "urn:ads:event:task.cancelled:1" => support::reencode::<EventTaskCancelledV1>(value),
        "urn:ads:event:task.completed:1" => support::reencode::<EventTaskCompletedV1>(value),
        "urn:ads:event:task.created:1" => support::reencode::<EventTaskCreatedV1>(value),
        "urn:ads:event:task.failed:1" => support::reencode::<EventTaskFailedV1>(value),
        "urn:ads:event:task.started:1" => support::reencode::<EventTaskStartedV1>(value),
        "urn:ads:event:usage.reconciled:1" => support::reencode::<EventUsageReconciledV1>(value),
        "urn:ads:event:usage.recorded:1" => support::reencode::<EventUsageRecordedV1>(value),
        "urn:ads:record:action-plan:1" => support::reencode::<RecordActionPlanV1>(value),
        "urn:ads:record:approval:1" => support::reencode::<RecordApprovalV1>(value),
        "urn:ads:record:host_event:1" => support::reencode::<RecordHostEventV1>(value),
        "urn:ads:record:host_sample:1" => support::reencode::<RecordHostSampleV1>(value),
        "urn:ads:record:parameter:1" => support::reencode::<RecordParameterV1>(value),
        "urn:ads:record:task-packet:1" => support::reencode::<RecordTaskPacketV1>(value),
        "urn:ads:record:update-manifest:1" => support::reencode::<RecordUpdateManifestV1>(value),
        "urn:ads:record:update-status:1" => support::reencode::<RecordUpdateStatusV1>(value),
        "urn:ads:record:usage_event:1" => support::reencode::<RecordUsageEventV1>(value),
        "urn:ads:relay:device:1" => support::reencode::<RelayDeviceV1>(value),
        "urn:ads:relay:envelope:1" => support::reencode::<RelayEnvelopeV1>(value),
        _ => return None,
    };
    Some(written)
}
