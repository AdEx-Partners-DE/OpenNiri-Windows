// Win32 helpers for the Windows Sandbox end-to-end test (guest-e2e.ps1).
// Compiled at run time with Add-Type; keep it C# 5 compatible (PowerShell 5.1).
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;

public static class E2E
{
    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left, Top, Right, Bottom; }

    [StructLayout(LayoutKind.Sequential)]
    public struct POINT { public int X, Y; }

    [StructLayout(LayoutKind.Sequential)]
    struct MOUSEINPUT { public int dx, dy; public uint mouseData, dwFlags, time; public IntPtr dwExtraInfo; }

    [StructLayout(LayoutKind.Sequential)]
    struct KEYBDINPUT { public ushort wVk, wScan; public uint dwFlags, time; public IntPtr dwExtraInfo; }

    [StructLayout(LayoutKind.Explicit)]
    struct INPUT
    {
        [FieldOffset(0)] public uint type;
        [FieldOffset(8)] public MOUSEINPUT mi;
        [FieldOffset(8)] public KEYBDINPUT ki;
    }

    delegate bool EnumWindowsProc(IntPtr hwnd, IntPtr lParam);

    [DllImport("user32.dll")] static extern bool EnumWindows(EnumWindowsProc cb, IntPtr lParam);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr hwnd);
    [DllImport("user32.dll")] static extern bool IsWindow(IntPtr hwnd);
    [DllImport("user32.dll")] static extern bool IsIconic(IntPtr hwnd);
    [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr hwnd, out RECT rect);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowTextW(IntPtr hwnd, StringBuilder sb, int max);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassNameW(IntPtr hwnd, StringBuilder sb, int max);
    [DllImport("user32.dll")] static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] static extern IntPtr GetAncestor(IntPtr hwnd, uint flags);
    [DllImport("user32.dll")] static extern IntPtr GetWindow(IntPtr hwnd, uint cmd);
    [DllImport("user32.dll")] static extern int GetSystemMetrics(int index);
    [DllImport("user32.dll")] static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] static extern bool GetCursorPos(out POINT p);
    [DllImport("user32.dll")] static extern uint SendInput(uint n, INPUT[] inputs, int size);
    [DllImport("user32.dll")] static extern IntPtr WindowFromPoint(POINT p);
    [DllImport("user32.dll")] static extern bool SetProcessDpiAwarenessContext(IntPtr value);
    [DllImport("dwmapi.dll")] static extern int DwmGetWindowAttribute(IntPtr hwnd, int attr, out int value, int size);

    const uint INPUT_MOUSE = 0, INPUT_KEYBOARD = 1;
    const uint MOUSEEVENTF_MOVE = 0x1, MOUSEEVENTF_LEFTDOWN = 0x2, MOUSEEVENTF_LEFTUP = 0x4, MOUSEEVENTF_ABSOLUTE = 0x8000, MOUSEEVENTF_VIRTUALDESK = 0x4000;
    const uint KEYEVENTF_KEYUP = 0x2;
    const int DWMWA_CLOAKED = 14;

    public static void InitDpi()
    {
        // Per-monitor aware v2, same as the daemon, so coordinates match.
        try { SetProcessDpiAwarenessContext(new IntPtr(-4)); } catch { }
    }

    public class WinInfo
    {
        public long Hwnd; public uint Pid; public string Title; public string Class;
        public int X, Y, W, H; public bool Visible, Iconic; public int Cloaked;
    }

    public static WinInfo Describe(IntPtr h)
    {
        var info = new WinInfo();
        info.Hwnd = h.ToInt64();
        uint pid; GetWindowThreadProcessId(h, out pid); info.Pid = pid;
        var sb = new StringBuilder(512); GetWindowTextW(h, sb, sb.Capacity); info.Title = sb.ToString();
        var cb = new StringBuilder(256); GetClassNameW(h, cb, cb.Capacity); info.Class = cb.ToString();
        RECT r; GetWindowRect(h, out r);
        info.X = r.Left; info.Y = r.Top; info.W = r.Right - r.Left; info.H = r.Bottom - r.Top;
        info.Visible = IsWindowVisible(h); info.Iconic = IsIconic(h);
        int cloaked; info.Cloaked = DwmGetWindowAttribute(h, DWMWA_CLOAKED, out cloaked, 4) == 0 ? cloaked : -1;
        return info;
    }

    /// Visible, unowned top-level windows with a title.
    public static List<WinInfo> TopLevelWindows()
    {
        var list = new List<WinInfo>();
        EnumWindows(delegate (IntPtr h, IntPtr l)
        {
            if (!IsWindowVisible(h)) return true;
            if (GetWindow(h, 4 /* GW_OWNER */) != IntPtr.Zero) return true;
            var info = Describe(h);
            if (info.Title.Length > 0) list.Add(info);
            return true;
        }, IntPtr.Zero);
        return list;
    }

    public static long Foreground()
    {
        var h = GetForegroundWindow();
        if (h == IntPtr.Zero) return 0;
        var root = GetAncestor(h, 2 /* GA_ROOT */);
        return (root == IntPtr.Zero ? h : root).ToInt64();
    }

    public static int[] VirtualScreen()
    {
        return new int[] { GetSystemMetrics(76), GetSystemMetrics(77), GetSystemMetrics(78), GetSystemMetrics(79) };
    }

    /// Root window under a screen point.
    public static long WindowAt(int x, int y)
    {
        POINT p; p.X = x; p.Y = y;
        var h = WindowFromPoint(p);
        if (h == IntPtr.Zero) return 0;
        var root = GetAncestor(h, 2);
        return (root == IntPtr.Zero ? h : root).ToInt64();
    }

    static INPUT MouseAbs(int x, int y, uint flags)
    {
        var vs = VirtualScreen();
        var input = new INPUT();
        input.type = INPUT_MOUSE;
        input.mi.dx = (int)Math.Round((x - vs[0]) * 65535.0 / Math.Max(1, vs[2] - 1));
        input.mi.dy = (int)Math.Round((y - vs[1]) * 65535.0 / Math.Max(1, vs[3] - 1));
        input.mi.dwFlags = flags | MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK;
        return input;
    }

    public static void Click(int x, int y)
    {
        var inputs = new INPUT[] { MouseAbs(x, y, 0), MouseAbs(x, y, MOUSEEVENTF_LEFTDOWN), MouseAbs(x, y, MOUSEEVENTF_LEFTUP) };
        SendInput((uint)inputs.Length, inputs, Marshal.SizeOf(typeof(INPUT)));
    }

    /// Inject a mouse move and return milliseconds until the cursor reaches it (-1 on timeout).
    /// A low-level mouse hook on a thread that does not pump messages delays this
    /// by the hook timeout on every event.
    public static double CursorLatencyMs(int x, int y, int timeoutMs)
    {
        var inputs = new INPUT[] { MouseAbs(x, y, 0) };
        var sw = Stopwatch.StartNew();
        SendInput(1, inputs, Marshal.SizeOf(typeof(INPUT)));
        while (sw.ElapsedMilliseconds < timeoutMs)
        {
            POINT p; GetCursorPos(out p);
            if (Math.Abs(p.X - x) <= 1 && Math.Abs(p.Y - y) <= 1) return sw.Elapsed.TotalMilliseconds;
            Thread.Sleep(1);
        }
        return -1;
    }

    /// Press modifier+key chords, e.g. Chord(0x12, 0x4C) for Alt+L.
    public static void Chord(params ushort[] vks)
    {
        var inputs = new List<INPUT>();
        foreach (var vk in vks) { var i = new INPUT(); i.type = INPUT_KEYBOARD; i.ki.wVk = vk; inputs.Add(i); }
        for (int k = vks.Length - 1; k >= 0; k--) { var i = new INPUT(); i.type = INPUT_KEYBOARD; i.ki.wVk = vks[k]; i.ki.dwFlags = KEYEVENTF_KEYUP; inputs.Add(i); }
        SendInput((uint)inputs.Count, inputs.ToArray(), Marshal.SizeOf(typeof(INPUT)));
    }

    [DllImport("advapi32.dll", CharSet = CharSet.Unicode)]
    static extern uint GetNamedSecurityInfoW(string name, int objectType, uint securityInfo,
        IntPtr owner, IntPtr group, IntPtr dacl, IntPtr sacl, out IntPtr securityDescriptor);
    [DllImport("advapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    static extern bool ConvertSecurityDescriptorToStringSecurityDescriptorW(IntPtr sd, uint revision,
        uint securityInfo, out IntPtr sddl, IntPtr length);
    [DllImport("kernel32.dll")] static extern IntPtr LocalFree(IntPtr mem);

    /// Owner and DACL of a named pipe (or file) as SDDL, or "error N".
    public static string SecuritySddl(string path)
    {
        const uint OWNER = 0x1, DACL = 0x4;
        IntPtr sd;
        uint err = GetNamedSecurityInfoW(path, 1 /* SE_FILE_OBJECT */, OWNER | DACL, IntPtr.Zero, IntPtr.Zero, IntPtr.Zero, IntPtr.Zero, out sd);
        if (err != 0) return "error " + err;
        try
        {
            IntPtr sddl;
            if (!ConvertSecurityDescriptorToStringSecurityDescriptorW(sd, 1, OWNER | DACL, out sddl, IntPtr.Zero))
                return "error " + Marshal.GetLastWin32Error();
            string text = Marshal.PtrToStringUni(sddl);
            LocalFree(sddl);
            return text;
        }
        finally { LocalFree(sd); }
    }

    public static void Screenshot(string path)
    {
        var vs = VirtualScreen();
        using (var bmp = new Bitmap(vs[2], vs[3]))
        using (var g = Graphics.FromImage(bmp))
        {
            g.CopyFromScreen(vs[0], vs[1], 0, 0, new Size(vs[2], vs[3]));
            bmp.Save(path, ImageFormat.Png);
        }
    }
}
