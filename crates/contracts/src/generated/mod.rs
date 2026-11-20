// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas. Não edite à mão.

use serde_json::Value;

use crate::support;

pub mod command_auth_hello_v1;
pub mod command_events_subscribe_v1;
pub mod command_harness_list_response_v1;
pub mod command_harness_list_v1;
pub mod command_request_v1;
pub mod command_response_v1;
pub mod command_response_v2;
pub mod command_session_close_v1;
pub mod command_session_open_v1;
pub mod command_session_resize_v1;
pub mod command_session_write_v1;
pub mod common_abspath_v1;
pub mod common_abspath_v2;
pub mod common_action_rule_v1;
pub mod common_actor_v1;
pub mod common_base64_v1;
pub mod common_data_class_v1;
pub mod common_id_v1;
pub mod common_queue_task_state_v1;
pub mod common_relpath_v1;
pub mod common_risk_v1;
pub mod common_sha256_v1;
pub mod common_timestamp_v1;
pub mod common_ulid_v1;
pub mod contract_harness_adapter_v1;
pub mod contract_local_api_error_code_v1;
pub mod contract_local_api_server_message_v1;
pub mod contract_local_api_server_message_v2;
pub mod contract_pty_host_frame_v1;
pub mod contract_pty_host_notification_v1;
pub mod contract_pty_host_request_v1;
pub mod contract_pty_host_response_v1;
pub mod event_account_status_observed_v1;
pub mod event_account_switched_v1;
pub mod event_action_denied_v1;
pub mod event_action_executed_v1;
pub mod event_action_failed_v1;
pub mod event_action_planned_v1;
pub mod event_action_started_v1;
pub mod event_approval_denied_v1;
pub mod event_approval_denied_v2;
pub mod event_approval_expired_v1;
pub mod event_approval_expired_v2;
pub mod event_approval_granted_v1;
pub mod event_approval_granted_v2;
pub mod event_approval_requested_v1;
pub mod event_approval_requested_v2;
pub mod event_envelope_v1;
pub mod event_host_alert_acked_v1;
pub mod event_host_alert_raised_v1;
pub mod event_host_sample_recorded_v1;
pub mod event_queue_clock_reanchored_v1;
pub mod event_queue_created_v1;
pub mod event_queue_enabled_v1;
pub mod event_queue_stopped_v1;
pub mod event_queue_task_created_v1;
pub mod event_queue_task_created_v2;
pub mod event_queue_task_state_changed_v1;
pub mod event_review_completed_v1;
pub mod event_review_failed_v1;
pub mod event_review_requested_v1;
pub mod event_routing_decided_v1;
pub mod event_routing_decided_v2;
pub mod event_routing_fallback_v1;
pub mod event_routing_fallback_v2;
pub mod event_routing_proposed_v1;
pub mod event_routing_proposed_v2;
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
pub mod record_action_plan_v2;
pub mod record_approval_v1;
pub mod record_host_event_v1;
pub mod record_host_sample_v1;
pub mod record_parameter_v1;
pub mod record_queue_task_v1;
pub mod record_task_packet_v1;
pub mod record_task_queue_v1;
pub mod record_update_manifest_v1;
pub mod record_update_status_v1;
pub mod record_usage_event_v1;
pub mod relay_device_v1;
pub mod relay_envelope_v1;

pub use command_auth_hello_v1::*;
pub use command_events_subscribe_v1::*;
pub use command_harness_list_response_v1::*;
pub use command_harness_list_v1::*;
pub use command_request_v1::*;
pub use command_response_v1::*;
pub use command_response_v2::*;
pub use command_session_close_v1::*;
pub use command_session_open_v1::*;
pub use command_session_resize_v1::*;
pub use command_session_write_v1::*;
pub use common_abspath_v1::*;
pub use common_abspath_v2::*;
pub use common_action_rule_v1::*;
pub use common_actor_v1::*;
pub use common_base64_v1::*;
pub use common_data_class_v1::*;
pub use common_id_v1::*;
pub use common_queue_task_state_v1::*;
pub use common_relpath_v1::*;
pub use common_risk_v1::*;
pub use common_sha256_v1::*;
pub use common_timestamp_v1::*;
pub use common_ulid_v1::*;
pub use contract_harness_adapter_v1::*;
pub use contract_local_api_error_code_v1::*;
pub use contract_local_api_server_message_v1::*;
pub use contract_local_api_server_message_v2::*;
pub use contract_pty_host_frame_v1::*;
pub use contract_pty_host_notification_v1::*;
pub use contract_pty_host_request_v1::*;
pub use contract_pty_host_response_v1::*;
pub use event_account_status_observed_v1::*;
pub use event_account_switched_v1::*;
pub use event_action_denied_v1::*;
pub use event_action_executed_v1::*;
pub use event_action_failed_v1::*;
pub use event_action_planned_v1::*;
pub use event_action_started_v1::*;
pub use event_approval_denied_v1::*;
pub use event_approval_denied_v2::*;
pub use event_approval_expired_v1::*;
pub use event_approval_expired_v2::*;
pub use event_approval_granted_v1::*;
pub use event_approval_granted_v2::*;
pub use event_approval_requested_v1::*;
pub use event_approval_requested_v2::*;
pub use event_envelope_v1::*;
pub use event_host_alert_acked_v1::*;
pub use event_host_alert_raised_v1::*;
pub use event_host_sample_recorded_v1::*;
pub use event_queue_clock_reanchored_v1::*;
pub use event_queue_created_v1::*;
pub use event_queue_enabled_v1::*;
pub use event_queue_stopped_v1::*;
pub use event_queue_task_created_v1::*;
pub use event_queue_task_created_v2::*;
pub use event_queue_task_state_changed_v1::*;
pub use event_review_completed_v1::*;
pub use event_review_failed_v1::*;
pub use event_review_requested_v1::*;
pub use event_routing_decided_v1::*;
pub use event_routing_decided_v2::*;
pub use event_routing_fallback_v1::*;
pub use event_routing_fallback_v2::*;
pub use event_routing_proposed_v1::*;
pub use event_routing_proposed_v2::*;
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
pub use record_action_plan_v2::*;
pub use record_approval_v1::*;
pub use record_host_event_v1::*;
pub use record_host_sample_v1::*;
pub use record_parameter_v1::*;
pub use record_queue_task_v1::*;
pub use record_task_packet_v1::*;
pub use record_task_queue_v1::*;
pub use record_update_manifest_v1::*;
pub use record_update_status_v1::*;
pub use record_usage_event_v1::*;
pub use relay_device_v1::*;
pub use relay_envelope_v1::*;

