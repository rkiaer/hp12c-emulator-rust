using System.Globalization;
using System.Windows;
using System.Windows.Input;
using System.Windows.Media;

namespace Hp12c.Wpf;

/// <summary>
/// Desenha a calculadora a partir do skin (em milímetros) e repassa cliques ao núcleo.
/// Porte de crates/hp12c-desktop/src/render.rs.
/// </summary>
public sealed class CalculatorView : FrameworkElement
{
    private readonly Skin _skin;
    private readonly Calculator _calc;
    private readonly Typeface _label = new(new FontFamily("Arial Narrow, Arial"), FontStyles.Normal, FontWeights.Bold, FontStretches.Normal);
    private readonly Typeface _badge = new(new FontFamily("Segoe UI Light, Segoe UI"), FontStyles.Normal, FontWeights.Light, FontStretches.Normal);
    private readonly HashSet<Key> _pressed = [];
    private Key? _mouseKey;

    public CalculatorView(Skin skin, Calculator calc)
    {
        _skin = skin;
        _calc = calc;
        Focusable = true;
    }

    private double Scale => ActualWidth / _skin.Width;

    /// <summary>Marca uma tecla como afundada (teclado do PC) e redesenha.</summary>
    public void SetPressed(Key key, bool down)
    {
        if (down) _pressed.Add(key); else _pressed.Remove(key);
        InvalidateVisual();
    }

    protected override void OnMouseLeftButtonDown(MouseButtonEventArgs e)
    {
        Focus();
        var p = e.GetPosition(this);
        var key = _skin.HitTest(new Point(p.X / Scale, p.Y / Scale));
        if (key is { } k)
        {
            _calc.Press(k);
            _mouseKey = k;
            CaptureMouse();
            InvalidateVisual();
        }
        else
        {
            Window.GetWindow(this)?.DragMove(); // fora das teclas: arrasta a janela
        }
    }

    protected override void OnMouseLeftButtonUp(MouseButtonEventArgs e)
    {
        if (_mouseKey is { } k)
        {
            _calc.Release(k);
            _mouseKey = null;
            ReleaseMouseCapture();
            InvalidateVisual();
        }
    }

    // ------------------------------------------------------------------ desenho

    private Brush B(string color) => new SolidColorBrush(_skin.Color(color));

    private FormattedText Text(string text, double size, Brush brush, Typeface? typeface = null) =>
        new(text, CultureInfo.InvariantCulture, FlowDirection.LeftToRight, typeface ?? _label, size, brush, 1.0);

    private static void DrawCentered(DrawingContext dc, FormattedText t, Point center) =>
        dc.DrawText(t, new Point(center.X - t.WidthIncludingTrailingWhitespace / 2, center.Y - t.Height / 2));

    protected override void OnRender(DrawingContext dc)
    {
        double s = Scale;
        if (s <= 0) return;
        dc.PushTransform(new ScaleTransform(s, s)); // daqui em diante tudo em mm
        var data = _skin.Data;
        double w = _skin.Width, h = _skin.Height, r = data.CornerRadius, metal = data.MetalStripHeight;

        // Corpo, painel e faixa metálica.
        dc.DrawRoundedRectangle(B("body"), null, new Rect(0, 0, w, h), r, r);
        var panel = B("panel");
        dc.DrawRoundedRectangle(panel, null, new Rect(1, metal, w - 2, h - metal - 1), r - 1, r - 1);
        dc.DrawRectangle(panel, null, new Rect(1, metal, w - 2, r));
        var metalBrush = new LinearGradientBrush(_skin.Color("metal_top"), _skin.Color("metal_bottom"), new Point(0, 1), new Point(0, metal))
        { MappingMode = BrushMappingMode.Absolute };
        dc.DrawRoundedRectangle(metalBrush, null, new Rect(1, 1, w - 2, metal - 1), r - 1, r - 1);
        dc.DrawRectangle(metalBrush, null, new Rect(1, metal - r, w - 2, r));
        DrawBrushedLines(dc, metal);
        dc.DrawLine(new Pen(B("panel_edge"), 0.25), new Point(1, metal), new Point(w - 1, metal));

        DrawLcd(dc, _calc.Display);
        DrawBadge(dc);
        DrawBrackets(dc);
        foreach (var key in _skin.Keys)
            DrawKey(dc, key, _mouseKey == key.Key || _pressed.Contains(key.Key));
        dc.Pop();
    }

    private void DrawBrushedLines(DrawingContext dc, double metal)
    {
        var line = _skin.Color("metal_line");
        uint seed = 12345;
        for (double y = 2.2; y < metal - 0.6; y += 0.22)
        {
            seed = unchecked(seed * 1103515245 + 12345);
            byte alpha = (byte)Math.Min(255, (18 + (seed >> 16) % 40) * 2);
            dc.DrawLine(new Pen(new SolidColorBrush(Color.FromArgb(alpha, line.R, line.G, line.B)), 0.06), new Point(2, y), new Point(_skin.Width - 2, y));
        }
    }

