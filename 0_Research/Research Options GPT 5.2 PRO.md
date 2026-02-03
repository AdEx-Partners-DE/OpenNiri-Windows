Do a deep research on the options, what is already available and reusability:

The short answer:
No one has fully done this yet. You cannot simply "reverse engineer" Niri for Windows because Niri is a Compositor (it draws the frames), whereas Windows locks you into using its own DWM (Desktop Window Manager).
To get Niri on Windows, you would have to rewrite it from scratch to "fight" the Windows display driver, which is essentially what existing projects are already doing.
1. Has someone already started?
Yes. The closest project is Komorebi.
While not a direct Niri clone, the developer (LGUG2Z) actively implements features inspired by the Linux tiling world. There are configurations for Komorebi that mimic the "infinite scroll" behavior, although it is functionally different under the hood.
Project: Komorebi (Open Source, GitHub)
Cost: Free (for personal use)
Vibe: It hooks deeply into the Windows API to force windows into tiles, similar to how Niri manages them.
2. Why can't we just "port" Niri?
It is a physics problem, not just a code problem.
On Linux (Niri): Niri is the display server (Wayland). It controls every pixel. It can say "Hey, render this window 5000 pixels to the right, off the screen," and the app obeys.
On Windows: Microsoft's DWM controls the screen. If you tell a window to move "off-screen" to create that infinite scroll effect, Windows tries to "snap" it back, hide it, or stop rendering it to save memory. A Niri clone on Windows has to constantly "fight" the OS to keep windows alive when they are scrolled out of view.
3. Your "Build it Yourself" Roadmap
If you (or a developer you know) wanted to build "OpenNiri-Windows", you wouldn't reverse engineer Niri. You would fork Komorebi and add the scrolling logic on top of it.
The Architecture:
Language: Rust (same as Niri and Komorebi).
Base: Fork LGUG2Z/komorebi.
The Missing Feature: Komorebi handles tiling. You would need to implement a "Virtual Coordinate System" where the workspace is 10,000px wide, and you map the visible 1920px viewport to that virtual space, instantly moving windows in/out of the "visible zone" when the user scrolls.
Verdict:
For now, install Komorebi. It is the only tool on Windows powerful enough to get close to what you want without writing a new display driver from scratch.
Thought for 21m 7s

Below is a “state of the landscape” style deep-research write‑up focused on (a) what’s already available on Windows today that’s closest to niri’s scrolling tiling, (b) what each option implies technically, and (c) what is realistically reusable (code + architecture + licensing) if you want to build an “OpenNiri‑Windows”.

0) One important update to your short answer (because it changed)

Your write‑up says “no one has fully done this yet” and that komorebi only has configs that mimic infinite scroll.

 

As of mid‑2025, komorebi implemented a first‑class “Scrolling” layout (not just a config hack), including:

A dedicated Scrolling layout value selectable per workspace.

A command to set visible columns for the scrolling layout (scrolling-layout-columns).

Declarative config support via layout_options.scrolling.columns.

So: someone has started, and the closest thing is not “a mimic”, it’s an implemented layout—with caveats (multi‑monitor constraints) that I’ll cover below.

1) What niri actually is (what you’re trying to replicate)

niri is a “scrollable‑tiling Wayland compositor”—i.e., it’s not just a tiler, it’s a compositor+display server in the Wayland stack.

 

Key niri characteristics that matter for a Windows “clone”:

Scrollable tiling as the core layout paradigm (the horizontal “strip” / scrollable columns idea).

Dynamic workspaces (GNOME‑like), overview, gestures, animations/shaders, etc.

KDL config with live reload.

Built atop Smithay (a Rust Wayland compositor framework) to handle the Wayland plumbing while niri focuses on its UI/tiling logic.

That “compositor” identity is the single biggest reason a direct “port” to Windows is not straightforward.

2) The Windows constraint: you cannot replace the compositor

On Windows, Desktop Window Manager (DWM) is always on (and APIs to enable/disable composition are deprecated since Windows 8).

 

That implies a hard architectural boundary:

On Wayland/niri: niri owns composition and can implement scrolling/animations as part of the scene graph.

On Windows: you can’t replace DWM with “niri‑win”. What you can do is run a userland window manager that manipulates window rectangles (move/resize/show/hide) while DWM still does the compositing.

This is exactly why Windows tilers describe themselves as extensions or helpers around DWM, not replacement compositors. (Komorebi explicitly positions itself this way.)

