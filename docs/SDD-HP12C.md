# SDD — Emulador de Calculadora Financeira HP-12C

| Item | Valor |
|---|---|
| Documento | Software Design Document (SDD) |
| Projeto | Emulador HP-12C para desktop |
| Versão | 0.1 — rascunho para aprovação |
| Data | 21/10/2026 |
| Linguagem | Rust (stable, edição 2021) — porte futuro para C# |
| Status | **Aprovado** (21/10/2026) — marcos M1 a M8 implementados (M8 como protótipo WPF; bindings C# escritos à mão em vez de csbindgen) |

---

## 1. Objetivo

Construir um emulador **fiel** da calculadora financeira HP-12C para rodar no computador (Windows primeiro, Linux/macOS depois), com:

1. **Visual idêntico** ao da calculadora real (layout horizontal, teclas pretas, legendas douradas `f` e azuis `g`, faixa superior metálica, display LCD de 7 segmentos com indicadores).
2. **Comportamento idêntico**: lógica RPN, pilha de 4 níveis, aritmética decimal de 10 dígitos, funções financeiras, estatísticas, datas, programação e mensagens de erro iguais às do manual do proprietário.
3. **Núcleo portável**: a lógica da calculadora fica isolada da interface gráfica, para ser portada para C# (ou chamada a partir de C#) sem reescrever a UI e sem divergência de resultados.

### 1.1 Fora do escopo (versão 1.0)

- Modo algébrico (ALG) da HP-12C Platinum — fica para uma versão futura (ver §12).
- Emulação do microcódigo original (ROM da HP). O emulador será uma **reimplementação de alto nível** (comportamento), não um emulador de CPU. Motivo: a ROM é propriedade da HP e não pode ser distribuída.
- Versões mobile/web.

---

## 2. Referência funcional (o que a HP-12C faz)

### 2.1 Teclado — 39 teclas, 4 linhas × 10 colunas (`ENTER` ocupa 2 linhas)

Legenda: **principal** (branco na tecla) / *f* (dourado, acima da tecla) / *g* (azul, na face inferior da tecla).

| Col → | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 |
|---|---|---|---|---|---|---|---|---|---|---|
| **Linha 1** | n / AMORT / 12× | i / INT / 12÷ | PV / NPV / CFo | PMT / RND / CFj | FV / IRR / Nj | CHS / — / DATE | 7 / — / BEG | 8 / — / END | 9 / — / MEM | ÷ / — / — |
| **Linha 2** | yˣ / PRICE / √x | 1/x / YTM / eˣ | %T / SL / LN | Δ% / SOYD / FRAC | % / DB / INTG | EEX / — / ΔDYS | 4 / — / D.MY | 5 / — / M.DY | 6 / — / x̄w | × / — / x² |
| **Linha 3** | R/S / P/R / PSE | SST / Σ / BST | R↓ / PRGM / GTO | x≷y / FIN / x≤y | CLx / REG / x=0 | ENTER / PREFIX / LSTx | 1 / — / x̂,r | 2 / — / ŷ,r | 3 / — / n! | − / — / — |
| **Linha 4** | ON | f | g | STO | RCL | *(ENTER)* | 0 / — / x̄ | . / — / s | Σ+ / — / Σ− | + / — / — |

Agrupamentos impressos no painel (colchetes dourados): **BOND** (PRICE, YTM), **DEPRECIATION** (SL, SOYD, DB), **CLEAR** (Σ, PRGM, FIN, REG).

> A tabela será conferida tecla a tecla contra o *HP 12C Owner's Handbook* antes da implementação da UI (tarefa M2-01).

### 2.2 Display

- LCD de 10 dígitos de 7 segmentos, cada um com ponto/vírgula; sinal negativo à esquerda.
- Indicadores (annunciators): `*` (bateria), `f`, `g`, `BEGIN`, `D.MY`, `C`, `PRGM`.
- Formatos: **FIX 0–9** (`f` + dígito) e **científico** (`f .`). Mantissa completa com `f PREFIX` (mostra os 10 dígitos enquanto a tecla é segurada).
- Separador decimal ponto ↔ vírgula (ligar com `.` pressionado).
- Mensagem `running` piscando durante cálculos longos (IRR, i).
- Mensagens: `Error 0` a `Error 9` e `Pr Error`.

### 2.3 Modelo de memória

| Elemento | Descrição |
|---|---|
| Pilha | X, Y, Z, T + LSTx; elevação de pilha (*stack lift*) habilitada/desabilitada como na máquina real |
| Financeiros | n, i, PV, PMT, FV |
| Armazenamento | R0–R9, R.0–R.9 (20 registros); aritmética de registro (`STO + 1` etc.) |
| Fluxo de caixa | CFo, CFj (até 20) com Nj (repetições até 99) — compartilham R0–R19 |
| Estatística | Σ em R1–R6 (n, Σx, Σx², Σy, Σy², Σxy) |
| Programa | até 99 linhas; a cada 7 linhas além de 8 consome um registro (R.9 → R0), como na original |
| Memória contínua | todo o estado é salvo ao fechar e restaurado ao abrir |

### 2.4 Funções

- **Aritméticas/matemáticas**: + − × ÷, yˣ, 1/x, √x, eˣ, LN, x², n!, FRAC, INTG, RND, %, Δ%, %T, CHS, EEX.
- **TVM**: n, i, PV, PMT, FV com modo BEGIN/END e período fracionário (indicador `C` = juros compostos no período ímpar, ativado por `STO EEX`); cálculo de `i` por método iterativo.
- **Fluxo de caixa**: NPV, IRR (iterativo, com `Error 3`/`Error 7` iguais aos da máquina).
- **Amortização**: AMORT, com atualização de PV e n.
- **Títulos (BOND)**: PRICE, YTM (base real/real, semestral).
- **Depreciação**: SL, SOYD, DB.
- **Datas**: DATE (data futura/passada + dia da semana), ΔDYS (real e base 30/360), formatos D.MY / M.DY; faixa 15/10/1582 a 25/11/4046.
- **Estatística**: Σ+, Σ−, x̄, s, x̄w, x̂,r, ŷ,r.
- **Programação**: P/R, R/S, SST, BST, GTO, PSE, testes x≤y e x=0, MEM, edição/inserção de linhas, exibição de códigos de tecla (linha-coluna, ex.: `05-   44 1`).
- **Testes de inicialização**: `ON + ×`, `ON + ÷`, `ON + −` (autoteste e teste de teclado/display).

---

## 3. Requisitos

### 3.1 Funcionais (RF)

| ID | Requisito |
|---|---|
| RF-01 | Executar todas as funções listadas em §2.4 com resultados idênticos aos da HP-12C (10 dígitos, mesmo arredondamento). |
| RF-02 | Operar via mouse (clique nas teclas) e via teclado do PC (mapeamento em §7.4). |
| RF-03 | Exibir o display com 7 segmentos, indicadores e mensagens exatamente como na máquina. |
| RF-04 | Salvar e restaurar o estado completo automaticamente (memória contínua). |
| RF-05 | Copiar o valor de X para a área de transferência (`Ctrl+C`) e colar número em X (`Ctrl+V`). |
| RF-06 | Importar/exportar programas em arquivo texto (lista de códigos de tecla). |
| RF-07 | Menu de contexto (botão direito): sempre no topo, zoom (75–300%), copiar/colar, reset (equivalente a CLEAR ALL da máquina), sobre. |

### 3.2 Não funcionais (RNF)

| ID | Requisito |
|---|---|
| RNF-01 | **Fidelidade visual**: proporções e cores da calculadora real (128 × 79 mm); comparação lado a lado com foto de referência sem diferença perceptível de layout. |
| RNF-02 | **Nitidez em qualquer DPI**: arte vetorial, sem pixelização em 4K / escala do Windows 150–200%. |
| RNF-03 | **Portabilidade**: o núcleo (`hp12c-core`) não depende de UI, de I/O nem de bibliotecas de terceiros. |
| RNF-04 | **Determinismo**: mesma entrada → mesma saída em Rust e C#, garantido por vetores de teste compartilhados (§9). |
| RNF-05 | Executável único, sem instalador obrigatório, < 15 MB; inicia em < 1 s. |
| RNF-06 | Sem `unsafe` no núcleo; cobertura de testes do núcleo ≥ 90%. |

---

## 4. Arquitetura

### 4.1 Visão geral

```
┌───────────────────────────────────────────────────────────────┐
│                     hp12c-desktop (binário)                    │
│   eframe/egui  ─  janela, mouse, teclado, menu, clipboard      │
└───────────────┬───────────────────────────────┬───────────────┘
                │ eventos de tecla (KeyCode)     │ estado do display
                ▼                                ▲
┌───────────────────────────┐    ┌──────────────────────────────┐
│ hp12c-skin                 │    │ hp12c-core  (lógica pura)    │
│  - desenho vetorial da     │    │  - Calculator (máquina de    │
│    calculadora e do LCD    │    │    estados de teclas)        │
│  - skin.json (geometria,   │    │  - HpDecimal (BCD 10 dígitos)│
│    cores, legendas)        │    │  - Stack, Registers, Program │
│  - hit-test das teclas     │    │  - financeiro, estatística,  │
└───────────────────────────┘    │    datas, display formatter  │
                                  └──────────────┬───────────────┘
                                                 │
                                  ┌──────────────▼───────────────┐
                                  │ hp12c-ffi  (cdylib, C ABI)   │
                                  │  → P/Invoke a partir de C#   │
                                  └──────────────────────────────┘
```

### 4.2 Estrutura do repositório (Cargo workspace)

```
HP12C/
├── Cargo.toml                 # workspace
├── crates/
│   ├── hp12c-core/            # lógica da calculadora (sem dependências)
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── decimal.rs     # HpDecimal
│   │       ├── keys.rs        # KeyCode (39 teclas)
│   │       ├── machine.rs     # Calculator + máquina de estados
│   │       ├── stack.rs
│   │       ├── registers.rs
│   │       ├── display.rs     # formatação FIX/SCI → DisplayState
│   │       ├── program.rs     # memória de programa, execução
│   │       ├── finance/       # tvm.rs, cashflow.rs, amort.rs, bond.rs, depreciation.rs
│   │       ├── stats.rs
│   │       ├── dates.rs
│   │       ├── math.rs        # ln, exp, raiz, potência em decimal
│   │       └── error.rs       # Error 0..9, Pr Error
│   ├── hp12c-skin/            # arte vetorial + geometria das teclas
│   ├── hp12c-desktop/         # aplicativo (eframe)
│   └── hp12c-ffi/             # API C para C#
├── assets/
│   ├── skin.json              # coordenadas, cores, textos das teclas
│   └── fonts/                 # fontes livres (licença OFL)
├── tests/vectors/             # vetores de teste JSON (compartilhados com C#)
└── docs/
    └── SDD-HP12C.md
```

### 4.3 Princípio central: "a UI só envia teclas e lê o display"

A interface **não conhece** nenhuma regra da calculadora. Ela só:

1. envia `press(KeyCode)` / `release(KeyCode)` ao núcleo;
2. lê `display()` → `DisplayState` e desenha.

Isso garante que portar para C# (WPF, Avalonia ou MAUI) exige refazer apenas a camada de desenho.

---

## 5. Design do núcleo (`hp12c-core`)

### 5.1 API pública

```rust
pub struct Calculator { /* estado completo */ }

impl Calculator {
    pub fn new() -> Self;
    pub fn press(&mut self, key: KeyCode);          // tecla pressionada
    pub fn release(&mut self, key: KeyCode);        // necessário p/ PREFIX, ON+tecla
    pub fn tick(&mut self, elapsed_ms: u32);        // execução de programa, PSE, "running"
    pub fn display(&self) -> DisplayState;
    pub fn save_state(&self) -> String;             // JSON versionado
    pub fn load_state(json: &str) -> Result<Self, StateError>;
}

pub struct DisplayState {
    pub digits: [Segment7; 10],    // cada dígito: segmentos + ponto/vírgula
    pub minus: bool,
    pub annunciators: Annunciators, // f, g, BEGIN, D.MY, C, PRGM, *
    pub blinking: bool,             // "running"
}
```

- `KeyCode` é um `enum` com 39 variantes (nomeadas pela posição, ex.: `K11` = n … `K49` = Σ+), igual ao código de tecla linha-coluna da HP. Isso também alimenta a exibição de programas (`25` = %, `43` = g, etc.).
- `tick()` permite que programas longos rodem sem travar a UI (execução em fatias de tempo) e simula o piscar de `running`.

### 5.2 Aritmética decimal — `HpDecimal`

A HP-12C usa **ponto flutuante decimal (BCD)**: mantissa de 10 dígitos, expoente de −99 a +99, cálculos internos com dígitos de guarda e arredondamento para 10 dígitos no resultado. Usar `f64` produziria diferenças visíveis (ex.: `0,1 + 0,2`, resultados de IRR na 10ª casa).

Decisão: **tipo próprio**, sem bibliotecas externas:

```rust
pub struct HpDecimal {
    negative: bool,
    mantissa: u64,   // até 13 dígitos significativos (10 + 3 de guarda)
    exponent: i16,   // −99..=99 após normalização
}
```

- Operações: soma, subtração, multiplicação, divisão, comparação, arredondamento para 10 dígitos, overflow → `9,999999999 E99` (com sinal, como na máquina), underflow → 0.
- Funções transcendentes (ln, eˣ, yˣ, √) implementadas em decimal com algoritmos clássicos (redução de argumento + séries / CORDIC decimal), com precisão interna maior e arredondamento final.
- **Por que tipo próprio**: comportamento idêntico ao da máquina e porte direto para C# (`ulong` + `short`), sem depender de `System.Decimal` (que tem 28 dígitos e arredondamento diferente).

### 5.3 Máquina de estados de entrada

```
           ┌──────────┐  f   ┌──────────┐
  ┌───────►│  Normal  │─────►│ PrefixF  │──► executa função dourada
  │        └────┬─────┘  g   └──────────┘
  │             ├──────────► PrefixG ──► executa função azul
  │             ├─ STO/RCL ► AguardaRegistro (0-9, ., +−×÷)
  │             ├─ GTO ────► AguardaLinha (2 dígitos)
  │             ├─ dígito ─► EntradaNumero (mantissa / EEX)
  │             └─ P/R ────► ModoPrograma (gravação)
  │  Erro ──(qualquer tecla limpa a mensagem)──┘
```

Estados explícitos (enum Rust), sem flags espalhadas — facilita testes e o porte para C# (`switch` sobre enum).

### 5.4 Algoritmos financeiros

| Função | Método |
|---|---|
| n, PV, PMT, FV | fórmula fechada da TVM (com BEGIN/END e período fracionário) |
| i | Newton-Raphson com chute inicial e limites iguais aos da máquina; `Error 5` sem solução |
| IRR | Newton/secante sobre NPV; `Error 3` (não convergiu) e `Error 7` (sem troca de sinal) |
| n (resultado) | arredondado para cima ao inteiro, como na HP-12C |
| AMORT, BOND, Depreciação, Datas | fórmulas do manual; datas por número de dia juliano |

Os resultados serão validados contra **todos os exemplos do Owner's Handbook e do Solutions Handbook** (§9).

### 5.5 Persistência

- Estado salvo em `%APPDATA%\HP12C\state.json` (Windows) / `~/.config/hp12c/` (Linux).
- JSON com campo `version` para migrações futuras; mesmo formato lido pela versão C#.

---

## 6. Design visual — "idêntico à HP-12C"

### 6.1 Abordagem

**Arte vetorial desenhada por código**, a partir de um arquivo de geometria (`skin.json`), em vez de foto da calculadora:

| Opção | Prós | Contras |
|---|---|---|
| Foto/scan da calculadora | rápido | direitos autorais da HP; borra em alta resolução |
| **Vetorial a partir de medidas (escolhida)** | nítido em qualquer zoom; sem problema de direitos; mesmo `skin.json` serve para C# | mais trabalho inicial |

### 6.2 Elementos a reproduzir

- Corpo horizontal 128 × 79 mm; moldura preta; **faixa superior em alumínio escovado/dourado** com o display.
- Janela do LCD cinza-esverdeado com dígitos de 7 segmentos (levemente inclinados, como o original) e indicadores na linha inferior.
- Painel do teclado em tom marrom/bronze escuro com **colchetes dourados** (BOND, DEPRECIATION, CLEAR).
- Teclas pretas com chanfro e brilho sutil; texto principal branco; legenda **f** dourada acima; legenda **g** azul na face inferior da tecla. Tecla `f` dourada, tecla `g` azul.
- Tecla ENTER dupla na vertical.
- Animação de tecla pressionada (afundamento de 1–2 px + sombra).
- Rótulo "12C" e faixa inferior; **sem o logotipo da HP** (marca registrada) — substituído por texto neutro ou deixado em branco (ver §12, decisão D4).

### 6.3 Paleta inicial (será ajustada com foto de referência)

| Elemento | Cor aprox. |
|---|---|
| Faixa metálica | #B8A57A → #8C7B55 (gradiente) |
| Painel do teclado | #3A2F24 |
| Teclas | #1C1C1C (topo) / #0A0A0A (lateral) |
| Legenda f (dourada) | #D4A73C |
| Legenda g (azul) | #5BA4D9 |
| LCD fundo | #A9B39A |
| LCD segmentos | #1E2418 |

### 6.4 Tipografia

Fonte sem serifa condensada de licença livre (ex.: *Liberation Sans Narrow* / *Archivo Narrow*, OFL), embutida no executável.

### 6.5 Interação

- Hit-test por retângulo de cada tecla definido no `skin.json`.
- Tecla segurada (necessária para `f PREFIX`, `ON + tecla`) tratada com `press`/`release`.
- Janela sem bordas do Windows, arrastável pelo corpo da calculadora, com sombra; menu por botão direito.

---

## 7. Interface desktop (`hp12c-desktop`)

### 7.1 Tecnologia de UI

| Opção | Avaliação |
|---|---|
| **eframe/egui (escolhida)** | desenho 2D vetorial com anti-aliasing, janela sem borda, clipboard, multiplataforma, binário pequeno, maduro |
| Slint | bom, mas licença (GPL/comercial) e menos controle de desenho livre |
| iced | API instável entre versões |
| Win32/Direct2D direto | só Windows, muito código |

### 7.2 Loop

1. Evento de mouse/teclado → `KeyCode` → `calculator.press/release`.
2. A cada quadro: `calculator.tick(dt)` e desenho de `calculator.display()`.
3. Redesenho só quando há mudança (economia de CPU) ou enquanto `running` pisca / programa executa.

### 7.3 Escala

Tamanho padrão 1:1 em relação à máquina física (≈ 128 mm na tela, conforme DPI) com zoom de 75% a 300%.

### 7.4 Mapeamento do teclado do PC (configurável)

| PC | HP-12C | PC | HP-12C |
|---|---|---|---|
| 0–9, `,` `.` | dígitos, ponto | Enter | ENTER |
| + − * / | + − × ÷ | Backspace | CLx (ou ← na edição) |
| F | f | G | g |
| S / R | STO / RCL | Tab | x≷y |
| N, I, P, M, V | n, i, PV, PMT, FV | E | EEX |
| C | CHS | Esc | (limpa prefixo) |

---

## 8. Estratégia de portabilidade para C#

Duas rotas, que podem coexistir:

**Rota A — reaproveitar o núcleo Rust via FFI (curto prazo)**
- `hp12c-ffi` gera uma DLL com API C mínima: `hp12c_new`, `hp12c_free`, `hp12c_press`, `hp12c_release`, `hp12c_tick`, `hp12c_display`, `hp12c_save`, `hp12c_load`.
- Bindings C# gerados automaticamente (crate `csbindgen`) → P/Invoke.
- UI em C# (WPF/Avalonia) reutilizando o mesmo `skin.json`.

**Rota B — porte nativo do núcleo para C# (longo prazo)**
- O código Rust do núcleo foi desenhado para tradução direta:
  - sem genéricos complexos, sem traits avançadas, sem macros próprias, sem `async`;
  - tipos simples (`u64`, `i16`, `bool`, arrays fixos) → `ulong`, `short`, `bool`, arrays;
  - `enum` com dados → classes/records + `switch`;
  - erros via `Result` → exceções internas ou `Result<T>` próprio.
- **Garantia de equivalência**: os mesmos vetores de teste JSON (§9) rodam nas duas implementações; o porte só é aceito com 100% de aprovação.

---

## 9. Estratégia de testes

| Nível | Conteúdo |
|---|---|
| Unitário | `HpDecimal` (arredondamento, overflow, transcendentes), pilha, cada função |
| Vetores de teste (JSON) | sequências de teclas → display esperado. Ex.: `{"keys":"1 ENTER 2 +","display":"3.00"}`. Fonte: todos os exemplos do *Owner's Handbook* e *Solutions Handbook* + casos de borda |
| Comparação com máquina real | lista de cálculos difíceis (i, IRR, yˣ, datas extremas) conferidos numa HP-12C física (se você tiver uma) |
| Visual | captura de tela comparada com imagem de referência (teste de regressão visual) |
| Propriedades | testes aleatórios (`proptest`) para aritmética decimal |

Os vetores ficam em `tests/vectors/` e são a **especificação executável** compartilhada com o C#.

---

## 10. Plano de entrega (marcos)

| Marco | Entrega | Critério de aceite |
|---|---|---|
| **M1** — Núcleo básico | `HpDecimal`, pilha, entrada de números, + − × ÷, CHS, EEX, FIX/SCI, STO/RCL, erros | vetores de aritmética passando |
| **M2** — Visual | skin vetorial completa, display 7 segmentos, mouse e teclado do PC | aprovação visual lado a lado com foto |
| **M3** — Matemática | yˣ, 1/x, √x, eˣ, LN, n!, FRAC, INTG, RND, %, Δ%, %T, LSTx | vetores do manual |
| **M4** — Financeiro | TVM, NPV/IRR, AMORT, BOND, depreciação, datas | todos os exemplos dos capítulos financeiros do manual |
| **M5** — Estatística | Σ+, Σ−, x̄, s, x̄w, estimativas lineares | vetores do manual |
| **M6** — Programação | P/R, edição, execução, GTO, testes, PSE, MEM, importação/exportação | programas do Solutions Handbook rodando |
| **M7** — Acabamento | memória contínua, menu, zoom, clipboard, autotestes, instalador opcional | checklist de RNF |
| **M8** — C# (futuro) | `hp12c-ffi` + protótipo de UI em C# | vetores passando via P/Invoke |

---

## 11. Riscos e mitigação

| Risco | Impacto | Mitigação |
|---|---|---|
| Algoritmos iterativos (i, IRR) divergirem da máquina na 10ª casa ou no número de iterações | médio | precisão interna maior + comparação com casos documentados e máquina real |
| Detalhes obscuros (stack lift, comportamento após erro, edição de programa) | médio | máquina de estados explícita + vetores de teste específicos |
| Direitos/marca da HP | baixo (uso pessoal) | arte própria, sem logotipo HP, sem ROM |
| Fidelidade visual subjetiva | médio | aprovação visual no M2 antes de seguir |

---

## 12. Decisões para sua aprovação

| # | Decisão | Proposta |
|---|---|---|
| D1 | Modelo emulado | **HP-12C original (gold), modo RPN**. Platinum (ALG, mais memória) fica para depois |
| D2 | Abordagem | **Reimplementação de alto nível** (não emular a ROM/CPU) |
| D3 | Biblioteca de UI | **eframe/egui** |
| D4 | Logotipo | **Sem logo HP**; manter "12C" e o restante do visual |
| D5 | Porte C# | **Rota A primeiro (DLL + P/Invoke)**, Rota B quando desejado, validada pelos vetores |
| D6 | Plataformas | **Windows 11** como alvo principal; Linux/macOS compilam mas sem validação formal |
| D7 | Idioma da interface (menus) | **Português**, com opção de inglês |

---

## 13. Glossário

- **RPN** — Notação Polonesa Reversa: operandos primeiro, operador depois (`2 ENTER 3 +`).
- **Stack lift** — regra que define quando um número digitado empurra a pilha para cima.
- **BCD** — decimal codificado em binário; aritmética em base 10, como a calculadora real.
- **TVM** — Valor do dinheiro no tempo (n, i, PV, PMT, FV).
- **FFI** — interface para chamar código Rust a partir de outra linguagem (C#).
- **Vetor de teste** — par "sequência de teclas → display esperado", usado para validar as duas implementações.
