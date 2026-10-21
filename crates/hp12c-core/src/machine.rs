//! A calculadora: estado completo e tratamento de cada tecla.

use crate::decimal::{saturate, HpDecimal, MathError};
use crate::display::{self, Annunciators, DisplayFormat, DisplayState};
use crate::entry::Entry;
use crate::error::ErrorCode;
use crate::keys::{self, Key, KEY_COUNT};
use crate::dates::{self, Date};
use crate::finance;
use crate::json::{self, Json};
use crate::math;
use crate::program::{self, Command, Instruction, Recorded};
use crate::stats;

/// R0–R9 e R.0–R.9.
pub const STORAGE_REGISTERS: usize = 20;
/// Aritmética de registrador (STO + n) só vale para R0–R4 na HP-12C.
const REGISTER_ARITHMETIC_LIMIT: u8 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FinReg {
    N = 0,
    I = 1,
    Pv = 2,
    Pmt = 3,
    Fv = 4,
}

impl FinReg {
    fn from_key(key: Key) -> Option<Self> {
        match key {
            Key::N => Some(Self::N),
            Key::I => Some(Self::I),
            Key::Pv => Some(Self::Pv),
            Key::Pmt => Some(Self::Pmt),
            Key::Fv => Some(Self::Fv),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stack {
    pub x: HpDecimal,
    pub y: HpDecimal,
    pub z: HpDecimal,
    pub t: HpDecimal,
    pub last_x: HpDecimal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ArithOp {
    Add,
    Sub,
    Mul,
    Div,
}

impl ArithOp {
    fn from_key(key: Key) -> Option<Self> {
        match key {
            Key::Add => Some(Self::Add),
            Key::Sub => Some(Self::Sub),
            Key::Mul => Some(Self::Mul),
            Key::Div => Some(Self::Div),
            _ => None,
        }
    }

    fn apply(self, a: HpDecimal, b: HpDecimal) -> Result<HpDecimal, MathError> {
        match self {
            Self::Add => a.add(b),
            Self::Sub => a.sub(b),
            Self::Mul => a.mul(b),
            Self::Div => a.div(b),
        }
    }
}

/// Tecla de prefixo pendente: a próxima tecla completa o comando.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Prefix {
    None,
    F,
    G,
    Sto,
    StoDot,
    StoArith(ArithOp),
    Rcl,
    RclDot,
    /// RCL g: recupera CFj ou Nj.
    RclG,
    /// g GTO no modo de execução: espera dois dígitos (o primeiro já digitado, se houver).
    Goto(Option<u8>),
}

/// Testes de inicialização (ligar com uma tecla pressionada).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SelfTest {
    /// ON + ×: todos os segmentos e indicadores acesos.
    Segments,
    /// ON + ÷: teclas na ordem física; guarda quantas já foram pressionadas.
    Keyboard(usize),
    /// Teste de teclado concluído com sucesso.
    KeyboardPassed,
}

/// Ordem do teste de teclado: da esquerda para a direita, de cima para baixo,
/// com ENTER nas linhas 3 e 4.
const KEYBOARD_TEST: [Key; 40] = [
    Key::N, Key::I, Key::Pv, Key::Pmt, Key::Fv, Key::Chs, Key::D7, Key::D8, Key::D9, Key::Div,
    Key::Pow, Key::Recip, Key::PctTotal, Key::PctChange, Key::Pct, Key::Eex, Key::D4, Key::D5, Key::D6, Key::Mul,
    Key::RunStop, Key::Sst, Key::RollDown, Key::Swap, Key::Clx, Key::Enter, Key::D1, Key::D2, Key::D3, Key::Sub,
    Key::On, Key::F, Key::G, Key::Sto, Key::Rcl, Key::Enter, Key::D0, Key::Dot, Key::SigmaPlus, Key::Add,
];

/// Versão do formato salvo por `save_state`.
const STATE_VERSION: i64 = 1;

/// Pausa de g PSE, em milissegundos.
const PAUSE_MS: u32 = 1000;
/// Linhas de programa executadas por chamada de `tick`.
const STEPS_PER_TICK: usize = 200;
/// Limite de passos em `tap_all` (protege contra laço infinito nos testes).
const MAX_STEPS_SYNC: usize = 1_000_000;

#[derive(Clone, Debug)]
pub struct Calculator {
    stack: Stack,
    /// Se verdadeiro, o próximo número digitado ou recuperado empurra a pilha.
    stack_lift: bool,
    registers: [HpDecimal; STORAGE_REGISTERS],
    financial: [HpDecimal; 5],
    entry: Option<Entry>,
    prefix: Prefix,
    format: DisplayFormat,
    comma_decimal: bool,
    begin_mode: bool,
    dmy_mode: bool,
    compound: bool,
    error: Option<ErrorCode>,
    powered: bool,
    held: [bool; KEY_COUNT],
    show_mantissa: bool,
    /// Ligado após guardar/calcular um registro financeiro: a próxima tecla
    /// financeira calcula em vez de guardar.
    fin_flag: bool,
    /// Valor de `fin_flag` antes da tecla atual.
    fin_ready: bool,
    /// A tecla atual não mexe em X (prefixo, formato, modo): preserva `fin_flag`.
    neutral: bool,
    /// Nj de cada fluxo de caixa (CFo = índice 0).
    flow_counts: [u8; STORAGE_REGISTERS],
    /// Resultado de DATE: texto da data e dia da semana, até a próxima tecla.
    date_view: Option<(String, u8)>,
    /// Memória de programa (linhas 01…); as vazias contêm GTO 00.
    program: Vec<Instruction>,
    /// Linha atual do programa (0 = início).
    pc: usize,
    /// Modo de programação (f P/R).
    program_mode: bool,
    /// Teclas acumuladas no modo de programação até formar uma instrução.
    recording: Vec<Key>,
    running: bool,
    /// Tempo restante de uma pausa (g PSE).
    pause_ms: u32,
    /// Verdadeiro enquanto uma linha de programa está sendo executada.
    executing: bool,
    /// Um teste (x≤y, x=0) deu falso: pula a próxima linha.
    skip_next: bool,
    /// SST no modo de execução: linha mostrada enquanto a tecla está pressionada
    /// e executada quando ela é solta.
    step_line: Option<usize>,
    /// BST no modo de execução: linha mostrada enquanto a tecla está pressionada.
    back_line: Option<usize>,
    /// g MEM: mostra a ocupação da memória até a próxima tecla.
    mem_view: bool,
    self_test: Option<SelfTest>,
}

impl Default for Calculator {
    fn default() -> Self {
        Self::new()
    }
}

impl Calculator {
    pub fn new() -> Self {
        Self {
            stack: Stack::default(),
            stack_lift: true,
            registers: [HpDecimal::ZERO; STORAGE_REGISTERS],
            financial: [HpDecimal::ZERO; 5],
            entry: None,
            prefix: Prefix::None,
            format: DisplayFormat::Fix(2),
            comma_decimal: false,
            begin_mode: false,
            dmy_mode: false,
            compound: false,
            error: None,
            powered: true,
            held: [false; KEY_COUNT],
            show_mantissa: false,
            fin_flag: false,
            fin_ready: false,
            neutral: false,
            flow_counts: [1; STORAGE_REGISTERS],
            date_view: None,
            program: vec![Instruction::EMPTY; program::BASE_LINES],
            pc: 0,
            program_mode: false,
            recording: Vec::new(),
            running: false,
            pause_ms: 0,
            executing: false,
            skip_next: false,
            step_line: None,
            back_line: None,
            mem_view: false,
            self_test: None,
        }
    }

    // ----- Consulta de estado -----

    pub fn x(&self) -> HpDecimal {
        self.stack.x
    }

    pub fn stack(&self) -> Stack {
        self.stack
    }

    /// Registro de armazenamento: 0–9 = R0–R9, 10–19 = R.0–R.9.
    pub fn register(&self, index: usize) -> HpDecimal {
        self.registers[index]
    }

    pub fn financial(&self, reg: FinReg) -> HpDecimal {
        self.financial[reg as usize]
    }

    pub fn format(&self) -> DisplayFormat {
        self.format
    }

    pub fn is_powered(&self) -> bool {
        self.powered
    }

    pub fn comma_decimal(&self) -> bool {
        self.comma_decimal
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Nj do fluxo de caixa `index` (0 = CFo).
    pub fn flow_count(&self, index: usize) -> u8 {
        self.flow_counts[index]
    }

    /// Índice do último fluxo de caixa (parte inteira de n), se for válido (0–19).
    pub fn last_flow(&self) -> Option<usize> {
        self.flow_index()
    }

    pub fn begin_mode(&self) -> bool {
        self.begin_mode
    }

    pub fn dmy_mode(&self) -> bool {
        self.dmy_mode
    }

    /// Período fracionário com juros compostos (indicador C).
    pub fn compound(&self) -> bool {
        self.compound
    }

    /// Linhas de programa alocadas (8 a 99).
    pub fn program_capacity(&self) -> usize {
        self.program.len()
    }

    /// Instruções do programa até a última linha não vazia.
    pub fn program_instructions(&self) -> &[Instruction] {
        let used = self.program.iter().rposition(|i| *i != Instruction::EMPTY).map_or(0, |p| p + 1);
        &self.program[..used]
    }

    /// Registradores com valor que seriam ocupados por um programa de `used` linhas.
    pub fn registers_lost_by(&self, used: usize) -> Vec<usize> {
        let after = STORAGE_REGISTERS
            - (program::capacity_for(used) - program::BASE_LINES) / program::LINES_PER_REGISTER;
        (after..self.registers_available()).filter(|&i| !self.registers[i].is_zero()).collect()
    }

    /// Substitui o programa inteiro (editor de programas). A memória é realocada:
    /// registradores que passam a ser do programa são zerados. Mais de 99 linhas → Error 4.
    pub fn set_program(&mut self, lines: Vec<Instruction>) -> Result<(), ErrorCode> {
        if lines.len() > program::MAX_LINES {
            return Err(ErrorCode::MEMORY);
        }
        let before = self.registers_available();
        let mut program = lines;
        program.resize(program::capacity_for(program.len()), Instruction::EMPTY);
        self.program = program;
        for index in self.registers_available()..before {
            self.registers[index] = HpDecimal::ZERO;
        }
        if self.pc > self.program.len() {
            self.pc = 0;
        }
        self.recording.clear();
        Ok(())
    }

    /// Posiciona o programa na linha `line` (como g GTO nn).
    pub fn go_to_line(&mut self, line: usize) {
        if line <= self.program.len() && !self.running {
            self.pc = line;
        }
    }

    pub fn is_program_mode(&self) -> bool {
        self.program_mode
    }

    /// Linha atual do programa.
    pub fn program_line(&self) -> usize {
        self.pc
    }

    pub fn display(&self) -> DisplayState {
        if !self.powered {
            return DisplayState::blank();
        }
        match self.self_test {
            Some(SelfTest::Segments) => return display::all_segments(),
            Some(SelfTest::Keyboard(done)) => return display::text(&format!("{done:>10}")),
            Some(SelfTest::KeyboardPassed) => return display::text("12"),
            None => {}
        }
        let shown_line = self.step_line.or(self.back_line);
        let mut state = if let Some(error) = self.error {
            display::text(&error.message())
        } else if self.running && self.pause_ms == 0 {
            let mut running = display::text("running");
            running.blinking = true;
            running
        } else if self.mem_view {
            display::text(&format!("P-{:02} r-{:02}", self.program.len(), self.registers_available()))
        } else if let Some(line) = shown_line {
            display::program_line(line, &self.line_codes(line), self.comma_decimal)
        } else if self.program_mode {
            display::program_line(self.pc, &self.line_codes(self.pc), self.comma_decimal)
        } else if self.show_mantissa {
            display::mantissa(self.stack.x)
        } else if let Some(entry) = &self.entry {
            display::entry(entry, self.comma_decimal)
        } else if let Some((text, weekday)) = &self.date_view {
            display::date(text, *weekday, self.comma_decimal)
        } else {
            display::number(self.stack.x, self.format, self.comma_decimal)
        };
        let pending = if self.program_mode { self.recording.last().copied() } else { None };
        state.annunciators = Annunciators {
            f: self.prefix == Prefix::F || (pending == Some(Key::F) && self.recording.len() == 1),
            g: self.prefix == Prefix::G || (pending == Some(Key::G) && self.recording.len() == 1),
            begin: self.begin_mode,
            dmy: self.dmy_mode,
            compound: self.compound,
            prgm: self.program_mode,
            ..Annunciators::default()
        };
        state
    }

    // ----- Entrada de teclas -----

    pub fn press(&mut self, key: Key) {
        self.held[key.index()] = true;
        if let Some(test) = self.self_test {
            self.self_test_key(test, key);
            return;
        }
        if key == Key::On {
            self.toggle_power();
            return;
        }
        if !self.powered {
            return;
        }
        // Qualquer tecla interrompe um programa em execução.
        if self.running {
            self.stop_program();
            return;
        }
        self.date_view = None;
        self.mem_view = false;
        // Com uma mensagem de erro na tela, a tecla só apaga a mensagem.
        if self.error.take().is_some() {
            return;
        }
        if self.program_mode {
            self.record_key(key);
            return;
        }
        self.process_key(key);
    }

    /// Executa uma tecla no modo normal (do teclado ou de uma linha de programa).
    fn process_key(&mut self, key: Key) {
        self.fin_ready = self.fin_flag;
        self.fin_flag = false;
        self.neutral = false;
        let prefix = core::mem::replace(&mut self.prefix, Prefix::None);
        self.dispatch(prefix, key);
        if self.neutral || self.prefix != Prefix::None {
            self.fin_flag = self.fin_ready;
        }
    }

    fn dispatch(&mut self, prefix: Prefix, key: Key) {
        match prefix {
            Prefix::None => self.press_normal(key),
            Prefix::F => self.press_f(key),
            Prefix::G => self.press_g(key),
            Prefix::Sto => self.press_sto(key),
            Prefix::StoDot => {
                if let Some(d) = key.digit() {
                    if self.register_ok(10 + d as usize) {
                        self.registers[10 + d as usize] = self.stack.x;
                        self.stack_lift = true;
                    }
                }
            }
            Prefix::StoArith(op) => self.press_sto_arith(op, key),
            Prefix::Rcl => self.press_rcl(key),
            Prefix::RclDot => {
                if let Some(d) = key.digit() {
                    if self.register_ok(10 + d as usize) {
                        self.recall(self.registers[10 + d as usize]);
                    }
                }
            }
            Prefix::RclG => self.press_rcl_g(key),
            Prefix::Goto(first) => self.press_goto(first, key),
        }
    }

    pub fn release(&mut self, key: Key) {
        self.held[key.index()] = false;
        // f PREFIX mostra a mantissa só enquanto a tecla fica pressionada.
        if key == Key::Enter {
            self.show_mantissa = false;
        }
        if key == Key::Sst {
            self.back_line = None;
            // SST executa a linha quando a tecla é solta.
            if let Some(line) = self.step_line.take() {
                if line == 0 {
                    self.pc = 0;
                } else {
                    self.execute_line(line);
                }
            }
        }
    }

    /// Coloca um número vindo de fora (ex.: colado da área de transferência) em X,
    /// elevando a pilha como RCL.
    pub fn input_value(&mut self, value: HpDecimal) {
        if !self.powered {
            return;
        }
        self.error = None;
        self.prefix = Prefix::None;
        self.end_entry();
        self.recall(value);
    }

    /// Cancela f, g, STO ou RCL pendente (equivale a f PREFIX na máquina real).
    pub fn cancel_prefix(&mut self) {
        self.prefix = Prefix::None;
    }

    /// Avança o programa em execução. A interface chama a cada quadro.
    pub fn tick(&mut self, elapsed_ms: u32) {
        if !self.running {
            return;
        }
        if self.pause_ms > 0 {
            self.pause_ms = self.pause_ms.saturating_sub(elapsed_ms);
            return;
        }
        for _ in 0..STEPS_PER_TICK {
            if !self.running || self.pause_ms > 0 {
                break;
            }
            self.step_program();
        }
    }

    /// Roda o programa até parar, sem pausas (para testes e sequências de teclas).
    pub fn run_until_stopped(&mut self, max_steps: usize) {
        let mut steps = 0;
        while self.running && steps < max_steps {
            self.pause_ms = 0;
            self.step_program();
            steps += 1;
        }
        if self.running {
            self.stop_program();
        }
    }

    /// Pressiona e solta uma tecla.
    pub fn tap(&mut self, key: Key) {
        self.press(key);
        self.release(key);
    }

    /// Executa uma sequência como `"12.5 ENTER 3 *"`. Programas iniciados com R/S
    /// rodam até o fim antes da tecla seguinte.
    pub fn tap_all(&mut self, sequence: &str) -> Result<(), String> {
        for key in keys::parse_sequence(sequence)? {
            self.tap(key);
            self.run_until_stopped(MAX_STEPS_SYNC);
        }
        Ok(())
    }

    // ----- Programação -----

    /// Registradores livres: cada 7 linhas além das 8 iniciais ocupam um (do R.9 para baixo).
    pub fn registers_available(&self) -> usize {
        STORAGE_REGISTERS - (self.program.len() - program::BASE_LINES) / program::LINES_PER_REGISTER
    }

    fn register_ok(&mut self, index: usize) -> bool {
        if index < self.registers_available() {
            true
        } else {
            self.error = Some(ErrorCode::STORAGE_REGISTERS);
            false
        }
    }

    fn line_codes(&self, line: usize) -> String {
        if line == 0 {
            String::new()
        } else {
            self.program[line - 1].codes()
        }
    }

    fn record_key(&mut self, key: Key) {
        let mut seq = core::mem::take(&mut self.recording);
        seq.push(key);
        match program::classify(&seq) {
            Recorded::Pending => self.recording = seq,
            Recorded::Restart(k) => self.recording = vec![k],
            Recorded::Cancel => {}
            Recorded::Instruction(instruction) => self.write_instruction(instruction),
            Recorded::Command(command) => self.program_command(command),
        }
    }

    /// Grava na linha seguinte (sobrescreve, como na 12C original). Passar do fim da
    /// memória converte um registrador em 7 linhas; acima de 99 linhas → Error 4.
    fn write_instruction(&mut self, instruction: Instruction) {
        let line = self.pc + 1;
        if line > self.program.len() {
            if self.program.len() + program::LINES_PER_REGISTER > program::MAX_LINES {
                self.error = Some(ErrorCode::MEMORY);
                return;
            }
            let taken = self.registers_available() - 1;
            self.registers[taken] = HpDecimal::ZERO;
            self.program.extend(core::iter::repeat_n(Instruction::EMPTY, program::LINES_PER_REGISTER));
        }
        self.program[line - 1] = instruction;
        self.pc = line;
    }

    fn program_command(&mut self, command: Command) {
        let last = self.program.len();
        match command {
            Command::Exit => {
                self.program_mode = false;
                self.pc = 0;
            }
            Command::Clear => self.clear_program(),
            Command::Step => self.pc = if self.pc >= last { 0 } else { self.pc + 1 },
            Command::Back => self.pc = if self.pc == 0 { last } else { self.pc - 1 },
            Command::Memory => self.mem_view = true,
            Command::GotoLine(line) => {
                if line as usize > last {
                    self.error = Some(ErrorCode::MEMORY);
                } else {
                    self.pc = line as usize;
                }
            }
        }
    }

    fn clear_program(&mut self) {
        self.program = vec![Instruction::EMPTY; program::BASE_LINES];
        self.pc = 0;
    }

    fn start_program(&mut self) {
        // R/S encerra a digitação e libera a elevação da pilha, como uma operação.
        self.end_entry();
        self.stack_lift = true;
        self.running = true;
        self.pause_ms = 0;
    }

    fn stop_program(&mut self) {
        self.running = false;
        self.pause_ms = 0;
        self.end_entry();
    }

    /// Próxima linha a executar no modo de execução (`pc` 0 equivale à linha 01).
    fn next_line(&self) -> usize {
        self.pc.max(1)
    }

    /// Executa a próxima linha; passar do fim da memória volta à linha 00 e para.
    fn step_program(&mut self) {
        let line = self.next_line();
        if line > self.program.len() {
            self.pc = 0;
            self.stop_program();
            return;
        }
        self.execute_line(line);
    }

    /// Executa uma linha; depois dela, `pc` aponta para a seguinte (ou para o destino do GTO).
    fn execute_line(&mut self, line: usize) {
        self.pc = line + 1;
        match self.program[line - 1].clone() {
            Instruction::Goto(target) => {
                self.pc = target as usize;
                if target == 0 && self.running {
                    self.stop_program();
                }
            }
            Instruction::Keys(keys) => match keys.as_slice() {
                [Key::RunStop] => {
                    if self.running {
                        self.stop_program();
                    }
                }
                [Key::G, Key::RunStop] => {
                    if self.running {
                        self.pause_ms = PAUSE_MS;
                    }
                }
                _ => {
                    self.executing = true;
                    for key in keys {
                        self.process_key(key);
                    }
                    self.executing = false;
                    self.prefix = Prefix::None;
                    if self.error.is_some() {
                        // Para na linha que falhou.
                        self.pc = line;
                        self.skip_next = false;
                        self.stop_program();
                    } else if core::mem::take(&mut self.skip_next) {
                        self.pc += 1;
                    }
                }
            },
        }
    }

    /// Teste x≤y ou x=0 numa linha de programa: falso pula a linha seguinte.
    fn test(&mut self, condition: bool) {
        self.end_entry();
        if self.executing && !condition {
            self.skip_next = true;
        }
        self.neutral = true;
    }

    fn press_goto(&mut self, first: Option<u8>, key: Key) {
        if key == Key::Dot && first.is_none() {
            self.prefix = Prefix::Goto(None);
            return;
        }
        let Some(d) = key.digit() else { return };
        match first {
            None => self.prefix = Prefix::Goto(Some(d)),
            Some(tens) => {
                let line = (tens * 10 + d) as usize;
                if line > self.program.len() {
                    self.error = Some(ErrorCode::MEMORY);
                } else {
                    self.pc = line;
                }
                self.neutral = true;
            }
        }
    }

    /// Listagem do programa em texto (até a última linha não vazia).
    pub fn program_listing(&self) -> String {
        let used = self.program.iter().rposition(|i| *i != Instruction::EMPTY).map_or(0, |p| p + 1);
        let mut text = String::from("# HP-12C programa\n");
        for (index, instruction) in self.program[..used].iter().enumerate() {
            text.push_str(&format!("{:02} {}\n", index + 1, instruction.listing()));
        }
        text
    }

    /// Carrega uma listagem gerada por `program_listing` (substitui o programa atual).
    pub fn load_program(&mut self, listing: &str) -> Result<(), String> {
        self.clear_program();
        self.program_mode = true;
        self.recording.clear();
        let mut result = Ok(());
        for (number, raw) in listing.lines().enumerate() {
            // Comentários depois de '#' são ignorados (linhas inteiras ou no fim da instrução).
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            // O primeiro campo é o número da linha.
            let body: Vec<&str> = line
                .split_whitespace()
                .skip(1)
                .map(|t| if t == "GTO" { "Rv" } else { t })
                .collect();
            let keys = match keys::parse_sequence(&body.join(" ")) {
                Ok(keys) => keys,
                Err(e) => {
                    result = Err(format!("linha {}: {e}", number + 1));
                    break;
                }
            };
            let before = self.pc;
            for key in keys {
                self.record_key(key);
            }
            if self.error.is_some() || self.pc != before + 1 || !self.recording.is_empty() {
                result = Err(format!("linha {}: instrução inválida", number + 1));
                break;
            }
        }
        self.error = None;
        self.recording.clear();
        self.program_mode = false;
        self.pc = 0;
        if result.is_err() {
            self.clear_program();
        }
        result
    }

    fn toggle_power(&mut self) {
        if self.powered {
            self.powered = false;
            self.prefix = Prefix::None;
            self.entry = None;
            self.show_mantissa = false;
        } else {
            self.powered = true;
            // Ligar com uma tecla pressionada: separador decimal ou testes.
            if self.held[Key::Dot.index()] {
                self.comma_decimal = !self.comma_decimal;
            } else if self.held[Key::Mul.index()] {
                self.self_test = Some(SelfTest::Segments);
            } else if self.held[Key::Div.index()] {
                self.self_test = Some(SelfTest::Keyboard(0));
            } else if self.held[Key::Sub.index()] {
                self.reset_memory();
            }
        }
    }

    /// ON + −: apaga toda a memória contínua e mostra "Pr Error".
    fn reset_memory(&mut self) {
        let held = self.held;
        *self = Calculator::new();
        self.held = held;
        self.error = Some(ErrorCode::POWER);
    }

    fn self_test_key(&mut self, test: SelfTest, key: Key) {
        match test {
            SelfTest::Keyboard(done) => {
                if KEYBOARD_TEST[done] != key {
                    self.self_test = None;
                    self.error = Some(ErrorCode::SERVICE);
                } else if done + 1 == KEYBOARD_TEST.len() {
                    self.self_test = Some(SelfTest::KeyboardPassed);
                } else {
                    self.self_test = Some(SelfTest::Keyboard(done + 1));
                }
            }
            // Qualquer tecla encerra a exibição do teste.
            SelfTest::Segments | SelfTest::KeyboardPassed => self.self_test = None,
        }
    }

    /// Mostra "Pr Error" (ex.: a interface não conseguiu ler a memória salva).
    pub fn signal_memory_lost(&mut self) {
        self.error = Some(ErrorCode::POWER);
    }

    // ----- Memória contínua -----

    /// Estado completo em JSON (versionado), para restaurar com `load_state`.
    pub fn save_state(&self) -> String {
        let num = |v: &HpDecimal| Json::Str(v.to_string());
        let nums = |values: &[HpDecimal]| Json::Array(values.iter().map(num).collect());
        let s = self.stack;
        let format = match self.format {
            DisplayFormat::Fix(n) => format!("FIX {n}"),
            DisplayFormat::Sci => "SCI".to_string(),
        };
        let field = |k: &str, v: Json| (k.to_string(), v);
        Json::Object(vec![
            field("app", Json::Str("HP-12C".into())),
            field("version", Json::Int(STATE_VERSION)),
            field("stack", nums(&[s.x, s.y, s.z, s.t, s.last_x])),
            field("stack_lift", Json::Bool(self.stack_lift)),
            field("registers", nums(&self.registers)),
            field("financial", nums(&self.financial)),
            field("flow_counts", Json::Array(self.flow_counts.iter().map(|c| Json::Int(*c as i64)).collect())),
            field("format", Json::Str(format)),
            field("comma_decimal", Json::Bool(self.comma_decimal)),
            field("begin", Json::Bool(self.begin_mode)),
            field("dmy", Json::Bool(self.dmy_mode)),
            field("compound", Json::Bool(self.compound)),
            field("program", Json::Array(self.program.iter().map(|i| Json::Str(i.listing())).collect())),
        ])
        .write()
    }

    /// Restaura um estado salvo por `save_state`. A calculadora volta ligada,
    /// no modo de execução, na linha 00.
    pub fn load_state(text: &str) -> Result<Calculator, String> {
        let root = json::parse(text)?;
        if root.get("version").and_then(Json::as_int) != Some(STATE_VERSION) {
            return Err("versão do estado desconhecida".into());
        }
        let decimals = |key: &str, len: usize| -> Result<Vec<HpDecimal>, String> {
            let items = root.get(key).and_then(Json::as_array).ok_or(format!("campo {key} ausente"))?;
            if items.len() != len {
                return Err(format!("campo {key} com tamanho errado"));
            }
            items
                .iter()
                .map(|v| v.as_str().and_then(HpDecimal::parse).ok_or(format!("número inválido em {key}")))
                .collect()
        };
        let flag = |key: &str| root.get(key).and_then(Json::as_bool).ok_or(format!("campo {key} ausente"));

        let mut calc = Calculator::new();
        let stack = decimals("stack", 5)?;
        calc.stack = Stack { x: stack[0], y: stack[1], z: stack[2], t: stack[3], last_x: stack[4] };
        calc.stack_lift = flag("stack_lift")?;
        calc.registers.copy_from_slice(&decimals("registers", STORAGE_REGISTERS)?);
        calc.financial.copy_from_slice(&decimals("financial", 5)?);

        let counts = root.get("flow_counts").and_then(Json::as_array).ok_or("campo flow_counts ausente")?;
        if counts.len() != STORAGE_REGISTERS {
            return Err("campo flow_counts com tamanho errado".into());
        }
        for (slot, value) in calc.flow_counts.iter_mut().zip(counts) {
            *slot = value.as_int().filter(|c| (1..=99).contains(c)).ok_or("Nj inválido")? as u8;
        }

        calc.format = match root.get("format").and_then(Json::as_str) {
            Some("SCI") => DisplayFormat::Sci,
            Some(text) => text
                .strip_prefix("FIX ")
                .and_then(|n| n.parse::<u8>().ok())
                .filter(|n| *n <= 9)
                .map(DisplayFormat::Fix)
                .ok_or("formato inválido")?,
            None => return Err("campo format ausente".into()),
        };
        calc.comma_decimal = flag("comma_decimal")?;
        calc.begin_mode = flag("begin")?;
        calc.dmy_mode = flag("dmy")?;
        calc.compound = flag("compound")?;

        let lines = root.get("program").and_then(Json::as_array).ok_or("campo program ausente")?;
        let valid_size = lines.len() >= program::BASE_LINES
            && lines.len() <= program::MAX_LINES
            && (lines.len() - program::BASE_LINES).is_multiple_of(program::LINES_PER_REGISTER);
        if !valid_size {
            return Err("tamanho de programa inválido".into());
        }
        calc.program = lines
            .iter()
            .enumerate()
            .map(|(i, v)| {
                v.as_str()
                    .and_then(program::parse_instruction)
                    .ok_or(format!("linha de programa {} inválida", i + 1))
            })
            .collect::<Result<_, _>>()?;
        Ok(calc)
    }

    fn press_normal(&mut self, key: Key) {
        if let Some(d) = key.digit() {
            self.entry_edit(|e| e.push_digit(d));
            return;
        }
        if let Some(op) = ArithOp::from_key(key) {
            self.binary(op);
            return;
        }
        if let Some(reg) = FinReg::from_key(key) {
            // Logo após outra tecla financeira, calcula; senão, guarda X.
            self.end_entry();
            if self.fin_ready {
                self.compute_tvm(reg);
            } else {
                self.financial[reg as usize] = self.stack.x;
                self.stack_lift = true;
            }
            self.fin_flag = true;
            return;
        }
        let y = self.stack.y;
        match key {
            Key::Pow => self.binary_fn(math::pow),
            Key::Recip => self.unary(math::reciprocal),
            Key::SigmaPlus => self.accumulate(false),
            // %, Δ% e %T usam Y mas não descem a pilha: Y continua lá (ex.: 200 ENTER 15 % +).
            Key::Pct => self.unary(|x| math::percent(y, x)),
            Key::PctChange => self.unary(|x| math::percent_change(y, x)),
            Key::PctTotal => self.unary(|x| math::percent_total(y, x)),
            Key::Dot => self.entry_edit(Entry::push_point),
            Key::Eex => self.entry_edit(Entry::start_exponent),
            Key::Chs => match &mut self.entry {
                Some(entry) => {
                    entry.change_sign();
                    self.stack.x = entry.value();
                }
                None => self.stack.x = self.stack.x.neg(),
            },
            Key::Enter => {
                self.end_entry();
                self.push();
                self.stack_lift = false;
            }
            Key::Clx => {
                self.end_entry();
                self.stack.x = HpDecimal::ZERO;
                self.stack_lift = false;
            }
            Key::Swap => {
                self.end_entry();
                core::mem::swap(&mut self.stack.x, &mut self.stack.y);
                self.stack_lift = true;
            }
            Key::RollDown => {
                self.end_entry();
                let s = self.stack;
                self.stack = Stack { x: s.y, y: s.z, z: s.t, t: s.x, last_x: s.last_x };
                self.stack_lift = true;
            }
            Key::Sto => {
                self.end_entry();
                self.prefix = Prefix::Sto;
            }
            Key::Rcl => {
                self.end_entry();
                self.prefix = Prefix::Rcl;
            }
            Key::F => self.prefix = Prefix::F,
            Key::G => self.prefix = Prefix::G,
            Key::RunStop => self.start_program(),
            Key::Sst => {
                // Mostra a próxima linha enquanto a tecla está pressionada; executa ao soltar.
                self.end_entry();
                self.stack_lift = true;
                let line = self.next_line();
                self.step_line = Some(if line > self.program.len() { 0 } else { line });
            }
            _ => {}
        }
    }

    fn press_f(&mut self, key: Key) {
        match key {
            Key::F => self.prefix = Prefix::F,
            Key::G => self.prefix = Prefix::G,
            _ => {
                self.end_entry();
                match key {
                    k if k.digit().is_some() => {
                        self.format = DisplayFormat::Fix(k.digit().unwrap());
                        self.neutral = true;
                    }
                    Key::Dot => {
                        self.format = DisplayFormat::Sci;
                        self.neutral = true;
                    }
                    Key::RunStop => {
                        self.program_mode = true;
                        self.recording.clear();
                    }
                    // CLEAR PRGM fora do modo de programação só volta à linha 00.
                    Key::RollDown => self.pc = 0,
                    Key::N => self.amortize(),
                    Key::I => self.simple_interest(),
                    Key::Pv => self.net_present_value(),
                    Key::Fv => self.internal_rate_of_return(),
                    Key::Pow => self.bond_price(),
                    Key::Recip => self.bond_yield(),
                    Key::PctTotal => self.depreciate(finance::Depreciation::Sl),
                    Key::PctChange => self.depreciate(finance::Depreciation::Soyd),
                    Key::Pct => self.depreciate(finance::Depreciation::Db),
                    Key::Sst => self.clear_statistics(),
                    Key::Swap => self.financial = [HpDecimal::ZERO; 5],
                    Key::Clx => self.clear_registers(),
                    Key::Enter => self.show_mantissa = true,
                    Key::Pmt => {
                        let format = self.format;
                        self.unary(|x| Ok(display::rounded_to_display(x, format)));
                    }
                    _ => {} // CLEAR PRGM e funções financeiras: próximos marcos.
                }
            }
        }
    }

    fn press_g(&mut self, key: Key) {
        match key {
            Key::F => self.prefix = Prefix::F,
            Key::G => self.prefix = Prefix::G,
            _ => {
                self.end_entry();
                match key {
                    Key::D7 | Key::D8 | Key::D4 | Key::D5 => {
                        match key {
                            Key::D7 => self.begin_mode = true,
                            Key::D8 => self.begin_mode = false,
                            Key::D4 => self.dmy_mode = true,
                            _ => self.dmy_mode = false,
                        }
                        self.neutral = true;
                    }
                    Key::N => self.scale_financial(FinReg::N, |x| x.mul(HpDecimal::from_i64(12))),
                    Key::I => self.scale_financial(FinReg::I, |x| x.div(HpDecimal::from_i64(12))),
                    Key::Pv => self.cash_flow_initial(),
                    Key::Pmt => self.cash_flow_next(),
                    Key::Fv => self.cash_flow_count(),
                    Key::Chs => self.date_plus_days(),
                    Key::Eex => self.days_between(),
                    Key::SigmaPlus => self.accumulate(true),
                    Key::D0 => self.push_pair(stats::mean(&self.sums())),
                    Key::Dot => self.push_pair(stats::std_dev(&self.sums())),
                    Key::D6 => match stats::weighted_mean(&self.sums()) {
                        Ok(value) => self.recall(value),
                        Err(code) => self.error = Some(code),
                    },
                    Key::D1 => self.estimate(stats::estimate_x),
                    Key::D2 => self.estimate(stats::estimate_y),
                    Key::Swap => self.test(self.stack.x <= self.stack.y),
                    Key::Clx => self.test(self.stack.x.is_zero()),
                    Key::RollDown => self.prefix = Prefix::Goto(None),
                    Key::Sst => {
                        // BST: volta uma linha e a mostra enquanto a tecla está pressionada.
                        self.pc = if self.pc == 0 { self.program.len() } else { self.pc - 1 };
                        self.back_line = Some(self.pc);
                    }
                    Key::D9 => self.mem_view = true,
                    Key::Enter => self.recall(self.stack.last_x),
                    Key::Pow => self.unary(math::sqrt),
                    Key::Recip => self.unary(math::exp),
                    Key::PctTotal => self.unary(math::ln),
                    Key::PctChange => self.unary(|x| Ok(x.fract())),
                    Key::Pct => self.unary(|x| Ok(x.trunc())),
                    Key::D3 => self.unary(math::factorial),
                    Key::Mul => self.unary(math::square),
                    _ => {} // Próximos marcos.
                }
            }
        }
    }

    fn press_sto(&mut self, key: Key) {
        if let Some(d) = key.digit() {
            if self.register_ok(d as usize) {
                self.registers[d as usize] = self.stack.x;
                self.stack_lift = true;
            }
        } else if let Some(op) = ArithOp::from_key(key) {
            self.prefix = Prefix::StoArith(op);
        } else if let Some(reg) = FinReg::from_key(key) {
            self.financial[reg as usize] = self.stack.x;
            self.stack_lift = true;
        } else {
            match key {
                Key::Dot => self.prefix = Prefix::StoDot,
                // STO EEX liga/desliga juros compostos no período fracionário (indicador C).
                Key::Eex => {
                    self.compound = !self.compound;
                    self.neutral = true;
                }
                _ => {}
            }
        }
    }

    fn press_sto_arith(&mut self, op: ArithOp, key: Key) {
        let Some(d) = key.digit().filter(|d| *d <= REGISTER_ARITHMETIC_LIMIT) else {
            return;
        };
        let reg = &mut self.registers[d as usize];
        match op.apply(*reg, self.stack.x) {
            Ok(value) => {
                *reg = value;
                self.stack_lift = true;
            }
            Err(MathError::Overflow { .. }) => self.error = Some(ErrorCode::STORAGE_OVERFLOW),
            Err(MathError::DivideByZero | MathError::Domain) => self.error = Some(ErrorCode::MATH),
        }
    }

    fn press_rcl(&mut self, key: Key) {
        if let Some(d) = key.digit() {
            if self.register_ok(d as usize) {
                self.recall(self.registers[d as usize]);
            }
        } else if let Some(reg) = FinReg::from_key(key) {
            self.recall(self.financial[reg as usize]);
        } else if key == Key::Dot {
            self.prefix = Prefix::RclDot;
        } else if key == Key::G {
            self.prefix = Prefix::RclG;
        }
    }

    /// RCL g CFj recupera o fluxo atual e volta n uma posição; RCL g Nj recupera o Nj atual.
    fn press_rcl_g(&mut self, key: Key) {
        let Some(index) = self.flow_index() else {
            self.error = Some(ErrorCode::STORAGE_REGISTERS);
            return;
        };
        match key {
            Key::Pmt => {
                self.recall(self.registers[index]);
                if index > 0 {
                    self.financial[FinReg::N as usize] = HpDecimal::from_i64(index as i64 - 1);
                }
            }
            Key::Fv => self.recall(HpDecimal::from_i64(self.flow_counts[index] as i64)),
            _ => {}
        }
    }

    // ----- Estatística -----

    fn sums(&self) -> stats::Sums {
        let mut sums = [HpDecimal::ZERO; 6];
        sums.copy_from_slice(&self.registers[1..=6]);
        sums
    }

    /// Σ+ / Σ−: X é x, Y é y. Mostra n e desliga a elevação da pilha.
    fn accumulate(&mut self, remove: bool) {
        self.end_entry();
        let s = self.stack;
        match stats::accumulate(&self.sums(), s.x, s.y, remove) {
            Ok(sums) => {
                self.registers[1..=6].copy_from_slice(&sums);
                self.stack.last_x = s.x;
                self.stack.x = sums[0];
                self.stack_lift = false;
            }
            Err(code) => self.error = Some(code),
        }
    }

    /// x̄ e s: resultado de x em X e de y em Y.
    fn push_pair(&mut self, result: Result<(HpDecimal, HpDecimal), ErrorCode>) {
        match result {
            Ok((x_value, y_value)) => {
                self.recall(y_value);
                self.recall(x_value);
            }
            Err(code) => self.error = Some(code),
        }
    }

    /// x̂,r e ŷ,r: X (valor conhecido) → X estimativa, Y coeficiente de correlação r.
    fn estimate(&mut self, f: stats::Estimator) {
        let s = self.stack;
        match f(&self.sums(), s.x) {
            Ok((estimate, r)) => {
                self.stack = Stack { x: estimate, y: r, z: s.y, t: s.z, last_x: s.x };
                self.stack_lift = true;
            }
            Err(code) => self.error = Some(code),
        }
    }

    // ----- Finanças -----

    fn tvm(&self) -> finance::Tvm {
        finance::Tvm {
            n: self.financial[FinReg::N as usize],
            i: self.financial[FinReg::I as usize],
            pv: self.financial[FinReg::Pv as usize],
            pmt: self.financial[FinReg::Pmt as usize],
            fv: self.financial[FinReg::Fv as usize],
            begin: self.begin_mode,
            compound: self.compound,
        }
    }

    fn compute_tvm(&mut self, reg: FinReg) {
        let tvm = self.tvm();
        let result = match reg {
            FinReg::N => tvm.solve_n(),
            FinReg::I => tvm.solve_i(),
            FinReg::Pv => tvm.solve_pv(),
            FinReg::Pmt => tvm.solve_pmt(),
            FinReg::Fv => tvm.solve_fv(),
        };
        match result {
            Ok(value) => {
                self.financial[reg as usize] = value;
                self.recall(value);
            }
            Err(code) => self.error = Some(code),
        }
    }

    /// g 12× e g 12÷: converte X e guarda em n ou i.
    fn scale_financial(&mut self, reg: FinReg, f: impl FnOnce(HpDecimal) -> Result<HpDecimal, MathError>) {
        match saturate(f(self.stack.x)) {
            Ok(value) => {
                self.financial[reg as usize] = value;
                self.stack.x = value;
                self.stack_lift = true;
                self.fin_flag = true;
            }
            Err(_) => self.error = Some(ErrorCode::MATH),
        }
    }

    /// Índice do fluxo atual (parte inteira de n), de 0 a 19.
    fn flow_index(&self) -> Option<usize> {
        self.financial[FinReg::N as usize]
            .to_small_int()
            .filter(|n| (0..STORAGE_REGISTERS as i64).contains(n))
            .map(|n| n as usize)
    }

    fn cash_flow_initial(&mut self) {
        self.registers[0] = self.stack.x;
        self.flow_counts[0] = 1;
        self.financial[FinReg::N as usize] = HpDecimal::ZERO;
        self.stack_lift = true;
    }

    fn cash_flow_next(&mut self) {
        match self.flow_index().map(|i| i + 1).filter(|i| *i < self.registers_available()) {
            Some(index) => {
                self.registers[index] = self.stack.x;
                self.flow_counts[index] = 1;
                self.financial[FinReg::N as usize] = HpDecimal::from_i64(index as i64);
                self.stack_lift = true;
            }
            None => self.error = Some(ErrorCode::STORAGE_REGISTERS),
        }
    }

    fn cash_flow_count(&mut self) {
        let count = self.stack.x.to_small_int().filter(|c| (1..=99).contains(c));
        match (self.flow_index(), count) {
            (Some(index), Some(count)) => {
                self.flow_counts[index] = count as u8;
                self.stack_lift = true;
            }
            _ => self.error = Some(ErrorCode::STORAGE_REGISTERS),
        }
    }

    fn flows(&self) -> Option<Vec<(HpDecimal, u8)>> {
        let last = self.flow_index()?;
        Some((0..=last).map(|j| (self.registers[j], self.flow_counts[j])).collect())
    }

    fn net_present_value(&mut self) {
        let Some(flows) = self.flows() else {
            self.error = Some(ErrorCode::STORAGE_REGISTERS);
            return;
        };
        match finance::npv(self.financial[FinReg::I as usize], &flows) {
            Ok(value) => {
                self.financial[FinReg::Pv as usize] = value;
                self.recall(value);
            }
            Err(code) => self.error = Some(code),
        }
    }

    fn internal_rate_of_return(&mut self) {
        let Some(flows) = self.flows() else {
            self.error = Some(ErrorCode::STORAGE_REGISTERS);
            return;
        };
        match finance::irr(&flows) {
            Ok(value) => {
                self.financial[FinReg::I as usize] = value;
                self.recall(value);
            }
            Err(code) => self.error = Some(code),
        }
    }

    /// f AMORT: X = pagamentos → X juros, Y principal, Z pagamentos; atualiza PV e n.
    fn amortize(&mut self) {
        let s = self.stack;
        match finance::amortize(s.x, &self.tvm(), self.format) {
            Ok(a) => {
                self.financial[FinReg::Pv as usize] = a.balance;
                self.financial[FinReg::N as usize] = a.n;
                self.stack = Stack { x: a.interest, y: a.principal, z: s.x, t: s.y, last_x: s.last_x };
                self.stack_lift = true;
            }
            Err(code) => self.error = Some(code),
        }
    }

    /// f INT: X juros 360 dias, Y principal (−PV), Z juros 365 dias.
    fn simple_interest(&mut self) {
        let s = self.stack;
        let f = &self.financial;
        match finance::simple_interest(f[FinReg::N as usize], f[FinReg::I as usize], f[FinReg::Pv as usize]) {
            Ok((i360, principal, i365)) => {
                self.stack = Stack { x: i360, y: principal, z: i365, t: s.x, last_x: s.last_x };
                self.stack_lift = true;
            }
            Err(code) => self.error = Some(code),
        }
    }

    /// SL, SOYD, DB: X = ano → X depreciação, Y valor depreciável restante.
    fn depreciate(&mut self, kind: finance::Depreciation) {
        let s = self.stack;
        let f = &self.financial;
        let result = finance::depreciation(
            kind,
            s.x,
            f[FinReg::Pv as usize],
            f[FinReg::Fv as usize],
            f[FinReg::N as usize],
            f[FinReg::I as usize],
        );
        match result {
            Ok((dep, remaining)) => {
                self.stack = Stack { x: dep, y: remaining, z: s.y, t: s.z, last_x: s.last_x };
                self.stack_lift = true;
            }
            Err(code) => self.error = Some(code),
        }
    }

    fn read_dates(&self) -> Result<(Date, Date), ErrorCode> {
        Ok((
            Date::from_decimal(self.stack.y, self.dmy_mode)?,
            Date::from_decimal(self.stack.x, self.dmy_mode)?,
        ))
    }

    /// f PRICE: Y liquidação, X vencimento, i rendimento, PMT cupom → X preço, Y juros acumulados.
    fn bond_price(&mut self) {
        let s = self.stack;
        let result = self.read_dates().and_then(|(settle, maturity)| {
            finance::bond_price(settle, maturity, self.financial[FinReg::Pmt as usize], self.financial[FinReg::I as usize])
        });
        match result {
            Ok((price, accrued)) => {
                self.financial[FinReg::Pv as usize] = price;
                self.stack = Stack { x: price, y: accrued, last_x: s.x, ..s };
                self.stack_lift = true;
            }
            Err(code) => self.error = Some(code),
        }
    }

    /// f YTM: Y liquidação, X vencimento, PV preço, PMT cupom → X rendimento (também em i).
    fn bond_yield(&mut self) {
        let s = self.stack;
        let result = self.read_dates().and_then(|(settle, maturity)| {
            finance::bond_yield(settle, maturity, self.financial[FinReg::Pmt as usize], self.financial[FinReg::Pv as usize])
        });
        match result {
            Ok(ytm) => {
                self.financial[FinReg::I as usize] = ytm;
                self.stack = Stack { x: ytm, y: s.z, z: s.t, t: s.t, last_x: s.x };
                self.stack_lift = true;
            }
            Err(code) => self.error = Some(code),
        }
    }

    /// g DATE: Y data, X dias → X nova data (mostrada com o dia da semana).
    fn date_plus_days(&mut self) {
        let s = self.stack;
        let result = Date::from_decimal(s.y, self.dmy_mode).and_then(|date| {
            let days = s.x.trunc().to_small_int().ok_or(ErrorCode::CALENDAR)?;
            date.add_days(days)
        });
        match result {
            Ok(date) => {
                let value = date.to_decimal(self.dmy_mode);
                self.stack = Stack { x: value, y: s.z, z: s.t, t: s.t, last_x: s.x };
                self.stack_lift = true;
                self.date_view = Some((date.text(self.dmy_mode), date.weekday()));
            }
            Err(code) => self.error = Some(code),
        }
    }

    /// g ΔDYS: Y data inicial, X data final → X dias corridos, Y dias em base 30/360.
    fn days_between(&mut self) {
        let s = self.stack;
        match self.read_dates() {
            Ok((a, b)) => {
                let actual = HpDecimal::from_i64(dates::actual_days(a, b));
                let basis = HpDecimal::from_i64(dates::days_30_360(a, b));
                self.stack = Stack { x: actual, y: basis, last_x: s.x, ..s };
                self.stack_lift = true;
            }
            Err(code) => self.error = Some(code),
        }
    }

    // ----- Operações de pilha -----

    /// Empurra a pilha: T se perde, X é copiado para Y.
    fn push(&mut self) {
        let s = &mut self.stack;
        s.t = s.z;
        s.z = s.y;
        s.y = s.x;
    }

    /// Coloca um valor em X (RCL, LSTx), empurrando a pilha se permitido.
    fn recall(&mut self, value: HpDecimal) {
        if self.stack_lift {
            self.push();
        }
        self.stack.x = value;
        self.stack_lift = true;
    }

    fn binary(&mut self, op: ArithOp) {
        self.binary_fn(|y, x| op.apply(y, x));
    }

    /// Função de X: guarda X em LSTx e põe o resultado no lugar. Erro → Error 0.
    fn unary(&mut self, f: impl FnOnce(HpDecimal) -> Result<HpDecimal, MathError>) {
        self.end_entry();
        let x = self.stack.x;
        match saturate(f(x)) {
            Ok(result) => {
                self.stack.last_x = x;
                self.stack.x = result;
                self.stack_lift = true;
            }
            Err(_) => self.error = Some(ErrorCode::MATH),
        }
    }

    /// Função de Y e X que desce a pilha (+ − × ÷ yˣ).
    fn binary_fn(&mut self, f: impl FnOnce(HpDecimal, HpDecimal) -> Result<HpDecimal, MathError>) {
        self.end_entry();
        let s = self.stack;
        match saturate(f(s.y, s.x)) {
            Ok(result) => {
                // A pilha desce e T é duplicado em Z.
                self.stack = Stack { x: result, y: s.z, z: s.t, t: s.t, last_x: s.x };
                self.stack_lift = true;
            }
            Err(_) => self.error = Some(ErrorCode::MATH),
        }
    }

    // ----- Digitação -----

    fn entry_edit(&mut self, edit: impl FnOnce(&mut Entry)) {
        if self.entry.is_none() {
            if self.stack_lift {
                self.push();
            }
            self.stack_lift = true;
            self.entry = Some(Entry::default());
        }
        let entry = self.entry.as_mut().unwrap();
        edit(entry);
        self.stack.x = entry.value();
    }

    fn end_entry(&mut self) {
        self.entry = None;
    }

    // ----- CLEAR -----

    fn clear_statistics(&mut self) {
        for reg in &mut self.registers[1..=6] {
            *reg = HpDecimal::ZERO;
        }
        let last_x = self.stack.last_x;
        self.stack = Stack { last_x, ..Stack::default() };
    }

    fn clear_registers(&mut self) {
        self.stack = Stack::default();
        self.registers = [HpDecimal::ZERO; STORAGE_REGISTERS];
        self.financial = [HpDecimal::ZERO; 5];
        self.flow_counts = [1; STORAGE_REGISTERS];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ligar_com_ponto_troca_separador() {
        let mut calc = Calculator::new();
        calc.tap_all("1234.5 ENTER").unwrap();
        assert_eq!(calc.display().to_text(), "1,234.50");
        calc.tap(Key::On); // desliga
        assert_eq!(calc.display().to_text(), "");
        calc.press(Key::Dot);
        calc.tap(Key::On); // liga com "." pressionado
        calc.release(Key::Dot);
        assert_eq!(calc.display().to_text(), "1.234,50");
    }

    #[test]
    fn indicadores() {
        let mut calc = Calculator::new();
        calc.tap(Key::F);
        assert!(calc.display().annunciators.f);
        calc.tap(Key::G);
        let a = calc.display().annunciators;
        assert!(a.g && !a.f);
        calc.tap(Key::D7); // g BEG
        let a = calc.display().annunciators;
        assert!(a.begin && !a.g);
        calc.tap_all("g 4 STO EEX").unwrap();
        let a = calc.display().annunciators;
        assert!(a.dmy && a.compound);
    }

    #[test]
    fn prefix_mostra_mantissa_enquanto_segura() {
        let mut calc = Calculator::new();
        calc.tap_all("2 ENTER 3 / f").unwrap();
        calc.press(Key::Enter);
        assert_eq!(calc.display().to_text(), "6666666667");
        calc.release(Key::Enter);
        assert_eq!(calc.display().to_text(), "0.67");
    }

    #[test]
    fn indicador_prgm_e_listagem() {
        let mut calc = Calculator::new();
        calc.tap_all("f R/S").unwrap();
        assert!(calc.display().annunciators.prgm);
        calc.tap_all("STO 0 0 STO 1 RCL 0 STO + 1 1 STO - 0 RCL 0 g CLx g Rv 12 g Rv 04 RCL 1 f R/S").unwrap();
        assert!(!calc.display().annunciators.prgm);
        let listing = calc.program_listing();
        assert!(listing.contains("05 STO + 1\n"));
        assert!(listing.contains("10 g GTO 12\n"));

        // Volta pela listagem e roda igual.
        let mut other = Calculator::new();
        other.load_program(&listing).unwrap();
        assert_eq!(other.program_listing(), listing);
        other.tap_all("10 R/S").unwrap();
        assert_eq!(other.display().to_text(), "55.00");
        assert!(other.load_program("# HP-12C programa\n01 STO 9 9\n").is_err());
        // Comentários no fim da linha são ignorados.
        other.load_program("# HP-12C programa\n01 2       # dois\n02 *  # vezes\n").unwrap();
        assert_eq!(other.program_instructions().len(), 2);
    }

    #[test]
    fn editor_substitui_programa_e_realoca_memoria() {
        let mut calc = Calculator::new();
        calc.tap_all("42 STO . 9 7 STO . 8").unwrap();
        let mut lines: Vec<Instruction> =
            ["2", "*"].iter().map(|t| program::parse_instruction(t).unwrap()).collect();
        assert!(calc.registers_lost_by(lines.len()).is_empty());
        calc.set_program(lines.clone()).unwrap();
        calc.tap_all("5 R/S").unwrap();
        assert_eq!(calc.display().to_text(), "10.00");

        // 9 linhas ocupam o R.9 (que tinha 42); R.8 continua livre.
        lines.extend(std::iter::repeat_n(program::parse_instruction("1").unwrap(), 7));
        assert_eq!(calc.registers_lost_by(lines.len()), vec![19]);
        calc.set_program(lines).unwrap();
        assert_eq!(calc.registers_available(), 19);
        assert_eq!(calc.register(18), HpDecimal::from_i64(7));
        assert_eq!(calc.program_instructions().len(), 9);

        // Volta a 2 linhas: o R.9 é devolvido (zerado).
        calc.set_program(calc.program_instructions()[..2].to_vec()).unwrap();
        assert_eq!(calc.registers_available(), 20);
        assert_eq!(calc.register(19), HpDecimal::ZERO);
        assert!(calc.set_program(vec![Instruction::Goto(1); 100]).is_err());
    }

    #[test]
    fn programa_roda_aos_poucos_com_tick() {
        let mut calc = Calculator::new();
        calc.tap_all("f R/S g Rv 01 f R/S").unwrap(); // laço infinito
        calc.press(Key::RunStop);
        calc.release(Key::RunStop);
        calc.tick(16);
        assert!(calc.is_running());
        assert_eq!(calc.display().to_text(), "running");
        assert!(calc.display().blinking);
        calc.tap(Key::Clx); // qualquer tecla para
        assert!(!calc.is_running());
    }

    #[test]
    fn memoria_continua_ida_e_volta() {
        let mut calc = Calculator::new();
        calc.tap_all("f R/S STO 0 0 STO 1 RCL 0 STO + 1 1 STO - 0 RCL 0 g CLx g Rv 12 g Rv 04 RCL 1 f R/S").unwrap();
        calc.tap_all("1.5 EEX 3 CHS STO 7 3 g n 10.5 g i 5750 PV g 7 g 4 STO EEX f 4 123.456 ENTER 2 /").unwrap();
        let saved = calc.save_state();

        let mut restored = Calculator::load_state(&saved).unwrap();
        assert_eq!(restored.save_state(), saved);
        assert_eq!(restored.display().to_text(), calc.display().to_text());
        assert_eq!(restored.register(7), HpDecimal::parse("0.0015").unwrap());
        let a = restored.display().annunciators;
        assert!(a.begin && a.dmy && a.compound);
        // O programa e os registros financeiros voltaram.
        restored.tap_all("RCL PV f 2").unwrap();
        assert_eq!(restored.display().to_text(), "5,750.00");
        restored.tap_all("RCL n").unwrap();
        assert_eq!(restored.display().to_text(), "36.00");
        restored.tap_all("10 R/S").unwrap();
        assert_eq!(restored.display().to_text(), "55.00");
    }

    #[test]
    fn estado_invalido_e_recusado() {
        assert!(Calculator::load_state("").is_err());
        assert!(Calculator::load_state("{\"version\": 99}").is_err());
        let saved = Calculator::new().save_state();
        assert!(Calculator::load_state(&saved.replace("FIX 2", "FIX 12")).is_err());
        assert!(Calculator::load_state(&saved.replace("g GTO 00", "STO 9 9")).is_err());
    }

    fn power_on_holding(calc: &mut Calculator, key: Key) {
        calc.tap(Key::On);
        calc.press(key);
        calc.tap(Key::On);
        calc.release(key);
    }

    #[test]
    fn testes_de_inicializacao() {
        let mut calc = Calculator::new();
        power_on_holding(&mut calc, Key::Mul);
        let d = calc.display();
        assert_eq!(d.to_text(), "-8,8,8,8,8,8,8,8,8,8,");
        assert!(d.annunciators.prgm && d.annunciators.begin);
        calc.tap(Key::Clx);
        assert_eq!(calc.display().to_text(), "0.00");

        // Teclado: todas as teclas na ordem → "12"; fora de ordem → Error 9.
        power_on_holding(&mut calc, Key::Div);
        for key in KEYBOARD_TEST {
            calc.tap(key);
        }
        assert_eq!(calc.display().to_text(), "12");
        calc.tap(Key::Clx);
        power_on_holding(&mut calc, Key::Div);
        calc.tap(Key::I);
        assert_eq!(calc.display().to_text(), "Error 9");

        // ON + −: apaga tudo e mostra Pr Error.
        calc.tap_all("CLx 42 STO 3").unwrap();
        power_on_holding(&mut calc, Key::Sub);
        assert_eq!(calc.display().to_text(), "Pr Error");
        calc.tap_all("CLx RCL 3").unwrap();
        assert_eq!(calc.display().to_text(), "0.00");
    }

    #[test]
    fn lastx_guarda_o_operando() {
        let mut calc = Calculator::new();
        calc.tap_all("10 ENTER 4 -").unwrap();
        assert_eq!(calc.stack().last_x, HpDecimal::from_i64(4));
    }
}