3) What’s already available on Windows (ranked by “niri-likeness”)
A) Closest to niri’s scroll behavior: komorebi (Scrolling layout)

What it is: A tiling window manager for Windows (Rust) that manages native windows via the Windows APIs (and sits “on top” of DWM).

 

Why it’s the closest today:

It includes a built‑in Scrolling layout (explicitly targeting “infinite scrollable tiling” style workflows).

You can set the number of visible columns dynamically using scrolling-layout-columns.

You can configure it declaratively (layout_options.scrolling.columns).

Critical caveat (multi‑monitor):
The author restricted early versions of the feature to avoid “windows bleeding onto adjacent monitors”, limiting Scrolling layout to single-monitor setups (or potentially only to configurations without horizontally adjacent monitors).

 

Animations:
Komorebi has optional movement animations with tunable duration and FPS, but the docs explicitly warn that higher FPS/duration increase CPU and the feature may show artifacts.

Reusability reality (license is the big deal)

Your short answer called komorebi “Open Source”. That’s not accurate in the OSI sense. It’s “source-available” under the Komorebi License (a PolyForm‑style license).

 

High-level constraints documented by the project:

Personal use is allowed.

Commercial use is not allowed without a separate license.

Redistribution / distributing new works (hard forks) is not allowed under the personal-use license framing.

So if your goal is “I want this workflow on my Windows machine”, komorebi is the best path today.

 

If your goal is “I want to build and publish OpenNiri‑Windows as a redistributable fork/product”, komorebi is a poor base because the license is intentionally hostile to redistribution.

B) Most reusable “real tiling WM” codebases (if you want to build a public Niri-like)

These aren’t niri-scroll clones today, but they are better legal/engineering bases for building one.

1) GlazeWM (Rust, GPL-3.0)

A tiling WM for Windows inspired by i3wm, with YAML config and multi-monitor support.

Open source under GPL‑3.0, and actively developed.

Reusability angle:

If you want to implement a new “Scrolling layout” inside an existing Windows tiler and publish it, GlazeWM is a strong candidate.

Licensing is also GPL‑family, which is compatible in spirit with niri’s GPL‑3.0 if you ever wanted to reuse code concepts (but see “code reuse” section below).

2) workspacer (C#, MIT)

A Windows tiling window manager with a plugin architecture (popular among people who want to hack/extend).

MIT license means you can reuse the code very freely.

Reusability angle:

Great if you want maximum freedom to publish, embed, or commercialize (MIT).

But: if you hoped to directly reuse niri source code (GPL), that becomes a licensing mismatch (you’d instead reimplement the algorithm, not copy code).

3) FancyWM (dynamic tiling, very user-friendly)

FancyWM is a “dynamic tiling” WM focused on being approachable on Windows (mouse + keyboard, no heavy config). It also integrates with Windows Virtual Desktops.

 

Why it’s not niri-like: it uses a panel/tree model (horizontal/vertical/stack panels), not a scrollable strip paradigm.

 

Why it may still matter: it documents real Windows constraints like respecting per-app min/max sizes via WM_GETMINMAXINFO, which any serious Windows tiler hits.

 

(If you’re evaluating “reusability”, FancyWM’s ecosystem also includes winman libraries that appear MIT-licensed per org listing, but the main fancywm licensing has varied over time—treat it as something you verify at build time.)

C) “Desktop environment / shell replacement” style options
Seelen UI

Seelen UI markets itself as a customizable “desktop environment” for Windows 10/11 and explicitly includes a “Tiling Windows Manager”.

 

Reusability angle:

This is less “tiling WM as a tool” and more “a bigger shell layer with tiling included”.

It can be attractive if you want an integrated experience (bar/launcher/widgets/tiling), but it’s a much larger surface area than “just implement niri-scroll”.

D) Built-in / official window arrangement tools (not true tilers, but pragmatic)
Windows 11 Snap Layouts

Good for quick, OS-native layouts; not a WM, not dynamic, not scrollable.

PowerToys FancyZones

A zone-based snapping tool: you define zones and snap windows into them (mouse/keyboard). Very useful, but not comparable to niri’s scroll paradigm.

4) “Reusability” in practice: what can actually be reused?
4.1 Reusing niri code directly (spoiler: very little, unless you’re building a compositor)

niri’s core is a Wayland compositor built on Smithay and implements Wayland protocols and compositor responsibilities.

 

