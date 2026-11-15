//! Every Schema against its golden files: each valid one is read into the generated type and
//! written back to the same JSON; each invalid one is refused.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use ads_contracts::generated::{SCHEMAS, reencode};
use serde_json::Value;

/// Invalid golden files the generated types cannot refuse: `(Schema key, file, why)`.
/// Kept empty on purpose; an entry is a debt, and one that no longer applies fails the test.
const EXCEPTIONS: &[(&str, &str, &str)] = &[];

fn contracts_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contracts")
}

fn json_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|err| panic!("{}: {err}", dir.display()))
        .map(|entry| entry.expect("directory entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    files.sort();
    files
}

fn read(path: &Path) -> Value {
    let text = fs::read_to_string(path).unwrap_or_else(|err| panic!("{}: {err}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|err| panic!("{}: {err}", path.display()))
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .expect("file name")
        .to_string_lossy()
        .into_owned()
}

#[test]
fn every_schema_round_trips_its_valid_files_and_refuses_its_invalid_ones() {
    let contracts = contracts_dir();
    let mut problems = Vec::new();
    let mut used_exceptions = BTreeSet::new();
    let (mut schemas, mut valid, mut invalid) = (0, 0, 0);

    for schema_file in json_files(&contracts.join("schemas")) {
        let name = file_name(&schema_file);
        let key = name.strip_suffix(".schema.json").expect("schema file name");
        let id = read(&schema_file)["$id"].as_str().expect("$id").to_owned();
        schemas += 1;
        if !SCHEMAS.contains(&(key, id.as_str())) {
            problems.push(format!("{key}: no generated type is listed for {id}"));
            continue;
        }

        for file in json_files(&contracts.join("golden").join(key).join("valid")) {
            valid += 1;
            let original = read(&file);
            match reencode(&id, original.clone()) {
                None => problems.push(format!("{key}: no generated type reads {id}")),
                Some(Err(err)) => {
                    problems.push(format!("{key}/valid/{}: refused ({err})", file_name(&file)))
                }
                Some(Ok(written)) if written != original => {
                    problems.push(format!(
                        "{key}/valid/{}: written back as {written}",
                        file_name(&file)
                    ));
                }
                Some(Ok(_)) => {}
            }
        }

        for file in json_files(&contracts.join("golden").join(key).join("invalid")) {
            let name = file_name(&file);
            if name == "index.json" {
                continue;
            }
            invalid += 1;
            let accepted = matches!(reencode(&id, read(&file)), Some(Ok(_)));
            let excepted = EXCEPTIONS
                .iter()
                .position(|(k, f, _)| *k == key && *f == name);
            match (accepted, excepted) {
                (true, None) => problems.push(format!("{key}/invalid/{name}: accepted")),
                (false, Some(_)) => problems.push(format!(
                    "{key}/invalid/{name}: is refused, drop the exception"
                )),
                (true, Some(at)) => {
                    used_exceptions.insert(at);
                }
                (false, None) => {}
            }
        }
    }

    for (at, (key, file, _)) in EXCEPTIONS.iter().enumerate() {
        if !used_exceptions.contains(&at) {
            problems.push(format!(
                "{key}/invalid/{file}: exception without a golden file it applies to"
            ));
        }
    }
    if SCHEMAS.len() != schemas {
        problems.push(format!(
            "{} generated types for {schemas} Schemas",
            SCHEMAS.len()
        ));
    }

    println!(
        "{schemas} schemas, {valid} valid files round-tripped, {invalid} invalid files, {} exceptions",
        EXCEPTIONS.len()
    );
    assert!(
        problems.is_empty(),
        "{} problem(s):\n{}",
        problems.len(),
        problems.join("\n")
    );
    assert!(schemas > 0 && valid >= 2 * schemas && invalid >= 3 * schemas);
}
