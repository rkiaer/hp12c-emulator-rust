using System.Text;

namespace Hp12c;

[Flags]
public enum Annunciators : byte
{
    None = 0,
    LowBattery = 1 << 0,
    F = 1 << 1,
    G = 1 << 2,
    Begin = 1 << 3,
    Dmy = 1 << 4,
    Compound = 1 << 5,
    Prgm = 1 << 6,
}

public enum Punct : byte { None = 0, Point = 1, Comma = 2 }

/// <summary>Uma posição do display: caractere de 7 segmentos e pontuação.</summary>
public readonly record struct Cell(char Glyph, Punct Punct);

public sealed record DisplayState(bool Minus, bool Blinking, Annunciators Annunciators, Cell[] Cells);

/// <summary>
/// Calculadora HP-12C. Toda a lógica roda no núcleo em Rust (hp12c_ffi.dll);
/// esta classe só traduz chamadas e dados.
/// </summary>
public sealed class Calculator : IDisposable
{
    /// <summary>Versão da API C que esta biblioteca espera.</summary>
    public const uint ExpectedApiVersion = 1;

    private readonly CalculatorHandle _handle;

    public Calculator() : this(NativeMethods.hp12c_new()) { }

    private Calculator(CalculatorHandle handle)
    {
        if (handle.IsInvalid)
            throw new InvalidOperationException("hp12c_ffi não criou a calculadora.");
        _handle = handle;
    }

    public static uint ApiVersion => NativeMethods.hp12c_api_version();

    /// <summary>Restaura um estado salvo por <see cref="SaveState"/>; null se inválido.</summary>
    public static Calculator? LoadState(string json)
    {
        var handle = NativeMethods.hp12c_load_state(json);
        if (handle.IsInvalid)
        {
            handle.Dispose();
            return null;
        }
        return new Calculator(handle);
    }

    /// <summary>Tecla pelo nome usado nas sequências ("ENTER", "x&lt;&gt;y", "7"…).</summary>
    public static Key? KeyFromToken(string token)
    {
        int index = NativeMethods.hp12c_key_from_token(token);
        return index < 0 ? null : (Key)index;
    }

    public void Press(Key key) => NativeMethods.hp12c_press(_handle, (byte)key);

    public void Release(Key key) => NativeMethods.hp12c_release(_handle, (byte)key);

    public void Tap(Key key)
    {
        Press(key);
        Release(key);
    }

    /// <summary>Executa "12 ENTER 3 /"; false se houver tecla desconhecida.</summary>
    public bool TapAll(string sequence) => NativeMethods.hp12c_tap_all(_handle, sequence) == 0;

    /// <summary>Avança um programa em execução; chame a cada quadro.</summary>
    public void Tick(uint elapsedMs) => NativeMethods.hp12c_tick(_handle, elapsedMs);

    public bool IsRunning => NativeMethods.hp12c_is_running(_handle) != 0;

    public unsafe DisplayState Display
    {
        get
        {
            NativeMethods.hp12c_display(_handle, out var d);
            var cells = new Cell[10];
            for (int i = 0; i < 10; i++)
                cells[i] = new Cell((char)d.Glyphs[i], (Punct)d.Punct[i]);
            return new DisplayState(d.Minus != 0, d.Blinking != 0, (Annunciators)d.Annunciators, cells);
        }
    }

    public unsafe string DisplayText => ReadText((b, n) => NativeMethods.hp12c_display_text(_handle, b, n));

    /// <summary>Valor completo de X, ex.: "3.333333333E-1".</summary>
    public unsafe string XText => ReadText((b, n) => NativeMethods.hp12c_x_text(_handle, b, n));

    public unsafe string SaveState() => ReadText((b, n) => NativeMethods.hp12c_save_state(_handle, b, n));

    public unsafe string ProgramListing => ReadText((b, n) => NativeMethods.hp12c_program_listing(_handle, b, n));

    public bool LoadProgram(string listing) => NativeMethods.hp12c_load_program(_handle, listing) == 0;

    public void Dispose() => _handle.Dispose();

    private unsafe delegate nuint TextFunction(byte* buffer, nuint capacity);

    /// <summary>Pergunta o tamanho, aloca e lê o texto UTF-8.</summary>
    private static unsafe string ReadText(TextFunction read)
    {
        int length = (int)read(null, 0);
        var buffer = new byte[length + 1];
        fixed (byte* p = buffer)
            read(p, (nuint)buffer.Length);
        return Encoding.UTF8.GetString(buffer, 0, length);
    }
}