A Windows tiler doesn’t speak Wayland; it manipulates HWNDs. So:

Compositor/Wayland integration code: not reusable for Windows window management.

Layout algorithm ideas (scrolling strip): reusable conceptually, but you’ll reimplement against Win32 abstractions.

One interesting partial reuse angle is niri’s configuration system:

niri uses KDL, live reload, and has a separated configuration crate (niri-config).
In theory, a GPL’d Windows WM could reuse that config approach. But it’s only worthwhile if you specifically want KDL + niri-like configuration semantics on Windows (and accept GPL constraints).

4.2 Reusing komorebi code (the best engineering base, the worst redistribution base)

Engineering-wise, komorebi is already deep into all the gnarly Windows edge cases, and it already has Scrolling layout.

 

But the license explicitly limits redistribution / hard forks and commercial use.

 

So the “reuse strategy” for komorebi is usually:

Contribute upstream (PRs / patches) rather than shipping a fork, if you want the community to benefit.

Or maintain a private patch set for personal use.

4.3 Reusing GlazeWM / workspacer code (best for a public “OpenNiri‑Windows”)

If your aim is a redistributable “niri-like scrolling tiler” for Windows:

GlazeWM (Rust, GPL‑3.0) is attractive if you want a Rust codebase + copyleft continuity and a larger user base.

workspacer (MIT) is attractive if you want maximal licensing flexibility and a plugin-oriented architecture.

5) What would a “niri-like scrolling tiler” on Windows actually need to implement?

At a high level, you need the same conceptual components niri has—but mapped onto Win32 constraints:

Core primitive: a viewport over a logical strip

Maintain a logical ordered list of columns/windows (infinite conceptual strip).

Maintain a viewport state: center_index (or left-most index) + visible_columns.

On scroll:

Update viewport state.

Recompute rects for visible columns.

Move/resize windows into those rects.

This is what komorebi’s Scrolling layout is already converging toward.

The “hard parts” on Windows (why it will never be 1:1)

Multi-monitor geometry: On Windows, moving a window horizontally can literally place it onto another monitor if you have side-by-side displays—hence komorebi’s “bleeding onto adjacent monitors” limitation.

App min/max constraints: Many Windows apps enforce minimum sizes; FancyWM explicitly calls out respecting WM_GETMINMAXINFO. Any scroll layout needs a strategy when too many columns would violate min widths.

Hide/off-screen strategy: A compositor can just not paint off-screen surfaces. A Windows WM must decide whether to:

Move windows off-screen,

Minimize them,

Or use a “cloak/hide” behavior (some WMs support DWM-based cloaking).
(This area is less standardized; you end up with app-specific edge cases.)

Smooth scrolling and animation

The only general way (without building an overlay compositor) is to animate window positions by repeated move/resize, which costs CPU and can artifact. Komorebi explicitly warns about CPU cost and artifacts in its animations feature.

6) Recommended decision tree (depending on what you want)
If you want “niri-feel on Windows” right now

Use komorebi + Scrolling layout.
That’s the closest actual implementation available today.
Just be aware of the multi-monitor caveat and the personal-use/non-redistribution licensing constraints.

If you want to build and publish a real “OpenNiri‑Windows”

Pick a base that you can legally redistribute:

GlazeWM if you want Rust + a big community + GPL‑3.

workspacer if you want MIT + plugins + maximum reuse freedom.

Then implement a new “ScrollingStrip” layout module similar in concept to niri/komorebi scrolling.

If you mainly want productivity tiling without scroll

FancyWM is arguably the easiest “tiling on Windows without becoming a config engineer,” plus virtual desktop integration.

FancyZones if you want “snap on steroids” rather than tiling WM behavior.

7) Bottom line

A literal niri “port” is blocked by Windows’ compositor architecture (DWM always on).

The niri-like scrolling workflow does exist on Windows now in the most serious Windows tiler (komorebi), but with practical constraints (notably multi-monitor) and a license that limits fork-based reuse.

