# Vetores de teste

Especificação executável do comportamento da HP-12C, compartilhada entre a versão
Rust e o futuro porte para C#. Cada arquivo `.json` tem o formato:

```json
{
  "suite": "nome",
  "cases": [
    { "name": "descrição", "keys": "1 ENTER 2 +", "display": "3.00", "x": "3" }
  ]
}
```

- `keys`: teclas separadas por espaço. Números com vários dígitos (`1234`) viram uma
  tecla por dígito. Nomes: `n i PV PMT FV CHS / y^x 1/x %T D% % EEX * R/S SST Rv x<>y
  CLx ENTER - ON f g STO RCL . S+ +`.
- `display`: texto do LCD com espaços finais removidos (separador padrão: ponto decimal).
- `x` (opcional): valor exato esperado no registrador X.
- `confirmar` (opcional): `true` quando o comportamento ainda precisa ser conferido numa
  HP-12C física.

Cada caso começa com uma calculadora recém-ligada (FIX 2, tudo zerado).
