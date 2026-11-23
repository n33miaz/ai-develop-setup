use super::*;

fn id(text: &str) -> SessionId {
    SessionId::new(text).unwrap()
}

#[test]
fn output_frame_has_the_documented_layout() {
    let frame = encode_output(&id("sess-01"), 7, b"hello").unwrap();
    let header = br#"{"session_id":"sess-01","seq":7,"length":5}"#;

    assert_eq!(frame[0], 0x02);
    let payload_len = u32::from_be_bytes(frame[1..5].try_into().unwrap()) as usize;
    assert_eq!(payload_len, frame.len() - 5);
    assert_eq!(payload_len, 2 + header.len() + 5);
    let header_len = usize::from(u16::from_be_bytes(frame[5..7].try_into().unwrap()));
    assert_eq!(header_len, header.len());
    assert_eq!(&frame[7..7 + header_len], header);
    assert_eq!(&frame[7 + header_len..], b"hello");
}

#[test]
fn json_frame_is_kind_length_and_the_message() {
    let frame = encode_json(&serde_json::json!({"a": 1})).unwrap();
    assert_eq!(frame, [&[0x01, 0, 0, 0, 7][..], br#"{"a":1}"#].concat());
}

#[tokio::test]
async fn frames_round_trip_through_the_reader() {
    let mut stream = encode_json(&serde_json::json!({"a": 1})).unwrap();
    stream.extend(encode_output(&id("s"), 3, &[0, 255, 10, 13]).unwrap());
    let mut reader = stream.as_slice();

    assert_eq!(
        read_frame(&mut reader, 1024).await.unwrap(),
        Some(Frame::Json(br#"{"a":1}"#.to_vec()))
    );
    let Some(Frame::Output { header, bytes }) = read_frame(&mut reader, 1024).await.unwrap() else {
        panic!("expected an output frame");
    };
    assert_eq!((header.session_id, header.seq.get()), (id("s"), 3));
    assert_eq!(bytes, [0, 255, 10, 13]);
    assert_eq!(read_frame(&mut reader, 1024).await.unwrap(), None);
}

#[tokio::test]
async fn unknown_kind_and_oversized_payload_are_refused() {
    let mut unknown = [0x03_u8, 0, 0, 0, 0].as_slice();
    assert!(matches!(
        read_frame(&mut unknown, 1024).await,
        Err(WireError::UnknownKind(0x03))
    ));
    let mut big = [0x01_u8, 0, 0, 4, 1].as_slice();
    assert!(matches!(
        read_frame(&mut big, 1024).await,
        Err(WireError::TooLarge {
            len: 1025,
            max: 1024
        })
    ));
    let mut cut = [0x01_u8, 0, 0, 0, 9, b'{'].as_slice();
    assert!(matches!(
        read_frame(&mut cut, 1024).await,
        Err(WireError::Io(_))
    ));
}

#[test]
fn output_frame_whose_sizes_do_not_add_up_is_refused() {
    let good = encode_output(&id("s"), 1, b"abc").unwrap();
    let payload = &good[5..];
    assert!(decode_payload(KIND_OUTPUT, payload.to_vec()).is_ok());

    // One byte more or less than `length` says.
    let mut longer = payload.to_vec();
    longer.push(b'd');
    assert!(matches!(
        decode_payload(KIND_OUTPUT, longer),
        Err(WireError::BadOutputFrame(_))
    ));
    let shorter = payload[..payload.len() - 1].to_vec();
    assert!(decode_payload(KIND_OUTPUT, shorter).is_err());
    // A header length that runs past the payload, and a payload with no room for it.
    let mut overrun = payload.to_vec();
    overrun[..2].copy_from_slice(&u16::MAX.to_be_bytes());
    assert!(decode_payload(KIND_OUTPUT, overrun).is_err());
    assert!(decode_payload(KIND_OUTPUT, vec![0]).is_err());
}

#[test]
fn empty_output_and_seq_zero_cannot_be_framed() {
    assert!(encode_output(&id("s"), 1, b"").is_err());
    assert!(encode_output(&id("s"), 0, b"a").is_err());
}

#[test]
fn base64_matches_the_rfc_vectors_and_refuses_loose_text() {
    let vectors: [(&[u8], &str); 7] = [
        (b"", ""),
        (b"f", "Zg=="),
        (b"fo", "Zm8="),
        (b"foo", "Zm9v"),
        (b"foob", "Zm9vYg=="),
        (b"fooba", "Zm9vYmE="),
        (b"foobar", "Zm9vYmFy"),
    ];
    for (bytes, text) in vectors {
        assert_eq!(base64_encode(bytes), text);
        assert_eq!(base64_decode(text).as_deref(), Some(bytes));
    }
    let all: Vec<u8> = (0..=255).collect();
    assert_eq!(base64_decode(&base64_encode(&all)), Some(all));
    for loose in ["Zg", "Zg=", "Z===", "Zm9v\n", "Zg==Zg==", "Zm 9", "=Zg="] {
        assert_eq!(base64_decode(loose), None, "{loose:?}");
    }
}

#[test]
fn session_id_follows_the_common_id_rule() {
    for good in ["a", "sess-01", "A.b_c:d-9", &"x".repeat(64)] {
        assert!(SessionId::new(good).is_ok(), "{good:?}");
    }
    for bad in ["", "-a", "a b", "a@b", "é", &"x".repeat(65)] {
        assert!(SessionId::new(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn request_parse_tells_the_failure_classes_apart() {
    let code = |payload: &str| Request::parse(payload.as_bytes()).unwrap_err();
    assert_eq!(code("{nope").code, ErrorCode::ParseError);
    assert_eq!(code("{nope").id, None);

    let invalid = code(r#"{"jsonrpc":"1.0","id":4,"method":"session.list","params":{}}"#);
    assert_eq!(
        (invalid.code, invalid.id),
        (ErrorCode::InvalidRequest, RequestId::new(4))
    );

    let unknown = code(r#"{"jsonrpc":"2.0","id":5,"method":"session.kill","params":{}}"#);
    assert_eq!(
        (unknown.code, unknown.id),
        (ErrorCode::MethodNotFound, RequestId::new(5))
    );

    let params = code(r#"{"jsonrpc":"2.0","id":6,"method":"session.close","params":{}}"#);
    assert_eq!(
        (params.code, params.id),
        (ErrorCode::InvalidParams, RequestId::new(6))
    );
}

#[test]
fn built_errors_always_fit_the_contract() {
    let long = ErrorBody::new(ErrorCode::InternalError, &"x".repeat(900), None);
    assert_eq!(long.message.chars().count(), 500);
    let empty = ErrorBody::new(ErrorCode::InternalError, "", None);
    assert!(!empty.message.is_empty());
}

// ---- where the host and the generated types differ: reading the generated type with
// success never stands in for the check the host makes on top of it.

fn output_payload(header: &str, bytes: &[u8]) -> Vec<u8> {
    [
        &(header.len() as u16).to_be_bytes()[..],
        header.as_bytes(),
        bytes,
    ]
    .concat()
}

#[test]
fn header_the_schema_accepts_with_a_length_over_32_bits_is_refused_by_the_codec() {
    for length in [u64::from(u32::MAX) + 1, u64::from(u32::MAX) + 4, u64::MAX] {
        let header = format!(r#"{{"session_id":"s","seq":1,"length":{length}}}"#);
        let generated: ContractPtyHostFrameV1 = serde_json::from_str(&header).unwrap();
        assert_eq!(generated.length.get(), length);

        assert!(OutputHeader::try_from(generated).is_err(), "{length}");
        assert!(serde_json::from_str::<OutputHeader>(&header).is_err());
        assert!(matches!(
            decode_payload(KIND_OUTPUT, output_payload(&header, b"abc")),
            Err(WireError::BadOutputFrame(_))
        ));
    }
    // The largest length the layout can say still converts both ways.
    let header = format!(r#"{{"session_id":"s","seq":1,"length":{}}}"#, u32::MAX);
    let read: OutputHeader = serde_json::from_str(&header).unwrap();
    assert_eq!(read.length.get(), u32::MAX);
    assert_eq!(serde_json::to_string(&read).unwrap(), header);
}

#[test]
fn hello_of_another_version_is_read_by_the_host_and_refused_by_the_generated_type() {
    let hello = |contract: &str| {
        format!(
            r#"{{"jsonrpc":"2.0","id":1,"method":"host.hello","params":{{"contract":"{contract}"}}}}"#
        )
    };
    type Generated = ads_contracts::ContractPtyHostRequestV1;

    assert!(serde_json::from_str::<Generated>(&hello(CONTRACT)).is_ok());
    let Call::Hello(params) = Request::parse(hello(CONTRACT).as_bytes()).unwrap().call else {
        panic!("not a hello");
    };
    assert!(params.is_supported());

    for other in ["pty-host/2", "pty-host/10", "PTY-HOST/1", " pty-host/1", ""] {
        assert!(serde_json::from_str::<Generated>(&hello(other)).is_err());
        // Read, so that the host can answer with its own code; never taken for supported.
        let Call::Hello(params) = Request::parse(hello(other).as_bytes()).unwrap().call else {
            panic!("not a hello");
        };
        assert_eq!(params.contract, other);
        assert!(!params.is_supported(), "{other:?}");
    }
}

#[test]
fn requests_the_host_writes_are_the_ones_the_generated_type_reads() {
    let session_id = id("sess-01");
    let calls = [
        Call::Hello(HelloParams {
            contract: CONTRACT.to_owned(),
        }),
        Call::Open(OpenParams {
            session_id: session_id.clone(),
            program: "sh".to_owned().try_into().unwrap(),
            args: vec!["-c".to_owned().try_into().unwrap()],
            cwd: Some("/work".to_owned().try_into().unwrap()),
            env: [("LANG".to_owned(), "C".to_owned())].into(),
            cols: 120.try_into().unwrap(),
            rows: 30.try_into().unwrap(),
        }),
        Call::Write(WriteParams {
            session_id: session_id.clone(),
            data: Base64(b"dir\r".to_vec()).into(),
        }),
        Call::Resize(ResizeParams {
            session_id: session_id.clone(),
            cols: 1.try_into().unwrap(),
            rows: 1000.try_into().unwrap(),
        }),
        Call::Subscribe(SubscribeParams {
            session_id: session_id.clone(),
            from_seq: None,
        }),
        Call::Close(SessionParams { session_id }),
        Call::List(NoParams {}),
    ];
    for call in calls {
        let request = Request {
            id: RequestId::MIN,
            call,
        };
        let written = serde_json::to_value(&request).unwrap();
        let generated: ads_contracts::ContractPtyHostRequestV1 =
            serde_json::from_value(written.clone()).unwrap();
        assert_eq!(serde_json::to_value(&generated).unwrap(), written);
        let payload = serde_json::to_vec(&generated).unwrap();
        assert_eq!(Request::parse(&payload).unwrap(), request);
    }
}

#[test]
fn response_with_result_and_error_or_with_neither_is_refused_past_the_field_types() {
    let both = r#"{"jsonrpc":"2.0","id":1,"result":{},"error":{"code":-32603,"message":"m"}}"#;
    let neither = r#"{"jsonrpc":"2.0","id":1}"#;
    for payload in [both, neither] {
        // Field by field the message is fine: each field has its type.
        assert!(serde_json::from_str::<ContractPtyHostResponseV1Fields>(payload).is_ok());
        assert!(serde_json::from_str::<Response>(payload).is_err());
        assert!(HostMessage::parse(payload.as_bytes()).is_err());
    }
    let one = r#"{"jsonrpc":"2.0","id":1,"result":{}}"#;
    let Ok(HostMessage::Response(response)) = HostMessage::parse(one.as_bytes()) else {
        panic!("refused");
    };
    assert_eq!(response.outcome, Ok(ResultBody::Empty(Empty {})));
    assert_eq!(serde_json::to_string(&response).unwrap(), one);
}

#[test]
fn error_codes_map_one_to_one_onto_the_numbers_of_the_contract() {
    let table = [
        (ErrorCode::ParseError, -32700),
        (ErrorCode::InvalidRequest, -32600),
        (ErrorCode::MethodNotFound, -32601),
        (ErrorCode::InvalidParams, -32602),
        (ErrorCode::InternalError, -32603),
        (ErrorCode::UnknownSession, -32001),
        (ErrorCode::SessionExists, -32002),
        (ErrorCode::TooManySessions, -32003),
        (ErrorCode::ReplayEvicted, -32004),
        (ErrorCode::ReplayAhead, -32005),
        (ErrorCode::SessionEnded, -32006),
        (ErrorCode::SpawnFailed, -32007),
        (ErrorCode::TerminalIo, -32008),
        (ErrorCode::UnsupportedContract, -32009),
    ];
    for (code, number) in table {
        assert_eq!(code.number(), number);
        let wire = ContractPtyHostResponseV1ErrorCode::try_from(number).unwrap();
        assert_eq!(ContractPtyHostResponseV1ErrorCode::from(code), wire);
        assert_eq!(ErrorCode::from(wire), code);
        // And through a whole message, in and out.
        let payload = format!(
            r#"{{"jsonrpc":"2.0","id":1,"error":{{"code":{number},"message":"m","data":{{"oldest_seq":1,"next_seq":2}}}}}}"#
        );
        let response: Response = serde_json::from_str(&payload).unwrap();
        assert_eq!(response.outcome.as_ref().unwrap_err().code, code);
        assert_eq!(serde_json::to_string(&response).unwrap(), payload);
    }
    let numbers: std::collections::HashSet<i64> = table.iter().map(|(_, n)| *n).collect();
    assert_eq!(numbers.len(), table.len());
}

#[test]
fn error_outside_the_contract_fails_to_encode_instead_of_going_out() {
    let respond = |error: ErrorBody| Response {
        id: None,
        outcome: Err(error),
    };
    let no_data = || ErrorData {
        oldest_seq: None,
        next_seq: None,
        limit: None,
    };
    let bound = NonZeroU64::new(9);
    let broken = [
        ErrorBody::new(ErrorCode::ReplayEvicted, "m", None),
        ErrorBody::new(ErrorCode::ReplayEvicted, "m", Some(no_data())),
        ErrorBody::new(
            ErrorCode::ReplayEvicted,
            "m",
            Some(ErrorData {
                next_seq: bound,
                ..no_data()
            }),
        ),
        ErrorBody::new(ErrorCode::ReplayAhead, "m", None),
        ErrorBody {
            code: ErrorCode::InternalError,
            message: String::new(),
            data: None,
        },
        ErrorBody {
            code: ErrorCode::InternalError,
            message: "x".repeat(501),
            data: None,
        },
    ];
    for error in broken {
        assert!(serde_json::to_string(&error).is_err(), "{error:?}");
        assert!(encode_json(&respond(error)).is_err());
    }
    let fine = [
        ErrorBody::new(
            ErrorCode::ReplayEvicted,
            "m",
            Some(ErrorData {
                oldest_seq: bound,
                ..no_data()
            }),
        ),
        ErrorBody::new(
            ErrorCode::ReplayAhead,
            "m",
            Some(ErrorData {
                next_seq: bound,
                ..no_data()
            }),
        ),
        ErrorBody::new(ErrorCode::InternalError, &"x".repeat(900), None),
        ErrorBody::new(ErrorCode::InternalError, "", None),
    ];
    for error in fine {
        let text = serde_json::to_string(&respond(error.clone())).unwrap();
        let read: Response = serde_json::from_str(&text).unwrap();
        assert_eq!(read.outcome, Err(error));
    }
}

#[test]
fn base64_text_the_schema_accepts_always_becomes_bytes_and_loose_text_never_gets_there() {
    for len in 0..=255_usize {
        let bytes: Vec<u8> = (0..len).map(|n| (n * 7) as u8).collect();
        let text = CommonBase64V1::from(Base64(bytes.clone()));
        assert_eq!(Base64::try_from(&text), Ok(Base64(bytes)));
    }
    // Padding bits that are not zero pass the pattern; they still decode, to the same bytes.
    for (text, bytes) in [("Zh==", b"f".as_slice()), ("Zm9=", b"fo".as_slice())] {
        let text = CommonBase64V1::new(text).unwrap();
        assert_eq!(Base64::try_from(&text), Ok(Base64(bytes.to_vec())));
    }
    for loose in [
        "Zg", "Zg=", "Z===", "Zm9v\n", "Zg==Zg==", "Zm 9", "=Zg=", "Zm9-",
    ] {
        assert!(CommonBase64V1::new(loose).is_err(), "{loose:?}");
        let payload = serde_json::json!({
            "jsonrpc": "2.0", "id": 3, "method": "session.write",
            "params": {"session_id": "a", "data": loose}
        });
        let rejection = Request::parse(payload.to_string().as_bytes()).unwrap_err();
        assert_eq!(rejection.code, ErrorCode::InvalidParams, "{loose:?}");
    }
}

#[test]
fn refused_params_are_not_echoed_in_the_answer() {
    let marker = "s3cr3t-marker";
    let request = |method: &str, params: Value| {
        serde_json::json!({"jsonrpc": "2.0", "id": 3, "method": method, "params": params})
            .to_string()
    };
    let open = |field: &str, value: Value| {
        let mut params = serde_json::json!({
            "session_id": "a", "program": "sh", "args": [], "cwd": null,
            "env": {}, "cols": 80, "rows": 24
        });
        params[field] = value;
        request("session.open", params)
    };
    let refused = [
        request(
            "session.close",
            serde_json::json!({"session_id": format!("a b {marker}")}),
        ),
        request(
            "session.write",
            serde_json::json!({"session_id": "a", "data": format!("{marker}!")}),
        ),
        open("program", format!("{marker}{}", "x".repeat(1100)).into()),
        open("cwd", format!("{marker}{}", "x".repeat(1100)).into()),
        open(
            "args",
            serde_json::json!([format!("{marker}{}", "x".repeat(8200))]),
        ),
    ];
    for payload in refused {
        let rejection = Request::parse(payload.as_bytes()).unwrap_err();
        assert_eq!(rejection.code, ErrorCode::InvalidParams, "{payload}");
        assert!(rejection.message.starts_with("not a valid "), "{payload}");
        assert!(!rejection.message.contains(marker), "{}", rejection.message);
    }
}