If “reusability” means “can I build a redistributable OpenNiri-Windows from an existing codebase?”, the best candidates are GlazeWM (GPL-3, Rust) and workspacer (MIT, C#), not komorebi.

If you want, I can also translate this into a concrete “implementation plan” (modules, data structures, window lifecycle handling, and the exact command set you’d need to feel like niri), but the research conclusion above should already make the trade space and reuse constraints clear.

tell me why and what the restrictions are from komorebi regarding to fork and re-use as opensource for everyone? then provide me with a valid and approach, steps and first guid for my AI coding agent to get started with. Goal? clone all 3 repos? Reverse engineer Niri as of Feb 3rd 2026 ? Mission? Technology stack to consider and why, etc. etc.
Thought for 16m 13s
1) Komorebi: why you can’t fork it into an open-source “OpenNiri-Windows” for everyone
What komorebi’s license allows (high-level)

Komorebi’s own docs and README are unusually explicit about intent:

You can use it for personal use, but not for redistribution.

You may make a personal fork, and you may make changes intended to be sent back upstream via PRs.

You may not distribute “new works / hard-forks” (i.e., a public fork that you ship as your own project).

The license does not permit commercial use, including “using komorebi at work.”

There’s a paid Individual Commercial Use License that adds commercial use for a specific individual while keeping other restrictions in place.

What that means for your goal (“open source for everyone”)

If your plan is: fork komorebi → add niri-like behavior → publish the fork on GitHub under an OSI license, you run straight into the “no redistribution / no hard forks” restriction. That’s not a minor constraint; it’s a direct block.

 

Even if you publish only your modifications, a public fork is still distribution of a new work based on the codebase.

Why the license is structured this way (the “why”)

Two “why’s” show through in the official text:

Project positioning: “educational source software”
Komorebi explicitly frames itself as “educational source software” (source-visible, not open-source in the OSI sense).

Economic / sustainability: commercial use is monetized
They offer an Individual Commercial Use License for paid workplace usage, while disallowing commercial use by default.

So the restrictions (no redistribution/hard forks + no commercial use) appear designed to:

prevent a competitive ecosystem of public forks, and

keep commercial usage as an explicit, paid permission.

Important: komorebi is “source-available”, not OSI “Open Source”

The Open Source Initiative is also explicit: Open Source licenses cannot restrict field of endeavor (e.g., “no commercial use”), and can’t require “purchase/registration” as an extra step to remove restrictions.

 

That doesn’t make komorebi “bad” — but it does mean you can’t meet your stated goal (“open-source for everyone”) by building on komorebi code.

Non-legal-advice translation: You can study komorebi and use it personally. You can contribute upstream. You cannot publish an open-source “komorebi-derived OpenNiri-Windows” fork.

2) Reality check as of Feb 3, 2026: komorebi already has a “Scrolling” layout (but the license still blocks your fork)

Two key points that matter for your “deep research”:

Komorebi already implements a scrolling layout concept

There is an official CLI command to set visible columns for the Scrolling layout:

komorebic.exe scrolling-layout-columns <COUNT>

And discussion by the author indicates:

“Scrolling layout” landed in a dedicated branch (“nirily”),

it supports layout_options.scrolling.columns,

it supports resizing (via komorebi resize commands, etc.),

and it was shipped via nightly at that time.

It also exposes the exact “Windows constraints” you’ll face

Komorebi’s docs spell out “window hiding behavior” options and explicitly mention that cloaking uses an undocumented function:

hide (deprecated / issues)

minimize (issues)

cloak (undocumented SetCloak)

This is basically the core workaround for the “infinite canvas” problem on Windows: windows outside the viewport need to be hidden/cloaked reliably.

Known limitation: multi-monitor adjacency can cause “bleeding”

In the “Infinite Scrollable Tiling” discussion, LGUG2Z notes the initial implementation restricted Scrolling layout to avoid cases where windows “bleed” onto adjacent monitors.

 

That matters for your clone: multi-monitor + infinite horizontal canvas is one of the hardest parts to get right on Windows.

3) Your actual options if the goal is “OpenNiri-Windows” as a true open-source project

You have three viable strategies, depending on what “open-source for everyone” means in practice.

Option A — Build an open-source extension for komorebi (legal, but not “OpenNiri”)

You don’t fork komorebi. You write:

a config generator

an IPC client that drives komorebi (its CLI / pipe / socket features)

extra UX tooling (overview UI, gestures, etc.)

This can be fully open-source because it’s a separate work that merely controls komorebi. But users still need komorebi installed, and komorebi’s own license still governs komorebi.
If your “for everyone” includes “everyone can redistribute the WM”, Option A does not satisfy the mission.

Option B — Implement scrolling layout in an OSI open-source Windows tiler (best fit)

Two strong candidates:

 

GlazeWM (Rust, GPL-3.0)

