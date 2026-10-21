//! API C do núcleo da HP-12C, para uso em C# (P/Invoke) ou C.
//!
//! Regras:
//! - A calculadora é um ponteiro opaco criado por `hp12c_new`/`hp12c_load_state`
//!   e liberado com `hp12c_free`. Ponteiro nulo é aceito e ignorado.
//! - Teclas são índices de 0 a 38, na ordem física (linha a linha), iguais a `Key::ALL`.
//! - Textos de entrada e saída são UTF-8. Funções que devolvem texto escrevem em um
//!   buffer do chamador e retornam o tamanho completo em bytes (sem o '\0'); se o
//!   buffer for pequeno, o texto é cortado. Chame com buffer nulo para saber o tamanho.
//!
//! A descrição em C está em `include/hp12c.h`.

use hp12c_core::{Calculator, Key, Punct};
use std::ffi::{c_char, CStr};

/// Versão da API (muda quando a interface C muda de forma incompatível).
pub const API_VERSION: u32 = 1;

/// Estado do display em formato fixo, fácil de copiar para C#.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Hp12cDisplay {
    /// 1 se o sinal de menos está aceso.
    pub minus: u8,
    /// 1 se o display está piscando ("running").
    pub blinking: u8,
    /// Indicadores, um bit cada: 0 *, 1 f, 2 g, 3 BEGIN, 4 D.MY, 5 C, 6 PRGM.
    pub annunciators: u8,
    pub reserved: u8,
    /// Caractere de cada posição (código Unicode; ' ' = apagado).
    pub glyphs: [u32; 10],
    /// Pontuação de cada posição: 0 nenhuma, 1 ponto, 2 vírgula.
    pub punct: [u8; 10],
}

pub const ANN_LOW_BATTERY: u8 = 1 << 0;
pub const ANN_F: u8 = 1 << 1;
pub const ANN_G: u8 = 1 << 2;
pub const ANN_BEGIN: u8 = 1 << 3;
pub const ANN_DMY: u8 = 1 << 4;
pub const ANN_COMPOUND: u8 = 1 << 5;
pub const ANN_PRGM: u8 = 1 << 6;

fn key(index: u8) -> Option<Key> {
    Key::ALL.get(index as usize).copied()
}

/// Lê um texto UTF-8 terminado em '\0'.
///
/// # Safety
/// `text` deve ser nulo ou apontar para um texto válido terminado em '\0'.
unsafe fn read_str<'a>(text: *const c_char) -> Option<&'a str> {
    if text.is_null() {
        return None;
    }
    // SAFETY: garantido pelo chamador (documentado na função pública).
    unsafe { CStr::from_ptr(text) }.to_str().ok()
}

/// Copia `text` para o buffer (cortando se preciso, sempre terminando em '\0').
///
/// # Safety
/// `buffer` deve ser nulo ou apontar para `capacity` bytes graváveis.
unsafe fn write_str(text: &str, buffer: *mut c_char, capacity: usize) -> usize {
    let bytes = text.as_bytes();
    if !buffer.is_null() && capacity > 0 {
        let n = bytes.len().min(capacity - 1);
        // SAFETY: o chamador garante `capacity` bytes graváveis; copiamos n < capacity.
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), buffer.cast::<u8>(), n);
            *buffer.add(n) = 0;
        }
    }
    bytes.len()
}

/// # Safety
/// `calc` deve ser nulo ou um ponteiro válido devolvido por esta biblioteca.
unsafe fn calc_ref<'a>(calc: *const Calculator) -> Option<&'a Calculator> {
    // SAFETY: garantido pelo chamador.
    unsafe { calc.as_ref() }
}

/// # Safety
/// `calc` deve ser nulo ou um ponteiro válido devolvido por esta biblioteca.
unsafe fn calc_mut<'a>(calc: *mut Calculator) -> Option<&'a mut Calculator> {
    // SAFETY: garantido pelo chamador.
    unsafe { calc.as_mut() }
}

#[no_mangle]
pub extern "C" fn hp12c_api_version() -> u32 {
    API_VERSION
}

/// Nova calculadora ligada e zerada.
#[no_mangle]
pub extern "C" fn hp12c_new() -> *mut Calculator {
    Box::into_raw(Box::new(Calculator::new()))
}

/// Libera uma calculadora.
///
/// # Safety
/// `calc` deve ser nulo ou um ponteiro desta biblioteca ainda não liberado.
#[no_mangle]
pub unsafe extern "C" fn hp12c_free(calc: *mut Calculator) {
    if !calc.is_null() {
        // SAFETY: o ponteiro veio de Box::into_raw e não foi liberado (contrato).
        drop(unsafe { Box::from_raw(calc) });
    }
}

