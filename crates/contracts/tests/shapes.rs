//! The shapes the golden files do not pin down by themselves: presence against `null`, tagged
//! and untagged unions, open objects, and validation on construction (not only on reading).

use std::fs;
use std::path::Path;

use ads_contracts::{
    CommandRequestV1, CommandRequestV1Params, CommandResponseV2, CommonAbspathV1, CommonIdV1,
    CommonTimestampV1, ContractPtyHostRequestV1, ContractPtyHostResponseV1,
    ContractPtyHostResponseV1Result, EventActionStartedV1, EventSessionCreatedV1,
    EventSessionCreatedV1DataCols, RecordActionPlanV1Targets, RecordApprovalV1,
    RecordApprovalV1Status, RecordHostEventV1, RecordTaskPacketV1BudgetQuotaPct,
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

fn golden(key: &str, file: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../contracts/golden")
        .join(key)
        .join("valid")
        .join(file);
    serde_json::from_str(&fs::read_to_string(&path).expect("golden file")).expect("golden JSON")
}

fn accepts<T: DeserializeOwned>(value: &Value) -> bool {
    serde_json::from_value::<T>(value.clone()).is_ok()
}

#[test]
fn a_required_field_that_takes_null_cannot_be_missing() {
    let mut event = golden("event.action.started.v1", "01-base.json");
    assert!(accepts::<EventActionStartedV1>(&event));
    event["data"]["approval_id"] = Value::Null;
    assert!(accepts::<EventActionStartedV1>(&event));
    event["data"].as_object_mut().unwrap().remove("approval_id");
    assert!(!accepts::<EventActionStartedV1>(&event));
}

#[test]
fn an_optional_field_that_does_not_take_null_refuses_it() {
    let mut event = golden("event.session.created.v1", "03-with-display-name.json");
    assert!(accepts::<EventSessionCreatedV1>(&event));
    event["data"]["display_name"] = Value::Null;
    assert!(!accepts::<EventSessionCreatedV1>(&event));
    event["data"]
        .as_object_mut()
        .unwrap()
        .remove("display_name");
    let read: EventSessionCreatedV1 = serde_json::from_value(event.clone()).unwrap();
    assert!(read.data.display_name.is_none());
    assert_eq!(serde_json::to_value(&read).unwrap(), event);
}

#[test]
fn an_optional_field_that_takes_null_keeps_missing_apart_from_null() {
    let mut record = golden("record.host_event.v1", "01-base.json");
    record.as_object_mut().unwrap().remove("task_id");
    let missing: RecordHostEventV1 = serde_json::from_value(record.clone()).unwrap();
    assert_eq!(missing.task_id, None);
    assert_eq!(serde_json::to_value(&missing).unwrap(), record);

    record["task_id"] = Value::Null;
    let null: RecordHostEventV1 = serde_json::from_value(record.clone()).unwrap();
    assert_eq!(null.task_id, Some(None));
    assert_eq!(serde_json::to_value(&null).unwrap(), record);

    record["task_id"] = json!("task-01");
    let set: RecordHostEventV1 = serde_json::from_value(record.clone()).unwrap();
    assert_eq!(set.task_id, Some(Some(CommonIdV1::new("task-01").unwrap())));
    assert_eq!(serde_json::to_value(&set).unwrap(), record);
}

#[test]
fn a_tagged_union_reads_by_the_tag_and_stays_closed() {
    let hello = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "host.hello",
        "params": {"contract": "pty-host/1"}
    });
    assert!(matches!(
        serde_json::from_value::<ContractPtyHostRequestV1>(hello.clone()).unwrap(),
        ContractPtyHostRequestV1::HostHello(_)
    ));
    let with = |change: fn(&mut Value)| {
        let mut changed = hello.clone();
        change(&mut changed);
        changed
    };
    for refused in [
        with(|v| v["method"] = json!("host.bye")),
        with(|v| v["extra"] = json!(1)),
        with(|v| v["params"]["extra"] = json!(1)),
        with(|v| v["jsonrpc"] = json!("1.0")),
        with(|v| v["id"] = json!(0)),
        with(|v| v["params"]["contract"] = json!("pty-host/2")),
        with(|v| {
            v.as_object_mut().unwrap().remove("method");
        }),
        // The fields of another variant do not fit this tag.
        with(|v| v["params"] = json!({"session_id": "s1"})),
    ] {
        assert!(!accepts::<ContractPtyHostRequestV1>(&refused), "{refused}");
    }
}

#[test]
fn an_untagged_union_picks_the_one_closed_shape_that_fits() {
    let read = |result: Value| {
        let response = json!({"jsonrpc": "2.0", "id": 1, "result": result});
        serde_json::from_value::<ContractPtyHostResponseV1>(response).map(|r| r.result.clone())
    };
    assert!(matches!(
        read(json!({})),
        Ok(Some(ContractPtyHostResponseV1Result::Empty(_)))
    ));
    assert!(matches!(
        read(json!({"exit_code": null})),
        Ok(Some(ContractPtyHostResponseV1Result::Closed(_)))
    ));
    assert!(matches!(
        read(json!({"oldest_seq": 1, "next_seq": 9})),
        Ok(Some(ContractPtyHostResponseV1Result::ReplayBounds(_)))
    ));
    assert!(read(json!({"oldest_seq": 0, "next_seq": 9})).is_err());
    assert!(read(json!({"exit_code": null, "extra": 1})).is_err());
}

