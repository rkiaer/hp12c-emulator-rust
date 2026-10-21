//! Os programas de `examples/programas` carregam (como no Ctrl+V do app) e dão os
//! resultados documentados no README da pasta.

use hp12c_core::Calculator;
use std::path::PathBuf;

fn load(name: &str) -> Calculator {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/programas").join(name);
    let listing = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut calc = Calculator::new();
    calc.load_program(&listing).unwrap_or_else(|e| panic!("{name}: {e}"));
    calc
}

#[test]
fn todos_os_exemplos_carregam() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/programas");
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "txt") {
            let name = path.file_name().unwrap().to_str().unwrap().to_string();
            assert!(!load(&name).program_instructions().is_empty(), "{name} está vazio");
        }
    }
}

#[test]
fn taxa_equivalente() {
    let cases = [
        ("12 ENTER 12 R/S", "0.9489"),
        ("30 ENTER 12 R/S", "2.2104"),
        ("1 ENTER 12 1/x R/S", "12.6825"),
        ("0.5 ENTER 6 1/x R/S", "3.0378"),
        ("50 ENTER 2 R/S", "22.4745"),
    ];
    for (keys, expected) in cases {
        let mut calc = load("taxa-equivalente.txt");
        assert_eq!(calc.program_instructions().len(), 16);
        calc.tap_all(&format!("f 4 {keys}")).unwrap();
        assert_eq!(calc.display().to_text(), expected, "teclas: {keys}");
    }
}
