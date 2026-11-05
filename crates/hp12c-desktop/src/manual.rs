//! Manual ilustrado embutido no executável (F1).
//!
//! O HTML e a foto vão dentro do .exe. Ao abrir, são gravados na pasta `manual`, ao lado
//! do executável (ou em %APPDATA%\HP12C), e abertos no navegador padrão, que mostra o mapa
//! interativo do teclado.

use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::Command;

const HTML: &str = include_str!("../../../docs/manual-usuario.html");
const PHOTO: &[u8] = include_bytes!("../../../docs/m2-visual.png");
const PHOTO_NAME: &str = "m2-visual.png";

/// Grava o manual (se mudou) e abre no navegador.
pub fn open() -> io::Result<()> {
    let dir = crate::storage::config_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("manual");
    fs::create_dir_all(&dir)?;
    let page = dir.join("manual-usuario.html");
    write_if_changed(&page, HTML.as_bytes())?;
    write_if_changed(&dir.join(PHOTO_NAME), PHOTO)?;
    launch(page)
}

fn write_if_changed(path: &PathBuf, bytes: &[u8]) -> io::Result<()> {
    if fs::read(path).is_ok_and(|old| old == bytes) {
        return Ok(());
    }
    fs::write(path, bytes)
}

fn launch(page: PathBuf) -> io::Result<()> {
    let mut cmd = if cfg!(windows) {
        // O Explorer abre o arquivo no programa padrão para .html, sem janela de console.
        let mut c = Command::new("explorer");
        c.arg(page);
        c
    } else if cfg!(target_os = "macos") {
        let mut c = Command::new("open");
        c.arg(page);
        c
    } else {
        let mut c = Command::new("xdg-open");
        c.arg(page);
        c
    };
    // O explorer.exe devolve código 1 mesmo quando abre o arquivo; basta ter iniciado.
    cmd.spawn().map(|_| ())
}

#[cfg(test)]
mod tests {
    #[test]
    fn manual_embutido_aponta_para_a_foto() {
        assert!(super::HTML.contains(&format!("src=\"{}\"", super::PHOTO_NAME)));
        assert!(super::PHOTO.starts_with(b"\x89PNG"));
    }
}
