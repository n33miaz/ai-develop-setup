//! What the generated types lean on: the error, the text patterns and the rules between fields.
//! Written by hand; `generated/` is the only caller besides the tests.

use std::collections::HashSet;
use std::fmt;
use std::hash::Hash;
use std::sync::OnceLock;

use regex_lite::Regex;
use serde::de::{DeserializeOwned, Error as _};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};

/// A value that does not fit the contract type it was meant for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractError {
    /// The generated type that refused the value.
    pub what: &'static str,
    /// The refused value, or the rule of the Schema that does not hold.
    pub detail: String,
}

impl ContractError {
    /// How much of a refused value the error keeps: enough to find it, not the whole payload.
    pub const DETAIL_MAX: usize = 120;

    pub fn new(what: &'static str, detail: impl Into<String>) -> Self {
        let mut detail = detail.into();
        if let Some((at, _)) = detail.char_indices().nth(Self::DETAIL_MAX) {
            detail.truncate(at);
            detail.push('…');
        }
        Self { what, detail }
    }
}

impl fmt::Display for ContractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "not a valid {}: {}", self.what, self.detail)
    }
}

impl std::error::Error for ContractError {}

/// One `pattern` of a Schema, compiled on first use.
#[derive(Debug)]
pub struct Pattern {
    source: &'static str,
    regex: OnceLock<Regex>,
}

impl Pattern {
    pub const fn new(source: &'static str) -> Self {
        Self {
            source,
            regex: OnceLock::new(),
        }
    }

    /// Whether the pattern is found in `text` (a JSON Schema pattern is not anchored).
    pub fn matches(&self, text: &str) -> bool {
        self.regex
            .get_or_init(|| {
                compile(self.source).expect("the generator emits patterns that compile")
            })
            .is_match(text)
    }
}

pub(crate) fn compile(source: &str) -> Result<Regex, regex_lite::Error> {
    Regex::new(source)
}

/// A required field that may be `null`: a plain `Option` would also accept a missing field.
pub fn nullable<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Option::<T>::deserialize(deserializer)
}

/// An optional field: when the key is there, its value has to be a `T` (`null` only if `T` takes it).
pub fn present<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    T::deserialize(deserializer).map(Some)
}

/// Reads a `const` text.
pub fn expect_str<'de, D: Deserializer<'de>>(
    deserializer: D,
    expected: &str,
) -> Result<(), D::Error> {
    let found = String::deserialize(deserializer)?;
    if found == expected {
        Ok(())
    } else {
        Err(D::Error::custom(format!(
            "expected {expected:?}, found {found:?}"
        )))
    }
}

/// Reads a `const` boolean.
pub fn expect_bool<'de, D: Deserializer<'de>>(
    deserializer: D,
    expected: bool,
) -> Result<(), D::Error> {
    let found = bool::deserialize(deserializer)?;
    if found == expected {
        Ok(())
    } else {
        Err(D::Error::custom(format!(
            "expected {expected}, found {found}"
        )))
    }
}

/// Reads a `const` integer.
pub fn expect_i64<'de, D: Deserializer<'de>>(
    deserializer: D,
    expected: i64,
) -> Result<(), D::Error> {
    let found = i64::deserialize(deserializer)?;
    if found == expected {
        Ok(())
    } else {
        Err(D::Error::custom(format!(
            "expected {expected}, found {found}"
        )))
    }
}

