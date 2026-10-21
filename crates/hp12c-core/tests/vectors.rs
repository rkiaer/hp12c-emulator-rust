//! Roda todos os vetores de teste de `tests/vectors/*.json` (raiz do repositório).

use hp12c_core::{Calculator, HpDecimal};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

#[derive(Deserialize)]
struct Suite {
    suite: String,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    keys: String,
    display: String,
    #[serde(default)]
    x: Option<String>,
}

#[test]
fn vetores_de_teste() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/vectors");
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .expect("pasta tests/vectors")
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "nenhum vetor encontrado em {}", dir.display());

    let mut total = 0;
    let mut failures = Vec::new();
    for path in files {
        let text = fs::read_to_string(&path).unwrap();
        let suite: Suite =
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        for case in &suite.cases {
            total += 1;
            let label = format!("[{}] {} (`{}`)", suite.suite, case.name, case.keys);
            let mut calc = Calculator::new();
            if let Err(e) = calc.tap_all(&case.keys) {
                failures.push(format!("{label}: {e}"));
                continue;
            }
            let shown = calc.display().to_text();
            if shown != case.display {
                failures.push(format!("{label}: display `{shown}`, esperado `{}`", case.display));
            }
            if let Some(x) = &case.x {
                let expected = HpDecimal::parse(x).unwrap_or_else(|| panic!("{label}: x inválido"));
                if calc.x() != expected {
                    failures.push(format!("{label}: X = {}, esperado {expected}", calc.x()));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} de {total} casos falharam:\n{}",
        failures.len(),
        failures.join("\n")
    );
    println!("{total} vetores aprovados");
}
