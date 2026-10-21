# Programas de exemplo

Programas prontos para a HP-12C. Para carregar no emulador:

1. Abra o arquivo `.txt`, selecione tudo e copie (Ctrl+A, Ctrl+C).
2. Clique na calculadora e pressione **Ctrl+V**. Aparece o aviso "Programa carregado".
3. Use conforme as instruções abaixo. Para ver ou alterar o programa, pressione **F3**.

Os comentários depois de `#` são só explicação; a calculadora ignora. Na HP-12C física, digite as
instruções na ordem, depois de `f P/R` (veja o manual da calculadora).

## Taxa equivalente — [`taxa-equivalente.txt`](taxa-equivalente.txt)

Converte uma taxa de juros compostos para outro período, nos dois sentidos.

**Como usar:** digite a taxa em %, `ENTER`, depois **k** e `R/S`.
**k** é quantos períodos curtos cabem no período da taxa que você tem:

| Quero saber | Teclas | Resultado (`f 4`) |
|---|---|---|
| 12% ao ano, ao mês | `12 ENTER 12 R/S` | 0,9489 % |
| 30% ao ano, ao mês | `30 ENTER 12 R/S` | 2,2104 % |
| 1% ao mês, ao ano | `1 ENTER 12 1/x R/S` | 12,6825 % |
| 0,5% ao mês, ao semestre | `0.5 ENTER 6 1/x R/S` | 3,0378 % |
| 50% em 2 anos, ao ano | `50 ENTER 2 R/S` | 22,4745 % |

Para ir de uma taxa curta para uma longa (mês → ano), use **1/k**: `12 1/x` em vez de `12`.

**Como funciona** (16 linhas): calcula `((1 + taxa/100)^(1/k) − 1) × 100`. Usa só a pilha e não
mexe nos registradores. Por ter mais de 8 linhas, o programa ocupa o registrador R.9.

**Precisão:** o resultado tem cerca de 7 dígitos significativos (ex.: 0,9488793), porque ao subtrair
1 de 1,009488793 os primeiros dígitos se cancelam. A HP-12C real, que também calcula com 10 dígitos,
dá o mesmo resultado. Para uso financeiro (2 a 4 casas) isso não faz diferença.