/// `format: date-time` for a text the pattern already shaped as `YYYY-MM-DDTHH:MM:SS[.f]Z`:
/// the date exists and the time is a real one (a leap second only as the last of a day).
pub fn is_date_time(text: &str) -> bool {
    let bytes = text.as_bytes();
    let shape = b"dddd-dd-ddTdd:dd:dd";
    if bytes.len() < shape.len() {
        return false;
    }
    let shaped = bytes.iter().zip(shape).all(|(byte, slot)| match slot {
        b'd' => byte.is_ascii_digit(),
        b'T' => byte.eq_ignore_ascii_case(slot),
        other => byte == other,
    });
    if !shaped {
        return false;
    }
    let number = |from: usize, to: usize| text[from..to].parse::<u32>().unwrap_or(u32::MAX);
    let (year, month, day) = (number(0, 4), number(5, 7), number(8, 10));
    let (hour, minute, second) = (number(11, 13), number(14, 16), number(17, 19));
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days).contains(&day)
        && ((hour < 24 && minute < 60 && second < 60)
            || (hour == 23 && minute == 59 && second == 60))
}

/// `uniqueItems`.
pub fn all_distinct<T: Eq + Hash>(items: &[T]) -> bool {
    let mut seen = HashSet::with_capacity(items.len());
    items.iter().all(|item| seen.insert(item))
}

/// `uniqueItems` for items that cannot be hashed.
pub fn all_distinct_slow<T: PartialEq>(items: &[T]) -> bool {
    (1..items.len()).all(|at| !items[..at].contains(&items[at]))
}

/// A `const` or one value of an `enum` inside a rule.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Literal {
    Null,
    Bool(bool),
    Int(i64),
    Str(&'static str),
}

impl Literal {
    fn is(self, value: &Value) -> bool {
        match (self, value) {
            (Self::Null, Value::Null) => true,
            (Self::Bool(expected), Value::Bool(found)) => expected == *found,
            // Numbers compare by value: `1.0` is `1`.
            (Self::Int(expected), Value::Number(found)) => {
                found.as_i64().is_some_and(|found| found == expected)
                    || (found.is_f64() && found.as_f64() == Some(expected as f64))
            }
            (Self::Str(expected), Value::String(found)) => expected == found,
            _ => false,
        }
    }
}

/// The `type` names of JSON Schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonType {
    Null,
    Boolean,
    Integer,
    Number,
    String,
    Array,
    Object,
}

impl JsonType {
    fn is(self, value: &Value) -> bool {
        match (self, value) {
            (Self::Null, Value::Null)
            | (Self::Boolean, Value::Bool(_))
            | (Self::Number, Value::Number(_))
            | (Self::String, Value::String(_))
            | (Self::Array, Value::Array(_))
            | (Self::Object, Value::Object(_)) => true,
            (Self::Integer, Value::Number(number)) => {
                !number.is_f64() || number.as_f64().is_some_and(|float| float.fract() == 0.0)
            }
            _ => false,
        }
    }
}

