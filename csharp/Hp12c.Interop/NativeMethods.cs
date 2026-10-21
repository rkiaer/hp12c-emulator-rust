using System.Runtime.InteropServices;

namespace Hp12c;

/// <summary>Ponteiro da calculadora no lado nativo; liberado automaticamente.</summary>
internal sealed class CalculatorHandle : SafeHandle
{
    public CalculatorHandle() : base(IntPtr.Zero, ownsHandle: true) { }

    public override bool IsInvalid => handle == IntPtr.Zero;

    protected override bool ReleaseHandle()
    {
        NativeMethods.hp12c_free(handle);
        return true;
    }
}

/// <summary>Espelho de <c>Hp12cDisplay</c> (repr(C) no Rust).</summary>
[StructLayout(LayoutKind.Sequential)]
internal unsafe struct NativeDisplay
{
    public byte Minus;
    public byte Blinking;
    public byte Annunciators;
    public byte Reserved;
    public fixed uint Glyphs[10];
    public fixed byte Punct[10];
}

/// <summary>Chamadas à DLL hp12c_ffi (ver crates/hp12c-ffi/include/hp12c.h).</summary>
internal static unsafe partial class NativeMethods
{
    private const string Library = "hp12c_ffi";

    [LibraryImport(Library)]
    internal static partial uint hp12c_api_version();

    [LibraryImport(Library)]
    internal static partial CalculatorHandle hp12c_new();

    [LibraryImport(Library)]
    internal static partial void hp12c_free(IntPtr calc);

    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    internal static partial CalculatorHandle hp12c_load_state(string json);

    [LibraryImport(Library)]
    internal static partial void hp12c_press(CalculatorHandle calc, byte key);

    [LibraryImport(Library)]
    internal static partial void hp12c_release(CalculatorHandle calc, byte key);

    [LibraryImport(Library)]
    internal static partial void hp12c_tick(CalculatorHandle calc, uint elapsedMs);

    [LibraryImport(Library)]
    internal static partial byte hp12c_is_running(CalculatorHandle calc);

    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    internal static partial int hp12c_tap_all(CalculatorHandle calc, string sequence);

    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    internal static partial int hp12c_key_from_token(string token);

    [LibraryImport(Library)]
    internal static partial void hp12c_display(CalculatorHandle calc, out NativeDisplay display);

    [LibraryImport(Library)]
    internal static partial nuint hp12c_display_text(CalculatorHandle calc, byte* buffer, nuint capacity);

    [LibraryImport(Library)]
    internal static partial nuint hp12c_x_text(CalculatorHandle calc, byte* buffer, nuint capacity);

    [LibraryImport(Library)]
    internal static partial nuint hp12c_save_state(CalculatorHandle calc, byte* buffer, nuint capacity);

    [LibraryImport(Library)]
    internal static partial nuint hp12c_program_listing(CalculatorHandle calc, byte* buffer, nuint capacity);

    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    internal static partial int hp12c_load_program(CalculatorHandle calc, string listing);
}
