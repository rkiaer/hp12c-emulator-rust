namespace Hp12c;

/// <summary>As 39 teclas na ordem física (mesmos índices do núcleo em Rust).</summary>
public enum Key : byte
{
    // Linha 1
    N, I, Pv, Pmt, Fv, Chs, D7, D8, D9, Div,
    // Linha 2
    Pow, Recip, PctTotal, PctChange, Pct, Eex, D4, D5, D6, Mul,
    // Linha 3
    RunStop, Sst, RollDown, Swap, Clx, Enter, D1, D2, D3, Sub,
    // Linha 4 (ENTER ocupa as linhas 3 e 4)
    On, F, G, Sto, Rcl, D0, Dot, SigmaPlus, Add,
}