/// # Safety
/// `calc` deve ser nulo ou um ponteiro válido desta biblioteca.
#[no_mangle]
pub unsafe extern "C" fn hp12c_press(calc: *mut Calculator, key_index: u8) {
    if let (Some(c), Some(k)) = (unsafe { calc_mut(calc) }, key(key_index)) {
        c.press(k);
    }
}

/// # Safety
/// `calc` deve ser nulo ou um ponteiro válido desta biblioteca.
#[no_mangle]
pub unsafe extern "C" fn hp12c_release(calc: *mut Calculator, key_index: u8) {
    if let (Some(c), Some(k)) = (unsafe { calc_mut(calc) }, key(key_index)) {
        c.release(k);
    }
}

/// Avança um programa em execução; chame a cada quadro da interface.
///
/// # Safety
/// `calc` deve ser nulo ou um ponteiro válido desta biblioteca.
#[no_mangle]
pub unsafe extern "C" fn hp12c_tick(calc: *mut Calculator, elapsed_ms: u32) {
    if let Some(c) = unsafe { calc_mut(calc) } {
        c.tick(elapsed_ms);
    }
}

/// 1 se um programa está rodando.
///
/// # Safety
/// `calc` deve ser nulo ou um ponteiro válido desta biblioteca.
#[no_mangle]
pub unsafe extern "C" fn hp12c_is_running(calc: *const Calculator) -> u8 {
    unsafe { calc_ref(calc) }.is_some_and(|c| c.is_running()) as u8
}

/// Executa uma sequência como "12 ENTER 3 /". Retorna 0 se ok, −1 se houver tecla desconhecida.
///
/// # Safety
/// `calc` deve ser nulo ou válido; `sequence` deve ser nulo ou um texto terminado em '\0'.
#[no_mangle]
pub unsafe extern "C" fn hp12c_tap_all(calc: *mut Calculator, sequence: *const c_char) -> i32 {
    match (unsafe { calc_mut(calc) }, unsafe { read_str(sequence) }) {
        (Some(c), Some(seq)) => {
            if c.tap_all(seq).is_ok() {
                0
            } else {
                -1
            }
        }
        _ => -1,
    }
}

/// Índice da tecla pelo nome usado nas sequências ("ENTER", "x<>y", "7"…), ou −1.
///
/// # Safety
/// `token` deve ser nulo ou um texto terminado em '\0'.
#[no_mangle]
pub unsafe extern "C" fn hp12c_key_from_token(token: *const c_char) -> i32 {
    unsafe { read_str(token) }.and_then(Key::from_token).map_or(-1, |k| k.index() as i32)
}

/// Preenche `out` com o estado do display.
///
/// # Safety
/// `calc` deve ser nulo ou válido; `out` deve ser nulo ou apontar para um `Hp12cDisplay`.
#[no_mangle]
pub unsafe extern "C" fn hp12c_display(calc: *const Calculator, out: *mut Hp12cDisplay) {
    let (Some(c), Some(out)) = (unsafe { calc_ref(calc) }, unsafe { out.as_mut() }) else { return };
    *out = display_of(c);
}

fn display_of(c: &Calculator) -> Hp12cDisplay {
    let d = c.display();
    let a = d.annunciators;
    let bit = |on: bool, mask: u8| if on { mask } else { 0 };
    let mut result = Hp12cDisplay {
        minus: d.minus as u8,
        blinking: d.blinking as u8,
        annunciators: bit(a.low_battery, ANN_LOW_BATTERY)
            | bit(a.f, ANN_F)
            | bit(a.g, ANN_G)
            | bit(a.begin, ANN_BEGIN)
            | bit(a.dmy, ANN_DMY)
            | bit(a.compound, ANN_COMPOUND)
            | bit(a.prgm, ANN_PRGM),
        ..Hp12cDisplay::default()
    };
    for (i, cell) in d.cells.iter().enumerate() {
        result.glyphs[i] = cell.glyph as u32;
        result.punct[i] = match cell.punct {
            Punct::None => 0,
            Punct::Point => 1,
            Punct::Comma => 2,
        };
    }
    result
}

/// Texto do display (ex.: "-1,234.50").
///
/// # Safety
/// `calc` deve ser nulo ou válido; `buffer` nulo ou com `capacity` bytes graváveis.
#[no_mangle]
pub unsafe extern "C" fn hp12c_display_text(calc: *const Calculator, buffer: *mut c_char, capacity: usize) -> usize {
    let text = unsafe { calc_ref(calc) }.map(|c| c.display().to_text()).unwrap_or_default();
    unsafe { write_str(&text, buffer, capacity) }
}

