//! Embute no .exe do Windows o ícone (visto no Explorer, na área de trabalho e nos atalhos)
//! e os dados de versão (Propriedades → Detalhes).
//!
//! Toolchain GNU: usa o windres do MinGW-w64. Ele é procurado na variável WINDRES, ao lado
//! do dlltool configurado em -C dlltool=..., ou no PATH. Sem windres, o build continua sem
//! ícone e mostra um aviso.

#[path = "src/icon_art.rs"]
mod icon_art;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const SIZES: [u32; 6] = [16, 24, 32, 48, 64, 256];

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/icon_art.rs");
    println!("cargo:rerun-if-env-changed=WINDRES");

    let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if os != "windows" {
        return;
    }
    if target_env != "gnu" {
        println!("cargo:warning=ícone do .exe só é gerado na toolchain GNU (windres)");
        return;
    }

    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let ico = out.join("hp12c.ico");
    fs::write(&ico, ico_file()).expect("gravar hp12c.ico");

    let rc = out.join("hp12c.rc");
    fs::write(&rc, resource_script(&ico)).expect("gravar hp12c.rc");

    let obj = out.join("hp12c-res.o");
    let windres = find_windres();
    let mut cmd = Command::new(&windres);
    // O windres chama o pré-processador (gcc) da mesma pasta.
    if let Some(dir) = windres.parent().filter(|d| !d.as_os_str().is_empty()) {
        let path = env::var_os("PATH").unwrap_or_default();
        let mut dirs = vec![dir.to_path_buf()];
        dirs.extend(env::split_paths(&path));
        cmd.env("PATH", env::join_paths(dirs).unwrap());
    }
    cmd.arg("-i").arg(&rc).arg("-o").arg(&obj).arg("-O").arg("coff");
    match cmd.status() {
        Ok(status) if status.success() => {
            println!("cargo:rustc-link-arg-bins={}", obj.display());
        }
        Ok(status) => println!("cargo:warning=windres falhou ({status}); .exe sem ícone"),
        Err(e) => println!("cargo:warning=windres não encontrado ({e}); .exe sem ícone"),
    }
}

fn find_windres() -> PathBuf {
    if let Some(path) = env::var_os("WINDRES") {
        return PathBuf::from(path);
    }
    // Mesmo MinGW do dlltool configurado em ~/.cargo/config.toml.
    let flags = env::var("CARGO_ENCODED_RUSTFLAGS").unwrap_or_default();
    for flag in flags.split('\x1f') {
        if let Some(dlltool) = flag.strip_prefix("dlltool=") {
            let candidate = Path::new(dlltool).with_file_name("windres.exe");
            if candidate.exists() {
                return candidate;
            }
        }
    }
    PathBuf::from("windres")
}

fn resource_script(ico: &Path) -> String {
    let version = env::var("CARGO_PKG_VERSION").unwrap();
    let mut parts: Vec<u16> = version.split('.').map(|p| p.parse().unwrap_or(0)).collect();
    parts.resize(4, 0);
    let commas = parts.iter().map(u16::to_string).collect::<Vec<_>>().join(",");
    // O windres aceita barras normais; aspas e barras invertidas precisariam de escape.
    let ico = ico.display().to_string().replace('\\', "/");
    format!(
        r#"1 ICON "{ico}"
1 VERSIONINFO
FILEVERSION {commas}
PRODUCTVERSION {commas}
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "041604B0"
    BEGIN
      VALUE "FileDescription", "Emulador HP-12C"
      VALUE "ProductName", "HP-12C Emulator"
      VALUE "FileVersion", "{version}"
      VALUE "ProductVersion", "{version}"
      VALUE "OriginalFilename", "HP12C.exe"
      VALUE "LegalCopyright", "MIT License"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x0416, 1200
  END
END
"#
    )
}

/// Arquivo .ico com uma imagem BMP de 32 bits (com alfa) para cada tamanho.
fn ico_file() -> Vec<u8> {
    let images: Vec<Vec<u8>> = SIZES.iter().map(|&s| bmp_image(s)).collect();
    let mut ico = Vec::new();
    ico.extend(0u16.to_le_bytes()); // reservado
    ico.extend(1u16.to_le_bytes()); // tipo: ícone
    ico.extend((SIZES.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * SIZES.len() as u32;
    for (size, image) in SIZES.iter().zip(&images) {
        let side = if *size >= 256 { 0 } else { *size as u8 };
        ico.extend([side, side, 0, 0]);
        ico.extend(1u16.to_le_bytes()); // planos
        ico.extend(32u16.to_le_bytes()); // bits por pixel
        ico.extend((image.len() as u32).to_le_bytes());
        ico.extend(offset.to_le_bytes());
        offset += image.len() as u32;
    }
    for image in images {
        ico.extend(image);
    }
    ico
}

fn bmp_image(size: u32) -> Vec<u8> {
    let rgba = icon_art::render(size);
    let mask_row = size.div_ceil(32) * 4;
    let pixels_len = size * size * 4;
    let mut bmp = Vec::new();
    bmp.extend(40u32.to_le_bytes());
    bmp.extend((size as i32).to_le_bytes());
    bmp.extend((size as i32 * 2).to_le_bytes()); // cor + máscara
    bmp.extend(1u16.to_le_bytes());
    bmp.extend(32u16.to_le_bytes());
    bmp.extend(0u32.to_le_bytes()); // sem compressão
    bmp.extend((pixels_len + mask_row * size).to_le_bytes());
    bmp.extend([0u8; 16]);
    // BGRA, de baixo para cima.
    for y in (0..size).rev() {
        for x in 0..size {
            let i = ((y * size + x) * 4) as usize;
            bmp.extend([rgba[i + 2], rgba[i + 1], rgba[i], rgba[i + 3]]);
        }
    }
    // Máscara AND zerada: a transparência vem do canal alfa.
    bmp.extend(vec![0u8; (mask_row * size) as usize]);
    bmp
}
