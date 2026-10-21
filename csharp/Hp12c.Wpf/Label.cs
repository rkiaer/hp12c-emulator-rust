namespace Hp12c.Wpf;

// Porte de hp12c-skin/src/label.rs: "y^x" (sobrescrito), "CF_o" (subscrito),
// barra/chapéu sobre o primeiro caractere ("x̄", "x̂", "ŷ").

public enum RunKind { Normal, Superscript, Subscript }

public enum Mark { Bar, Hat }

public sealed record Run(string Text, RunKind Kind);

public sealed record Label(IReadOnlyList<Run> Runs, Mark? FirstMark)
{
    private const char CombiningMacron = '̄';
    private const char CombiningCircumflex = '̂';

    public string Plain => string.Concat(Runs.Select(r => r.Text));

    public static Label Parse(string text)
    {
        var runs = new List<Run>();
        Mark? mark = null;
        var kind = RunKind.Normal;
        var current = new System.Text.StringBuilder();
        int seen = 0;
        foreach (char c in text)
        {
            switch (c)
            {
                case '^' or '_':
                    if (current.Length > 0) { runs.Add(new Run(current.ToString(), kind)); current.Clear(); }
                    kind = c == '^' ? RunKind.Superscript : RunKind.Subscript;
                    break;
                case CombiningMacron or CombiningCircumflex when seen == 1:
                    mark = c == CombiningMacron ? Mark.Bar : Mark.Hat;
                    break;
                case 'ŷ' when seen == 0:
                    current.Append('y');
                    seen++;
                    mark = Mark.Hat;
                    break;
                default:
                    current.Append(c);
                    seen++;
                    break;
            }
        }
        if (current.Length > 0) runs.Add(new Run(current.ToString(), kind));
        return new Label(runs, mark);
    }
}