    private void DrawLcd(DrawingContext dc, DisplayState display)
    {
        var lcd = _skin.Data.Lcd;
        var bezel = new Rect(lcd.Bezel[0], lcd.Bezel[1], lcd.Bezel[2], lcd.Bezel[3]);
        dc.DrawRoundedRectangle(B("bezel"), new Pen(B("bezel_edge"), 0.3), bezel, 1.2, 1.2);
        var glass = new Rect(lcd.Glass[0], lcd.Glass[1], lcd.Glass[2], lcd.Glass[3]);
        var glassBrush = new LinearGradientBrush(_skin.Color("lcd_shadow"), _skin.Color("lcd"), 90);
        dc.DrawRoundedRectangle(glassBrush, null, glass, 0.6, 0.6);
        dc.DrawRectangle(B("lcd_shadow"), null, new Rect(glass.X, glass.Y, glass.Width, 0.7));

        // "running" pisca.
        if (display.Blinking && (Environment.TickCount64 / 400) % 2 == 1) return;

        var ink = B("segment");
        var polys = Seg7.Polygons(lcd.DigitWidth, lcd.DigitHeight, lcd.SegmentThickness, lcd.Slant);
        if (display.Minus)
        {
            var minus = Seg7.Polygons(lcd.DigitWidth * 0.8, lcd.DigitHeight, lcd.SegmentThickness, lcd.Slant);
            DrawSegments(dc, minus, Seg7.Segments('-'), new Point(lcd.MinusX, lcd.DigitTop), ink);
        }
        for (int i = 0; i < 10; i++)
        {
            var origin = new Point(lcd.FirstDigitX + i * lcd.DigitPitch, lcd.DigitTop);
            DrawSegments(dc, polys, Seg7.Segments(display.Cells[i].Glyph), origin, ink);
            if (display.Cells[i].Punct != Punct.None)
                DrawPunct(dc, origin, display.Cells[i].Punct == Punct.Comma, ink);
        }

        foreach (var ann in lcd.Annunciators)
        {
            var flag = ann.Name switch
            {
                "low_battery" => Annunciators.LowBattery,
                "f" => Annunciators.F,
                "g" => Annunciators.G,
                "begin" => Annunciators.Begin,
                "dmy" => Annunciators.Dmy,
                "compound" => Annunciators.Compound,
                "prgm" => Annunciators.Prgm,
                _ => Annunciators.None,
            };
            if (flag != Annunciators.None && display.Annunciators.HasFlag(flag))
                DrawCentered(dc, Text(ann.Text, lcd.AnnunciatorSize, ink), new Point(ann.X, lcd.AnnunciatorY));
        }
    }

    private static void DrawSegments(DrawingContext dc, Point[][] polys, byte lit, Point origin, Brush ink)
    {
        for (int i = 0; i < 7; i++)
        {
            if ((lit & (1 << i)) == 0) continue;
            var geometry = new StreamGeometry();
            using (var ctx = geometry.Open())
            {
                var p = polys[i];
                ctx.BeginFigure(new Point(origin.X + p[0].X, origin.Y + p[0].Y), true, true);
                for (int j = 1; j < p.Length; j++)
                    ctx.LineTo(new Point(origin.X + p[j].X, origin.Y + p[j].Y), false, false);
            }
            geometry.Freeze();
            dc.DrawGeometry(ink, null, geometry);
        }
    }

    private void DrawPunct(DrawingContext dc, Point origin, bool comma, Brush ink)
    {
        var lcd = _skin.Data.Lcd;
        double t = lcd.SegmentThickness * 1.05;
        double x = origin.X + lcd.DigitWidth + (lcd.DigitPitch - lcd.DigitWidth) / 2 - t / 2;
        double y = origin.Y + lcd.DigitHeight - t;
        dc.DrawRectangle(ink, null, new Rect(x, y, t, t));
        if (!comma) return;
        var tail = new StreamGeometry();
        using (var ctx = tail.Open())
        {
            ctx.BeginFigure(new Point(x + t * 0.15, y + t), true, true);
            ctx.LineTo(new Point(x + t, y + t), false, false);
            ctx.LineTo(new Point(x - t * 0.45, y + t * 2.3), false, false);
        }
        dc.DrawGeometry(ink, null, tail);
    }

    private void DrawBadge(DrawingContext dc)
    {
        var b = _skin.Data.Badge;
        DrawCentered(dc, Text(b.Text, b.Size, B("badge_text"), _badge), new Point(b.X, b.Y));
        var sub = Text(b.Sub, b.SubSize, B("badge_sub"));
        sub.SetFontStretch(FontStretches.Normal);
        DrawCentered(dc, sub, new Point(b.X, b.SubY));
    }

