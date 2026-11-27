//! Shared by the test device registries of `ads-core`.

use ads_contracts::CommonTimestampV1;

/// Whether `at` is strictly earlier than `limit`: at the limit itself the change already holds.
pub fn is_before(at: &CommonTimestampV1, limit: &str) -> bool {
    let limit = CommonTimestampV1::new(limit).unwrap();
    key(at) < key(&limit)
}

/// The fraction is optional and of variable width: as text, `...:00.5Z` sorts before `...:00Z`.
fn key(ts: &CommonTimestampV1) -> (&str, String) {
    let text = ts.as_str().trim_end_matches('Z');
    let (seconds, fraction) = text.split_once('.').unwrap_or((text, ""));
    (seconds, format!("{fraction:0<6}"))
}
