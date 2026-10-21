//! Janela "Registradores": pilha, registros financeiros, memória, fluxos de caixa
//! e programa, atualizados a cada tecla. Só lê o estado; não altera a calculadora.

use crate::input::format_for_copy;
use eframe::egui::{self, Color32, RichText, Ui};
use hp12c_core::{Calculator, FinReg, HpDecimal};

/// Somatórios estatísticos guardados em R1–R6.
const STAT_NAMES: [&str; 6] = ["n", "Σx", "Σx²", "Σy", "Σy²", "Σxy"];
const GOLD: Color32 = Color32::from_rgb(226, 169, 59);
const BLUE: Color32 = Color32::from_rgb(107, 182, 234);

fn value(calc: &Calculator, v: HpDecimal) -> RichText {
    RichText::new(format_for_copy(v, calc.comma_decimal())).monospace()
}

fn section(ui: &mut Ui, title: &str) {
    ui.add_space(12.0);
    ui.label(RichText::new(title).strong().size(15.0));
    ui.separator();
}

pub fn show(ui: &mut Ui, calc: &Calculator) {
    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        stack(ui, calc);
        financial(ui, calc);
        memory(ui, calc);
        cash_flows(ui, calc);
        program(ui, calc);
        ui.add_space(8.0);
    });
}

fn stack(ui: &mut Ui, calc: &Calculator) {
    section(ui, "Pilha");
    let s = calc.stack();
    egui::Grid::new("pilha").num_columns(2).striped(true).spacing([18.0, 4.0]).show(ui, |ui| {
        for (name, v) in [("T", s.t), ("Z", s.z), ("Y", s.y)] {
            ui.monospace(name);
            ui.label(value(calc, v));
            ui.end_row();
        }
        ui.label(RichText::new("X").monospace().color(GOLD).strong());
        ui.label(value(calc, s.x).color(GOLD).strong());
        ui.end_row();
        ui.label(RichText::new("LSTx").monospace().weak());
        ui.label(value(calc, s.last_x).weak());
        ui.end_row();
    });
}

fn financial(ui: &mut Ui, calc: &Calculator) {
    section(ui, "Financeiro");
    egui::Grid::new("financeiro").num_columns(2).striped(true).spacing([18.0, 4.0]).show(ui, |ui| {
        for (name, reg) in [("n", FinReg::N), ("i %", FinReg::I), ("PV", FinReg::Pv), ("PMT", FinReg::Pmt), ("FV", FinReg::Fv)] {
            ui.monospace(name);
            ui.label(value(calc, calc.financial(reg)));
            ui.end_row();
        }
    });
    ui.add_space(4.0);
    let modes = [
        if calc.begin_mode() { "Pagamentos no início (BEGIN)" } else { "Pagamentos no fim (END)" },
        if calc.dmy_mode() { "Datas dia.mês (D.MY)" } else { "Datas mês.dia (M.DY)" },
        if calc.compound() { "Período fracionário composto (C)" } else { "Período fracionário simples" },
    ];
    for mode in modes {
        ui.label(RichText::new(mode).small().weak());
    }
}

fn memory(ui: &mut Ui, calc: &Calculator) {
    section(ui, "Registradores");
    let available = calc.registers_available();
    ui.columns(2, |cols| {
        for (col, base) in cols.iter_mut().zip([0usize, 10]) {
            egui::Grid::new(("memoria", base)).num_columns(3).striped(true).spacing([10.0, 4.0]).show(col, |ui| {
                for d in 0..10 {
                    let index = base + d;
                    let name = if base == 0 { format!("R{d}") } else { format!("R.{d}") };
                    ui.monospace(name);
                    if index >= available {
                        ui.label(RichText::new("programa").small().italics().weak());
                        ui.label("");
                    } else {
                        ui.label(value(calc, calc.register(index)));
                        let note = if (1..=6).contains(&index) { STAT_NAMES[index - 1] } else { "" };
                        ui.label(RichText::new(note).small().color(BLUE));
                    }
                    ui.end_row();
                }
            });
        }
    });
    ui.label(RichText::new("Em azul: somatórios estatísticos (Σ+) guardados em R1–R6.").small().weak());
}

fn cash_flows(ui: &mut Ui, calc: &Calculator) {
    let Some(last) = calc.last_flow() else { return };
    let used = last > 0 || !calc.register(0).is_zero();
    if !used {
        return;
    }
    section(ui, "Fluxos de caixa");
    egui::Grid::new("fluxos").num_columns(3).striped(true).spacing([18.0, 4.0]).show(ui, |ui| {
        ui.label(RichText::new("").small());
        ui.label(RichText::new("valor").small().weak());
        ui.label(RichText::new("Nj").small().weak());
        ui.end_row();
        for j in 0..=last {
            ui.monospace(if j == 0 { "CFo".to_string() } else { format!("CF{j}") });
            ui.label(value(calc, calc.register(j)));
            ui.monospace(calc.flow_count(j).to_string());
            ui.end_row();
        }
    });
}

fn program(ui: &mut Ui, calc: &Calculator) {
    section(ui, "Programa");
    let current = calc.program_line();
    let mode = if calc.is_program_mode() { " · modo de programação" } else { "" };
    ui.label(
        RichText::new(format!(
            "Linha {current:02} · memória P-{:02} r-{:02}{mode}",
            calc.program_capacity(),
            calc.registers_available()
        ))
        .small()
        .weak(),
    );
    let listing = calc.program_listing();
    let lines: Vec<&str> = listing.lines().filter(|l| !l.starts_with('#')).collect();
    if lines.is_empty() {
        ui.label(RichText::new("Nenhum programa gravado (f P/R para começar).").small().weak());
        return;
    }
    for line in lines {
        let number: usize = line.get(..2).and_then(|n| n.parse().ok()).unwrap_or(0);
        let text = RichText::new(line).monospace();
        if number == current {
            ui.label(text.color(GOLD).strong());
        } else {
            ui.label(text);
        }
    }
}
