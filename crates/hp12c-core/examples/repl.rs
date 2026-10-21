//! Calculadora no terminal, para experimentar o núcleo antes da interface gráfica.
//! Uso: cargo run -p hp12c-core --example repl

use hp12c_core::Calculator;
use std::io::{self, BufRead, Write};

fn main() {
    let mut calc = Calculator::new();
    println!("HP-12C (núcleo M1). Digite teclas separadas por espaço, ex.: 12 ENTER 3 /");
    println!("Teclas: n i PV PMT FV CHS / EEX * Rv x<>y CLx ENTER - f g STO RCL . +  |  'sair' encerra");
    show(&calc);
    for line in io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let line = line.trim();
        if line == "sair" || line == "quit" {
            break;
        }
        if let Err(e) = calc.tap_all(line) {
            println!("  ! {e}");
        }
        show(&calc);
    }
}

fn show(calc: &Calculator) {
    let d = calc.display();
    let a = d.annunciators;
    let flags: Vec<&str> = [
        (a.f, "f"),
        (a.g, "g"),
        (a.begin, "BEGIN"),
        (a.dmy, "D.MY"),
        (a.compound, "C"),
        (a.prgm, "PRGM"),
    ]
    .iter()
    .filter(|(on, _)| *on)
    .map(|(_, name)| *name)
    .collect();
    println!("  [ {:<14} ] {}", d.to_text(), flags.join(" "));
    print!("> ");
    io::stdout().flush().ok();
}
