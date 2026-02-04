Quote

Theo - t3.gg

@theo

·
Feb 1

 Article

The Agentic Code Problem
*ding*
You hear a notification sound from a Claude Code workflow finishing. Which terminal tab was it?
Hop around terminal windows and tabs for a bit, finally find it. It was Project B. Okay, now...

Please research more about Nary and if there are alternative Windows solutions that are similar.  https://x.com/theo/status/2018091358251372601?s=20

Thought for 1m 56s

The X post you linked from @theo
 (Theo Browne, dated Feb 1, 2026) shares an article discussing pain points in modern AI/agentic coding workflows. Developers often end up with dozens of separate windows and tabs open simultaneously — multiple AI agent sessions (e.g., Claude/Cursor/Codex tabs), code editors, git diff views, browser previews, terminals, and more — leading to excessive context switching and reduced efficiency. Replies echo this, with ideas like better orchestration GUIs or an "OS-level tab system" to group/color-code windows across apps.I could not find any reference to a tool, app, or software called "Nary" in the post, article link, replies (265+ scanned), Theo's recent activity, or broader searches. It may be a typo, misremembering, or perhaps a reference to the archaic word "nary" (meaning "not any") if the article complains about the lack of integrated solutions.Assuming the query seeks Windows alternatives to macOS window/tab management tools (which Theo, as a macOS user, likely relies on for similar issues), here are the strongest options. These directly help organize the kind of multi-window chaos described in the post.Recommended Windows ToolsCategory
Tool
Key Features
Pricing
Why It Helps with the Post's Problem
Snapping/Layouts
PowerToys FancyZones
Custom grid zones, keyboard shortcuts for snapping, multi-monitor support
Free (Microsoft)
Precise organization of many windows without overlap
Advanced Snapping
AquaSnap
Tiling, shaking to maximize, multi-monitor, aqua glue/aero shake
Free/Paid
More flexible than built-in Windows snapping
Tiling Manager
Komorebi
Full automatic/manual tiling, keyboard-driven, highly configurable (Rust-based)
Free
See multiple agent windows side-by-side effortlessly
Tiling Manager
FancyWM
Dynamic tiling, floating windows support
Free
Lightweight alternative for automatic layouts
Tab Grouping
Groupy 2 (Stardock)
Tabs any windows/apps together, saved groups, Chrome-like interface
Paid
Closest to "OS-level tabs" – group unrelated apps into one window
Tab Grouping
TidyTabs
Adds tabs to any window's title bar, auto-grouping rules
Free/Paid
Quick tabbing without full regrouping

PowerToys FancyZones examples:learn.microsoft.com

learn.microsoft.com

learn.microsoft.com

learn.microsoft.com

Komorebi tiling examples:xda-developers.com

reddit.com

reddit.com

Groupy 2 tab grouping examples:neowin.net

stardock.com

stardock.com

TidyTabs example:apps.microsoft.com

These tools (especially FancyZones + Komorebi or Groupy) are popular among developers for handling complex, multi-app workflows. Start with PowerToys — it's free and integrates natively.If this doesn't match what you meant by "Nary," share more details (e.g., a screenshot or quote from the thread) for better research!

Compare Komorebi vs FancyWM

Linux tiling window managers

I meant niri (not nary) which only runs on linux...

Thought for 38s

Niri is a modern, Rust-based scrollable tiling Wayland compositor exclusively for Linux. It arranges windows in columns on an infinite horizontal strip: you scroll left/right to navigate, and new windows append without resizing existing ones—ideal for managing dozens of AI/agent tabs, editors, terminals, and previews without constant resizing or overlap.Key features include:Smooth scrolling animations and workspace overview.
Keyboard-driven controls (highly configurable via ~/.config/niri/config.kdl).
Multi-monitor support, gaps, borders, and output scaling.
Lightweight design with excellent performance on Wayland.

It's praised in communities (e.g., Reddit, Hacker News, Phoronix) for reducing context switching in complex workflows like the one Theo described.Niri screenshots (showing the infinite scroll in action):phoronix.com

reddit.com

youtube.com

wiki.archcraft.io

No direct Windows equivalent exists with Niri's exact infinite scrolling paradigm (searches confirm this; the closest macOS attempt is Paneru). However, several excellent tiling window managers for Windows 10/11 can approximate the efficiency for multi-window coding chaos, especially with keyboard-driven layouts and stacking.Closest Windows AlternativesTool
Key Similarities to Niri
Differences/Limitations
Pricing
Why It Helps with Multi-Window Workflows
Komorebi
Keyboard-driven tiling, stacking/monocle modes (full-screen focus), highly configurable layouts
Fixed workspaces (no infinite scroll), requires setup (AutoHotkey integration)
Free (open-source)
Best overall—developers love it for i3-like control on Windows
GlazeWM
Horizontal/vertical stacking, dynamic tiling, gaps/borders, smooth animations
No infinite horizontal strip; more traditional tiling
Free (open-source)
Great for side-by-side agents/editors with transparency ricing
FancyWM
Dynamic tiling, panel support, floating windows, keyboard shortcuts
More manual panel management than automatic scrolling
Free (Microsoft Store)
Lightweight and intuitive for grouping many apps
PowerToys FancyZones
Custom zones/grids, snap shortcuts, multi-monitor
Manual snapping (not fully automatic tiling)
Free (Microsoft)
Quick start—no extra software beyond PowerToys

Komorebi screenshots:github.com

deskto.ps

reddit.com

GlazeWM screenshots:reddit.com

reddit.com

FancyWM screenshots:reddit.com

github.com

Recommendation: Start with Komorebi if you want deep customization (many ex-Linux users switch to it). Pair any with PowerToys FancyZones for hybrid workflows. If you're open to dual-booting or WSL, Niri itself is fantastic on Linux! Let me know if you need setup guides.