#[test]
fn result_and_error_exclude_each_other() {
    let ok = golden("command.response.v2", "01-base.json");
    let failed = golden("command.response.v2", "03-error.json");
    assert!(accepts::<CommandResponseV2>(&ok));
    assert!(accepts::<CommandResponseV2>(&failed));

    let mut both = ok.clone();
    both["error"] = failed["error"].clone();
    assert!(!accepts::<CommandResponseV2>(&both));
    let mut neither = ok.clone();
    neither.as_object_mut().unwrap().remove("result");
    assert!(!accepts::<CommandResponseV2>(&neither));

    // The same rule holds when the value is built in code instead of read.
    let mut fields = serde_json::from_value::<CommandResponseV2>(ok)
        .unwrap()
        .into_fields();
    fields.error = serde_json::from_value::<CommandResponseV2>(failed)
        .unwrap()
        .into_fields()
        .error;
    let refused = CommandResponseV2::new(fields).unwrap_err();
    assert_eq!(
        refused.to_string(),
        "not a valid CommandResponseV2: rule #/oneOf does not hold"
    );
}

#[test]
fn an_open_object_keeps_the_unnamed_keys_and_refuses_a_named_one_twice() {
    let request = golden("command.request.v1", "01-base.json");
    let read: CommandRequestV1 = serde_json::from_value(request.clone()).unwrap();
    assert!(!read.params.extra.contains_key("_meta"));
    assert_eq!(serde_json::to_value(&read).unwrap(), request);

    let mut fields = read.params.clone().into_fields();
    fields.extra.insert("anything".to_owned(), json!([1, 2]));
    let open = CommandRequestV1Params::new(fields.clone()).unwrap();
    assert_eq!(
        serde_json::to_value(&open).unwrap()["anything"],
        json!([1, 2])
    );

    fields.extra.insert("_meta".to_owned(), json!({}));
    assert!(CommandRequestV1Params::new(fields).is_err());
}

#[test]
fn a_rule_between_fields_is_checked_on_construction() {
    let approval = golden("record.approval.v1", "01-base.json");
    let read: RecordApprovalV1 = serde_json::from_value(approval).unwrap();
    let mut fields = read.clone().into_fields();
    // Whatever the state was, moving it to the other side of the rule without the fields that
    // go with it is refused.
    fields.status = match fields.status {
        RecordApprovalV1Status::Pending | RecordApprovalV1Status::Expired => {
            RecordApprovalV1Status::Granted
        }
        RecordApprovalV1Status::Granted | RecordApprovalV1Status::Denied => {
            RecordApprovalV1Status::Pending
        }
    };
    assert!(RecordApprovalV1::new(fields).is_err());
    assert!(RecordApprovalV1::new(read.into_fields()).is_ok());
}

#[test]
fn constructors_refuse_what_the_schema_refuses() {
    assert!(CommonIdV1::new("task-01").is_ok());
    assert!(CommonIdV1::new("has space").is_err());
    assert!(CommonIdV1::new("").is_err());
    assert!(CommonIdV1::try_from("a".repeat(65)).is_err());

    assert!(CommonAbspathV1::new("/work/proj").is_ok());
    assert!(CommonAbspathV1::new("C:\\work\\proj").is_ok());
    for refused in [
        "work/proj",
        "/work/../etc",
        "/work/$VAR",
        "/work/~user",
        "/work/*",
        "/work/a\u{7}b",
        "\\\\host\\share",
    ] {
        assert!(CommonAbspathV1::new(refused).is_err(), "{refused}");
    }

    assert!(CommonTimestampV1::new("2026-01-31T10:00:00.123Z").is_ok());
    assert!(CommonTimestampV1::new("2026-02-30T10:00:00Z").is_err());
    assert!(CommonTimestampV1::new("2026-01-31T10:00:00+00:00").is_err());

    assert!(EventSessionCreatedV1DataCols::new(1000).is_ok());
    assert!(EventSessionCreatedV1DataCols::new(0).is_err());
    assert!(EventSessionCreatedV1DataCols::new(1001).is_err());
    assert!(serde_json::from_value::<EventSessionCreatedV1DataCols>(json!(80.5)).is_err());

    let pct = |value: Value| serde_json::from_value::<RecordTaskPacketV1BudgetQuotaPct>(value);
    assert_eq!(
        serde_json::to_value(pct(json!(12)).unwrap()).unwrap(),
        json!(12)
    );
    assert_eq!(
        serde_json::to_value(pct(json!(12.5)).unwrap()).unwrap(),
        json!(12.5)
    );
    assert!(pct(json!(100.5)).is_err());
    assert!(pct(json!(-1)).is_err());

    assert!(RecordActionPlanV1Targets::new(Vec::new()).is_err());
    let target = |path: &str| {
        serde_json::from_value(json!({"path": path, "pattern": null, "repo_root": false})).unwrap()
    };
    assert!(RecordActionPlanV1Targets::new(vec![target("/a"), target("/b")]).is_ok());
    assert!(RecordActionPlanV1Targets::new(vec![target("/a"), target("/a")]).is_err());
}