/// Valor completo de X (ex.: "3.333333333E-1").
///
/// # Safety
/// `calc` deve ser nulo ou válido; `buffer` nulo ou com `capacity` bytes graváveis.
#[no_mangle]
pub unsafe extern "C" fn hp12c_x_text(calc: *const Calculator, buffer: *mut c_char, capacity: usize) -> usize {
    let text = unsafe { calc_ref(calc) }.map(|c| c.x().to_string()).unwrap_or_default();
    unsafe { write_str(&text, buffer, capacity) }
}

/// Estado completo em JSON (memória contínua).
///
/// # Safety
/// `calc` deve ser nulo ou válido; `buffer` nulo ou com `capacity` bytes graváveis.
#[no_mangle]
pub unsafe extern "C" fn hp12c_save_state(calc: *const Calculator, buffer: *mut c_char, capacity: usize) -> usize {
    let text = unsafe { calc_ref(calc) }.map(|c| c.save_state()).unwrap_or_default();
    unsafe { write_str(&text, buffer, capacity) }
}

/// Cria uma calculadora a partir de um estado salvo; nulo se o texto for inválido.
///
/// # Safety
/// `json` deve ser nulo ou um texto terminado em '\0'.
#[no_mangle]
pub unsafe extern "C" fn hp12c_load_state(json: *const c_char) -> *mut Calculator {
    match unsafe { read_str(json) }.map(Calculator::load_state) {
        Some(Ok(calc)) => Box::into_raw(Box::new(calc)),
        _ => std::ptr::null_mut(),
    }
}

/// Listagem do programa em texto.
///
/// # Safety
/// `calc` deve ser nulo ou válido; `buffer` nulo ou com `capacity` bytes graváveis.
#[no_mangle]
pub unsafe extern "C" fn hp12c_program_listing(calc: *const Calculator, buffer: *mut c_char, capacity: usize) -> usize {
    let text = unsafe { calc_ref(calc) }.map(|c| c.program_listing()).unwrap_or_default();
    unsafe { write_str(&text, buffer, capacity) }
}

/// Carrega uma listagem de programa. 0 se ok, −1 se inválida.
///
/// # Safety
/// `calc` deve ser nulo ou válido; `listing` nulo ou um texto terminado em '\0'.
#[no_mangle]
pub unsafe extern "C" fn hp12c_load_program(calc: *mut Calculator, listing: *const c_char) -> i32 {
    match (unsafe { calc_mut(calc) }, unsafe { read_str(listing) }) {
        (Some(c), Some(text)) => {
            if c.load_program(text).is_ok() {
                0
            } else {
                -1
            }
        }
        _ => -1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    fn text(f: impl Fn(*mut c_char, usize) -> usize) -> String {
        let len = f(std::ptr::null_mut(), 0);
        let mut buf = vec![0u8; len + 1];
        f(buf.as_mut_ptr().cast(), buf.len());
        String::from_utf8(buf[..len].to_vec()).unwrap()
    }

    #[test]
    fn ciclo_completo_pela_api_c() {
        unsafe {
            let calc = hp12c_new();
            let seq = CString::new("2 ENTER 3 /").unwrap();
            assert_eq!(hp12c_tap_all(calc, seq.as_ptr()), 0);
            assert_eq!(text(|b, n| hp12c_display_text(calc, b, n)), "0.67");
            assert_eq!(text(|b, n| hp12c_x_text(calc, b, n)), "6.666666667E-1");

            let mut d = Hp12cDisplay::default();
            hp12c_display(calc, &mut d);
            assert_eq!(d.glyphs[0], '0' as u32);
            assert_eq!(d.punct[0], 1);

            let enter = CString::new("ENTER").unwrap();
            assert_eq!(hp12c_key_from_token(enter.as_ptr()), Key::Enter.index() as i32);
            hp12c_press(calc, Key::F.index() as u8);
            hp12c_release(calc, Key::F.index() as u8);
            hp12c_display(calc, &mut d);
            assert_eq!(d.annunciators & ANN_F, ANN_F);

            let state = text(|b, n| hp12c_save_state(calc, b, n));
            let state_c = CString::new(state).unwrap();
            let copy = hp12c_load_state(state_c.as_ptr());
            assert!(!copy.is_null());
            assert_eq!(text(|b, n| hp12c_display_text(copy, b, n)), "0.67");

            let bad = CString::new("x").unwrap();
            assert!(hp12c_load_state(bad.as_ptr()).is_null());
            hp12c_free(copy);
            hp12c_free(calc);
            hp12c_free(std::ptr::null_mut());
        }
    }

    #[test]
    fn buffer_pequeno_corta_o_texto() {
        unsafe {
            let calc = hp12c_new();
            let mut buf = [0i8; 3];
            let len = hp12c_display_text(calc, buf.as_mut_ptr().cast(), buf.len());
            assert_eq!(len, 4); // "0.00"
            assert_eq!(CStr::from_ptr(buf.as_ptr().cast()).to_str().unwrap(), "0.");
            hp12c_free(calc);
        }
    }
}
