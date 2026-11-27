use super::*;

// The doc tests above only fail to compile; this stops compiling, with any toolchain, the
// day an action or an execution permit can be read from a document.
trait AmbiguousIfDeserialize<Marker> {
    fn check() {}
}
impl<T: ?Sized> AmbiguousIfDeserialize<()> for T {}
struct Deserializable;
impl<T: DeserializeOwned> AmbiguousIfDeserialize<Deserializable> for T {}
const _: fn() = || {
    <crate::gate::Action as AmbiguousIfDeserialize<_>>::check();
    <crate::gate::ExecutionPermit as AmbiguousIfDeserialize<_>>::check();
};

#[test]
fn text_types_follow_the_contract_patterns() {
    assert!(Id::new("act-01").is_ok());
    assert!(Id::new("a".repeat(64)).is_ok());
    for bad in ["", "-a", "a b", "a@b", &"a".repeat(65)] {
        assert!(Id::new(bad).is_err(), "{bad}");
    }
    assert!(Ulid::new("01ARZ3NDEKTSV4RRFFQ69G5FAB").is_ok());
    for bad in [
        "81ARZ3NDEKTSV4RRFFQ69G5FAB",
        "01ARZ3NDEKTSV4RRFFQ69G5FAI",
        "01ARZ3",
    ] {
        assert!(Ulid::new(bad).is_err(), "{bad}");
    }
    assert!(PlanHash::new(format!("sha256:{}", "ab".repeat(32))).is_ok());
    for bad in [
        format!("sha256:{}", "AB".repeat(32)),
        format!("sha256:{}", "ab".repeat(31)),
        "ab".repeat(32),
    ] {
        assert!(PlanHash::new(bad.clone()).is_err(), "{bad}");
    }
    assert!(Verb::new("fs.delete_recursive").is_ok());
    for bad in ["", "Fs.delete", "fs..delete", "fs.", "apagar tudo", "fs.1x"] {
        assert!(Verb::new(bad).is_err(), "{bad}");
    }
    assert!(ErrorCode::new("E_PLAN_CHANGED").is_ok());
    for bad in ["E_", "PLAN_CHANGED", "E_plan", "plan changed"] {
        assert!(ErrorCode::new(bad).is_err(), "{bad}");
    }
    assert!(ShortText::new("á".repeat(500)).is_ok());
    assert!(ShortText::new("a".repeat(501)).is_err());
    assert!(ShortText::new("").is_err());
}

#[test]
fn sha256_matches_a_known_vector() {
    assert_eq!(
        PlanHash::of("abc").as_str(),
        "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn sha256_text_keeps_leading_zeros_in_lower_case() {
    assert_eq!(
        super::sha256_text(b"ads-36"),
        "sha256:0bbb0f7bfade40c1b69e98b0c534101d8b237370dad6404dc139d3c2a977b5ae"
    );
}

#[test]
fn rule_codes_paths_and_patterns_follow_the_contract_patterns() {
    for good in ["repository_root", "protected.user_home", "a1_b.c2"] {
        assert!(RuleCode::new(good).is_ok(), "{good}");
    }
    let long = "a".repeat(65);
    for bad in ["", "Protected", "a.b.c", "a.", ".a", "1a", "a b", &long] {
        assert!(RuleCode::new(bad).is_err(), "{bad}");
    }
    for good in ["C:\\", "/", "C:\\work\\My Files\\a.b", "/work/proj"] {
        assert!(PathText::new(good).is_ok(), "{good}");
    }
    let long = format!("/{}", "a".repeat(4096));
    for bad in [
        "c:\\work",
        "C:/work",
        "C:\\work\\",
        "/work/",
        "/work//proj",
        "/work/./proj",
        "/work/../proj",
        "/work/*",
        "work",
        "\\\\server\\share",
        "C:\\PROGRA~1",
        long.as_str(),
    ] {
        assert!(PathText::new(bad).is_err(), "{bad}");
    }
    assert!(PathText::new(&long[..4096]).is_ok());
    for good in ["*", "*.tmp", "a?c"] {
        assert!(PatternText::new(good).is_ok(), "{good}");
    }
    let long = "*".repeat(256);
    for bad in ["", "name", "a/*", "a\\*", "$*", "%*", &long] {
        assert!(PatternText::new(bad).is_err(), "{bad}");
    }
}

#[test]
fn rule_list_has_one_to_a_hundred_distinct_codes() {
    let code = |text: &str| RuleCode::new(text).unwrap();
    assert!(RuleList::new(vec![code("a"), code("b")]).is_ok());
    assert!(RuleList::new(vec![]).is_err());
    assert!(RuleList::new(vec![code("a"), code("b"), code("a")]).is_err());
    let many = |count: usize| (0..count).map(|n| code(&format!("r{n}"))).collect();
    assert!(RuleList::new(many(100)).is_ok());
    assert!(RuleList::new(many(101)).is_err());
}

#[test]
fn timestamps_are_validated_and_compared_as_instants() {
    let at = |text: &str| Timestamp::new(text).unwrap();
    for bad in [
        "2030-02-31T03:04:05Z",
        "2031-02-29T03:04:05Z",
        "2030-01-02T03:04:60Z",
        "2030-01-02T03:04:05",
        "2030-01-02 03:04:05Z",
        "2030-13-02T03:04:05Z",
        "2030-01-02T24:04:05Z",
        "2030-01-02T03:04:05.Z",
        "2030-01-02T03:04:05.1234567Z",
        "2030-01-02T03:04:05+00:00",
    ] {
        assert!(Timestamp::new(bad).is_err(), "{bad}");
    }
    // As plain text "05Z" sorts after "05.5Z"; as instants it comes first.
    assert!(at("2030-01-02T03:04:05Z").is_before(&at("2030-01-02T03:04:05.5Z")));
    assert!(!at("2030-01-02T03:04:05.5Z").is_before(&at("2030-01-02T03:04:05Z")));
    assert!(at("2030-01-02T03:04:05.05Z").is_before(&at("2030-01-02T03:04:05.5Z")));
    assert!(!at("2030-01-02T03:04:05Z").is_before(&at("2030-01-02T03:04:05.000Z")));
    assert!(at("2030-01-02T03:04:05.999999Z").is_before(&at("2030-01-02T03:04:06Z")));
}
