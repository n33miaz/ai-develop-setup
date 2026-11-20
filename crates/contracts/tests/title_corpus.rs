//! The task title alphabet: for every probe in `contracts/corpus/title-chars.json`, the generated type has to accept
//! exactly the listed code points, the same list `contracts/tools/corpus.test.mjs` checks the Schema against.

use std::path::Path;

use ads_contracts::RecordQueueTaskV1Title;
use serde_json::{Value, json};

fn accepted_ranges(template: &str) -> Vec<(u32, u32)> {
    let mut ranges: Vec<(u32, u32)> = Vec::new();
    for code in 0..=0x10FFFF_u32 {
        let Some(c) = char::from_u32(code) else {
            continue;
        };
        let title = template.replace("{c}", c.encode_utf8(&mut [0; 4]));
        if RecordQueueTaskV1Title::new(title).is_err() {
            continue;
        }
        match ranges.last_mut() {
            Some(last) if last.1 + 1 == code => last.1 = code,
            _ => ranges.push((code, code)),
        }
    }
    ranges
}

fn hex(value: &Value) -> u32 {
    u32::from_str_radix(value.as_str().expect("hex text"), 16).expect("hex code point")
}

fn listed(ranges: &[(u32, u32)]) -> Vec<[String; 2]> {
    ranges
        .iter()
        .map(|(a, b)| [format!("{a:04X}"), format!("{b:04X}")])
        .collect()
}

#[test]
fn title_type_accepts_exactly_the_corpus_alphabet() {
    let file =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contracts/corpus/title-chars.json");
    let corpus: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    let probes = corpus["probes"].as_array().unwrap();
    assert!(probes.len() >= 3, "the corpus probes several positions");
    let mut actual = Vec::new();
    let mut differ = Vec::new();
    for probe in probes {
        let template = probe["template"].as_str().unwrap();
        let expected: Vec<(u32, u32)> = probe["accepted"]
            .as_array()
            .unwrap()
            .iter()
            .map(|pair| (hex(&pair[0]), hex(&pair[1])))
            .collect();
        let ranges = accepted_ranges(template);
        if ranges != expected {
            differ.push(template.to_owned());
        }
        actual.push(json!({ "template": template, "accepted": listed(&ranges) }));
    }
    if !differ.is_empty() {
        let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("title-chars.probes.json");
        std::fs::write(&out, serde_json::to_string_pretty(&actual).unwrap()).unwrap();
        panic!(
            "the title type and the corpus disagree for {differ:?}; the type accepts the ranges in {}",
            out.display()
        );
    }
}
