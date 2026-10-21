using System.IO;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Threading;
using WpfKey = System.Windows.Input.Key;

namespace Hp12c.Wpf;

/// <summary>
/// Protótipo em WPF: janela sem bordas com a calculadora. A lógica está toda na
/// DLL do núcleo (Rust); aqui só há desenho, mouse, teclado e memória contínua.
/// </summary>
public sealed class MainWindow : Window
{
    private const double PointsPerMm = 96.0 / 25.4;
    private static readonly double[] Zooms = [1.0, 1.25, 1.5, 2.0, 2.5];

    /// <summary>Mesmo arquivo da versão Rust: as duas compartilham a memória contínua.</summary>
    private static readonly string StatePath =
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "HP12C", "state.json");

    private readonly Skin _skin;
    private readonly Calculator _calc;
    private readonly CalculatorView _view;
    private readonly DispatcherTimer _timer;
    private double _zoom = 1.5;

    public MainWindow()
    {
        _skin = Skin.Load(Path.Combine(AppContext.BaseDirectory, "skin.json"));
        _calc = LoadCalculator();
        _view = new CalculatorView(_skin, _calc);

        Title = "HP-12C (C#)";
        WindowStyle = WindowStyle.None;
        AllowsTransparency = true;
        Background = Brushes.Transparent;
        ResizeMode = ResizeMode.NoResize;
        Content = _view;
        ApplyZoom(_zoom);
        ContextMenu = BuildMenu();

        PreviewKeyDown += OnKeyDown;
        PreviewKeyUp += OnKeyUp;
        TextInput += OnTextInput;
        Closing += (_, _) => SaveCalculator();

        // ~60 quadros por segundo: avança programas e faz o "running" piscar.
        _timer = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(16) };
        _timer.Tick += (_, _) =>
        {
            _calc.Tick(16);
            if (_calc.IsRunning || _calc.Display.Blinking)
                _view.InvalidateVisual();
        };
        _timer.Start();
        Loaded += (_, _) => _view.Focus();
    }

    private static Calculator LoadCalculator()
    {
        try
        {
            if (File.Exists(StatePath) && Calculator.LoadState(File.ReadAllText(StatePath)) is { } saved)
                return saved;
        }
        catch (IOException) { }
        return new Calculator();
    }

    private void SaveCalculator()
    {
        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(StatePath)!);
            File.WriteAllText(StatePath, _calc.SaveState());
        }
        catch (IOException) { }
    }

    private void ApplyZoom(double zoom)
    {
        _zoom = zoom;
        Width = _skin.Width * PointsPerMm * zoom;
        Height = _skin.Height * PointsPerMm * zoom;
    }

    private ContextMenu BuildMenu()
    {
        var menu = new ContextMenu();
        var copy = new MenuItem { Header = "Copiar X (Ctrl+C)" };
        copy.Click += (_, _) => Clipboard.SetText(_calc.XText);
        menu.Items.Add(copy);

        var zoom = new MenuItem { Header = "Zoom" };
        foreach (double z in Zooms)
        {
            var item = new MenuItem { Header = $"{z * 100:0}%" };
            item.Click += (_, _) => ApplyZoom(z);
            zoom.Items.Add(item);
        }
        menu.Items.Add(zoom);

        var top = new MenuItem { Header = "Sempre no topo", IsCheckable = true };
        top.Click += (_, _) => Topmost = top.IsChecked;
        menu.Items.Add(top);
        menu.Items.Add(new Separator());

        var exit = new MenuItem { Header = "Sair" };
        exit.Click += (_, _) => Close();
        menu.Items.Add(exit);
        menu.Items.Add(new Separator());
        menu.Items.Add(new MenuItem { Header = $"Protótipo C# • API {Calculator.ApiVersion}", IsEnabled = false });
        return menu;
    }

    /// <summary>Teclas do PC que precisam de pressionar/soltar (mesmo mapa da versão Rust).</summary>
    private static Key? MapKey(WpfKey key) => key switch
    {
        WpfKey.Enter => Key.Enter,
        WpfKey.Back or WpfKey.Delete => Key.Clx,
        WpfKey.Tab => Key.Swap,
        WpfKey.F => Key.F,
        WpfKey.G => Key.G,
        WpfKey.S => Key.Sto,
        WpfKey.R => Key.Rcl,
        WpfKey.N => Key.N,
        WpfKey.I => Key.I,
        WpfKey.P => Key.Pv,
        WpfKey.M => Key.Pmt,
        WpfKey.V => Key.Fv,
        WpfKey.E => Key.Eex,
        WpfKey.C => Key.Chs,
        WpfKey.D => Key.RollDown,
        _ => null,
    };

    private void OnKeyDown(object sender, KeyEventArgs e)
    {
        if (Keyboard.Modifiers.HasFlag(ModifierKeys.Control) && e.Key == WpfKey.C)
        {
            Clipboard.SetText(_calc.XText);
            e.Handled = true;
            return;
        }
        if (Keyboard.Modifiers != ModifierKeys.None || e.IsRepeat || MapKey(e.Key) is not { } key) return;
        _calc.Press(key);
        _view.SetPressed(key, true);
        e.Handled = true;
    }

    private void OnKeyUp(object sender, KeyEventArgs e)
    {
        if (MapKey(e.Key) is not { } key) return;
        _calc.Release(key);
        _view.SetPressed(key, false);
        e.Handled = true;
    }

    /// <summary>Dígitos e símbolos chegam como texto (respeita o layout do teclado).</summary>
    private void OnTextInput(object sender, TextCompositionEventArgs e)
    {
        foreach (char c in e.Text)
        {
            Key? key = c switch
            {
                >= '0' and <= '9' => (Key)Calculator.KeyFromToken(c.ToString())!,
                '.' or ',' => Key.Dot,
                '+' => Key.Add,
                '-' => Key.Sub,
                '*' => Key.Mul,
                '/' => Key.Div,
                _ => null,
            };
            if (key is { } k)
                _calc.Tap(k);
        }
        _view.InvalidateVisual();
    }
}

public static class Program
{
    /// <summary>
    /// Uso normal: abre a janela. Para testes:
    /// <c>HP12C.Wpf.exe --snapshot saida.png "12 ENTER 3 /"</c> desenha a calculadora
    /// numa imagem, sem abrir janela.
    /// </summary>
    [STAThread]
    public static int Main(string[] args)
    {
        if (args.Length >= 2 && args[0] == "--snapshot")
            return Snapshot(args[1], args.Length > 2 ? args[2] : "");
        new Application().Run(new MainWindow());
        return 0;
    }

    private static int Snapshot(string output, string keys)
    {
        var skin = Skin.Load(Path.Combine(AppContext.BaseDirectory, "skin.json"));
        using var calc = new Calculator();
        if (!calc.TapAll(keys))
            return 2;
        var view = new CalculatorView(skin, calc);
        double width = skin.Width * 96.0 / 25.4 * 1.5, height = skin.Height * 96.0 / 25.4 * 1.5;
        view.Measure(new Size(width, height));
        view.Arrange(new Rect(0, 0, width, height));
        view.UpdateLayout();
        var bitmap = new System.Windows.Media.Imaging.RenderTargetBitmap((int)width, (int)height, 96, 96, PixelFormats.Pbgra32);
        bitmap.Render(view);
        var encoder = new System.Windows.Media.Imaging.PngBitmapEncoder();
        encoder.Frames.Add(System.Windows.Media.Imaging.BitmapFrame.Create(bitmap));
        using var file = File.Create(output);
        encoder.Save(file);
        return 0;
    }
}
