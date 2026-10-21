using System.Windows;

namespace Hp12c.Wpf;

// Porte de hp12c-skin/src/seg7.rs: segmentos a…g (bits 0…6) e seus polígonos.

public static class Seg7
{
    private const byte A = 1, B = 2, C = 4, D = 8, E = 16, F = 32, G = 64;

    public static byte Segments(char ch) => ch switch
    {
        '0' or 'O' => A | B | C | D | E | F,
        '1' => B | C,
        '2' => A | B | G | E | D,
        '3' => A | B | G | C | D,
        '4' => F | G | B | C,
        '5' or 'S' => A | F | G | C | D,
        '6' => A | F | G | E | D | C,
        '7' => A | B | C,
        '8' => A | B | C | D | E | F | G,
        '9' or 'g' => A | B | C | D | F | G,
        '-' => G,
        'A' => A | B | C | E | F | G,
        'b' => C | D | E | F | G,
        'C' => A | D | E | F,
        'c' => D | E | G,
        'd' => B | C | D | E | G,
        'E' => A | D | E | F | G,
        'F' => A | E | F | G,
        'H' => B | C | E | F | G,
        'i' => E,
        'L' => D | E | F,
        'n' => C | E | G,
        'o' => C | D | E | G,
        'P' => A | B | E | F | G,
        'r' => E | G,
        't' => D | E | F | G,
        'U' => B | C | D | E | F,
        'u' => C | D | E,
        'y' => B | C | D | F | G,
        _ => 0,
    };

    /// <summary>Hexágonos dos 7 segmentos (ordem a…g), origem no canto superior esquerdo.</summary>
    public static Point[][] Polygons(double width, double height, double thickness, double slant)
    {
        double half = thickness / 2, gap = thickness * 0.14;
        double left = half, right = width - half, top = half, mid = height / 2, bottom = height - half;

        Point[] Horizontal(double y)
        {
            double x0 = left + gap, x1 = right - gap;
            return [new(x0, y), new(x0 + half, y - half), new(x1 - half, y - half), new(x1, y), new(x1 - half, y + half), new(x0 + half, y + half)];
        }
        Point[] Vertical(double x, double y0, double y1)
        {
            y0 += gap; y1 -= gap;
            return [new(x, y0), new(x + half, y0 + half), new(x + half, y1 - half), new(x, y1), new(x - half, y1 - half), new(x - half, y0 + half)];
        }

        var polys = new[]
        {
            Horizontal(top), Vertical(right, top, mid), Vertical(right, mid, bottom),
            Horizontal(bottom), Vertical(left, mid, bottom), Vertical(left, top, mid), Horizontal(mid),
        };
        foreach (var poly in polys)
            for (int i = 0; i < poly.Length; i++)
                poly[i] = new Point(poly[i].X + slant * (height - poly[i].Y), poly[i].Y);
        return polys;
    }
}
