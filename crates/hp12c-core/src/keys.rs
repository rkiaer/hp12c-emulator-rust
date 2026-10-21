//! As 39 teclas da HP-12C, na ordem física (linha a linha, da esquerda para a direita).

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    // Linha 1
    N, I, Pv, Pmt, Fv, Chs, D7, D8, D9, Div,
    // Linha 2
    Pow, Recip, PctTotal, PctChange, Pct, Eex, D4, D5, D6, Mul,
    // Linha 3
    RunStop, Sst, RollDown, Swap, Clx, Enter, D1, D2, D3, Sub,
    // Linha 4 (ENTER ocupa a coluna 6 das linhas 3 e 4)
    On, F, G, Sto, Rcl, D0, Dot, SigmaPlus, Add,
}

pub const KEY_COUNT: usize = 39;

use Key::*;

impl Key {
    pub const ALL: [Key; KEY_COUNT] = [
        N, I, Pv, Pmt, Fv, Chs, D7, D8, D9, Div,
        Pow, Recip, PctTotal, PctChange, Pct, Eex, D4, D5, D6, Mul,
        RunStop, Sst, RollDown, Swap, Clx, Enter, D1, D2, D3, Sub,
        On, F, G, Sto, Rcl, D0, Dot, SigmaPlus, Add,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    /// Linha (1–4) e coluna (1–10) no teclado físico.
    pub fn row_col(self) -> (u8, u8) {
        let i = self.index();
        if i < 30 {
            ((i / 10 + 1) as u8, (i % 10 + 1) as u8)
        } else {
            let j = i - 30;
            let col = if j < 5 { j + 1 } else { j + 2 };
            (4, col as u8)
        }
    }

    /// Código de tecla da HP (o que aparece no modo de programação):
    /// linha×10 + coluna (coluna 10 vira 0); dígitos usam o próprio valor.
    pub fn code(self) -> u8 {
        if let Some(d) = self.digit() {
            return d;
        }
        let (row, col) = self.row_col();
        row * 10 + col % 10
    }

    pub fn digit(self) -> Option<u8> {
        match self {
            D0 => Some(0),
            D1 => Some(1),
            D2 => Some(2),
            D3 => Some(3),
            D4 => Some(4),
            D5 => Some(5),
            D6 => Some(6),
            D7 => Some(7),
            D8 => Some(8),
            D9 => Some(9),
            _ => None,
        }
    }

    pub fn from_digit(d: u8) -> Option<Key> {
        const DIGITS: [Key; 10] = [D0, D1, D2, D3, D4, D5, D6, D7, D8, D9];
        DIGITS.get(d as usize).copied()
    }

    /// Nome usado em sequências de teclas (testes, importação de programas).
    pub fn token(self) -> &'static str {
        match self {
            N => "n", I => "i", Pv => "PV", Pmt => "PMT", Fv => "FV", Chs => "CHS",
            D7 => "7", D8 => "8", D9 => "9", Div => "/",
            Pow => "y^x", Recip => "1/x", PctTotal => "%T", PctChange => "D%", Pct => "%",
            Eex => "EEX", D4 => "4", D5 => "5", D6 => "6", Mul => "*",
            RunStop => "R/S", Sst => "SST", RollDown => "Rv", Swap => "x<>y", Clx => "CLx",
            Enter => "ENTER", D1 => "1", D2 => "2", D3 => "3", Sub => "-",
            On => "ON", F => "f", G => "g", Sto => "STO", Rcl => "RCL",
            D0 => "0", Dot => ".", SigmaPlus => "S+", Add => "+",
        }
    }

    pub fn from_token(token: &str) -> Option<Key> {
        let alias = match token {
            "÷" => Some(Div),
            "×" | "x" => Some(Mul),
            "−" => Some(Sub),
            "R↓" => Some(RollDown),
            "x≷y" => Some(Swap),
            "Σ+" => Some(SigmaPlus),
            "Δ%" => Some(PctChange),
            _ => None,
        };
        alias.or_else(|| Key::ALL.iter().copied().find(|k| k.token() == token))
    }
}

/// Converte `"12.5 ENTER 3 *"` em teclas. Números com vários dígitos viram uma tecla por dígito.
pub fn parse_sequence(text: &str) -> Result<Vec<Key>, String> {
    let mut keys = Vec::new();
    for token in text.split_whitespace() {
        if let Some(key) = Key::from_token(token) {
            keys.push(key);
        } else if token.chars().all(|c| c.is_ascii_digit() || c == '.') {
            for c in token.chars() {
                keys.push(if c == '.' { Dot } else { Key::from_digit(c as u8 - b'0').unwrap() });
            }
        } else {
            return Err(format!("tecla desconhecida: `{token}`"));
        }
    }
    Ok(keys)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codigos_de_tecla_iguais_aos_da_hp() {
        assert_eq!(N.code(), 11);
        assert_eq!(Div.code(), 10);
        assert_eq!(Mul.code(), 20);
        assert_eq!(Enter.code(), 36);
        assert_eq!(Sub.code(), 30);
        assert_eq!(G.code(), 43);
        assert_eq!(Dot.code(), 48);
        assert_eq!(SigmaPlus.code(), 49);
        assert_eq!(Add.code(), 40);
        assert_eq!(D7.code(), 7);
    }

    #[test]
    fn ordem_bate_com_o_indice() {
        for (i, k) in Key::ALL.iter().enumerate() {
            assert_eq!(k.index(), i);
            assert_eq!(Key::from_token(k.token()), Some(*k));
        }
    }

    #[test]
    fn sequencia() {
        assert_eq!(parse_sequence("12 ENTER").unwrap(), vec![D1, D2, Enter]);
        assert!(parse_sequence("foo").is_err());
    }
}