It’s a Windows tiling WM inspired by i3 and written in Rust.

There’s explicit demand for “Scrollable tiling like Niri or PaperWM”.

If you ship under GPL-3.0, commercial use is allowed, redistribution is allowed, modifications are allowed (with copyleft).

workspacer (C#, MIT)

MIT-licensed, explicitly open.

If you want permissive licensing (Apache/MIT style), workspacer is the cleanest “reuse” base — but you’ll be in C# and likely rewriting a lot for the exact niri feel.

Option C — Clean-room implement a new WM from scratch (max control)

If you want:

Rust like niri,

permissive license (MIT/Apache),

and zero GPL entanglement,

…you implement from scratch and only use niri as a behavioral reference (ideas, not code), or you reuse only what’s compatible with your chosen license.

4) “Reverse engineer Niri” (as of Feb 3, 2026): what you should actually do

First: “Reverse engineer” doesn’t mean “decompile”. Niri is open-source, and its behavior is documented.

 

Niri’s defining behavior:

windows arranged in columns on an infinite horizontal strip per monitor,

opening a new window does not resize existing windows (predictability),

focus/scroll moves a viewport over that strip.

What you want is a Windows implementation of the paradigm, not a port of the compositor.

 

So your “reverse engineering” outputs should be:

a behavioral spec (what happens when you open/focus/move/resize)

a layout math spec (column widths, viewport rules, centering rules)

a command model (focus left/right, move, resize, overview, etc.)

a Windows feasibility map (what must be emulated with cloaking/hiding)

5) Windows constraints you must design around (the real blockers)

This is the “why it’s hard” portion, but grounded in Windows realities.

A) DWM is the compositor; you are a userland window controller

You cannot “be the display server” like niri is on Wayland. You are moving HWNDs around and asking DWM to composite them.

B) Off-screen/occluded windows behave differently

Windows apps can stop rendering when occluded to save power (DXGI pattern); this is an OS-level expectation.
For a scrolling WM, that’s OK if you hide/cloak non-visible windows cleanly — but it affects previews, thumbnails, and overview modes.

C) Hiding/cloaking is central

Microsoft documents cloaking via DwmSetWindowAttribute with DWMWA_CLOAK (in DirectComposition scenarios).
Komorebi exposes cloak as its preferred “window hiding behavior” and notes it uses an undocumented SetCloak.

 

Design implication: you need an abstraction like:

HideStrategy::CloakDwmAttribute (if feasible)

HideStrategy::UndocumentedSetCloak (if you’re willing)

HideStrategy::Minimize

HideStrategy::Hide (last resort; komorebi says it’s EOL and problematic)

D) Window size constraints exist and must be respected

Apps can define min/max tracking sizes via WM_GETMINMAXINFO.
As an external WM you can’t reliably query those limits without tricks, so you usually:

attempt resize,

detect refusal/clamping,

and “float” windows that won’t fit.

6) Recommended mission and stack
Mission statement

Build OpenNiri-Windows: an OSI open-source Windows tiling window manager that implements the scrollable-tiling paradigm (niri/PaperWM style) using Win32 window control + DWM-friendly hiding/cloaking. Prioritize per-monitor correctness and predictable window sizing over fancy effects.

Stack recommendation

If your goal is “closest to niri, long-term maintainable, best reuse surface”:

 

Language: Rust

matches niri’s ecosystem and your prior direction

good for correctness in state/event-heavy code

Win32 bindings: windows (windows-rs)

modern Rust projections of Win32 APIs

Architecture: split the “layout brain” from the “Windows hands”

core-layout crate (pure, testable)

platform-win32 crate (HWND, hooks, DPI, monitors)

daemon crate (event loop, state machine)

cli crate (commands)

optional ui-overlay crate later (overview)

Config: start simple

If you base on GlazeWM: keep YAML to integrate quickly.

If you want niri-like configs: consider a dedicated config layer later.

IPC: Named pipes or local sockets for commands/events
(you’ll want this for scripting and for an eventual bar)

7) The “clone 3 repos?” answer (with the legal-safe framing)

If you want maximum learning with minimal licensing risk:

Clone niri (behavior + layout paradigm reference; GPL code is reusable only if you accept GPL)

Clone GlazeWM (open-source Rust WM base; good reuse candidate)

Clone komorebi (reference implementation of Windows tiling + hiding/cloaking strategies; do not base your distributable WM on its code)

