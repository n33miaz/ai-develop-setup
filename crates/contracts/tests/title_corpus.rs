//! The task title alphabet: the generated type has to accept exactly the code points that
//! `contracts/corpus/title-chars.json` lists, the same list `contracts/tools/corpus.test.mjs` checks the Schema against.

use std::path::Path;

use ads_contracts::RecordQueueTaskV1Title;
use serde_json::Value;

fn accepted_ranges(probe: &str) -> Vec<(u32, u32)> {
    let mut ranges: Vec<(u32, u32)> = Vec::new();
    for code in 0..=0x10FFFF_u32 {
        let Some(c) = char::from_u32(code) else {
            continue;
        };
        if RecordQueueTaskV1Title::new(format!("{probe}{c}")).is_err() {
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

#[test]
fn title_type_accepts_exactly_the_corpus_alphabet() {
    let file =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contracts/corpus/title-chars.json");
    let corpus: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    let expected: Vec<(u32, u32)> = corpus["accepted"]
        .as_array()
        .unwrap()
        .iter()
        .map(|pair| (hex(&pair[0]), hex(&pair[1])))
        .collect();
    let actual = accepted_ranges(corpus["probe"].as_str().unwrap());
    if actual != expected {
        let listed: Vec<[String; 2]> = actual
            .iter()
            .map(|(a, b)| [format!("{a:04X}"), format!("{b:04X}")])
            .collect();
        let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("title-chars.accepted.json");
        std::fs::write(&out, serde_json::to_string_pretty(&listed).unwrap()).unwrap();
        panic!(
            "the title type and the corpus disagree; the type accepts the ranges in {}",
            out.display()
        );
    }
}