/// A rule between the fields of one object, as the Schema writes it (`if`/`then`/`else`,
/// `oneOf`, `not`…). Each keyword keeps the meaning it has in JSON Schema, including holding
/// for a value of another type: `properties` says nothing about a text.
#[derive(Debug)]
pub enum Rule {
    Type(&'static [JsonType]),
    Const(Literal),
    Enum(&'static [Literal]),
    Required(&'static [&'static str]),
    Properties(&'static [(&'static str, Rule)]),
    Pattern(&'static Pattern),
    MinLength(usize),
    MaxLength(usize),
    Minimum(f64),
    Maximum(f64),
    MinItems(usize),
    MaxItems(usize),
    Items(&'static Rule),
    Not(&'static Rule),
    AllOf(&'static [Rule]),
    AnyOf(&'static [Rule]),
    OneOf(&'static [Rule]),
    If {
        when: &'static Rule,
        then: Option<&'static Rule>,
        otherwise: Option<&'static Rule>,
    },
}

impl Rule {
    pub fn holds(&self, value: &Value) -> bool {
        match self {
            Self::Type(types) => types.iter().any(|kind| kind.is(value)),
            Self::Const(literal) => literal.is(value),
            Self::Enum(literals) => literals.iter().any(|literal| literal.is(value)),
            Self::Required(names) => value
                .as_object()
                .is_none_or(|object| names.iter().all(|name| object.contains_key(*name))),
            Self::Properties(rules) => value.as_object().is_none_or(|object| {
                rules
                    .iter()
                    .all(|(name, rule)| object.get(*name).is_none_or(|field| rule.holds(field)))
            }),
            Self::Pattern(pattern) => value.as_str().is_none_or(|text| pattern.matches(text)),
            Self::MinLength(min) => value
                .as_str()
                .is_none_or(|text| text.chars().count() >= *min),
            Self::MaxLength(max) => value
                .as_str()
                .is_none_or(|text| text.chars().count() <= *max),
            Self::Minimum(min) => value.as_f64().is_none_or(|number| number >= *min),
            Self::Maximum(max) => value.as_f64().is_none_or(|number| number <= *max),
            Self::MinItems(min) => value.as_array().is_none_or(|items| items.len() >= *min),
            Self::MaxItems(max) => value.as_array().is_none_or(|items| items.len() <= *max),
            Self::Items(rule) => value
                .as_array()
                .is_none_or(|items| items.iter().all(|item| rule.holds(item))),
            Self::Not(rule) => !rule.holds(value),
            Self::AllOf(rules) => rules.iter().all(|rule| rule.holds(value)),
            Self::AnyOf(rules) => rules.iter().any(|rule| rule.holds(value)),
            Self::OneOf(rules) => rules.iter().filter(|rule| rule.holds(value)).count() == 1,
            Self::If {
                when,
                then,
                otherwise,
            } => {
                let branch = if when.holds(value) { then } else { otherwise };
                branch.is_none_or(|rule| rule.holds(value))
            }
        }
    }
}

/// Checks the rules between fields of a generated type against the JSON its fields write.
/// Each rule comes with the place of the Schema it was read from, which names it in the error.
pub fn check_rules<T: Serialize>(
    what: &'static str,
    fields: &T,
    rules: &[(&'static str, Rule)],
) -> Result<(), ContractError> {
    let value =
        serde_json::to_value(fields).map_err(|err| ContractError::new(what, err.to_string()))?;
    match rules.iter().find(|(_, rule)| !rule.holds(&value)) {
        None => Ok(()),
        Some((place, _)) => Err(ContractError::new(
            what,
            format!("rule {place} does not hold"),
        )),
    }
}

/// An open object keeps what the Schema does not name in a map; a key the Schema names has
/// its own field and cannot come in the map as well.
pub fn check_extra(
    what: &'static str,
    extra: &Map<String, Value>,
    named: &[&str],
) -> Result<(), ContractError> {
    match named.iter().find(|name| extra.contains_key(**name)) {
        None => Ok(()),
        Some(name) => Err(ContractError::new(
            what,
            format!("{name:?} has its own field"),
        )),
    }
}

/// Reads `value` as a `T` and writes it back.
pub fn reencode<T: Serialize + DeserializeOwned>(value: Value) -> Result<Value, serde_json::Error> {
    serde_json::to_value(serde_json::from_value::<T>(value)?)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    static WORD: Pattern = Pattern::new("^[a-z]+$");

    #[test]
    fn a_pattern_is_searched_not_anchored() {
        static DOLLAR: Pattern = Pattern::new("[$%]");
        assert!(DOLLAR.matches("a$b"));
        assert!(!DOLLAR.matches("ab"));
        assert!(WORD.matches("abc"));
        assert!(!WORD.matches("abc\n"));
    }

    #[test]
    fn date_time_refuses_dates_and_times_that_do_not_exist() {
        for good in [
            "2026-01-31T00:00:00Z",
            "2024-02-29T23:59:59.123456Z",
            "2026-12-31T23:59:60Z",
        ] {
            assert!(is_date_time(good), "{good}");
        }
        for bad in [
            "2026-02-29T00:00:00Z",
            "2026-13-01T00:00:00Z",
            "2026-04-31T00:00:00Z",
            "2026-01-00T00:00:00Z",
            "2026-01-01T24:00:00Z",
            "2026-01-01T00:60:00Z",
            "2026-01-01T00:00:60Z",
            "2026-01-01",
            "",
        ] {
            assert!(!is_date_time(bad), "{bad}");
        }
    }

    #[test]
    fn the_error_keeps_only_the_start_of_a_long_value() {
        let short = ContractError::new("X", "çã");
        assert_eq!(short.detail, "çã");
        let long = ContractError::new("X", "ç".repeat(500));
        assert_eq!(long.detail.chars().count(), ContractError::DETAIL_MAX + 1);
        assert!(long.detail.ends_with('…'));
    }

    #[test]
    fn distinct_items() {
        assert!(all_distinct::<u8>(&[]));
        assert!(all_distinct(&[1, 2, 3]));
        assert!(!all_distinct(&[1, 2, 1]));
        assert!(all_distinct_slow(&[1.0, 2.0]));
        assert!(!all_distinct_slow(&[1.0, 1.0]));
    }

    #[test]
    fn a_keyword_says_nothing_about_a_value_of_another_type() {
        let text = json!("text");
        let number = json!(7);
        let object = json!({});
        assert!(Rule::Required(&["a"]).holds(&text));
        assert!(Rule::Properties(&[("a", Rule::Const(Literal::Null))]).holds(&number));
        assert!(Rule::Pattern(&WORD).holds(&number));
        assert!(Rule::MinLength(9).holds(&object));
        assert!(Rule::MaxLength(0).holds(&number));
        assert!(Rule::Minimum(9.0).holds(&text));
        assert!(Rule::Maximum(0.0).holds(&text));
        assert!(Rule::MinItems(1).holds(&object));
        assert!(Rule::MaxItems(0).holds(&text));
        assert!(Rule::Items(&Rule::Const(Literal::Null)).holds(&object));
    }

    #[test]
    fn each_keyword_judges_its_own_type() {
        assert!(Rule::Required(&["a"]).holds(&json!({"a": null})));
        assert!(!Rule::Required(&["a", "b"]).holds(&json!({"a": 1})));
        let a_is_one = Rule::Properties(&[("a", Rule::Const(Literal::Int(1)))]);
        assert!(a_is_one.holds(&json!({"a": 1})));
        assert!(a_is_one.holds(&json!({"b": 2})));
        assert!(!a_is_one.holds(&json!({"a": 2})));
        assert!(Rule::Pattern(&WORD).holds(&json!("abc")));
        assert!(!Rule::Pattern(&WORD).holds(&json!("ABC")));
        // Length counts characters, not bytes.
        assert!(Rule::MaxLength(2).holds(&json!("çã")));
        assert!(!Rule::MinLength(3).holds(&json!("çã")));
        assert!(Rule::Minimum(0.0).holds(&json!(0)));
        assert!(!Rule::Minimum(0.0).holds(&json!(-0.5)));
        assert!(Rule::Maximum(1.0).holds(&json!(1)));
        assert!(!Rule::Maximum(1.0).holds(&json!(1.5)));
        assert!(Rule::MinItems(1).holds(&json!([0])));
        assert!(!Rule::MinItems(1).holds(&json!([])));
        assert!(Rule::MaxItems(0).holds(&json!([])));
        assert!(!Rule::MaxItems(0).holds(&json!([0])));
        let all_null = Rule::Items(&Rule::Const(Literal::Null));
        assert!(all_null.holds(&json!([null, null])));
        assert!(!all_null.holds(&json!([null, 1])));
    }

    #[test]
    fn types_and_literals_compare_numbers_by_value() {
        assert!(Rule::Type(&[JsonType::Integer]).holds(&json!(1)));
        assert!(Rule::Type(&[JsonType::Integer]).holds(&json!(1.0)));
        assert!(!Rule::Type(&[JsonType::Integer]).holds(&json!(1.5)));
        assert!(Rule::Type(&[JsonType::Number]).holds(&json!(1)));
        assert!(!Rule::Type(&[JsonType::Number]).holds(&json!("1")));
        assert!(Rule::Type(&[JsonType::Null, JsonType::String]).holds(&json!(null)));
        assert!(!Rule::Type(&[JsonType::Object]).holds(&json!([])));
        assert!(Rule::Const(Literal::Int(-32004)).holds(&json!(-32004)));
        assert!(Rule::Const(Literal::Int(1)).holds(&json!(1.0)));
        assert!(!Rule::Const(Literal::Int(1)).holds(&json!("1")));
        assert!(!Rule::Const(Literal::Int(1)).holds(&json!(true)));
        assert!(Rule::Const(Literal::Bool(true)).holds(&json!(true)));
        assert!(Rule::Enum(&[Literal::Str("a"), Literal::Str("b")]).holds(&json!("b")));
        assert!(!Rule::Enum(&[Literal::Str("a"), Literal::Str("b")]).holds(&json!("c")));
    }

    #[test]
    fn combinators() {
        static IS_TEXT: Rule = Rule::Type(&[JsonType::String]);
        static IS_SHORT: Rule = Rule::MaxLength(2);
        assert!(Rule::Not(&IS_TEXT).holds(&json!(1)));
        assert!(!Rule::Not(&IS_TEXT).holds(&json!("a")));
        assert!(Rule::AllOf(&[]).holds(&json!(1)));
        assert!(!Rule::AnyOf(&[]).holds(&json!(1)));
        let both = Rule::OneOf(&[Rule::Required(&["result"]), Rule::Required(&["error"])]);
        assert!(both.holds(&json!({"result": {}})));
        assert!(both.holds(&json!({"error": {}})));
        assert!(!both.holds(&json!({})));
        assert!(!both.holds(&json!({"result": {}, "error": {}})));
        let when_text_then_short = Rule::If {
            when: &IS_TEXT,
            then: Some(&IS_SHORT),
            otherwise: Some(&Rule::Const(Literal::Null)),
        };
        assert!(when_text_then_short.holds(&json!("ab")));
        assert!(!when_text_then_short.holds(&json!("abc")));
        assert!(when_text_then_short.holds(&json!(null)));
        assert!(!when_text_then_short.holds(&json!(1)));
        let only_then = Rule::If {
            when: &IS_TEXT,
            then: Some(&IS_SHORT),
            otherwise: None,
        };
        assert!(only_then.holds(&json!(1)));
    }

    #[test]
    fn every_generated_pattern_compiles() {
        for source in crate::generated::PATTERNS {
            assert!(compile(source).is_ok(), "{source}");
        }
        assert!(!crate::generated::PATTERNS.is_empty());
    }

    #[test]
    fn rules_are_checked_against_what_the_fields_write() {
        #[derive(Serialize)]
        struct Fields {
            a: Option<u8>,
        }
        static RULES: &[(&str, Rule)] = &[(
            "#/required",
            Rule::Properties(&[("a", Rule::Type(&[JsonType::Integer]))]),
        )];
        assert_eq!(check_rules("Fields", &Fields { a: Some(1) }, RULES), Ok(()));
        let refused = check_rules("Fields", &Fields { a: None }, RULES).unwrap_err();
        assert_eq!(
            refused.to_string(),
            "not a valid Fields: rule #/required does not hold"
        );
    }

    #[test]
    fn a_named_key_cannot_hide_in_the_extra_map() {
        let mut extra = Map::new();
        extra.insert("other".to_owned(), json!(1));
        assert_eq!(check_extra("Open", &extra, &["_meta"]), Ok(()));
        extra.insert("_meta".to_owned(), json!(1));
        assert!(check_extra("Open", &extra, &["_meta"]).is_err());
    }
}
