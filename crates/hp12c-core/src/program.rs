//! Memória de programa da HP-12C (marco M6).
//!
//! Cada linha guarda uma instrução completa: uma tecla ou uma sequência com
//! prefixo (ex.: STO + 1). Executar a linha é reproduzir essas teclas.

use crate::keys::Key;

/// Linhas sempre disponíveis.
pub const BASE_LINES: usize = 8;
/// Linhas que cada registrador convertido acrescenta.
pub const LINES_PER_REGISTER: usize = 7;
/// Máximo de linhas na HP-12C original.
pub const MAX_LINES: usize = 99;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Instruction {
    Keys(Vec<Key>),
    /// g GTO nn (as linhas vazias contêm GTO 00).
    Goto(u8),
}

impl Instruction {
    pub const EMPTY: Instruction = Instruction::Goto(0);

    /// Códigos de tecla como aparecem no display, ex.: "44 40 1" ou "43,33 05".
    pub fn codes(&self) -> String {
        match self {
            Instruction::Goto(line) => format!("43,33 {line:02}"),
            Instruction::Keys(keys) => keys.iter().map(|k| k.code().to_string()).collect::<Vec<_>>().join(" "),
        }
    }

    /// Instrução em texto para a listagem, ex.: "STO + 1" ou "g GTO 05".
    pub fn listing(&self) -> String {
        match self {
            Instruction::Goto(line) => format!("g GTO {line:02}"),
            Instruction::Keys(keys) => keys.iter().map(|k| k.token()).collect::<Vec<_>>().join(" "),
        }
    }
}

/// Resultado de interpretar as teclas digitadas no modo de programação.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Recorded {
    /// Falta tecla para completar.
    Pending,
    /// f ou g trocou o prefixo pendente.
    Restart(Key),
    /// Sequência descartada (ex.: f PREFIX).
    Cancel,
    Instruction(Instruction),
    Command(Command),
}

/// Teclas que agem no modo de programação em vez de serem gravadas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Command {
    /// f P/R: volta ao modo de execução.
    Exit,
    /// f CLEAR PRGM.
    Clear,
    /// SST: próxima linha.
    Step,
    /// g BST: linha anterior.
    Back,
    /// g MEM.
    Memory,
    /// g GTO . nn: vai para a linha nn.
    GotoLine(u8),
}

/// Linhas alocadas para um programa de `used` linhas: 8, 15, 22, … até 99.
pub fn capacity_for(used: usize) -> usize {
    if used <= BASE_LINES {
        BASE_LINES
    } else {
        BASE_LINES + (used - BASE_LINES).div_ceil(LINES_PER_REGISTER) * LINES_PER_REGISTER
    }
}

/// Ajusta os destinos de GTO depois de inserir (`delta = 1`) ou apagar (`delta = -1`)
/// a linha `line` (1 = primeira). GTO 00 nunca muda.
pub fn shift_gotos(lines: &mut [Instruction], line: usize, delta: i32) {
    for instruction in lines.iter_mut() {
        if let Instruction::Goto(target) = instruction {
            let t = *target as usize;
            let moves = if delta > 0 { t >= line } else { t > line };
            if t != 0 && moves {
                *target = (t as i32 + delta).clamp(0, MAX_LINES as i32) as u8;
            }
        }
    }
}

/// Troca as referências de GTO entre as linhas `a` e `b` (ao mover uma linha).
pub fn swap_gotos(lines: &mut [Instruction], a: usize, b: usize) {
    for instruction in lines.iter_mut() {
        if let Instruction::Goto(target) = instruction {
            if *target as usize == a {
                *target = b as u8;
            } else if *target as usize == b {
                *target = a as u8;
            }
        }
    }
}

/// Lê uma instrução no formato da listagem (ex.: "STO + 1", "g GTO 05").
pub fn parse_instruction(text: &str) -> Option<Instruction> {
    let tokens: Vec<&str> = text.split_whitespace().map(|t| if t == "GTO" { "Rv" } else { t }).collect();
    let keys = crate::keys::parse_sequence(&tokens.join(" ")).ok()?;
    match classify(&keys) {
        Recorded::Instruction(instruction) => Some(instruction),
        _ => None,
    }
}

fn two_digits(a: Key, b: Key) -> Option<u8> {
    Some(a.digit()? * 10 + b.digit()?)
}

fn is_arith(k: Key) -> bool {
    matches!(k, Key::Add | Key::Sub | Key::Mul | Key::Div)
}

fn is_fin(k: Key) -> bool {
    matches!(k, Key::N | Key::I | Key::Pv | Key::Pmt | Key::Fv)
}

