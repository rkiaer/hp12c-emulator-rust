//! Legendas das teclas com uma marcação mínima:
//! - `y^x`  → "x" sobrescrito
//! - `CF_o` → "o" subscrito
//! - `x̄`, `x̂`, `ŷ` → barra/chapéu sobre o primeiro caractere (desenhados à parte,
//!   porque as fontes comuns não posicionam bem os acentos combinantes).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunKind {
    Normal,
    Superscript,
    Subscript,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Run {
    pub text: String,
    pub kind: RunKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    Bar,
    Hat,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    pub runs: Vec<Run>,
    /// Marca sobre o primeiro caractere.
    pub first_mark: Option<Mark>,
}

const COMBINING_MACRON: char = '\u{0304}';
const COMBINING_CIRCUMFLEX: char = '\u{0302}';

impl Label {
    pub fn parse(text: &str) -> Label {
        let mut runs: Vec<Run> = Vec::new();
        let mut first_mark = None;
        let mut kind = RunKind::Normal;
        let mut current = String::new();
        let mut chars_seen = 0;

        for ch in text.chars() {
            match ch {
                '^' | '_' => {
                    if !current.is_empty() {
                        runs.push(Run { text: std::mem::take(&mut current), kind });
                    }
                    kind = if ch == '^' { RunKind::Superscript } else { RunKind::Subscript };
                }
                COMBINING_MACRON | COMBINING_CIRCUMFLEX if chars_seen == 1 => {
                    first_mark = Some(if ch == COMBINING_MACRON { Mark::Bar } else { Mark::Hat });
                }
                // "ŷ" pré-composto (U+0177).
                'ŷ' if chars_seen == 0 => {
                    current.push('y');
                    chars_seen += 1;
                    first_mark = Some(Mark::Hat);
                }
                _ => {
                    current.push(ch);
                    chars_seen += 1;
                }
            }
        }
        if !current.is_empty() {
            runs.push(Run { text: current, kind });
        }
        Label { runs, first_mark }
    }

    /// Texto sem marcação (para acessibilidade e depuração).
    pub fn plain(&self) -> String {
        self.runs.iter().map(|r| r.text.as_str()).collect()
    }

    /// Texto para listas, sem desenho especial: "yˣ", "x²", "CFj", "x̄w", "ŷ,r".
    pub fn text(&self) -> String {
        let mut out = String::new();
        for run in &self.runs {
            match run.kind {
                RunKind::Superscript => out.extend(run.text.chars().map(|c| match c {
                    'x' => 'ˣ',
                    '2' => '²',
                    other => other,
                })),
                _ => out.push_str(&run.text),
            }
        }
        match (self.first_mark, out.chars().next()) {
            (Some(Mark::Hat), Some('y')) => out.replacen('y', "ŷ", 1),
            (Some(mark), Some(first)) => {
                let combining = if mark == Mark::Bar { COMBINING_MACRON } else { COMBINING_CIRCUMFLEX };
                let mut marked = String::from(first);
                marked.push(combining);
                marked.push_str(&out[first.len_utf8()..]);
                marked
            }
            _ => out,
        }
    }

    pub fn first_char(&self) -> Option<char> {
        self.runs.first().and_then(|r| r.text.chars().next())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sobrescrito_e_subscrito() {
        let l = Label::parse("y^x");
        assert_eq!(l.runs, vec![
            Run { text: "y".into(), kind: RunKind::Normal },
            Run { text: "x".into(), kind: RunKind::Superscript },
        ]);
        assert_eq!(Label::parse("CF_j").runs[1], Run { text: "j".into(), kind: RunKind::Subscript });
    }

    #[test]
    fn texto_legivel() {
        assert_eq!(Label::parse("y^x").text(), "yˣ");
        assert_eq!(Label::parse("x^2").text(), "x²");
        assert_eq!(Label::parse("CF_j").text(), "CFj");
        assert_eq!(Label::parse("ŷ,r").text(), "ŷ,r");
        assert_eq!(Label::parse("x̄w").text(), "x\u{0304}w");
    }

    #[test]
    fn barra_sobre_o_x() {
        let l = Label::parse("x̄w");
        assert_eq!(l.plain(), "xw");
        assert_eq!(l.first_mark, Some(Mark::Bar));
        assert_eq!(Label::parse("ŷ,r").first_mark, Some(Mark::Hat));
    }
}
