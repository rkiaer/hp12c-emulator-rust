# Guia de desenvolvimento

## Estrutura

| Pasta | O que tem |
|---|---|
| `crates/hp12c-core` | Lógica da calculadora: decimal de 10 dígitos, pilha, matemática, finanças, estatística, datas, programação, memória contínua (JSON). Sem dependências e sem `unsafe` |
| `crates/hp12c-skin` | Geometria, cores e legendas lidas de `assets/skin.json`. Não depende de biblioteca gráfica |
| `crates/hp12c-desktop` | Aplicativo (eframe/egui): desenho vetorial, mouse, teclado, menu, janelas de registradores e de programa |
| `crates/hp12c-ffi` | DLL com API C (`include/hp12c.h`) para usar o núcleo a partir de C# ou C |
| `csharp/` | Biblioteca `Hp12c.Interop`, executor dos vetores em C# e protótipo WPF |
| `tests/vectors` | 240 casos "teclas → display esperado", usados pelo Rust e pelo C# |
| `assets/skin.json` | Aparência da calculadora, em milímetros |
| `docs/` | Documento de design (SDD), manual ilustrado e este guia |

## Compilar no Windows

O repositório fixa a toolchain **GNU** do Rust em `rust-toolchain.toml`, para não exigir o C++ do
Visual Studio. Ela precisa do MinGW-w64 para gerar bibliotecas de importação:

```bash
winget install --id BrechtSanders.WinLibs.POSIX.UCRT -e
```

Depois, no `%USERPROFILE%\.cargo\config.toml`, aponte o `dlltool` do MinGW:

```toml
[target.x86_64-pc-windows-gnu]
rustflags = ["-C", "dlltool=<pasta do WinLibs>\\mingw64\\bin\\dlltool.exe"]
```

Quem já tem a carga "Desenvolvimento para desktop com C++" do Visual Studio pode usar a toolchain
MSVC: troque o conteúdo de `rust-toolchain.toml` por `channel = "stable"`.

## Linux e macOS

Troque `rust-toolchain.toml` por `channel = "stable"`. No Linux, o egui precisa das bibliotecas de
janela do sistema (por exemplo, no Debian/Ubuntu: `libxkbcommon-dev libgl1-mesa-dev libx11-dev`).
Essas plataformas compilam, mas ainda não foram testadas a fundo.

## Testes

```bash
cargo test --workspace
```

Os vetores em `tests/vectors/*.json` descrevem o comportamento esperado (formato em
[tests/vectors/README.md](../tests/vectors/README.md)). Para corrigir ou mudar um comportamento,
escreva primeiro o caso de teste.

Para experimentar o núcleo sem interface:

```bash
cargo run -p hp12c-core --example repl
```

O aplicativo aceita `--keys "12 ENTER 3 /"` (executa teclas ao abrir) e a variável de ambiente
`HP12C_CONFIG_DIR` (troca a pasta da memória salva), úteis em testes e demonstrações.

## Pacote para distribuir

```bash
pwsh scripts/release.ps1
```

Roda os testes, compila em modo release e gera `dist/HP12C-<versão>-windows.zip`. O executável só
depende de DLLs do próprio Windows.

## C# (.NET 10)

A lógica continua no núcleo em Rust; o C# chama a DLL via P/Invoke.

```bash
cargo build --release -p hp12c-ffi
dotnet run --project csharp/Hp12c.VectorTests
dotnet run --project csharp/Hp12c.Wpf
```

- `Hp12c.Interop`: classe `Calculator` (IDisposable) sobre a DLL.
- `Hp12c.VectorTests`: roda `tests/vectors` pelo C# e precisa dar 240 de 240.
- `Hp12c.Wpf`: protótipo de interface com o mesmo `skin.json`.
  `HP12C.Wpf.exe --snapshot saida.png "12 ENTER 3 /"` gera uma imagem sem abrir janela.

## Formato dos programas em texto

"Copiar programa" (menu) gera uma listagem; colar com Ctrl+V um texto que comece com `# HP-12C`
carrega o programa:

```text
# HP-12C programa
01 STO 0
02 0
03 STO 1
04 RCL 0
05 STO + 1
06 1
07 STO - 0
08 RCL 0
09 g CLx
10 g GTO 12
11 g GTO 04
12 RCL 1
```

Soma de 1 a N: digite N e pressione R/S. `g CLx` é o teste x=0 e `g x<>y` é x≤y.
