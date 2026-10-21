//! Janela "Programa": vê e edita o programa gravado.
//! Lista de linhas (editar, inserir, apagar, mover, ir para) ou edição como texto.

use eframe::egui::{self, Color32, RichText, Ui};
use hp12c_core::program::{self, Instruction};
use hp12c_core::{Calculator, Key};
use hp12c_skin::Skin;

const GOLD: Color32 = Color32::from_rgb(226, 169, 59);
const RED: Color32 = Color32::from_rgb(230, 110, 100);
const GREEN: Color32 = Color32::from_rgb(110, 200, 140);

/// Alteração esperando confirmação porque vai apagar registradores com valor.
struct Pending {
    lines: Vec<Instruction>,
    lost: Vec<usize>,
    select: Option<usize>,
}

#[derive(Default)]
pub struct ProgramEditor {
    /// Linha selecionada (0 = linha 01).
    selected: Option<usize>,
    /// Instrução digitada no campo de edição.
    edit: String,
    message: Option<(String, bool)>,
    text_mode: bool,
    text: String,
    keep_gotos: bool,
    pending: Option<Pending>,
}

impl ProgramEditor {
    pub fn new() -> Self {
        Self { keep_gotos: false, ..Self::default() }
    }

    pub fn show(&mut self, ui: &mut Ui, calc: &mut Calculator, skin: &Skin) {
        header(ui, calc);
        if calc.is_running() {
            ui.label(RichText::new("Programa em execução — pressione uma tecla na calculadora para parar e editar.").color(GOLD));
            ui.add_enabled_ui(false, |ui| self.list(ui, calc, skin));
            return;
        }
        if let Some(pending) = &self.pending {
            let names: Vec<String> = pending.lost.iter().map(|&i| register_name(i)).collect();
            let mut decision = None;
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.label(RichText::new("Atenção").strong().color(GOLD));
                ui.label(format!(
                    "Com {} linhas, o programa passa a ocupar {}, que {} valor guardado. Esse valor será apagado.",
                    pending.lines.len(),
                    names.join(", "),
                    if names.len() == 1 { "tem" } else { "têm" }
                ));
                ui.horizontal(|ui| {
                    if ui.button("Aplicar mesmo assim").clicked() {
                        decision = Some(true);
                    }
                    if ui.button("Cancelar").clicked() {
                        decision = Some(false);
                    }
                });
            });
            match decision {
                Some(true) => {
                    let p = self.pending.take().unwrap();
                    self.apply(calc, p.lines, p.select);
                }
                Some(false) => {
                    self.pending = None;
                    self.message = Some(("Alteração cancelada.".into(), false));
                }
                None => {}
            }
            return;
        }

        ui.horizontal(|ui| {
            if ui.selectable_label(!self.text_mode, "Linhas").clicked() {
                self.text_mode = false;
            }
            if ui.selectable_label(self.text_mode, "Editar como texto").clicked() && !self.text_mode {
                self.text_mode = true;
                self.text = calc.program_instructions().iter().map(|i| i.listing() + "\n").collect();
            }
        });
        ui.separator();
        if let Some((text, error)) = &self.message {
            ui.label(RichText::new(text).color(if *error { RED } else { GREEN }));
        }
        if self.text_mode {
            self.text_editor(ui, calc);
        } else {
            self.line_tools(ui, calc);
            ui.add_space(6.0);
            self.list(ui, calc, skin);
        }
    }

    fn list(&mut self, ui: &mut Ui, calc: &mut Calculator, skin: &Skin) {
        let lines = calc.program_instructions().to_vec();
        let current = calc.program_line();
        if lines.is_empty() {
            ui.label(RichText::new("Nenhum programa. Digite uma instrução acima e use \"Inserir no fim\", ou grave pela calculadora (f P/R).").weak());
            return;
        }
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            for (index, instruction) in lines.iter().enumerate() {
                let number = index + 1;
                let marker = if number == current { "▶" } else { " " };
                let text = format!("{marker} {number:02}  {:<9} {}", instruction.codes(), describe(instruction, skin));
                let mut rich = RichText::new(text).monospace();
                if number == current {
                    rich = rich.color(GOLD);
                }
                if ui.selectable_label(self.selected == Some(index), rich).clicked() {
                    self.selected = Some(index);
                    self.edit = instruction.listing();
                    self.message = None;
                }
            }
        });
    }

    fn line_tools(&mut self, ui: &mut Ui, calc: &mut Calculator) {
        let lines = calc.program_instructions().to_vec();
        let selected = self.selected.filter(|&s| s < lines.len());
        ui.horizontal(|ui| {
            ui.label(match selected {
                Some(s) => format!("Linha {:02}:", s + 1),
                None => "Instrução:".to_string(),
            });
            let field = ui.add(
                egui::TextEdit::singleline(&mut self.edit)
                    .font(egui::TextStyle::Monospace)
                    .hint_text("ex.: STO + 1, g 0, f n, g GTO 05")
                    .desired_width(180.0),
            );
            let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if enter {
                match selected {
                    Some(s) => self.replace(calc, &lines, s),
                    None => self.insert(calc, &lines, lines.len()),
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            match selected {
                Some(s) => {
                    if ui.button("Substituir linha").clicked() {
                        self.replace(calc, &lines, s);
                    }
                    if ui.button("Inserir depois").clicked() {
                        self.insert(calc, &lines, s + 1);
                    }
                    if ui.button("Apagar linha").clicked() {
                        let mut new = lines.clone();
                        new.remove(s);
                        if !self.keep_gotos {
                            program::shift_gotos(&mut new, s + 1, -1);
                        }
                        let select = if new.is_empty() { None } else { Some(s.min(new.len() - 1)) };
                        self.commit(calc, new, select);
                    }
                    if ui.add_enabled(s > 0, egui::Button::new("↑")).clicked() {
                        self.swap(calc, &lines, s, s - 1);
                    }
                    if ui.add_enabled(s + 1 < lines.len(), egui::Button::new("↓")).clicked() {
                        self.swap(calc, &lines, s, s + 1);
                    }
                    if ui.button("Ir para esta linha").clicked() {
                        calc.go_to_line(s + 1);
                        self.message = Some((format!("R/S começa na linha {:02}.", s + 1), false));
                    }
                }
                None => {
                    if ui.button("Inserir no fim").clicked() {
                        self.insert(calc, &lines, lines.len());
                    }
                }
            }
            if selected.is_some() && ui.button("Desmarcar").clicked() {
                self.selected = None;
                self.edit.clear();
            }
        });
        ui.checkbox(&mut self.keep_gotos, "Não ajustar os GTO ao inserir, apagar ou mover (como na 12C real)");
        ui.label(
            RichText::new(
                "Nomes das teclas: 0–9 . ENTER + - * / CHS EEX x<>y Rv CLx STO RCL f g n i PV PMT FV y^x 1/x % %T D% S+ R/S",
            )
            .small()
            .weak(),
        );
    }

    fn parse_edit(&mut self) -> Option<Instruction> {
        match program::parse_instruction(self.edit.trim()) {
            Some(i) => Some(i),
            None => {
                self.message = Some((format!("«{}» não é uma instrução válida.", self.edit.trim()), true));
                None
            }
        }
    }

    fn replace(&mut self, calc: &mut Calculator, lines: &[Instruction], index: usize) {
        let Some(instruction) = self.parse_edit() else { return };
        let mut new = lines.to_vec();
        new[index] = instruction;
        self.commit(calc, new, Some(index));
    }

    /// Insere a instrução do campo na posição `at` (0 = antes da linha 01).
    fn insert(&mut self, calc: &mut Calculator, lines: &[Instruction], at: usize) {
        let Some(instruction) = self.parse_edit() else { return };
        let mut new = lines.to_vec();
        if !self.keep_gotos {
            program::shift_gotos(&mut new, at + 1, 1);
        }
        new.insert(at, instruction);
        self.commit(calc, new, Some(at));
    }

    fn swap(&mut self, calc: &mut Calculator, lines: &[Instruction], a: usize, b: usize) {
        let mut new = lines.to_vec();
        new.swap(a, b);
        if !self.keep_gotos {
            program::swap_gotos(&mut new, a + 1, b + 1);
        }
        self.commit(calc, new, Some(b));
    }

    fn text_editor(&mut self, ui: &mut Ui, calc: &mut Calculator) {
        ui.label(
            RichText::new("Uma instrução por linha. Número da linha no começo e comentários depois de # são opcionais. Aqui os GTO não são ajustados.")
                .small()
                .weak(),
        );
        ui.horizontal(|ui| {
            if ui.button("Aplicar").clicked() {
                match parse_text(&self.text) {
                    Ok(lines) => {
                        self.commit(calc, lines, None);
                        if self.pending.is_none() && !self.message.as_ref().is_some_and(|m| m.1) {
                            self.text_mode = false;
                        }
                    }
                    Err(e) => self.message = Some((e, true)),
                }
            }
            if ui.button("Descartar").clicked() {
                self.text_mode = false;
                self.message = None;
            }
        });
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            ui.add(
                egui::TextEdit::multiline(&mut self.text)
                    .code_editor()
                    .desired_width(f32::INFINITY)
                    .desired_rows(24),
            );
        });
    }

    /// Aplica já ou pede confirmação se registradores com valor seriam ocupados.
    fn commit(&mut self, calc: &mut Calculator, lines: Vec<Instruction>, select: Option<usize>) {
        if lines.len() > program::MAX_LINES {
            self.message = Some(("A HP-12C aceita no máximo 99 linhas.".into(), true));
            return;
        }
        let lost = calc.registers_lost_by(lines.len());
        if lost.is_empty() {
            self.apply(calc, lines, select);
        } else {
            self.pending = Some(Pending { lines, lost, select });
        }
    }

    fn apply(&mut self, calc: &mut Calculator, lines: Vec<Instruction>, select: Option<usize>) {
        let count = lines.len();
        match calc.set_program(lines) {
            Ok(()) => {
                self.selected = select;
                self.edit = select
                    .and_then(|s| calc.program_instructions().get(s).map(|i| i.listing()))
                    .unwrap_or_default();
                self.message = Some((format!("Programa atualizado ({count} linhas)."), false));
            }
            Err(_) => self.message = Some(("A HP-12C aceita no máximo 99 linhas.".into(), true)),
        }
    }
}