/// Decide o que fazer com as teclas acumuladas no modo de programação.
pub(crate) fn classify(seq: &[Key]) -> Recorded {
    use Key::*;
    let done = || Recorded::Instruction(Instruction::Keys(seq.to_vec()));
    match seq {
        [F] | [G] | [Sto] | [Rcl] => Recorded::Pending,
        [F | G, k @ (F | G)] => Recorded::Restart(*k),
        [F, RunStop] => Recorded::Command(Command::Exit),
        [F, RollDown] => Recorded::Command(Command::Clear),
        [F, Enter] => Recorded::Cancel,
        [F, _] => done(),

        [G, Sst] => Recorded::Command(Command::Back),
        [G, D9] => Recorded::Command(Command::Memory),
        [G, RollDown] | [G, RollDown, Dot] => Recorded::Pending,
        [G, RollDown, Dot, d] if d.digit().is_some() => Recorded::Pending,
        [G, RollDown, Dot, a, b] => match two_digits(*a, *b) {
            Some(line) => Recorded::Command(Command::GotoLine(line)),
            None => Recorded::Cancel,
        },
        [G, RollDown, d] if d.digit().is_some() => Recorded::Pending,
        [G, RollDown, a, b] => match two_digits(*a, *b) {
            Some(line) => Recorded::Instruction(Instruction::Goto(line)),
            None => Recorded::Cancel,
        },
        [G, RollDown, ..] => Recorded::Cancel,
        [G, _] => done(),

        [Sto | Rcl, Dot] => Recorded::Pending,
        [Sto | Rcl, Dot, d] if d.digit().is_some() => done(),
        [Sto, op] if is_arith(*op) => Recorded::Pending,
        [Sto, op, d] if is_arith(*op) && d.digit().is_some_and(|v| v <= 4) => done(),
        [Sto | Rcl, k] if k.digit().is_some() || is_fin(*k) => done(),
        [Sto, Eex] => done(),
        [Rcl, G] => Recorded::Pending,
        [Rcl, G, Pmt | Fv] => done(),

        [Sst] => Recorded::Command(Command::Step),
        [On] => Recorded::Cancel,
        [_] => done(),
        _ => Recorded::Cancel,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Key::*;

    #[test]
    fn codigos_no_display() {
        assert_eq!(Instruction::Keys(vec![Sto, Add, D1]).codes(), "44 40 1");
        assert_eq!(Instruction::Keys(vec![Pct]).codes(), "25");
        assert_eq!(Instruction::Keys(vec![G, D0]).codes(), "43 0");
        assert_eq!(Instruction::Goto(5).codes(), "43,33 05");
    }

    #[test]
    fn capacidade_e_ajuste_de_gto() {
        assert_eq!(capacity_for(0), 8);
        assert_eq!(capacity_for(8), 8);
        assert_eq!(capacity_for(9), 15);
        assert_eq!(capacity_for(99), 99);

        let mut lines = vec![Instruction::Goto(3), Instruction::Goto(1), Instruction::Goto(0), Instruction::Goto(5)];
        shift_gotos(&mut lines, 2, 1); // inseriu na linha 2
        assert_eq!(lines, vec![Instruction::Goto(4), Instruction::Goto(1), Instruction::Goto(0), Instruction::Goto(6)]);
        shift_gotos(&mut lines, 2, -1); // apagou a linha 2
        assert_eq!(lines, vec![Instruction::Goto(3), Instruction::Goto(1), Instruction::Goto(0), Instruction::Goto(5)]);
        swap_gotos(&mut lines, 1, 3);
        assert_eq!(lines, vec![Instruction::Goto(1), Instruction::Goto(3), Instruction::Goto(0), Instruction::Goto(5)]);
    }

    #[test]
    fn classificacao() {
        assert_eq!(classify(&[Sto]), Recorded::Pending);
        assert_eq!(classify(&[Sto, Add, D1]), Recorded::Instruction(Instruction::Keys(vec![Sto, Add, D1])));
        assert_eq!(classify(&[Sto, Add, D7]), Recorded::Cancel);
        assert_eq!(classify(&[G, RollDown, D1, D2]), Recorded::Instruction(Instruction::Goto(12)));
        assert_eq!(classify(&[G, RollDown, Dot, D0, D3]), Recorded::Command(Command::GotoLine(3)));
        assert_eq!(classify(&[F, RunStop]), Recorded::Command(Command::Exit));
        assert_eq!(classify(&[F, G]), Recorded::Restart(G));
        assert_eq!(classify(&[D5]), Recorded::Instruction(Instruction::Keys(vec![D5])));
    }
}
