using System.IO;
using System.Text.Json;
using System.Windows;
using System.Windows.Media;

namespace Hp12c.Wpf;

// Leitura de assets/skin.json — mesmas regras de geometria do crate hp12c-skin (Rust).

public sealed class LcdSpec
{
    public float[] Bezel { get; set; } = [];
    public float[] Glass { get; set; } = [];
    public float MinusX { get; set; }
    public float FirstDigitX { get; set; }
    public float DigitPitch { get; set; }
    public float DigitTop { get; set; }
    public float DigitWidth { get; set; }
    public float DigitHeight { get; set; }
    public float SegmentThickness { get; set; }
    public float Slant { get; set; }
    public float AnnunciatorY { get; set; }
    public float AnnunciatorSize { get; set; }
    public List<AnnunciatorSpec> Annunciators { get; set; } = [];
}

public sealed class AnnunciatorSpec
{
    public string Name { get; set; } = "";
    public string Text { get; set; } = "";
    public float X { get; set; }
}

public sealed class BadgeSpec
{
    public string Text { get; set; } = "";
    public string Sub { get; set; } = "";
    public float X { get; set; }
    public float Y { get; set; }
    public float Size { get; set; }
    public float SubSize { get; set; }
    public float SubY { get; set; }
}

public sealed class KeyboardSpec
{
    public float[] Origin { get; set; } = [];
    public float[] Pitch { get; set; } = [];
    public float[] KeySize { get; set; } = [];
    public float KeyOffsetY { get; set; }
    public float FaceSplit { get; set; }
    public float KeyRadius { get; set; }
    public float MainSize { get; set; }
    public float DigitSize { get; set; }
    public float FSize { get; set; }
    public float GSize { get; set; }
    public float BracketY { get; set; }
    public float FLabelY { get; set; }
}

public sealed class KeySpec
{
    public string Key { get; set; } = "";
    public int Row { get; set; }
    public int Col { get; set; }
    public string Main { get; set; } = "";
    public string F { get; set; } = "";
    public string G { get; set; } = "";
    public int Rows { get; set; } = 1;
    public bool Vertical { get; set; }
    public string Style { get; set; } = "normal";
}

public sealed class BracketSpec
{
    public string Text { get; set; } = "";
    public int Row { get; set; }
    public int FromCol { get; set; }
    public int ToCol { get; set; }
}

public sealed class SkinFile
{
    public string Name { get; set; } = "";
    public float[] Size { get; set; } = [];
    public float CornerRadius { get; set; }
    public Dictionary<string, string> Colors { get; set; } = [];
    public float MetalStripHeight { get; set; }
    public LcdSpec Lcd { get; set; } = new();
    public BadgeSpec Badge { get; set; } = new();
    public KeyboardSpec Keyboard { get; set; } = new();
    public List<KeySpec> Keys { get; set; } = [];
    public List<BracketSpec> Brackets { get; set; } = [];
}

public sealed record KeyGeometry(
    Key Key, Rect Rect, Label Main, Label? F, Label? G, string Style, bool Vertical, Point FLabelCenter, double FrontHeight)
{
    public Rect TopFace => new(Rect.X, Rect.Y, Rect.Width, Rect.Height - FrontHeight);
    public Rect FrontFace => new(Rect.X, Rect.Bottom - FrontHeight, Rect.Width, FrontHeight);
}

public sealed record Bracket(string Text, double X0, double X1, double Y);

public sealed class Skin
{
    public required SkinFile Data { get; init; }
    public required List<KeyGeometry> Keys { get; init; }
    public required List<Bracket> Brackets { get; init; }

    public double Width => Data.Size[0];
    public double Height => Data.Size[1];

    public Color Color(string name) => (Color)ColorConverter.ConvertFromString(Data.Colors[name]);

    public static Skin Load(string path)
    {
        var options = new JsonSerializerOptions
        {
            PropertyNamingPolicy = JsonNamingPolicy.SnakeCaseLower,
            PropertyNameCaseInsensitive = true,
        };
        var data = JsonSerializer.Deserialize<SkinFile>(File.ReadAllText(path), options)
            ?? throw new InvalidDataException("skin.json vazio");
        var kb = data.Keyboard;
        double frontHeight = kb.KeySize[1] * (1 - kb.FaceSplit);

        var keys = new List<KeyGeometry>();
        foreach (var spec in data.Keys)
        {
            var key = Calculator.KeyFromToken(spec.Key) ?? throw new InvalidDataException($"tecla desconhecida: {spec.Key}");
            double rowTop = kb.Origin[1] + (spec.Row - 1) * kb.Pitch[1];
            double x = kb.Origin[0] + (spec.Col - 1) * kb.Pitch[0];
            var rect = new Rect(x, rowTop + kb.KeyOffsetY, kb.KeySize[0], kb.KeySize[1] + (spec.Rows - 1) * kb.Pitch[1]);
            keys.Add(new KeyGeometry(
                key, rect, Label.Parse(spec.Main),
                spec.F.Length > 0 ? Label.Parse(spec.F) : null,
                spec.G.Length > 0 ? Label.Parse(spec.G) : null,
                spec.Style, spec.Vertical,
                new Point(x + kb.KeySize[0] / 2, rowTop + kb.FLabelY),
                frontHeight));
        }
        if (keys.Count != 39)
            throw new InvalidDataException($"o skin tem {keys.Count} teclas; esperado 39");

        var brackets = data.Brackets.Select(b => new Bracket(
            b.Text,
            kb.Origin[0] + (b.FromCol - 1) * kb.Pitch[0],
            kb.Origin[0] + (b.ToCol - 1) * kb.Pitch[0] + kb.KeySize[0],
            kb.Origin[1] + (b.Row - 1) * kb.Pitch[1] + kb.BracketY)).ToList();

        return new Skin { Data = data, Keys = keys, Brackets = brackets };
    }

    /// <summary>Tecla no ponto (mm), com a mesma folga de 0,8 mm do Rust.</summary>
    public Key? HitTest(Point mm)
    {
        foreach (var k in Keys)
        {
            var r = k.Rect;
            r.Inflate(0.8, 0.8);
            if (r.Contains(mm))
                return k.Key;
        }
        return null;
    }
}
