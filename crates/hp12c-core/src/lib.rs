//! Núcleo lógico do emulador HP-12C.
//!
//! A interface gráfica só precisa de duas coisas:
//! enviar teclas ([`Calculator::press`] / [`Calculator::release`]) e
//! desenhar o resultado de [`Calculator::display`].

#![forbid(unsafe_code)]

pub mod dates;
pub mod decimal;
pub mod finance;
mod json;
pub mod display;
mod entry;
pub mod error;
pub mod keys;
pub mod machine;
pub mod math;
pub mod program;
pub mod stats;
mod wide;

pub use decimal::{HpDecimal, MathError};
pub use display::{Annunciators, Cell, DisplayFormat, DisplayState, Punct};
pub use error::ErrorCode;
pub use keys::Key;
pub use machine::{Calculator, FinReg, Stack};
