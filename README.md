# Emulador HP-12C

Emulador da calculadora financeira HP-12C em Rust. Design: [docs/SDD-HP12C.md](docs/SDD-HP12C.md).

![HP-12C](docs/m2-visual.png)

## Estado

- **M1 — Núcleo básico: concluído.** Aritmética decimal de 10 dígitos, pilha RPN, entrada de
  números (EEX, CHS), formatos FIX/SCI, STO/RCL com aritmética de registrador, CLEAR, LSTx e
  mensagens de erro.
- **M2 — Visual: concluído.** Janela sem bordas com a calculadora desenhada em vetor,
  display de 7 segmentos, mouse, teclado do PC, menu (botão direito), zoom e área de transferência.
- **M3 — Matemática: concluído.** yˣ, 1/x, √x, eˣ, LN, n!, x², FRAC, INTG, RND, %, Δ%, %T e LSTx.
  Contas internas com 19 dígitos, arredondadas para os 10 da calculadora.
- **M4 — Financeiro: concluído.** n, i, PV, PMT, FV (BEGIN/END, período fracionário), 12×, 12÷,
  CFo/CFj/Nj, NPV, IRR, AMORT, INT, SL/SOYD/DB, PRICE/YTM, DATE e ΔDYS.
- **M5 — Estatística: concluído.** Σ+, Σ−, x̄, s, x̄w, x̂,r e ŷ,r (somatórios em R1–R6).
- **M6 — Programação: concluído.** P/R, R/S, SST, BST, GTO, GTO ., PSE, x≤y, x=0, MEM e
  CLEAR PRGM. Até 99 linhas, compartilhando memória com os registradores, como na 12C original.
  Programas podem ser copiados e colados como texto (menu do botão direito / Ctrl+V).
- **M7 — Acabamento: concluído.** Memória contínua (o estado é salvo sozinho e volta ao abrir),
  zoom, "sempre no topo" e posição da janela lembrados, testes de inicialização, ícone e pacote
  portátil (`scripts/release.ps1`).
- **M8 — C#: concluído (protótipo).** DLL `hp12c_ffi.dll` com API C, biblioteca C#
  `Hp12c.Interop`, os 240 vetores aprovados também pelo C#, e um protótipo WPF que usa o
  mesmo `skin.json`.

## Usar

```bash
cargo run --release
```

- **Mouse:** clique nas teclas. Clique fora das teclas e arraste para mover a janela.
- **Botão direito:** copiar X, zoom (75–300%), sempre no topo, ligar/desligar, reiniciar, sair.
- **Teclado do PC:**

| PC | HP-12C | PC | HP-12C |
|---|---|---|---|
| 0–9, `.` `,` | dígitos, ponto | Enter | ENTER |
| `+ - * /` | + − × ÷ | Backspace / Delete | CLx |
| F / G | f / g | Tab | x≷y |
| S / R | STO / RCL | D | R↓ |
| N, I, P, M, V | n, i, PV, PMT, FV | E | EEX |
| C | CHS | Esc | cancela f/g/STO/RCL |
| Ctrl+C / Ctrl+V | copia X / cola em X | | |

### Memória contínua

Como na calculadora real, nada se perde ao fechar: pilha, registradores, programa e modos
são salvos em `%APPDATA%\HP12C\state.json` (Linux/macOS: `~/.config/hp12c`). Se o arquivo
estiver corrompido, a calculadora começa zerada e mostra `Pr Error` (o arquivo antigo fica
como `state.json.bad`).

### Ligar segurando uma tecla

No botão direito, **Ligar segurando…** reproduz as combinações da máquina:

| Combinação | O que faz |
|---|---|
| ON + × | Autoteste: acende todos os segmentos e indicadores |
| ON + ÷ | Teste de teclado: pressione todas as teclas da esquerda para a direita, de cima para baixo (ENTER duas vezes, nas linhas 3 e 4); termina com `12`, ou `Error 9` se errar a ordem |
| ON + . | Troca ponto/vírgula decimal |
| ON + − | Apaga toda a memória e mostra `Pr Error` |

### Pacote para distribuir

```bash
pwsh scripts/release.ps1
```

Gera `dist/HP12C-<versão>-windows.zip` com o `HP12C.exe` portátil (só depende de DLLs do
Windows; não precisa instalar nada).

### Programas em texto

"Copiar programa" (botão direito) gera uma listagem como esta; colar (Ctrl+V) um texto que
comece com `# HP-12C` carrega o programa:

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

(Soma de 1 a N: digite N e pressione R/S. `g CLx` é o teste x=0; `g x<>y` é x≤y.)

## Desenvolvimento

```bash
cargo test --workspace
```

```bash
cargo run -p hp12c-core --example repl
```

O `repl` é uma calculadora no terminal, útil para testar o núcleo sem interface.

### Ambiente (Windows)

O projeto usa a toolchain GNU do Rust (fixada em `rust-toolchain.toml`), sem precisar do C++ do
Visual Studio. Ela precisa do MinGW-w64 para gerar bibliotecas de importação:

```bash
winget install --id BrechtSanders.WinLibs.POSIX.UCRT -e
```

E, no `~/.cargo/config.toml` do usuário, apontar o `dlltool` do MinGW:

```toml
[target.x86_64-pc-windows-gnu]
rustflags = ["-C", "dlltool=<pasta do WinLibs>\\mingw64\\bin\\dlltool.exe"]
```

## C# (.NET 10)

Toda a lógica continua no núcleo em Rust; o C# chama a DLL via P/Invoke.

```bash
cargo build --release -p hp12c-ffi
```

```bash
dotnet run --project csharp/Hp12c.VectorTests
```

```bash
dotnet run --project csharp/Hp12c.Wpf
```

- `crates/hp12c-ffi` — DLL com a API C (`include/hp12c.h`): criar/liberar, teclas, display,
  textos, estado salvo e programas. Único lugar com código `unsafe`.
- `csharp/Hp12c.Interop` — classe `Calculator` (IDisposable) sobre a DLL.
- `csharp/Hp12c.VectorTests` — roda `tests/vectors` pelo C# (precisa dar 240 de 240).
- `csharp/Hp12c.Wpf` — protótipo de interface. `HP12C.Wpf.exe --snapshot saida.png "12 ENTER 3 /"`
  gera uma imagem sem abrir janela. Usa o mesmo `state.json` da versão Rust (não rode as duas ao
  mesmo tempo: a última a fechar grava por cima).

## Estrutura

- `crates/hp12c-core` — lógica da calculadora, sem dependências externas.
- `crates/hp12c-skin` — geometria, cores e legendas (lê `assets/skin.json`), sem biblioteca gráfica.
- `crates/hp12c-desktop` — aplicativo (eframe/egui): desenho, mouse, teclado e menu.
- `assets/skin.json` — aparência da calculadora (milímetros), reutilizável no futuro porte C#.
- `tests/vectors` — vetores de teste (teclas → display esperado), compartilhados com o porte C#.