fn header(ui: &mut Ui, calc: &Calculator) {
    let used = calc.program_instructions().len();
    let mode = if calc.is_program_mode() { " · calculadora em modo de programação" } else { "" };
    ui.label(
        RichText::new(format!(
            "{used} linhas · memória P-{:02} r-{:02} · próxima linha {:02}{mode}",
            calc.program_capacity(),
            calc.registers_available(),
            calc.program_line()
        ))
        .small()
        .weak(),
    );
    ui.add_space(4.0);
}

fn register_name(index: usize) -> String {
    if index < 10 {
        format!("R{index}")
    } else {
        format!("R.{}", index - 10)
    }
}

/// Lê o editor de texto: uma instrução por linha, número e comentário opcionais.
fn parse_text(text: &str) -> Result<Vec<Instruction>, String> {
    let mut lines = Vec::new();
    for (n, raw) in text.lines().enumerate() {
        let body = raw.split('#').next().unwrap_or("").trim();
        if body.is_empty() {
            continue;
        }
        let tokens: Vec<&str> = body.split_whitespace().collect();
        let body = if tokens.len() > 1 && tokens[0].len() == 2 && tokens[0].chars().all(|c| c.is_ascii_digit()) {
            tokens[1..].join(" ")
        } else {
            tokens.join(" ")
        };
        let instruction = program::parse_instruction(&body)
            .ok_or_else(|| format!("Linha {} do texto: «{body}» não é uma instrução válida.", n + 1))?;
        lines.push(instruction);
    }
    Ok(lines)
}