Optional: clone workspacer if you want a permissive-license baseline in C#.

8) First guide for your AI coding agent: concrete steps to start building

Below is a practical “agent brief” you can paste into your coding agent. It’s ordered so that each step produces a runnable artifact.

Step 0 — Decide your licensing strategy (this determines reuse)

Agent task:

Create LICENSE decision doc with two paths:

Path 0A (GPL path): You may reuse GPL code from niri/glazewm; project must be GPL-compatible.
Path 0B (permissive path): You must not copy code from GPL projects; use them only as behavioral reference.

 

Given your “for everyone” wording, GPL is usually acceptable (commercial use allowed), but decide explicitly.

Step 1 — Repo setup (Rust workspace skeleton)

Agent deliverable:

Create a Cargo workspace:

openniri-win/
  crates/
    core_layout/
    platform_win32/
    daemon/
    cli/
  docs/
    SPEC.md


Add CI (GitHub Actions) to run cargo test and cargo fmt --check.

Step 2 — Minimal Win32 “window mover” (prove you can control HWNDs)

Agent task:

In platform_win32, implement:

enumerate top-level windows (filter visible, not tool windows)

get/set window rect

basic “move focused window to left half/right half” command

Success criteria: a CLI command that reliably moves a normal app window.

Step 3 — Event ingestion (you can’t tile without events)

Agent task:

Install WinEvent hooks:

foreground change (focus)

object create/destroy (new windows)

minimize / restore

Push events into a channel; process them on a single state thread.

Success criteria: logs show window create/focus/destroy events in real time.

Step 4 — Monitor + work area correctness (per-monitor is non-negotiable)

Agent task:

Enumerate monitors and their work areas.

Maintain MonitorId -> Rect map.

Assign each managed window to a monitor (by current rect intersection).

Success criteria: moving a window between monitors updates its monitor assignment.

Step 5 — Implement the core scrollable tiling model in core_layout

Agent task:

Write a pure layout model:

Workspace:
  columns: Vec<Column>
  focused_column: usize
  scroll_x: i32
  visible_columns: usize
Column:
  width: i32
  windows: Vec<WindowId>   // vertical stack within a column


Implement functions:

insert_window_right_of_focus(window)

focus_left/right()

resize_column(delta)

ensure_focused_visible(mode: Centered|JustInView)

compute_placements(viewport_rect) -> Vec<(WindowId, Rect, VisibleFlag)>

Tie to niri: infinite horizontal strip + stable window sizes.

 

Success criteria: unit tests confirm placements for typical scenarios.

Step 6 — Apply placements on Windows + hide out-of-viewport windows

Agent task:

In platform_win32, implement “visibility strategy”:

Preferred: cloak/uncloak

Try a DWM-based cloaking approach (DwmSetWindowAttribute) if viable.
Fallbacks:

minimize

hide (last resort; known problematic)

Also implement “don’t bleed onto adjacent monitors” by hiding non-visible columns instead of physically placing them on the next monitor (this is a known trap in scrolling layouts).

 

Success criteria: when scrolling/focusing across many columns, only the visible set is shown on-screen; others are not visible on any monitor.

Step 7 — Command surface (CLI + hotkeys)

Agent task:

Provide CLI commands:

focus left/right/up/down

scroll left/right

set visible_columns N

move window to new column, etc.

Komorebi exposes a similar concept (scrolling-layout-columns)—this is a proven control surface.

 

Then:

add global hotkeys (RegisterHotKey) for MVP.

Step 8 — “Niri parity mapping” document

Agent task:

Write docs/SPEC.md with a table of:

niri behavior → Windows equivalent

limitations (elevated windows, cloaking quirks, multi-monitor adjacency)

what you will/won’t implement in MVP

9) What I’d pick if I were optimizing for success

Given your stated mission (“open-source for everyone” + “niri-like”), the most direct path is:

Base on GlazeWM (GPL-3.0) and implement a new Scrolling layout inspired by niri.

Use niri as the UX spec (and optionally code reference if you accept GPL).

Use komorebi docs/issues as Windows reality checks (especially hiding/cloaking and multi-monitor pitfalls), but do not derive your distributable code from komorebi due to the no-redistribution/no-hard-forks license.

If you tell me your preferred license outcome (GPL vs permissive), I can tighten the plan into a “no ambiguity” engineering spec (module boundaries + exact Win32 APIs + test plan) without needing any more web lookups.