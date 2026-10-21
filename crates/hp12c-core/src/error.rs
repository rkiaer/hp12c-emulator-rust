//! Códigos de erro mostrados no display (`Error 0` … `Error 9`).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ErrorCode(pub u8);

impl ErrorCode {
    /// Operação matemática impossível (ex.: divisão por zero).
    pub const MATH: Self = Self(0);
    /// Estouro em aritmética de registrador (resultado > 9,999999999 × 10^99).
    pub const STORAGE_OVERFLOW: Self = Self(1);
    pub const STATISTICS: Self = Self(2);
    pub const IRR: Self = Self(3);
    pub const MEMORY: Self = Self(4);
    pub const COMPOUND_INTEREST: Self = Self(5);
    pub const STORAGE_REGISTERS: Self = Self(6);
    pub const IRR_NO_SIGN_CHANGE: Self = Self(7);
    pub const CALENDAR: Self = Self(8);
    pub const SERVICE: Self = Self(9);
    /// Memória contínua perdida (mostrado como "Pr Error").
    pub const POWER: Self = Self(100);

    pub fn message(&self) -> String {
        if *self == Self::POWER {
            "Pr Error".to_string()
        } else {
            format!("Error {}", self.0)
        }
    }
}