/// Instrução por extenso, com as legendas da calculadora: "STO + 1", "x=0", "AMORT", "GTO 12".
pub fn describe(instruction: &Instruction, skin: &Skin) -> String {
    let keys = match instruction {
        Instruction::Goto(line) => return format!("GTO {line:02}"),
        Instruction::Keys(keys) => keys,
    };
    let main = |k: Key| match k {
        Key::Dot => ".".to_string(),
        Key::Enter => "ENTER".to_string(),
        _ => skin.key(k).main.text(),
    };
    let f_label = |k: Key| skin.key(k).f.as_ref().map(|l| l.text());
    let g_label = |k: Key| skin.key(k).g.as_ref().map(|l| l.text());
    match keys.as_slice() {
        [Key::F, k] if k.digit().is_some() => format!("FIX {}", k.digit().unwrap()),
        [Key::F, Key::Dot] => "SCI".to_string(),
        [Key::F, k] => match f_label(*k).as_deref() {
            Some("Σ" | "PRGM" | "FIN" | "REG") => format!("CLEAR {}", f_label(*k).unwrap()),
            Some(label) => label.to_string(),
            None => format!("f {}", main(*k)),
        },
        [Key::G, k] => g_label(*k).unwrap_or_else(|| format!("g {}", main(*k))),
        [Key::Rcl, Key::G, k] => format!("RCL {}", g_label(*k).unwrap_or_default()),
        [Key::Sto, Key::Eex] => "STO EEX (liga/desliga C)".to_string(),
        _ => keys.iter().map(|k| main(*k)).collect::<Vec<_>>().join(" "),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn i(text: &str) -> Instruction {
        program::parse_instruction(text).unwrap()
    }

    #[test]
    fn descricoes() {
        let skin = Skin::builtin();
        assert_eq!(describe(&i("STO + 1"), &skin), "STO + 1");
        assert_eq!(describe(&i("g CLx"), &skin), "x=0");
        assert_eq!(describe(&i("g x<>y"), &skin), "x≤y");
        assert_eq!(describe(&i("f n"), &skin), "AMORT");
        assert_eq!(describe(&i("f 4"), &skin), "FIX 4");
        assert_eq!(describe(&i("f SST"), &skin), "CLEAR Σ");
        assert_eq!(describe(&i("g GTO 12"), &skin), "GTO 12");
        assert_eq!(describe(&i("RCL g PMT"), &skin), "RCL CFj");
        assert_eq!(describe(&i("y^x"), &skin), "yˣ");
        assert_eq!(describe(&i("*"), &skin), "×");
    }

    fn listing(calc: &Calculator) -> Vec<String> {
        calc.program_instructions().iter().map(|i| i.listing()).collect()
    }

    #[test]
    fn acoes_do_editor() {
        let mut calc = Calculator::new();
        calc.load_program("# HP-12C programa\n01 1\n02 +\n03 g GTO 01\n").unwrap();
        let mut ed = ProgramEditor::new();
        let lines = calc.program_instructions().to_vec();

        // Inserir antes da linha 01: o GTO 01 passa a GTO 02.
        ed.edit = "ENTER".into();
        ed.insert(&mut calc, &lines, 0);
        assert_eq!(listing(&calc), ["ENTER", "1", "+", "g GTO 02"]);

        // Substituir a linha 03 (+) por ×.
        let lines = calc.program_instructions().to_vec();
        ed.edit = "*".into();
        ed.replace(&mut calc, &lines, 2);
        assert_eq!(listing(&calc), ["ENTER", "1", "*", "g GTO 02"]);

        // Mover a linha 02 para cima troca as referências 01 ↔ 02.
        let lines = calc.program_instructions().to_vec();
        ed.swap(&mut calc, &lines, 1, 0);
        assert_eq!(listing(&calc), ["1", "ENTER", "*", "g GTO 01"]);

        // Instrução inválida não muda nada e mostra erro.
        let lines = calc.program_instructions().to_vec();
        ed.edit = "STO 9 9".into();
        ed.insert(&mut calc, &lines, 0);
        assert_eq!(listing(&calc).len(), 4);
        assert!(ed.message.as_ref().is_some_and(|m| m.1));
    }

    #[test]
    fn aviso_antes_de_apagar_registrador() {
        let mut calc = Calculator::new();
        calc.tap_all("42 STO . 9").unwrap();
        let mut ed = ProgramEditor::new();
        let nine = vec![i("1"); 9];
        ed.commit(&mut calc, nine, None);
        assert!(ed.pending.is_some(), "deve pedir confirmação");
        assert_eq!(calc.program_instructions().len(), 0, "nada aplicado ainda");
        let p = ed.pending.take().unwrap();
        assert_eq!(p.lost, vec![19]);
        ed.apply(&mut calc, p.lines, p.select);
        assert_eq!(calc.program_instructions().len(), 9);
    }

    #[test]
    fn texto_com_numeros_e_comentarios() {
        let lines = parse_text("# HP-12C programa\n01 STO 0   # guarda\n\n2\ng GTO 01\n").unwrap();
        assert_eq!(lines, vec![i("STO 0"), i("2"), i("g GTO 01")]);
        assert!(parse_text("STO 9 9").unwrap_err().contains("Linha 1"));
    }
}