    private void DrawBrackets(DrawingContext dc)
    {
        var gold = B("f_label");
        var pen = new Pen(gold, 0.22);
        foreach (var b in _skin.Brackets)
        {
            var text = Text(b.Text, _skin.Data.Keyboard.FSize, gold);
            double cx = (b.X0 + b.X1) / 2, half = text.WidthIncludingTrailingWhitespace / 2 + 0.6;
            DrawCentered(dc, text, new Point(cx, b.Y));
            dc.DrawLine(pen, new Point(b.X0, b.Y), new Point(cx - half, b.Y));
            dc.DrawLine(pen, new Point(cx + half, b.Y), new Point(b.X1, b.Y));
            dc.DrawLine(pen, new Point(b.X0, b.Y), new Point(b.X0, b.Y + 0.85));
            dc.DrawLine(pen, new Point(b.X1, b.Y), new Point(b.X1, b.Y + 0.85));
        }
    }

    private void DrawKey(DrawingContext dc, KeyGeometry key, bool pressed)
    {
        var kb = _skin.Data.Keyboard;
        if (key.F is { } f)
            DrawLabel(dc, f, key.FLabelCenter, kb.FSize, B("f_label"));

        var (top, topLight, face, text) = key.Style switch
        {
            "gold" => ("f_key_top", "f_key_top", "f_key_face", "prefix_key_text"),
            "blue" => ("g_key_top", "g_key_top", "g_key_face", "prefix_key_text"),
            _ => ("key_top", "key_top_light", "key_face", "key_text"),
        };
        double r = kb.KeyRadius, sink = pressed ? 0.35 : 0;
        var edge = key.Rect;
        edge.Inflate(0.3, 0.3);
        dc.DrawRoundedRectangle(B("key_edge"), null, edge, r + 0.3, r + 0.3);
        dc.PushTransform(new TranslateTransform(0, sink));
        dc.DrawRoundedRectangle(B(face), null, key.Rect, r, r);
        Brush topBrush = pressed && key.Style == "normal"
            ? B("key_pressed")
            : new LinearGradientBrush(_skin.Color(topLight), _skin.Color(top), 90);
        dc.DrawRoundedRectangle(topBrush, null, key.TopFace, r, r);

        var center = new Point(key.TopFace.X + key.TopFace.Width / 2, key.TopFace.Y + key.TopFace.Height / 2);
        string plain = key.Main.Plain;
        bool single = plain.Length == 1;
        double size = single && !char.IsLetter(plain[0]) ? kb.DigitSize : single ? kb.MainSize * 1.15 : kb.MainSize;
        if (key.Vertical)
        {
            double step = size * 0.98, start = center.Y - step * (plain.Length - 1) / 2;
            for (int i = 0; i < plain.Length; i++)
                DrawCentered(dc, Text(plain[i].ToString(), size, B(text)), new Point(center.X, start + step * i));
        }
        else
        {
            DrawLabel(dc, key.Main, center, size, B(text));
        }
        if (key.G is { } g)
        {
            var front = key.FrontFace;
            DrawLabel(dc, g, new Point(front.X + front.Width / 2, front.Y + front.Height / 2), kb.GSize, B("g_label"));
        }
        dc.Pop();
    }

    /// <summary>Legenda com sobrescrito/subscrito e barra/chapéu sobre o primeiro caractere.</summary>
    private void DrawLabel(DrawingContext dc, Label label, Point center, double size, Brush brush)
    {
        var parts = label.Runs.Select(run =>
        {
            double scale = run.Kind == RunKind.Normal ? 1 : 0.68;
            return (run, text: Text(run.Text, size * scale, brush));
        }).ToList();
        double width = parts.Sum(p => p.text.WidthIncludingTrailingWhitespace);
        double baseline = center.Y + size * 0.36;
        double x = center.X - width / 2;
        double startX = x;
        foreach (var (run, t) in parts)
        {
            double shift = run.Kind switch { RunKind.Superscript => -size * 0.38, RunKind.Subscript => size * 0.16, _ => 0 };
            dc.DrawText(t, new Point(x, baseline + shift - t.Baseline));
            x += t.WidthIncludingTrailingWhitespace;
        }
        if (label.FirstMark is { } mark && label.Plain.Length > 0)
        {
            double charWidth = Text(label.Plain[0].ToString(), size, brush).WidthIncludingTrailingWhitespace;
            double y = baseline - size * 0.64;
            var pen = new Pen(brush, size * 0.085);
            if (mark == Mark.Bar)
            {
                dc.DrawLine(pen, new Point(startX + charWidth * 0.1, y), new Point(startX + charWidth * 0.9, y));
            }
            else
            {
                var peak = new Point(startX + charWidth * 0.5, y - size * 0.1);
                dc.DrawLine(pen, new Point(startX + charWidth * 0.15, y + size * 0.05), peak);
                dc.DrawLine(pen, peak, new Point(startX + charWidth * 0.85, y + size * 0.05));
            }
        }
    }
}
