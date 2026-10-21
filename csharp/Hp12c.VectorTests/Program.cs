// Roda os vetores de teste (tests/vectors/*.json) pela DLL do núcleo, a partir do C#.
// Garante que o porte se comporta igual à versão Rust. Código de saída 1 se algo falhar.
using System.Globalization;
using System.Text.Json;
using Hp12c;

string vectors = FindVectorsFolder();
if (Calculator.ApiVersion != Calculator.ExpectedApiVersion)
{
    Console.Error.WriteLine($"Versão da API incompatível: DLL {Calculator.ApiVersion}, esperado {Calculator.ExpectedApiVersion}.");
    return 1;
}

int total = 0;
var failures = new List<string>();
foreach (string file in Directory.GetFiles(vectors, "*.json").Order())
{
    using var doc = JsonDocument.Parse(File.ReadAllText(file));
    string suite = doc.RootElement.GetProperty("suite").GetString()!;
    foreach (var testCase in doc.RootElement.GetProperty("cases").EnumerateArray())
    {
        total++;
        string name = testCase.GetProperty("name").GetString()!;
        string keys = testCase.GetProperty("keys").GetString()!;
        string expected = testCase.GetProperty("display").GetString()!;
        string label = $"[{suite}] {name} (`{keys}`)";

        using var calc = new Calculator();
        if (!calc.TapAll(keys))
        {
            failures.Add($"{label}: tecla desconhecida");
            continue;
        }
        if (calc.DisplayText != expected)
            failures.Add($"{label}: display `{calc.DisplayText}`, esperado `{expected}`");

        // Valores com até 10 dígitos são iguais exatamente quando viram o mesmo double.
        if (testCase.TryGetProperty("x", out var x))
        {
            double want = double.Parse(x.GetString()!, CultureInfo.InvariantCulture);
            double got = double.Parse(calc.XText, CultureInfo.InvariantCulture);
            if (want != got)
                failures.Add($"{label}: X = {calc.XText}, esperado {x.GetString()}");
        }
    }
}

foreach (string failure in failures)
    Console.WriteLine("FALHOU " + failure);
Console.WriteLine($"{total - failures.Count} de {total} vetores aprovados (C# via hp12c_ffi.dll).");
return failures.Count == 0 ? 0 : 1;

static string FindVectorsFolder()
{
    for (var dir = new DirectoryInfo(AppContext.BaseDirectory); dir != null; dir = dir.Parent)
    {
        string candidate = Path.Combine(dir.FullName, "tests", "vectors");
        if (Directory.Exists(candidate))
            return candidate;
    }
    throw new DirectoryNotFoundException("pasta tests/vectors não encontrada");
}
