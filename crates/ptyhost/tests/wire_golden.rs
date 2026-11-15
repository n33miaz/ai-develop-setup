//! The hand-written wire types against the golden files of the `pty-host/1` contract.
//! A valid golden file that stops matching the types, or an invalid one they start to accept,
//! fails here.

use std::path::{Path, PathBuf};

use ads_ptyhost::wire::{Notification, OutputHeader, Request, Response};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

/// Invalid golden files the types are known not to refuse, as `schema/file`. Empty on purpose:
/// every rule the Schemas state today is also stated by the types.
const NOT_COVERED_BY_TYPES: &[&str] = &[];

fn golden_dir(schema: &str, kind: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../contracts/golden")
        .join(format!("contract.pty-host.{schema}.v1"))
        .join(kind)
}

fn golden_files(schema: &str, kind: &str) -> Vec<(String, Value)> {
    let dir = golden_dir(schema, kind);
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|err| panic!("{}: {err}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        // The index describes why each invalid file is invalid; it is not an example.
        .filter(|path| path.file_name().is_some_and(|name| name != "index.json"))
        .collect();
    files.sort();
    files
        .into_iter()
        .map(|path| {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let text = std::fs::read_to_string(&path).unwrap();
            (name, serde_json::from_str(&text).unwrap())
        })
        .collect()
}

fn check<T: DeserializeOwned + Serialize>(schema: &str, at_least_valid: usize) {
    let valid = golden_files(schema, "valid");
    assert!(
        valid.len() >= at_least_valid,
        "{schema}: expected at least {at_least_valid} valid golden files, found {}",
        valid.len()
    );
    for (name, golden) in valid {
        let typed: T = serde_json::from_value(golden.clone())
            .unwrap_or_else(|err| panic!("{schema}/valid/{name} no longer matches: {err}"));
        // Compared as JSON values: key order is not part of the contract.
        assert_eq!(
            serde_json::to_value(&typed).unwrap(),
            golden,
            "{schema}/valid/{name} changed on the way back"
        );
    }

    let invalid = golden_files(schema, "invalid");
    assert!(!invalid.is_empty(), "{schema}: no invalid golden files");
    for (name, golden) in invalid {
        let accepted = serde_json::from_value::<T>(golden).is_ok();
        let known_gap = NOT_COVERED_BY_TYPES.contains(&format!("{schema}/{name}").as_str());
        assert_eq!(
            accepted, known_gap,
            "{schema}/invalid/{name}: accepted = {accepted}, listed as a known gap = {known_gap}"
        );
    }
}

#[test]
fn requests_match_the_golden_files() {
    check::<Request>("request", 6);
}

#[test]
fn responses_match_the_golden_files() {
    check::<Response>("response", 6);
}

#[test]
fn notifications_match_the_golden_files() {
    check::<Notification>("notification", 3);
}

#[test]
fn output_headers_match_the_golden_files() {
    check::<OutputHeader>("frame", 2);
}

#[test]
fn request_parse_accepts_every_valid_request_and_refuses_every_invalid_one() {
    for (name, golden) in golden_files("request", "valid") {
        let payload = serde_json::to_vec(&golden).unwrap();
        assert!(Request::parse(&payload).is_ok(), "valid/{name}");
    }
    for (name, golden) in golden_files("request", "invalid") {
        let payload = serde_json::to_vec(&golden).unwrap();
        assert!(Request::parse(&payload).is_err(), "invalid/{name}");
    }
}