/// Every Schema a type was generated from: its file key and its `$id`.
pub const SCHEMAS: &[(&str, &str)] = &[
    ("command.auth.hello.v1", "urn:ads:command:auth.hello:1"),
    (
        "command.events.subscribe.v1",
        "urn:ads:command:events.subscribe:1",
    ),
    (
        "command.harness.list.response.v1",
        "urn:ads:command:harness.list.response:1",
    ),
    ("command.harness.list.v1", "urn:ads:command:harness.list:1"),
    ("command.request.v1", "urn:ads:command:request:1"),
    ("command.response.v1", "urn:ads:command:response:1"),
    ("command.response.v2", "urn:ads:command:response:2"),
    (
        "command.session.close.v1",
        "urn:ads:command:session.close:1",
    ),
    ("command.session.open.v1", "urn:ads:command:session.open:1"),
    (
        "command.session.resize.v1",
        "urn:ads:command:session.resize:1",
    ),
    (
        "command.session.write.v1",
        "urn:ads:command:session.write:1",
    ),
    ("common.abspath.v1", "urn:ads:common:abspath:1"),
    ("common.abspath.v2", "urn:ads:common:abspath:2"),
    ("common.action-rule.v1", "urn:ads:common:action-rule:1"),
    ("common.actor.v1", "urn:ads:common:actor:1"),
    ("common.base64.v1", "urn:ads:common:base64:1"),
    ("common.data-class.v1", "urn:ads:common:data-class:1"),
    ("common.id.v1", "urn:ads:common:id:1"),
    (
        "common.queue-task-state.v1",
        "urn:ads:common:queue-task-state:1",
    ),
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
        "contract.local-api.error-code.v1",
        "urn:ads:contract:local-api.error-code:1",
    ),
    (
        "contract.local-api.server-message.v1",
        "urn:ads:contract:local-api.server-message:1",
    ),
    (
        "contract.local-api.server-message.v2",
        "urn:ads:contract:local-api.server-message:2",
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
    (
        "event.account.status_observed.v1",
        "urn:ads:event:account.status_observed:1",
    ),
    (
        "event.account.switched.v1",
        "urn:ads:event:account.switched:1",
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
        "event.approval.denied.v2",
        "urn:ads:event:approval.denied:2",
    ),
    (
        "event.approval.expired.v1",
        "urn:ads:event:approval.expired:1",
    ),
    (
        "event.approval.expired.v2",
        "urn:ads:event:approval.expired:2",
    ),
    (
        "event.approval.granted.v1",
        "urn:ads:event:approval.granted:1",
    ),
    (
        "event.approval.granted.v2",
        "urn:ads:event:approval.granted:2",
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
        "event.queue.clock_reanchored.v1",
        "urn:ads:event:queue.clock_reanchored:1",
    ),
    ("event.queue.created.v1", "urn:ads:event:queue.created:1"),
    ("event.queue.enabled.v1", "urn:ads:event:queue.enabled:1"),
    ("event.queue.stopped.v1", "urn:ads:event:queue.stopped:1"),
    (
        "event.queue.task_created.v1",
        "urn:ads:event:queue.task_created:1",
    ),
    (
        "event.queue.task_created.v2",
        "urn:ads:event:queue.task_created:2",
    ),
    (
        "event.queue.task_state_changed.v1",
        "urn:ads:event:queue.task_state_changed:1",
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
        "event.routing.decided.v2",
        "urn:ads:event:routing.decided:2",
    ),
    (
        "event.routing.fallback.v1",
        "urn:ads:event:routing.fallback:1",
    ),
    (
        "event.routing.fallback.v2",
        "urn:ads:event:routing.fallback:2",
    ),
    (
        "event.routing.proposed.v1",
        "urn:ads:event:routing.proposed:1",
    ),
    (
        "event.routing.proposed.v2",
        "urn:ads:event:routing.proposed:2",
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
    ("record.action-plan.v2", "urn:ads:record:action-plan:2"),
    ("record.approval.v1", "urn:ads:record:approval:1"),
    ("record.host_event.v1", "urn:ads:record:host_event:1"),
    ("record.host_sample.v1", "urn:ads:record:host_sample:1"),
    ("record.parameter.v1", "urn:ads:record:parameter:1"),
    ("record.queue-task.v1", "urn:ads:record:queue-task:1"),
    ("record.task-packet.v1", "urn:ads:record:task-packet:1"),
    ("record.task-queue.v1", "urn:ads:record:task-queue:1"),
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
    r"(^((con|prn|aux|nul|com[0-9]|lpt[0-9])(\.|$)))|\.\.",
    r"(^|/)\.\.(/|$)",
    r"(^|[\\/])\.\.?([\\/]|$)",
    r"(^|[\\/])~",
    r"[$%]",
    r"[*?]",
    r"[\x{0000}-\x{001f}\x{007f}-\x{009f}]",
    r"\.\.|\.lock(/|$)|\.(/|$)|(^|/)(con|prn|aux|nul|com[0-9]|lpt[0-9])(\.|/|$)",
    r"^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$",
    r#"^([A-Z]:\\([^\\/:"<>|]*[^\\/:"<>|. ](\\[^\\/:"<>|]*[^\\/:"<>|. ])*)?|/([^/\\]+(/[^/\\]+)*)?)$"#,
    r"^(feature|fix)/[a-z0-9][a-z0-9._-]*(/[a-z0-9][a-z0-9._-]*)*$",
    r"^E_[A-Z0-9_]{1,60}$",
    r"^T[0-3]$",
    r"^[0-7][0-9A-HJKMNP-TV-Z]{25}$",
    r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(\.[0-9]{1,6})?Z$",
    r"^[0-9a-f]{40}$",
    r"^[A-Z]:\\([^\\]*\\)*([Cc][Oo][Nn]|[Pp][Rr][Nn]|[Aa][Uu][Xx]|[Nn][Uu][Ll]|([Cc][Oo][Mm]|[Ll][Pp][Tt])[0-9\x{00b9}\x{00b2}\x{00b3}])[ \x{00a0}\x{1680}\x{2000}-\x{200a}\x{2028}\x{2029}\x{202f}\x{205f}\x{3000}]*(\.[^\\]*)?(\\|$)",
    r"^[A-Z]:\\([^\\]*\\)*[^\\]*~[0-9]+(\.|\\|$)",
    r"^[A-Za-z0-9+/]{43}=$",
    r"^[A-Za-z0-9._-][A-Za-z0-9._/-]*$",
    r"^[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$",
    r"^[A-Za-z0-9_-]{43}$",
    r"^[\x{0020}-\x{007e}\x{00a1}-\x{00ac}\x{00ae}-\x{034e}\x{0350}-\x{0377}\x{037a}-\x{037f}\x{0384}-\x{038a}\x{038c}\x{038e}-\x{03a1}\x{03a3}-\x{052f}\x{0531}-\x{0556}\x{0559}-\x{058a}\x{058d}-\x{058f}\x{0591}-\x{05c7}\x{05d0}-\x{05ea}\x{05ef}-\x{05f4}\x{0606}-\x{061b}\x{061d}-\x{06dc}\x{06de}-\x{070d}\x{0710}-\x{074a}\x{074d}-\x{07b1}\x{07c0}-\x{07fa}\x{07fd}-\x{082d}\x{0830}-\x{083e}\x{0840}-\x{085b}\x{085e}\x{0860}-\x{086a}\x{0870}-\x{088e}\x{0897}-\x{08e1}\x{08e3}-\x{0983}\x{0985}-\x{098c}\x{098f}-\x{0990}\x{0993}-\x{09a8}\x{09aa}-\x{09b0}\x{09b2}\x{09b6}-\x{09b9}\x{09bc}-\x{09c4}\x{09c7}-\x{09c8}\x{09cb}-\x{09ce}\x{09d7}\x{09dc}-\x{09dd}\x{09df}-\x{09e3}\x{09e6}-\x{09fe}\x{0a01}-\x{0a03}\x{0a05}-\x{0a0a}\x{0a0f}-\x{0a10}\x{0a13}-\x{0a28}\x{0a2a}-\x{0a30}\x{0a32}-\x{0a33}\x{0a35}-\x{0a36}\x{0a38}-\x{0a39}\x{0a3c}\x{0a3e}-\x{0a42}\x{0a47}-\x{0a48}\x{0a4b}-\x{0a4d}\x{0a51}\x{0a59}-\x{0a5c}\x{0a5e}\x{0a66}-\x{0a76}\x{0a81}-\x{0a83}\x{0a85}-\x{0a8d}\x{0a8f}-\x{0a91}\x{0a93}-\x{0aa8}\x{0aaa}-\x{0ab0}\x{0ab2}-\x{0ab3}\x{0ab5}-\x{0ab9}\x{0abc}-\x{0ac5}\x{0ac7}-\x{0ac9}\x{0acb}-\x{0acd}\x{0ad0}\x{0ae0}-\x{0ae3}\x{0ae6}-\x{0af1}\x{0af9}-\x{0aff}\x{0b01}-\x{0b03}\x{0b05}-\x{0b0c}\x{0b0f}-\x{0b10}\x{0b13}-\x{0b28}\x{0b2a}-\x{0b30}\x{0b32}-\x{0b33}\x{0b35}-\x{0b39}\x{0b3c}-\x{0b44}\x{0b47}-\x{0b48}\x{0b4b}-\x{0b4d}\x{0b55}-\x{0b57}\x{0b5c}-\x{0b5d}\x{0b5f}-\x{0b63}\x{0b66}-\x{0b77}\x{0b82}-\x{0b83}\x{0b85}-\x{0b8a}\x{0b8e}-\x{0b90}\x{0b92}-\x{0b95}\x{0b99}-\x{0b9a}\x{0b9c}\x{0b9e}-\x{0b9f}\x{0ba3}-\x{0ba4}\x{0ba8}-\x{0baa}\x{0bae}-\x{0bb9}\x{0bbe}-\x{0bc2}\x{0bc6}-\x{0bc8}\x{0bca}-\x{0bcd}\x{0bd0}\x{0bd7}\x{0be6}-\x{0bfa}\x{0c00}-\x{0c0c}\x{0c0e}-\x{0c10}\x{0c12}-\x{0c28}\x{0c2a}-\x{0c39}\x{0c3c}-\x{0c44}\x{0c46}-\x{0c48}\x{0c4a}-\x{0c4d}\x{0c55}-\x{0c56}\x{0c58}-\x{0c5a}\x{0c5d}\x{0c60}-\x{0c63}\x{0c66}-\x{0c6f}\x{0c77}-\x{0c8c}\x{0c8e}-\x{0c90}\x{0c92}-\x{0ca8}\x{0caa}-\x{0cb3}\x{0cb5}-\x{0cb9}\x{0cbc}-\x{0cc4}\x{0cc6}-\x{0cc8}\x{0cca}-\x{0ccd}\x{0cd5}-\x{0cd6}\x{0cdd}-\x{0cde}\x{0ce0}-\x{0ce3}\x{0ce6}-\x{0cef}\x{0cf1}-\x{0cf3}\x{0d00}-\x{0d0c}\x{0d0e}-\x{0d10}\x{0d12}-\x{0d44}\x{0d46}-\x{0d48}\x{0d4a}-\x{0d4f}\x{0d54}-\x{0d63}\x{0d66}-\x{0d7f}\x{0d81}-\x{0d83}\x{0d85}-\x{0d96}\x{0d9a}-\x{0db1}\x{0db3}-\x{0dbb}\x{0dbd}\x{0dc0}-\x{0dc6}\x{0dca}\x{0dcf}-\x{0dd4}\x{0dd6}\x{0dd8}-\x{0ddf}\x{0de6}-\x{0def}\x{0df2}-\x{0df4}\x{0e01}-\x{0e3a}\x{0e3f}-\x{0e5b}\x{0e81}-\x{0e82}\x{0e84}\x{0e86}-\x{0e8a}\x{0e8c}-\x{0ea3}\x{0ea5}\x{0ea7}-\x{0ebd}\x{0ec0}-\x{0ec4}\x{0ec6}\x{0ec8}-\x{0ece}\x{0ed0}-\x{0ed9}\x{0edc}-\x{0edf}\x{0f00}-\x{0f47}\x{0f49}-\x{0f6c}\x{0f71}-\x{0f97}\x{0f99}-\x{0fbc}\x{0fbe}-\x{0fcc}\x{0fce}-\x{0fda}\x{1000}-\x{10c5}\x{10c7}\x{10cd}\x{10d0}-\x{115e}\x{1161}-\x{1248}\x{124a}-\x{124d}\x{1250}-\x{1256}\x{1258}\x{125a}-\x{125d}\x{1260}-\x{1288}\x{128a}-\x{128d}\x{1290}-\x{12b0}\x{12b2}-\x{12b5}\x{12b8}-\x{12be}\x{12c0}\x{12c2}-\x{12c5}\x{12c8}-\x{12d6}\x{12d8}-\x{1310}\x{1312}-\x{1315}\x{1318}-\x{135a}\x{135d}-\x{137c}\x{1380}-\x{1399}\x{13a0}-\x{13f5}\x{13f8}-\x{13fd}\x{1400}-\x{167f}\x{1681}-\x{169c}\x{16a0}-\x{16f8}\x{1700}-\x{1715}\x{171f}-\x{1736}\x{1740}-\x{1753}\x{1760}-\x{176c}\x{176e}-\x{1770}\x{1772}-\x{1773}\x{1780}-\x{17b3}\x{17b6}-\x{17dd}\x{17e0}-\x{17e9}\x{17f0}-\x{17f9}\x{1800}-\x{180a}\x{1810}-\x{1819}\x{1820}-\x{1878}\x{1880}-\x{18aa}\x{18b0}-\x{18f5}\x{1900}-\x{191e}\x{1920}-\x{192b}\x{1930}-\x{193b}\x{1940}\x{1944}-\x{196d}\x{1970}-\x{1974}\x{1980}-\x{19ab}\x{19b0}-\x{19c9}\x{19d0}-\x{19da}\x{19de}-\x{1a1b}\x{1a1e}-\x{1a5e}\x{1a60}-\x{1a7c}\x{1a7f}-\x{1a89}\x{1a90}-\x{1a99}\x{1aa0}-\x{1aad}\x{1ab0}-\x{1ace}\x{1b00}-\x{1b4c}\x{1b4e}-\x{1bf3}\x{1bfc}-\x{1c37}\x{1c3b}-\x{1c49}\x{1c4d}-\x{1c8a}\x{1c90}-\x{1cba}\x{1cbd}-\x{1cc7}\x{1cd0}-\x{1cfa}\x{1d00}-\x{1f15}\x{1f18}-\x{1f1d}\x{1f20}-\x{1f45}\x{1f48}-\x{1f4d}\x{1f50}-\x{1f57}\x{1f59}\x{1f5b}\x{1f5d}\x{1f5f}-\x{1f7d}\x{1f80}-\x{1fb4}\x{1fb6}-\x{1fc4}\x{1fc6}-\x{1fd3}\x{1fd6}-\x{1fdb}\x{1fdd}-\x{1fef}\x{1ff2}-\x{1ff4}\x{1ff6}-\x{1ffe}\x{2010}-\x{2027}\x{2030}-\x{205e}\x{2070}-\x{2071}\x{2074}-\x{208e}\x{2090}-\x{209c}\x{20a0}-\x{20c0}\x{20d0}-\x{20f0}\x{2100}-\x{218b}\x{2190}-\x{2429}\x{2440}-\x{244a}\x{2460}-\x{27ff}\x{2801}-\x{2b73}\x{2b76}-\x{2b95}\x{2b97}-\x{2cf3}\x{2cf9}-\x{2d25}\x{2d27}\x{2d2d}\x{2d30}-\x{2d67}\x{2d6f}-\x{2d70}\x{2d7f}-\x{2d96}\x{2da0}-\x{2da6}\x{2da8}-\x{2dae}\x{2db0}-\x{2db6}\x{2db8}-\x{2dbe}\x{2dc0}-\x{2dc6}\x{2dc8}-\x{2dce}\x{2dd0}-\x{2dd6}\x{2dd8}-\x{2dde}\x{2de0}-\x{2e5d}\x{2e80}-\x{2e99}\x{2e9b}-\x{2ef3}\x{2f00}-\x{2fd5}\x{2ff0}-\x{2fff}\x{3001}-\x{303f}\x{3041}-\x{3096}\x{3099}-\x{30ff}\x{3105}-\x{312f}\x{3131}-\x{3163}\x{3165}-\x{318e}\x{3190}-\x{31e5}\x{31ef}-\x{321e}\x{3220}-\x{a48c}\x{a490}-\x{a4c6}\x{a4d0}-\x{a62b}\x{a640}-\x{a6f7}\x{a700}-\x{a7cd}\x{a7d0}-\x{a7d1}\x{a7d3}\x{a7d5}-\x{a7dc}\x{a7f2}-\x{a82c}\x{a830}-\x{a839}\x{a840}-\x{a877}\x{a880}-\x{a8c5}\x{a8ce}-\x{a8d9}\x{a8e0}-\x{a953}\x{a95f}-\x{a97c}\x{a980}-\x{a9cd}\x{a9cf}-\x{a9d9}\x{a9de}-\x{a9fe}\x{aa00}-\x{aa36}\x{aa40}-\x{aa4d}\x{aa50}-\x{aa59}\x{aa5c}-\x{aac2}\x{aadb}-\x{aaf6}\x{ab01}-\x{ab06}\x{ab09}-\x{ab0e}\x{ab11}-\x{ab16}\x{ab20}-\x{ab26}\x{ab28}-\x{ab2e}\x{ab30}-\x{ab6b}\x{ab70}-\x{abed}\x{abf0}-\x{abf9}\x{ac00}-\x{d7a3}\x{d7b0}-\x{d7c6}\x{d7cb}-\x{d7fb}\x{f900}-\x{fa6d}\x{fa70}-\x{fad9}\x{fb00}-\x{fb06}\x{fb13}-\x{fb17}\x{fb1d}-\x{fb36}\x{fb38}-\x{fb3c}\x{fb3e}\x{fb40}-\x{fb41}\x{fb43}-\x{fb44}\x{fb46}-\x{fbc2}\x{fbd3}-\x{fd8f}\x{fd92}-\x{fdc7}\x{fdcf}\x{fdf0}-\x{fdff}\x{fe10}-\x{fe19}\x{fe20}-\x{fe52}\x{fe54}-\x{fe66}\x{fe68}-\x{fe6b}\x{fe70}-\x{fe74}\x{fe76}-\x{fefc}\x{ff01}-\x{ff9f}\x{ffa1}-\x{ffbe}\x{ffc2}-\x{ffc7}\x{ffca}-\x{ffcf}\x{ffd2}-\x{ffd7}\x{ffda}-\x{ffdc}\x{ffe0}-\x{ffe6}\x{ffe8}-\x{ffee}\x{fffd}\x{10000}-\x{1000B}\x{1000D}-\x{10026}\x{10028}-\x{1003A}\x{1003C}-\x{1003D}\x{1003F}-\x{1004D}\x{10050}-\x{1005D}\x{10080}-\x{100FA}\x{10100}-\x{10102}\x{10107}-\x{10133}\x{10137}-\x{1018E}\x{10190}-\x{1019C}\x{101A0}\x{101D0}-\x{101FD}\x{10280}-\x{1029C}\x{102A0}-\x{102D0}\x{102E0}-\x{102FB}\x{10300}-\x{10323}\x{1032D}-\x{1034A}\x{10350}-\x{1037A}\x{10380}-\x{1039D}\x{1039F}-\x{103C3}\x{103C8}-\x{103D5}\x{10400}-\x{1049D}\x{104A0}-\x{104A9}\x{104B0}-\x{104D3}\x{104D8}-\x{104FB}\x{10500}-\x{10527}\x{10530}-\x{10563}\x{1056F}-\x{1057A}\x{1057C}-\x{1058A}\x{1058C}-\x{10592}\x{10594}-\x{10595}\x{10597}-\x{105A1}\x{105A3}-\x{105B1}\x{105B3}-\x{105B9}\x{105BB}-\x{105BC}\x{105C0}-\x{105F3}\x{10600}-\x{10736}\x{10740}-\x{10755}\x{10760}-\x{10767}\x{10780}-\x{10785}\x{10787}-\x{107B0}\x{107B2}-\x{107BA}\x{10800}-\x{10805}\x{10808}\x{1080A}-\x{10835}\x{10837}-\x{10838}\x{1083C}\x{1083F}-\x{10855}\x{10857}-\x{1089E}\x{108A7}-\x{108AF}\x{108E0}-\x{108F2}\x{108F4}-\x{108F5}\x{108FB}-\x{1091B}\x{1091F}-\x{10939}\x{1093F}\x{10980}-\x{109B7}\x{109BC}-\x{109CF}\x{109D2}-\x{10A03}\x{10A05}-\x{10A06}\x{10A0C}-\x{10A13}\x{10A15}-\x{10A17}\x{10A19}-\x{10A35}\x{10A38}-\x{10A3A}\x{10A3F}-\x{10A48}\x{10A50}-\x{10A58}\x{10A60}-\x{10A9F}\x{10AC0}-\x{10AE6}\x{10AEB}-\x{10AF6}\x{10B00}-\x{10B35}\x{10B39}-\x{10B55}\x{10B58}-\x{10B72}\x{10B78}-\x{10B91}\x{10B99}-\x{10B9C}\x{10BA9}-\x{10BAF}\x{10C00}-\x{10C48}\x{10C80}-\x{10CB2}\x{10CC0}-\x{10CF2}\x{10CFA}-\x{10D27}\x{10D30}-\x{10D39}\x{10D40}-\x{10D65}\x{10D69}-\x{10D85}\x{10D8E}-\x{10D8F}\x{10E60}-\x{10E7E}\x{10E80}-\x{10EA9}\x{10EAB}-\x{10EAD}\x{10EB0}-\x{10EB1}\x{10EC2}-\x{10EC4}\x{10EFC}-\x{10F27}\x{10F30}-\x{10F59}\x{10F70}-\x{10F89}\x{10FB0}-\x{10FCB}\x{10FE0}-\x{10FF6}\x{11000}-\x{1104D}\x{11052}-\x{11075}\x{1107F}-\x{110BC}\x{110BE}-\x{110C2}\x{110D0}-\x{110E8}\x{110F0}-\x{110F9}\x{11100}-\x{11134}\x{11136}-\x{11147}\x{11150}-\x{11176}\x{11180}-\x{111DF}\x{111E1}-\x{111F4}\x{11200}-\x{11211}\x{11213}-\x{11241}\x{11280}-\x{11286}\x{11288}\x{1128A}-\x{1128D}\x{1128F}-\x{1129D}\x{1129F}-\x{112A9}\x{112B0}-\x{112EA}\x{112F0}-\x{112F9}\x{11300}-\x{11303}\x{11305}-\x{1130C}\x{1130F}-\x{11310}\x{11313}-\x{11328}\x{1132A}-\x{11330}\x{11332}-\x{11333}\x{11335}-\x{11339}\x{1133B}-\x{11344}\x{11347}-\x{11348}\x{1134B}-\x{1134D}\x{11350}\x{11357}\x{1135D}-\x{11363}\x{11366}-\x{1136C}\x{11370}-\x{11374}\x{11380}-\x{11389}\x{1138B}\x{1138E}\x{11390}-\x{113B5}\x{113B7}-\x{113C0}\x{113C2}\x{113C5}\x{113C7}-\x{113CA}\x{113CC}-\x{113D5}\x{113D7}-\x{113D8}\x{113E1}-\x{113E2}\x{11400}-\x{1145B}\x{1145D}-\x{11461}\x{11480}-\x{114C7}\x{114D0}-\x{114D9}\x{11580}-\x{115B5}\x{115B8}-\x{115DD}\x{11600}-\x{11644}\x{11650}-\x{11659}\x{11660}-\x{1166C}\x{11680}-\x{116B9}\x{116C0}-\x{116C9}\x{116D0}-\x{116E3}\x{11700}-\x{1171A}\x{1171D}-\x{1172B}\x{11730}-\x{11746}\x{11800}-\x{1183B}\x{118A0}-\x{118F2}\x{118FF}-\x{11906}\x{11909}\x{1190C}-\x{11913}\x{11915}-\x{11916}\x{11918}-\x{11935}\x{11937}-\x{11938}\x{1193B}-\x{11946}\x{11950}-\x{11959}\x{119A0}-\x{119A7}\x{119AA}-\x{119D7}\x{119DA}-\x{119E4}\x{11A00}-\x{11A47}\x{11A50}-\x{11AA2}\x{11AB0}-\x{11AF8}\x{11B00}-\x{11B09}\x{11BC0}-\x{11BE1}\x{11BF0}-\x{11BF9}\x{11C00}-\x{11C08}\x{11C0A}-\x{11C36}\x{11C38}-\x{11C45}\x{11C50}-\x{11C6C}\x{11C70}-\x{11C8F}\x{11C92}-\x{11CA7}\x{11CA9}-\x{11CB6}\x{11D00}-\x{11D06}\x{11D08}-\x{11D09}\x{11D0B}-\x{11D36}\x{11D3A}\x{11D3C}-\x{11D3D}\x{11D3F}-\x{11D47}\x{11D50}-\x{11D59}\x{11D60}-\x{11D65}\x{11D67}-\x{11D68}\x{11D6A}-\x{11D8E}\x{11D90}-\x{11D91}\x{11D93}-\x{11D98}\x{11DA0}-\x{11DA9}\x{11EE0}-\x{11EF8}\x{11F00}-\x{11F10}\x{11F12}-\x{11F3A}\x{11F3E}-\x{11F5A}\x{11FB0}\x{11FC0}-\x{11FF1}\x{11FFF}-\x{12399}\x{12400}-\x{1246E}\x{12470}-\x{12474}\x{12480}-\x{12543}\x{12F90}-\x{12FF2}\x{13000}-\x{1342F}\x{13440}-\x{13455}\x{13460}-\x{143FA}\x{14400}-\x{14646}\x{16100}-\x{16139}\x{16800}-\x{16A38}\x{16A40}-\x{16A5E}\x{16A60}-\x{16A69}\x{16A6E}-\x{16ABE}\x{16AC0}-\x{16AC9}\x{16AD0}-\x{16AED}\x{16AF0}-\x{16AF5}\x{16B00}-\x{16B45}\x{16B50}-\x{16B59}\x{16B5B}-\x{16B61}\x{16B63}-\x{16B77}\x{16B7D}-\x{16B8F}\x{16D40}-\x{16D79}\x{16E40}-\x{16E9A}\x{16F00}-\x{16F4A}\x{16F4F}-\x{16F87}\x{16F8F}-\x{16F9F}\x{16FE0}-\x{16FE3}\x{16FF0}-\x{16FF1}\x{17000}-\x{187F7}\x{18800}-\x{18CD5}\x{18CFF}-\x{18D08}\x{1AFF0}-\x{1AFF3}\x{1AFF5}-\x{1AFFB}\x{1AFFD}-\x{1AFFE}\x{1B000}-\x{1B122}\x{1B132}\x{1B150}-\x{1B152}\x{1B155}\x{1B164}-\x{1B167}\x{1B170}-\x{1B2FB}\x{1BC00}-\x{1BC6A}\x{1BC70}-\x{1BC7C}\x{1BC80}-\x{1BC88}\x{1BC90}-\x{1BC99}\x{1BC9C}-\x{1BC9F}\x{1CC00}-\x{1CCF9}\x{1CD00}-\x{1CEB3}\x{1CF00}-\x{1CF2D}\x{1CF30}-\x{1CF46}\x{1CF50}-\x{1CFC3}\x{1D000}-\x{1D0F5}\x{1D100}-\x{1D126}\x{1D129}-\x{1D158}\x{1D15A}-\x{1D172}\x{1D17B}-\x{1D1EA}\x{1D200}-\x{1D245}\x{1D2C0}-\x{1D2D3}\x{1D2E0}-\x{1D2F3}\x{1D300}-\x{1D356}\x{1D360}-\x{1D378}\x{1D400}-\x{1D454}\x{1D456}-\x{1D49C}\x{1D49E}-\x{1D49F}\x{1D4A2}\x{1D4A5}-\x{1D4A6}\x{1D4A9}-\x{1D4AC}\x{1D4AE}-\x{1D4B9}\x{1D4BB}\x{1D4BD}-\x{1D4C3}\x{1D4C5}-\x{1D505}\x{1D507}-\x{1D50A}\x{1D50D}-\x{1D514}\x{1D516}-\x{1D51C}\x{1D51E}-\x{1D539}\x{1D53B}-\x{1D53E}\x{1D540}-\x{1D544}\x{1D546}\x{1D54A}-\x{1D550}\x{1D552}-\x{1D6A5}\x{1D6A8}-\x{1D7CB}\x{1D7CE}-\x{1DA8B}\x{1DA9B}-\x{1DA9F}\x{1DAA1}-\x{1DAAF}\x{1DF00}-\x{1DF1E}\x{1DF25}-\x{1DF2A}\x{1E000}-\x{1E006}\x{1E008}-\x{1E018}\x{1E01B}-\x{1E021}\x{1E023}-\x{1E024}\x{1E026}-\x{1E02A}\x{1E030}-\x{1E06D}\x{1E08F}\x{1E100}-\x{1E12C}\x{1E130}-\x{1E13D}\x{1E140}-\x{1E149}\x{1E14E}-\x{1E14F}\x{1E290}-\x{1E2AE}\x{1E2C0}-\x{1E2F9}\x{1E2FF}\x{1E4D0}-\x{1E4F9}\x{1E5D0}-\x{1E5FA}\x{1E5FF}\x{1E7E0}-\x{1E7E6}\x{1E7E8}-\x{1E7EB}\x{1E7ED}-\x{1E7EE}\x{1E7F0}-\x{1E7FE}\x{1E800}-\x{1E8C4}\x{1E8C7}-\x{1E8D6}\x{1E900}-\x{1E94B}\x{1E950}-\x{1E959}\x{1E95E}-\x{1E95F}\x{1EC71}-\x{1ECB4}\x{1ED01}-\x{1ED3D}\x{1EE00}-\x{1EE03}\x{1EE05}-\x{1EE1F}\x{1EE21}-\x{1EE22}\x{1EE24}\x{1EE27}\x{1EE29}-\x{1EE32}\x{1EE34}-\x{1EE37}\x{1EE39}\x{1EE3B}\x{1EE42}\x{1EE47}\x{1EE49}\x{1EE4B}\x{1EE4D}-\x{1EE4F}\x{1EE51}-\x{1EE52}\x{1EE54}\x{1EE57}\x{1EE59}\x{1EE5B}\x{1EE5D}\x{1EE5F}\x{1EE61}-\x{1EE62}\x{1EE64}\x{1EE67}-\x{1EE6A}\x{1EE6C}-\x{1EE72}\x{1EE74}-\x{1EE77}\x{1EE79}-\x{1EE7C}\x{1EE7E}\x{1EE80}-\x{1EE89}\x{1EE8B}-\x{1EE9B}\x{1EEA1}-\x{1EEA3}\x{1EEA5}-\x{1EEA9}\x{1EEAB}-\x{1EEBB}\x{1EEF0}-\x{1EEF1}\x{1F000}-\x{1F02B}\x{1F030}-\x{1F093}\x{1F0A0}-\x{1F0AE}\x{1F0B1}-\x{1F0BF}\x{1F0C1}-\x{1F0CF}\x{1F0D1}-\x{1F0F5}\x{1F100}-\x{1F1AD}\x{1F1E6}-\x{1F202}\x{1F210}-\x{1F23B}\x{1F240}-\x{1F248}\x{1F250}-\x{1F251}\x{1F260}-\x{1F265}\x{1F300}-\x{1F6D7}\x{1F6DC}-\x{1F6EC}\x{1F6F0}-\x{1F6FC}\x{1F700}-\x{1F776}\x{1F77B}-\x{1F7D9}\x{1F7E0}-\x{1F7EB}\x{1F7F0}\x{1F800}-\x{1F80B}\x{1F810}-\x{1F847}\x{1F850}-\x{1F859}\x{1F860}-\x{1F887}\x{1F890}-\x{1F8AD}\x{1F8B0}-\x{1F8BB}\x{1F8C0}-\x{1F8C1}\x{1F900}-\x{1FA53}\x{1FA60}-\x{1FA6D}\x{1FA70}-\x{1FA7C}\x{1FA80}-\x{1FA89}\x{1FA8F}-\x{1FAC6}\x{1FACE}-\x{1FADC}\x{1FADF}-\x{1FAE9}\x{1FAF0}-\x{1FAF8}\x{1FB00}-\x{1FB92}\x{1FB94}-\x{1FBF9}\x{20000}-\x{2A6DF}\x{2A700}-\x{2B739}\x{2B740}-\x{2B81D}\x{2B820}-\x{2CEA1}\x{2CEB0}-\x{2EBE0}\x{2EBF0}-\x{2EE5D}\x{2F800}-\x{2FA1D}\x{30000}-\x{3134A}\x{31350}-\x{323AF}]*$",
    r"^[^\\/$%]*[*?][^\\/$%]*$",
    r"^[^\x{0000}-\x{001f}\x{007f}-\x{009f}]*$",
    r"^[^\x{0030}-\x{0039}\x{0041}-\x{005a}\x{0061}-\x{007a}\x{00aa}\x{00b2}-\x{00b3}\x{00b5}\x{00b9}-\x{00ba}\x{00bc}-\x{00be}\x{00c0}-\x{00d6}\x{00d8}-\x{00f6}\x{00f8}-\x{02c1}\x{02c6}-\x{02d1}\x{02e0}-\x{02e4}\x{02ec}\x{02ee}\x{0370}-\x{0374}\x{0376}-\x{0377}\x{037a}-\x{037d}\x{037f}\x{0386}\x{0388}-\x{038a}\x{038c}\x{038e}-\x{03a1}\x{03a3}-\x{03f5}\x{03f7}-\x{0481}\x{048a}-\x{052f}\x{0531}-\x{0556}\x{0559}\x{0560}-\x{0588}\x{05d0}-\x{05ea}\x{05ef}-\x{05f2}\x{0620}-\x{064a}\x{0660}-\x{0669}\x{066e}-\x{066f}\x{0671}-\x{06d3}\x{06d5}\x{06e5}-\x{06e6}\x{06ee}-\x{06fc}\x{06ff}\x{0710}\x{0712}-\x{072f}\x{074d}-\x{07a5}\x{07b1}\x{07c0}-\x{07ea}\x{07f4}-\x{07f5}\x{07fa}\x{0800}-\x{0815}\x{081a}\x{0824}\x{0828}\x{0840}-\x{0858}\x{0860}-\x{086a}\x{0870}-\x{0887}\x{0889}-\x{088e}\x{08a0}-\x{08c9}\x{0904}-\x{0939}\x{093d}\x{0950}\x{0958}-\x{0961}\x{0966}-\x{096f}\x{0971}-\x{0980}\x{0985}-\x{098c}\x{098f}-\x{0990}\x{0993}-\x{09a8}\x{09aa}-\x{09b0}\x{09b2}\x{09b6}-\x{09b9}\x{09bd}\x{09ce}\x{09dc}-\x{09dd}\x{09df}-\x{09e1}\x{09e6}-\x{09f1}\x{09f4}-\x{09f9}\x{09fc}\x{0a05}-\x{0a0a}\x{0a0f}-\x{0a10}\x{0a13}-\x{0a28}\x{0a2a}-\x{0a30}\x{0a32}-\x{0a33}\x{0a35}-\x{0a36}\x{0a38}-\x{0a39}\x{0a59}-\x{0a5c}\x{0a5e}\x{0a66}-\x{0a6f}\x{0a72}-\x{0a74}\x{0a85}-\x{0a8d}\x{0a8f}-\x{0a91}\x{0a93}-\x{0aa8}\x{0aaa}-\x{0ab0}\x{0ab2}-\x{0ab3}\x{0ab5}-\x{0ab9}\x{0abd}\x{0ad0}\x{0ae0}-\x{0ae1}\x{0ae6}-\x{0aef}\x{0af9}\x{0b05}-\x{0b0c}\x{0b0f}-\x{0b10}\x{0b13}-\x{0b28}\x{0b2a}-\x{0b30}\x{0b32}-\x{0b33}\x{0b35}-\x{0b39}\x{0b3d}\x{0b5c}-\x{0b5d}\x{0b5f}-\x{0b61}\x{0b66}-\x{0b6f}\x{0b71}-\x{0b77}\x{0b83}\x{0b85}-\x{0b8a}\x{0b8e}-\x{0b90}\x{0b92}-\x{0b95}\x{0b99}-\x{0b9a}\x{0b9c}\x{0b9e}-\x{0b9f}\x{0ba3}-\x{0ba4}\x{0ba8}-\x{0baa}\x{0bae}-\x{0bb9}\x{0bd0}\x{0be6}-\x{0bf2}\x{0c05}-\x{0c0c}\x{0c0e}-\x{0c10}\x{0c12}-\x{0c28}\x{0c2a}-\x{0c39}\x{0c3d}\x{0c58}-\x{0c5a}\x{0c5d}\x{0c60}-\x{0c61}\x{0c66}-\x{0c6f}\x{0c78}-\x{0c7e}\x{0c80}\x{0c85}-\x{0c8c}\x{0c8e}-\x{0c90}\x{0c92}-\x{0ca8}\x{0caa}-\x{0cb3}\x{0cb5}-\x{0cb9}\x{0cbd}\x{0cdd}-\x{0cde}\x{0ce0}-\x{0ce1}\x{0ce6}-\x{0cef}\x{0cf1}-\x{0cf2}\x{0d04}-\x{0d0c}\x{0d0e}-\x{0d10}\x{0d12}-\x{0d3a}\x{0d3d}\x{0d4e}\x{0d54}-\x{0d56}\x{0d58}-\x{0d61}\x{0d66}-\x{0d78}\x{0d7a}-\x{0d7f}\x{0d85}-\x{0d96}\x{0d9a}-\x{0db1}\x{0db3}-\x{0dbb}\x{0dbd}\x{0dc0}-\x{0dc6}\x{0de6}-\x{0def}\x{0e01}-\x{0e30}\x{0e32}-\x{0e33}\x{0e40}-\x{0e46}\x{0e50}-\x{0e59}\x{0e81}-\x{0e82}\x{0e84}\x{0e86}-\x{0e8a}\x{0e8c}-\x{0ea3}\x{0ea5}\x{0ea7}-\x{0eb0}\x{0eb2}-\x{0eb3}\x{0ebd}\x{0ec0}-\x{0ec4}\x{0ec6}\x{0ed0}-\x{0ed9}\x{0edc}-\x{0edf}\x{0f00}\x{0f20}-\x{0f33}\x{0f40}-\x{0f47}\x{0f49}-\x{0f6c}\x{0f88}-\x{0f8c}\x{1000}-\x{102a}\x{103f}-\x{1049}\x{1050}-\x{1055}\x{105a}-\x{105d}\x{1061}\x{1065}-\x{1066}\x{106e}-\x{1070}\x{1075}-\x{1081}\x{108e}\x{1090}-\x{1099}\x{10a0}-\x{10c5}\x{10c7}\x{10cd}\x{10d0}-\x{10fa}\x{10fc}-\x{115e}\x{1161}-\x{1248}\x{124a}-\x{124d}\x{1250}-\x{1256}\x{1258}\x{125a}-\x{125d}\x{1260}-\x{1288}\x{128a}-\x{128d}\x{1290}-\x{12b0}\x{12b2}-\x{12b5}\x{12b8}-\x{12be}\x{12c0}\x{12c2}-\x{12c5}\x{12c8}-\x{12d6}\x{12d8}-\x{1310}\x{1312}-\x{1315}\x{1318}-\x{135a}\x{1369}-\x{137c}\x{1380}-\x{138f}\x{13a0}-\x{13f5}\x{13f8}-\x{13fd}\x{1401}-\x{166c}\x{166f}-\x{167f}\x{1681}-\x{169a}\x{16a0}-\x{16ea}\x{16ee}-\x{16f8}\x{1700}-\x{1711}\x{171f}-\x{1731}\x{1740}-\x{1751}\x{1760}-\x{176c}\x{176e}-\x{1770}\x{1780}-\x{17b3}\x{17d7}\x{17dc}\x{17e0}-\x{17e9}\x{17f0}-\x{17f9}\x{1810}-\x{1819}\x{1820}-\x{1878}\x{1880}-\x{1884}\x{1887}-\x{18a8}\x{18aa}\x{18b0}-\x{18f5}\x{1900}-\x{191e}\x{1946}-\x{196d}\x{1970}-\x{1974}\x{1980}-\x{19ab}\x{19b0}-\x{19c9}\x{19d0}-\x{19da}\x{1a00}-\x{1a16}\x{1a20}-\x{1a54}\x{1a80}-\x{1a89}\x{1a90}-\x{1a99}\x{1aa7}\x{1b05}-\x{1b33}\x{1b45}-\x{1b4c}\x{1b50}-\x{1b59}\x{1b83}-\x{1ba0}\x{1bae}-\x{1be5}\x{1c00}-\x{1c23}\x{1c40}-\x{1c49}\x{1c4d}-\x{1c7d}\x{1c80}-\x{1c8a}\x{1c90}-\x{1cba}\x{1cbd}-\x{1cbf}\x{1ce9}-\x{1cec}\x{1cee}-\x{1cf3}\x{1cf5}-\x{1cf6}\x{1cfa}\x{1d00}-\x{1dbf}\x{1e00}-\x{1f15}\x{1f18}-\x{1f1d}\x{1f20}-\x{1f45}\x{1f48}-\x{1f4d}\x{1f50}-\x{1f57}\x{1f59}\x{1f5b}\x{1f5d}\x{1f5f}-\x{1f7d}\x{1f80}-\x{1fb4}\x{1fb6}-\x{1fbc}\x{1fbe}\x{1fc2}-\x{1fc4}\x{1fc6}-\x{1fcc}\x{1fd0}-\x{1fd3}\x{1fd6}-\x{1fdb}\x{1fe0}-\x{1fec}\x{1ff2}-\x{1ff4}\x{1ff6}-\x{1ffc}\x{2070}-\x{2071}\x{2074}-\x{2079}\x{207f}-\x{2089}\x{2090}-\x{209c}\x{2102}\x{2107}\x{210a}-\x{2113}\x{2115}\x{2119}-\x{211d}\x{2124}\x{2126}\x{2128}\x{212a}-\x{212d}\x{212f}-\x{2139}\x{213c}-\x{213f}\x{2145}-\x{2149}\x{214e}\x{2150}-\x{2189}\x{2460}-\x{249b}\x{24ea}-\x{24ff}\x{2776}-\x{2793}\x{2c00}-\x{2ce4}\x{2ceb}-\x{2cee}\x{2cf2}-\x{2cf3}\x{2cfd}\x{2d00}-\x{2d25}\x{2d27}\x{2d2d}\x{2d30}-\x{2d67}\x{2d6f}\x{2d80}-\x{2d96}\x{2da0}-\x{2da6}\x{2da8}-\x{2dae}\x{2db0}-\x{2db6}\x{2db8}-\x{2dbe}\x{2dc0}-\x{2dc6}\x{2dc8}-\x{2dce}\x{2dd0}-\x{2dd6}\x{2dd8}-\x{2dde}\x{2e2f}\x{3005}-\x{3007}\x{3021}-\x{3029}\x{3031}-\x{3035}\x{3038}-\x{303c}\x{3041}-\x{3096}\x{309d}-\x{309f}\x{30a1}-\x{30fa}\x{30fc}-\x{30ff}\x{3105}-\x{312f}\x{3131}-\x{3163}\x{3165}-\x{318e}\x{3192}-\x{3195}\x{31a0}-\x{31bf}\x{31f0}-\x{31ff}\x{3220}-\x{3229}\x{3248}-\x{324f}\x{3251}-\x{325f}\x{3280}-\x{3289}\x{32b1}-\x{32bf}\x{3400}-\x{4dbf}\x{4e00}-\x{a48c}\x{a4d0}-\x{a4fd}\x{a500}-\x{a60c}\x{a610}-\x{a62b}\x{a640}-\x{a66e}\x{a67f}-\x{a69d}\x{a6a0}-\x{a6ef}\x{a717}-\x{a71f}\x{a722}-\x{a788}\x{a78b}-\x{a7cd}\x{a7d0}-\x{a7d1}\x{a7d3}\x{a7d5}-\x{a7dc}\x{a7f2}-\x{a801}\x{a803}-\x{a805}\x{a807}-\x{a80a}\x{a80c}-\x{a822}\x{a830}-\x{a835}\x{a840}-\x{a873}\x{a882}-\x{a8b3}\x{a8d0}-\x{a8d9}\x{a8f2}-\x{a8f7}\x{a8fb}\x{a8fd}-\x{a8fe}\x{a900}-\x{a925}\x{a930}-\x{a946}\x{a960}-\x{a97c}\x{a984}-\x{a9b2}\x{a9cf}-\x{a9d9}\x{a9e0}-\x{a9e4}\x{a9e6}-\x{a9fe}\x{aa00}-\x{aa28}\x{aa40}-\x{aa42}\x{aa44}-\x{aa4b}\x{aa50}-\x{aa59}\x{aa60}-\x{aa76}\x{aa7a}\x{aa7e}-\x{aaaf}\x{aab1}\x{aab5}-\x{aab6}\x{aab9}-\x{aabd}\x{aac0}\x{aac2}\x{aadb}-\x{aadd}\x{aae0}-\x{aaea}\x{aaf2}-\x{aaf4}\x{ab01}-\x{ab06}\x{ab09}-\x{ab0e}\x{ab11}-\x{ab16}\x{ab20}-\x{ab26}\x{ab28}-\x{ab2e}\x{ab30}-\x{ab5a}\x{ab5c}-\x{ab69}\x{ab70}-\x{abe2}\x{abf0}-\x{abf9}\x{ac00}-\x{d7a3}\x{d7b0}-\x{d7c6}\x{d7cb}-\x{d7fb}\x{f900}-\x{fa6d}\x{fa70}-\x{fad9}\x{fb00}-\x{fb06}\x{fb13}-\x{fb17}\x{fb1d}\x{fb1f}-\x{fb28}\x{fb2a}-\x{fb36}\x{fb38}-\x{fb3c}\x{fb3e}\x{fb40}-\x{fb41}\x{fb43}-\x{fb44}\x{fb46}-\x{fbb1}\x{fbd3}-\x{fd3d}\x{fd50}-\x{fd8f}\x{fd92}-\x{fdc7}\x{fdf0}-\x{fdfb}\x{fe70}-\x{fe74}\x{fe76}-\x{fefc}\x{ff10}-\x{ff19}\x{ff21}-\x{ff3a}\x{ff41}-\x{ff5a}\x{ff66}-\x{ff9f}\x{ffa1}-\x{ffbe}\x{ffc2}-\x{ffc7}\x{ffca}-\x{ffcf}\x{ffd2}-\x{ffd7}\x{ffda}-\x{ffdc}\x{10000}-\x{1000B}\x{1000D}-\x{10026}\x{10028}-\x{1003A}\x{1003C}-\x{1003D}\x{1003F}-\x{1004D}\x{10050}-\x{1005D}\x{10080}-\x{100FA}\x{10107}-\x{10133}\x{10140}-\x{10178}\x{1018A}-\x{1018B}\x{10280}-\x{1029C}\x{102A0}-\x{102D0}\x{102E1}-\x{102FB}\x{10300}-\x{10323}\x{1032D}-\x{1034A}\x{10350}-\x{10375}\x{10380}-\x{1039D}\x{103A0}-\x{103C3}\x{103C8}-\x{103CF}\x{103D1}-\x{103D5}\x{10400}-\x{1049D}\x{104A0}-\x{104A9}\x{104B0}-\x{104D3}\x{104D8}-\x{104FB}\x{10500}-\x{10527}\x{10530}-\x{10563}\x{10570}-\x{1057A}\x{1057C}-\x{1058A}\x{1058C}-\x{10592}\x{10594}-\x{10595}\x{10597}-\x{105A1}\x{105A3}-\x{105B1}\x{105B3}-\x{105B9}\x{105BB}-\x{105BC}\x{105C0}-\x{105F3}\x{10600}-\x{10736}\x{10740}-\x{10755}\x{10760}-\x{10767}\x{10780}-\x{10785}\x{10787}-\x{107B0}\x{107B2}-\x{107BA}\x{10800}-\x{10805}\x{10808}\x{1080A}-\x{10835}\x{10837}-\x{10838}\x{1083C}\x{1083F}-\x{10855}\x{10858}-\x{10876}\x{10879}-\x{1089E}\x{108A7}-\x{108AF}\x{108E0}-\x{108F2}\x{108F4}-\x{108F5}\x{108FB}-\x{1091B}\x{10920}-\x{10939}\x{10980}-\x{109B7}\x{109BC}-\x{109CF}\x{109D2}-\x{10A00}\x{10A10}-\x{10A13}\x{10A15}-\x{10A17}\x{10A19}-\x{10A35}\x{10A40}-\x{10A48}\x{10A60}-\x{10A7E}\x{10A80}-\x{10A9F}\x{10AC0}-\x{10AC7}\x{10AC9}-\x{10AE4}\x{10AEB}-\x{10AEF}\x{10B00}-\x{10B35}\x{10B40}-\x{10B55}\x{10B58}-\x{10B72}\x{10B78}-\x{10B91}\x{10BA9}-\x{10BAF}\x{10C00}-\x{10C48}\x{10C80}-\x{10CB2}\x{10CC0}-\x{10CF2}\x{10CFA}-\x{10D23}\x{10D30}-\x{10D39}\x{10D40}-\x{10D65}\x{10D6F}-\x{10D85}\x{10E60}-\x{10E7E}\x{10E80}-\x{10EA9}\x{10EB0}-\x{10EB1}\x{10EC2}-\x{10EC4}\x{10F00}-\x{10F27}\x{10F30}-\x{10F45}\x{10F51}-\x{10F54}\x{10F70}-\x{10F81}\x{10FB0}-\x{10FCB}\x{10FE0}-\x{10FF6}\x{11003}-\x{11037}\x{11052}-\x{1106F}\x{11071}-\x{11072}\x{11075}\x{11083}-\x{110AF}\x{110D0}-\x{110E8}\x{110F0}-\x{110F9}\x{11103}-\x{11126}\x{11136}-\x{1113F}\x{11144}\x{11147}\x{11150}-\x{11172}\x{11176}\x{11183}-\x{111B2}\x{111C1}-\x{111C4}\x{111D0}-\x{111DA}\x{111DC}\x{111E1}-\x{111F4}\x{11200}-\x{11211}\x{11213}-\x{1122B}\x{1123F}-\x{11240}\x{11280}-\x{11286}\x{11288}\x{1128A}-\x{1128D}\x{1128F}-\x{1129D}\x{1129F}-\x{112A8}\x{112B0}-\x{112DE}\x{112F0}-\x{112F9}\x{11305}-\x{1130C}\x{1130F}-\x{11310}\x{11313}-\x{11328}\x{1132A}-\x{11330}\x{11332}-\x{11333}\x{11335}-\x{11339}\x{1133D}\x{11350}\x{1135D}-\x{11361}\x{11380}-\x{11389}\x{1138B}\x{1138E}\x{11390}-\x{113B5}\x{113B7}\x{113D1}\x{113D3}\x{11400}-\x{11434}\x{11447}-\x{1144A}\x{11450}-\x{11459}\x{1145F}-\x{11461}\x{11480}-\x{114AF}\x{114C4}-\x{114C5}\x{114C7}\x{114D0}-\x{114D9}\x{11580}-\x{115AE}\x{115D8}-\x{115DB}\x{11600}-\x{1162F}\x{11644}\x{11650}-\x{11659}\x{11680}-\x{116AA}\x{116B8}\x{116C0}-\x{116C9}\x{116D0}-\x{116E3}\x{11700}-\x{1171A}\x{11730}-\x{1173B}\x{11740}-\x{11746}\x{11800}-\x{1182B}\x{118A0}-\x{118F2}\x{118FF}-\x{11906}\x{11909}\x{1190C}-\x{11913}\x{11915}-\x{11916}\x{11918}-\x{1192F}\x{1193F}\x{11941}\x{11950}-\x{11959}\x{119A0}-\x{119A7}\x{119AA}-\x{119D0}\x{119E1}\x{119E3}\x{11A00}\x{11A0B}-\x{11A32}\x{11A3A}\x{11A50}\x{11A5C}-\x{11A89}\x{11A9D}\x{11AB0}-\x{11AF8}\x{11BC0}-\x{11BE0}\x{11BF0}-\x{11BF9}\x{11C00}-\x{11C08}\x{11C0A}-\x{11C2E}\x{11C40}\x{11C50}-\x{11C6C}\x{11C72}-\x{11C8F}\x{11D00}-\x{11D06}\x{11D08}-\x{11D09}\x{11D0B}-\x{11D30}\x{11D46}\x{11D50}-\x{11D59}\x{11D60}-\x{11D65}\x{11D67}-\x{11D68}\x{11D6A}-\x{11D89}\x{11D98}\x{11DA0}-\x{11DA9}\x{11EE0}-\x{11EF2}\x{11F02}\x{11F04}-\x{11F10}\x{11F12}-\x{11F33}\x{11F50}-\x{11F59}\x{11FB0}\x{11FC0}-\x{11FD4}\x{12000}-\x{12399}\x{12400}-\x{1246E}\x{12480}-\x{12543}\x{12F90}-\x{12FF0}\x{13000}-\x{1342F}\x{13441}-\x{13446}\x{13460}-\x{143FA}\x{14400}-\x{14646}\x{16100}-\x{1611D}\x{16130}-\x{16139}\x{16800}-\x{16A38}\x{16A40}-\x{16A5E}\x{16A60}-\x{16A69}\x{16A70}-\x{16ABE}\x{16AC0}-\x{16AC9}\x{16AD0}-\x{16AED}\x{16B00}-\x{16B2F}\x{16B40}-\x{16B43}\x{16B50}-\x{16B59}\x{16B5B}-\x{16B61}\x{16B63}-\x{16B77}\x{16B7D}-\x{16B8F}\x{16D40}-\x{16D6C}\x{16D70}-\x{16D79}\x{16E40}-\x{16E96}\x{16F00}-\x{16F4A}\x{16F50}\x{16F93}-\x{16F9F}\x{16FE0}-\x{16FE1}\x{16FE3}\x{17000}-\x{187F7}\x{18800}-\x{18CD5}\x{18CFF}-\x{18D08}\x{1AFF0}-\x{1AFF3}\x{1AFF5}-\x{1AFFB}\x{1AFFD}-\x{1AFFE}\x{1B000}-\x{1B122}\x{1B132}\x{1B150}-\x{1B152}\x{1B155}\x{1B164}-\x{1B167}\x{1B170}-\x{1B2FB}\x{1BC00}-\x{1BC6A}\x{1BC70}-\x{1BC7C}\x{1BC80}-\x{1BC88}\x{1BC90}-\x{1BC99}\x{1CCF0}-\x{1CCF9}\x{1D2C0}-\x{1D2D3}\x{1D2E0}-\x{1D2F3}\x{1D360}-\x{1D378}\x{1D400}-\x{1D454}\x{1D456}-\x{1D49C}\x{1D49E}-\x{1D49F}\x{1D4A2}\x{1D4A5}-\x{1D4A6}\x{1D4A9}-\x{1D4AC}\x{1D4AE}-\x{1D4B9}\x{1D4BB}\x{1D4BD}-\x{1D4C3}\x{1D4C5}-\x{1D505}\x{1D507}-\x{1D50A}\x{1D50D}-\x{1D514}\x{1D516}-\x{1D51C}\x{1D51E}-\x{1D539}\x{1D53B}-\x{1D53E}\x{1D540}-\x{1D544}\x{1D546}\x{1D54A}-\x{1D550}\x{1D552}-\x{1D6A5}\x{1D6A8}-\x{1D6C0}\x{1D6C2}-\x{1D6DA}\x{1D6DC}-\x{1D6FA}\x{1D6FC}-\x{1D714}\x{1D716}-\x{1D734}\x{1D736}-\x{1D74E}\x{1D750}-\x{1D76E}\x{1D770}-\x{1D788}\x{1D78A}-\x{1D7A8}\x{1D7AA}-\x{1D7C2}\x{1D7C4}-\x{1D7CB}\x{1D7CE}-\x{1D7FF}\x{1DF00}-\x{1DF1E}\x{1DF25}-\x{1DF2A}\x{1E030}-\x{1E06D}\x{1E100}-\x{1E12C}\x{1E137}-\x{1E13D}\x{1E140}-\x{1E149}\x{1E14E}\x{1E290}-\x{1E2AD}\x{1E2C0}-\x{1E2EB}\x{1E2F0}-\x{1E2F9}\x{1E4D0}-\x{1E4EB}\x{1E4F0}-\x{1E4F9}\x{1E5D0}-\x{1E5ED}\x{1E5F0}-\x{1E5FA}\x{1E7E0}-\x{1E7E6}\x{1E7E8}-\x{1E7EB}\x{1E7ED}-\x{1E7EE}\x{1E7F0}-\x{1E7FE}\x{1E800}-\x{1E8C4}\x{1E8C7}-\x{1E8CF}\x{1E900}-\x{1E943}\x{1E94B}\x{1E950}-\x{1E959}\x{1EC71}-\x{1ECAB}\x{1ECAD}-\x{1ECAF}\x{1ECB1}-\x{1ECB4}\x{1ED01}-\x{1ED2D}\x{1ED2F}-\x{1ED3D}\x{1EE00}-\x{1EE03}\x{1EE05}-\x{1EE1F}\x{1EE21}-\x{1EE22}\x{1EE24}\x{1EE27}\x{1EE29}-\x{1EE32}\x{1EE34}-\x{1EE37}\x{1EE39}\x{1EE3B}\x{1EE42}\x{1EE47}\x{1EE49}\x{1EE4B}\x{1EE4D}-\x{1EE4F}\x{1EE51}-\x{1EE52}\x{1EE54}\x{1EE57}\x{1EE59}\x{1EE5B}\x{1EE5D}\x{1EE5F}\x{1EE61}-\x{1EE62}\x{1EE64}\x{1EE67}-\x{1EE6A}\x{1EE6C}-\x{1EE72}\x{1EE74}-\x{1EE77}\x{1EE79}-\x{1EE7C}\x{1EE7E}\x{1EE80}-\x{1EE89}\x{1EE8B}-\x{1EE9B}\x{1EEA1}-\x{1EEA3}\x{1EEA5}-\x{1EEA9}\x{1EEAB}-\x{1EEBB}\x{1F100}-\x{1F10C}\x{1FBF0}-\x{1FBF9}\x{20000}-\x{2A6DF}\x{2A700}-\x{2B739}\x{2B740}-\x{2B81D}\x{2B820}-\x{2CEA1}\x{2CEB0}-\x{2EBE0}\x{2EBF0}-\x{2EE5D}\x{2F800}-\x{2FA1D}\x{30000}-\x{3134A}\x{31350}-\x{323AF}]*$|^ | $|  |[\x{0300}-\x{034e}\x{0350}-\x{036f}\x{0483}-\x{0489}\x{0591}-\x{05bd}\x{05bf}\x{05c1}-\x{05c2}\x{05c4}-\x{05c5}\x{05c7}\x{0610}-\x{061a}\x{064b}-\x{065f}\x{0670}\x{06d6}-\x{06dc}\x{06df}-\x{06e4}\x{06e7}-\x{06e8}\x{06ea}-\x{06ed}\x{0711}\x{0730}-\x{074a}\x{07a6}-\x{07b0}\x{07eb}-\x{07f3}\x{07fd}\x{0816}-\x{0819}\x{081b}-\x{0823}\x{0825}-\x{0827}\x{0829}-\x{082d}\x{0859}-\x{085b}\x{0897}-\x{089f}\x{08ca}-\x{08e1}\x{08e3}-\x{0903}\x{093a}-\x{093c}\x{093e}-\x{094f}\x{0951}-\x{0957}\x{0962}-\x{0963}\x{0981}-\x{0983}\x{09bc}\x{09be}-\x{09c4}\x{09c7}-\x{09c8}\x{09cb}-\x{09cd}\x{09d7}\x{09e2}-\x{09e3}\x{09fe}\x{0a01}-\x{0a03}\x{0a3c}\x{0a3e}-\x{0a42}\x{0a47}-\x{0a48}\x{0a4b}-\x{0a4d}\x{0a51}\x{0a70}-\x{0a71}\x{0a75}\x{0a81}-\x{0a83}\x{0abc}\x{0abe}-\x{0ac5}\x{0ac7}-\x{0ac9}\x{0acb}-\x{0acd}\x{0ae2}-\x{0ae3}\x{0afa}-\x{0aff}\x{0b01}-\x{0b03}\x{0b3c}\x{0b3e}-\x{0b44}\x{0b47}-\x{0b48}\x{0b4b}-\x{0b4d}\x{0b55}-\x{0b57}\x{0b62}-\x{0b63}\x{0b82}\x{0bbe}-\x{0bc2}\x{0bc6}-\x{0bc8}\x{0bca}-\x{0bcd}\x{0bd7}\x{0c00}-\x{0c04}\x{0c3c}\x{0c3e}-\x{0c44}\x{0c46}-\x{0c48}\x{0c4a}-\x{0c4d}\x{0c55}-\x{0c56}\x{0c62}-\x{0c63}\x{0c81}-\x{0c83}\x{0cbc}\x{0cbe}-\x{0cc4}\x{0cc6}-\x{0cc8}\x{0cca}-\x{0ccd}\x{0cd5}-\x{0cd6}\x{0ce2}-\x{0ce3}\x{0cf3}\x{0d00}-\x{0d03}\x{0d3b}-\x{0d3c}\x{0d3e}-\x{0d44}\x{0d46}-\x{0d48}\x{0d4a}-\x{0d4d}\x{0d57}\x{0d62}-\x{0d63}\x{0d81}-\x{0d83}\x{0dca}\x{0dcf}-\x{0dd4}\x{0dd6}\x{0dd8}-\x{0ddf}\x{0df2}-\x{0df3}\x{0e31}\x{0e34}-\x{0e3a}\x{0e47}-\x{0e4e}\x{0eb1}\x{0eb4}-\x{0ebc}\x{0ec8}-\x{0ece}\x{0f18}-\x{0f19}\x{0f35}\x{0f37}\x{0f39}\x{0f3e}-\x{0f3f}\x{0f71}-\x{0f84}\x{0f86}-\x{0f87}\x{0f8d}-\x{0f97}\x{0f99}-\x{0fbc}\x{0fc6}\x{102b}-\x{103e}\x{1056}-\x{1059}\x{105e}-\x{1060}\x{1062}-\x{1064}\x{1067}-\x{106d}\x{1071}-\x{1074}\x{1082}-\x{108d}\x{108f}\x{109a}-\x{109d}\x{135d}-\x{135f}\x{1712}-\x{1715}\x{1732}-\x{1734}\x{1752}-\x{1753}\x{1772}-\x{1773}\x{17b6}-\x{17d3}\x{17dd}\x{1885}-\x{1886}\x{18a9}\x{1920}-\x{192b}\x{1930}-\x{193b}\x{1a17}-\x{1a1b}\x{1a55}-\x{1a5e}\x{1a60}-\x{1a7c}\x{1a7f}\x{1ab0}-\x{1ace}\x{1b00}-\x{1b04}\x{1b34}-\x{1b44}\x{1b6b}-\x{1b73}\x{1b80}-\x{1b82}\x{1ba1}-\x{1bad}\x{1be6}-\x{1bf3}\x{1c24}-\x{1c37}\x{1cd0}-\x{1cd2}\x{1cd4}-\x{1ce8}\x{1ced}\x{1cf4}\x{1cf7}-\x{1cf9}\x{1dc0}-\x{1dff}\x{20d0}-\x{20f0}\x{2cef}-\x{2cf1}\x{2d7f}\x{2de0}-\x{2dff}\x{302a}-\x{302f}\x{3099}-\x{309a}\x{a66f}-\x{a672}\x{a674}-\x{a67d}\x{a69e}-\x{a69f}\x{a6f0}-\x{a6f1}\x{a802}\x{a806}\x{a80b}\x{a823}-\x{a827}\x{a82c}\x{a880}-\x{a881}\x{a8b4}-\x{a8c5}\x{a8e0}-\x{a8f1}\x{a8ff}\x{a926}-\x{a92d}\x{a947}-\x{a953}\x{a980}-\x{a983}\x{a9b3}-\x{a9c0}\x{a9e5}\x{aa29}-\x{aa36}\x{aa43}\x{aa4c}-\x{aa4d}\x{aa7b}-\x{aa7d}\x{aab0}\x{aab2}-\x{aab4}\x{aab7}-\x{aab8}\x{aabe}-\x{aabf}\x{aac1}\x{aaeb}-\x{aaef}\x{aaf5}-\x{aaf6}\x{abe3}-\x{abea}\x{abec}-\x{abed}\x{fb1e}\x{fe20}-\x{fe2f}\x{101FD}\x{102E0}\x{10376}-\x{1037A}\x{10A01}-\x{10A03}\x{10A05}-\x{10A06}\x{10A0C}-\x{10A0F}\x{10A38}-\x{10A3A}\x{10A3F}\x{10AE5}-\x{10AE6}\x{10D24}-\x{10D27}\x{10D69}-\x{10D6D}\x{10EAB}-\x{10EAC}\x{10EFC}-\x{10EFF}\x{10F46}-\x{10F50}\x{10F82}-\x{10F85}\x{11000}-\x{11002}\x{11038}-\x{11046}\x{11070}\x{11073}-\x{11074}\x{1107F}-\x{11082}\x{110B0}-\x{110BA}\x{110C2}\x{11100}-\x{11102}\x{11127}-\x{11134}\x{11145}-\x{11146}\x{11173}\x{11180}-\x{11182}\x{111B3}-\x{111C0}\x{111C9}-\x{111CC}\x{111CE}-\x{111CF}\x{1122C}-\x{11237}\x{1123E}\x{11241}\x{112DF}-\x{112EA}\x{11300}-\x{11303}\x{1133B}-\x{1133C}\x{1133E}-\x{11344}\x{11347}-\x{11348}\x{1134B}-\x{1134D}\x{11357}\x{11362}-\x{11363}\x{11366}-\x{1136C}\x{11370}-\x{11374}\x{113B8}-\x{113C0}\x{113C2}\x{113C5}\x{113C7}-\x{113CA}\x{113CC}-\x{113D0}\x{113D2}\x{113E1}-\x{113E2}\x{11435}-\x{11446}\x{1145E}\x{114B0}-\x{114C3}\x{115AF}-\x{115B5}\x{115B8}-\x{115C0}\x{115DC}-\x{115DD}\x{11630}-\x{11640}\x{116AB}-\x{116B7}\x{1171D}-\x{1172B}\x{1182C}-\x{1183A}\x{11930}-\x{11935}\x{11937}-\x{11938}\x{1193B}-\x{1193E}\x{11940}\x{11942}-\x{11943}\x{119D1}-\x{119D7}\x{119DA}-\x{119E0}\x{119E4}\x{11A01}-\x{11A0A}\x{11A33}-\x{11A39}\x{11A3B}-\x{11A3E}\x{11A47}\x{11A51}-\x{11A5B}\x{11A8A}-\x{11A99}\x{11C2F}-\x{11C36}\x{11C38}-\x{11C3F}\x{11C92}-\x{11CA7}\x{11CA9}-\x{11CB6}\x{11D31}-\x{11D36}\x{11D3A}\x{11D3C}-\x{11D3D}\x{11D3F}-\x{11D45}\x{11D47}\x{11D8A}-\x{11D8E}\x{11D90}-\x{11D91}\x{11D93}-\x{11D97}\x{11EF3}-\x{11EF6}\x{11F00}-\x{11F01}\x{11F03}\x{11F34}-\x{11F3A}\x{11F3E}-\x{11F42}\x{11F5A}\x{13440}\x{13447}-\x{13455}\x{1611E}-\x{1612F}\x{16AF0}-\x{16AF4}\x{16B30}-\x{16B36}\x{16F4F}\x{16F51}-\x{16F87}\x{16F8F}-\x{16F92}\x{16FF0}-\x{16FF1}\x{1BC9D}-\x{1BC9E}\x{1CF00}-\x{1CF2D}\x{1CF30}-\x{1CF46}\x{1D165}-\x{1D169}\x{1D16D}-\x{1D172}\x{1D17B}-\x{1D182}\x{1D185}-\x{1D18B}\x{1D1AA}-\x{1D1AD}\x{1D242}-\x{1D244}\x{1DA00}-\x{1DA36}\x{1DA3B}-\x{1DA6C}\x{1DA75}\x{1DA84}\x{1DA9B}-\x{1DA9F}\x{1DAA1}-\x{1DAAF}\x{1E000}-\x{1E006}\x{1E008}-\x{1E018}\x{1E01B}-\x{1E021}\x{1E023}-\x{1E024}\x{1E026}-\x{1E02A}\x{1E08F}\x{1E130}-\x{1E136}\x{1E2AE}\x{1E2EC}-\x{1E2EF}\x{1E4EC}-\x{1E4EF}\x{1E5EE}-\x{1E5EF}\x{1E8D0}-\x{1E8D6}\x{1E944}-\x{1E94A}]{4}",
    r"^[a-z0-9]([a-z0-9._-]{0,62}[a-z0-9_-])?$",
    r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$",
    r"^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)*$",
    r"^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)+$",
    r"^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)?$",
    r"^[a-z][a-z0-9_]{0,31}$",
    r"^account/",
    r"^action/",
    r"^approval/",
    r"^dev-[0-9]{8}-[0-9a-f]{7,12}$",
    r"^host/",
    r"^host\.disk\.free_(gb|pct)$",
    r"^queue/",
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
        "urn:ads:command:auth.hello:1" => support::reencode::<CommandAuthHelloV1>(value),
        "urn:ads:command:events.subscribe:1" => {
            support::reencode::<CommandEventsSubscribeV1>(value)
        }
        "urn:ads:command:harness.list.response:1" => {
            support::reencode::<CommandHarnessListResponseV1>(value)
        }
        "urn:ads:command:harness.list:1" => support::reencode::<CommandHarnessListV1>(value),
        "urn:ads:command:request:1" => support::reencode::<CommandRequestV1>(value),
        "urn:ads:command:response:1" => support::reencode::<CommandResponseV1>(value),
        "urn:ads:command:response:2" => support::reencode::<CommandResponseV2>(value),
        "urn:ads:command:session.close:1" => support::reencode::<CommandSessionCloseV1>(value),
        "urn:ads:command:session.open:1" => support::reencode::<CommandSessionOpenV1>(value),
        "urn:ads:command:session.resize:1" => support::reencode::<CommandSessionResizeV1>(value),
        "urn:ads:command:session.write:1" => support::reencode::<CommandSessionWriteV1>(value),
        "urn:ads:common:abspath:1" => support::reencode::<CommonAbspathV1>(value),
        "urn:ads:common:abspath:2" => support::reencode::<CommonAbspathV2>(value),
        "urn:ads:common:action-rule:1" => support::reencode::<CommonActionRuleV1>(value),
        "urn:ads:common:actor:1" => support::reencode::<CommonActorV1>(value),
        "urn:ads:common:base64:1" => support::reencode::<CommonBase64V1>(value),
        "urn:ads:common:data-class:1" => support::reencode::<CommonDataClassV1>(value),
        "urn:ads:common:id:1" => support::reencode::<CommonIdV1>(value),
        "urn:ads:common:queue-task-state:1" => support::reencode::<CommonQueueTaskStateV1>(value),
        "urn:ads:common:relpath:1" => support::reencode::<CommonRelpathV1>(value),
        "urn:ads:common:risk:1" => support::reencode::<CommonRiskV1>(value),
        "urn:ads:common:sha256:1" => support::reencode::<CommonSha256V1>(value),
        "urn:ads:common:timestamp:1" => support::reencode::<CommonTimestampV1>(value),
        "urn:ads:common:ulid:1" => support::reencode::<CommonUlidV1>(value),
        "urn:ads:contract:harness-adapter:1" => {
            support::reencode::<ContractHarnessAdapterV1>(value)
        }
        "urn:ads:contract:local-api.error-code:1" => {
            support::reencode::<ContractLocalApiErrorCodeV1>(value)
        }
        "urn:ads:contract:local-api.server-message:1" => {
            support::reencode::<ContractLocalApiServerMessageV1>(value)
        }
        "urn:ads:contract:local-api.server-message:2" => {
            support::reencode::<ContractLocalApiServerMessageV2>(value)
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
        "urn:ads:event:account.status_observed:1" => {
            support::reencode::<EventAccountStatusObservedV1>(value)
        }
        "urn:ads:event:account.switched:1" => support::reencode::<EventAccountSwitchedV1>(value),
        "urn:ads:event:action.denied:1" => support::reencode::<EventActionDeniedV1>(value),
        "urn:ads:event:action.executed:1" => support::reencode::<EventActionExecutedV1>(value),
        "urn:ads:event:action.failed:1" => support::reencode::<EventActionFailedV1>(value),
        "urn:ads:event:action.planned:1" => support::reencode::<EventActionPlannedV1>(value),
        "urn:ads:event:action.started:1" => support::reencode::<EventActionStartedV1>(value),
        "urn:ads:event:approval.denied:1" => support::reencode::<EventApprovalDeniedV1>(value),
        "urn:ads:event:approval.denied:2" => support::reencode::<EventApprovalDeniedV2>(value),
        "urn:ads:event:approval.expired:1" => support::reencode::<EventApprovalExpiredV1>(value),
        "urn:ads:event:approval.expired:2" => support::reencode::<EventApprovalExpiredV2>(value),
        "urn:ads:event:approval.granted:1" => support::reencode::<EventApprovalGrantedV1>(value),
        "urn:ads:event:approval.granted:2" => support::reencode::<EventApprovalGrantedV2>(value),
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
        "urn:ads:event:queue.clock_reanchored:1" => {
            support::reencode::<EventQueueClockReanchoredV1>(value)
        }
        "urn:ads:event:queue.created:1" => support::reencode::<EventQueueCreatedV1>(value),
        "urn:ads:event:queue.enabled:1" => support::reencode::<EventQueueEnabledV1>(value),
        "urn:ads:event:queue.stopped:1" => support::reencode::<EventQueueStoppedV1>(value),
        "urn:ads:event:queue.task_created:1" => support::reencode::<EventQueueTaskCreatedV1>(value),
        "urn:ads:event:queue.task_created:2" => support::reencode::<EventQueueTaskCreatedV2>(value),
        "urn:ads:event:queue.task_state_changed:1" => {
            support::reencode::<EventQueueTaskStateChangedV1>(value)
        }
        "urn:ads:event:review.completed:1" => support::reencode::<EventReviewCompletedV1>(value),
        "urn:ads:event:review.failed:1" => support::reencode::<EventReviewFailedV1>(value),
        "urn:ads:event:review.requested:1" => support::reencode::<EventReviewRequestedV1>(value),
        "urn:ads:event:routing.decided:1" => support::reencode::<EventRoutingDecidedV1>(value),
        "urn:ads:event:routing.decided:2" => support::reencode::<EventRoutingDecidedV2>(value),
        "urn:ads:event:routing.fallback:1" => support::reencode::<EventRoutingFallbackV1>(value),
        "urn:ads:event:routing.fallback:2" => support::reencode::<EventRoutingFallbackV2>(value),
        "urn:ads:event:routing.proposed:1" => support::reencode::<EventRoutingProposedV1>(value),
        "urn:ads:event:routing.proposed:2" => support::reencode::<EventRoutingProposedV2>(value),
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
        "urn:ads:record:action-plan:2" => support::reencode::<RecordActionPlanV2>(value),
        "urn:ads:record:approval:1" => support::reencode::<RecordApprovalV1>(value),
        "urn:ads:record:host_event:1" => support::reencode::<RecordHostEventV1>(value),
        "urn:ads:record:host_sample:1" => support::reencode::<RecordHostSampleV1>(value),
        "urn:ads:record:parameter:1" => support::reencode::<RecordParameterV1>(value),
        "urn:ads:record:queue-task:1" => support::reencode::<RecordQueueTaskV1>(value),
        "urn:ads:record:task-packet:1" => support::reencode::<RecordTaskPacketV1>(value),
        "urn:ads:record:task-queue:1" => support::reencode::<RecordTaskQueueV1>(value),
        "urn:ads:record:update-manifest:1" => support::reencode::<RecordUpdateManifestV1>(value),
        "urn:ads:record:update-status:1" => support::reencode::<RecordUpdateStatusV1>(value),
        "urn:ads:record:usage_event:1" => support::reencode::<RecordUsageEventV1>(value),
        "urn:ads:relay:device:1" => support::reencode::<RelayDeviceV1>(value),
        "urn:ads:relay:envelope:1" => support::reencode::<RelayEnvelopeV1>(value),
        _ => return None,
    };
    Some(written)
}
