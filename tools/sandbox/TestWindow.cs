// Minimal resizable test window for the sandbox end-to-end test.
// Usage: TestWindow.exe <title> <activation-log-path>
using System;
using System.Drawing;
using System.IO;
using System.Windows.Forms;

public static class TestWindowProgram
{
    [STAThread]
    public static void Main(string[] args)
    {
        string title = args.Length > 0 ? args[0] : "OpenNiri E2E";
        string log = args.Length > 1 ? args[1] : null;
        var form = new Form();
        form.Text = title;
        form.Width = 700;
        form.Height = 500;
        form.StartPosition = FormStartPosition.Manual;
        form.Left = 100 + Math.Abs(title.GetHashCode() % 300);
        form.Top = 100;
        int hue = Math.Abs(title.GetHashCode()) % 200;
        form.BackColor = Color.FromArgb(55 + hue / 2, 90, 255 - hue);
        var label = new Label();
        label.Text = title;
        label.Dock = DockStyle.Fill;
        label.TextAlign = ContentAlignment.MiddleCenter;
        label.Font = new Font("Segoe UI", 28, FontStyle.Bold);
        label.ForeColor = Color.White;
        form.Controls.Add(label);
        if (log != null)
        {
            form.Activated += delegate
            {
                try { File.AppendAllText(log, DateTime.UtcNow.ToString("o") + " activated " + title + Environment.NewLine); }
                catch { }
            };
        }
        Application.Run(form);
    }
}
