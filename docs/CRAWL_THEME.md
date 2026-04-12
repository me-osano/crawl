# crawl-theme

Theme management for the crawl ecosystem — predefined palettes and
matugen wallpaper-driven dynamic theming, delivered to GTK, Ghostty,
and Quickshell over a single SSE event stream.

---

## Overview

```
crawl theme --set=rose-pine           # switch predefined theme
crawl theme --wallpaper=~/wall.jpg    # set wallpaper + generate palette via matugen
crawl theme --dark / --light          # toggle variant
crawl theme --list                    # see all available themes
crawl theme                           # show current palette + status
```

Every time the palette changes — whether from a preset switch or matugen
finishing — crawl writes updated configs for GTK, Ghostty, and shell
environment variables, then fires a `PaletteChanged` event over the SSE
stream that Quickshell reacts to instantly with no restart required.

---

## Architecture

```
                ┌─────────────────────────────────┐
                │          crawl-theme             │
                │                                  │
  predefined ───┤  themes.rs                       │
  TOML files    │  (15 built-ins + user ~/.config) │
                │                                  │
  wallpaper ────┤  matugen.rs                      │──► Palette struct
  inotify watch │  (subprocess + Material You map) │
                │                                  │
                └──────────────┬──────────────────┘
                               │
              ┌────────────────┼────────────────┐
              │                │                │
         writers/          writers/         writers/
         gtk.rs             ghostty.rs       shell.rs + json.rs
              │                │                │
    ~/.config/gtk-{3,4}.0/  ~/.config/ghostty/  ~/.config/crawl/
    colors.css              themes/crawl        theme.sh
    gtk.css (import)                            theme.zsh
                                                current-palette.json
              └────────────────┴────────────────┘
                               │
                    CrawlEvent::Theme(PaletteChanged)
                               │
                        SSE /events stream
                               │
                    Quickshell CrawlTheme.qml
                               │
                       Theme.qml singleton
                    (18 bound color properties)
                               │
                  Every QML component in the shell
```

---

## Files

### Rust (crawl-theme crate)

| File | Purpose |
|---|---|
| `src/lib.rs` | Domain entry point, `Config`, error type, `ThemeEvent` enum, domain runner, public API |
| `src/palette.rs` | `Palette` struct (18 roles), `Variant`, `ThemeSource`, `ThemeState`, validation |
| `src/themes.rs` | Predefined palette registry — 15 built-ins + user TOML loader |
| `src/matugen.rs` | matugen subprocess runner + Material You → semantic palette mapping |
| `src/writers/gtk.rs` | GTK3 + GTK4 CSS custom property writer |
| `src/writers/ghostty.rs` | Ghostty theme file writer (16-color ANSI + cursor/selection) |
| `src/writers/shell.rs` | `theme.sh` (POSIX) + `theme.zsh` (associative array) writer |
| `src/writers/json.rs` | `current-palette.json` canonical output |
| `src/router_theme.rs` | axum handler stubs for `/theme/*` routes |
| `src/cli_cmd_theme.rs` | clap `ThemeArgs` + handler for `crawl theme` subcommand |

### Quickshell / QML

| File | Purpose |
|---|---|
| `quickshell/theme/Theme.qml` | Singleton holding all 18 color properties + `applyState()` |
| `quickshell/theme/CrawlTheme.qml` | SSE consumer — startup load + live updates |
| `quickshell/theme/ThemeTransition.qml` | Optional animated transition wrapper |
| `quickshell/theme/qmldir` | Module registration |

### Config

| File | Purpose |
|---|---|
| `config/theme-section.toml` | The `[theme]` block to add to `~/.config/crawl/crawl.toml` |
| `config/themes/my-custom-theme.toml` | Annotated example custom theme TOML |

---

## Built-in themes

| Name | Style |
|---|---|
| `catppuccin-mocha` | Mocha — dark purple/pastel |
| `catppuccin-macchiato` | Macchiato — slightly lighter |
| `catppuccin-frappe` | Frappé — medium dark |
| `catppuccin-latte` | Latte — light variant |
| `rose-pine` | Rosé Pine — warm dark |
| `rose-pine-moon` | Moon — cooler dark |
| `rose-pine-dawn` | Dawn — light warm |
| `tokyo-night` | Deep blue-dark |
| `tokyo-night-storm` | Storm — slightly lighter |
| `nord` | Arctic cool blue-grey |
| `gruvbox-dark` | Warm earthy dark |
| `gruvbox-light` | Warm earthy light |
| `dracula` | Classic purple-dark |
| `one-dark` | Atom One Dark |
| `kanagawa` | Japanese ink aesthetic |

---

## Custom themes

Drop a `.toml` file in `~/.config/crawl/themes/`:

```toml
# ~/.config/crawl/themes/my-theme.toml
name    = "My Theme"
variant = "dark"

[palette]
base      = "#1a1a2e"
mantle    = "#16213e"
crust     = "#0f3460"
surface0  = "#1f2b47"
surface1  = "#263355"
surface2  = "#2e3d64"
text      = "#e2e2e2"
subtext1  = "#c0c0c0"
subtext0  = "#9e9e9e"
primary   = "#e94560"
secondary = "#0f3460"
tertiary  = "#53d8fb"
error     = "#ff4444"
warning   = "#ffaa00"
info      = "#4fc3f7"
overlay0  = "#404860"
overlay1  = "#505878"
overlay2  = "#606890"
```

Then: `crawl theme --set=my-theme`

---

## matugen / dynamic theming

### Install matugen

```bash
# AUR
yay -S matugen

# or from source
cargo install matugen
```

### Enable dynamic mode

```toml
# ~/.config/crawl/crawl.toml
[theme]
active      = "dynamic"
variant     = "dark"
wallpaper_cmd = "swww img {path} --transition-type fade --transition-duration 0.8"
```

### Set a wallpaper

```bash
crawl theme --wallpaper=~/Pictures/forest.jpg
```

crawl will:
1. Set the wallpaper via `swww` (or your configured command)
2. Write the path to `~/.config/crawl/current_wallpaper`
3. Run `matugen image <path> --json hex`
4. Map Material You color roles → semantic Palette
5. Write GTK CSS, Ghostty theme, shell vars, JSON
6. Fire `PaletteChanged` — Quickshell updates instantly

### Material You role mapping

| Material You role | Semantic role | Rationale |
|---|---|---|
| `surface` | `base` | Main background |
| `surface_container_lowest` | `mantle` | Slightly recessed surface |
| `surface_dim` | `crust` | Darkest surface layer |
| `surface_container` | `surface0` | Card/input background |
| `surface_container_high` | `surface1` | Hover state |
| `surface_container_highest` | `surface2` | Active/pressed state |
| `on_surface` | `text` | Primary readable text |
| `on_surface_variant` | `subtext1` | Muted secondary text |
| `outline` | `subtext0` | Borders, placeholders |
| `primary` | `primary` | Main accent color |
| `secondary` | `secondary` | Complementary accent |
| `tertiary` | `tertiary` | Positive / success |
| `error` | `error` | Error / danger |
| `error_container` | `warning` | Warning (closest available) |
| `secondary_container` | `info` | Informational |
| `outline_variant` | `overlay0` | Subtle separator |
| `outline` | `overlay1` | Visible border |
| `on_surface_variant` | `overlay2` | Strong overlay |

You can adjust this mapping in `src/matugen.rs` — the `map_material_you()`
function is the only place that decides how Material You maps to your palette.

---

## Writer outputs

### GTK (`write_gtk = true`)

Writes `~/.config/gtk-{3,4}.0/colors.css` with `@define-color` declarations
for all 18 crawl roles plus GTK4/libadwaita standard token names
(`accent_color`, `window_bg_color`, `headerbar_bg_color`, etc.).

Also creates `gtk.css` with `@import 'colors.css';` if it doesn't exist.

GTK apps pick up colors on next launch (or theme refresh — some apps like
Nautilus support live reload via `gsettings`).

**Force GTK refresh without restart:**
```bash
gsettings set org.gnome.desktop.interface color-scheme 'prefer-dark'
# or toggle and toggle back to force a reload
```

### Ghostty (`write_ghostty = true`)

Writes `~/.config/ghostty/themes/crawl` with:
- `background` / `foreground`
- `cursor-color` / `cursor-text`
- `selection-background` / `selection-foreground`
- Full 16-color ANSI palette (`palette = 0..15`)

Ghostty reloads theme files automatically on change.

**Wire into Ghostty config** (one time):
```
# ~/.config/ghostty/config
theme = crawl
```

### Shell (`write_shell = true`)

Writes two files:

**`~/.config/crawl/theme.sh`** — POSIX `export` statements:
```bash
source "$XDG_CONFIG_HOME/crawl/theme.sh"
echo $CRAWL_PRIMARY    # #cba6f7
echo $CRAWL_BASE       # #1e1e2e
```

**`~/.config/crawl/theme.zsh`** — Zsh associative array:
```zsh
source "$XDG_CONFIG_HOME/crawl/theme.zsh"
echo $crawl[primary]   # #cba6f7
```

**Wire into `.zshrc`** (one time):
```zsh
# ~/.zshrc
[[ -f "$XDG_CONFIG_HOME/crawl/theme.sh" ]] && source "$XDG_CONFIG_HOME/crawl/theme.sh"
```

### JSON (`write_json = true`)

Writes `~/.config/crawl/current-palette.json`:
```json
{
  "source":    { "predefined": { "name": "catppuccin-mocha" } },
  "variant":   "dark",
  "wallpaper": null,
  "palette": {
    "base":    "#1e1e2e",
    "primary": "#cba6f7",
    ...
  },
  "palette_bare": {
    "base":    "1e1e2e",
    "primary": "cba6f7",
    ...
  }
}
```

Any tool can read this — waybar custom modules, eww, AGS, Python scripts, etc.

---

## Quickshell integration

### Setup (one time)

1. Copy `quickshell/theme/` into your RUSTIQ shell directory, e.g. `~/.config/quickshell/theme/`

2. Add `CrawlTheme {}` once in your root `shell.qml`:
```qml
// shell.qml
import Quickshell
import "./theme" as ThemeModule

ShellRoot {
    ThemeModule.CrawlTheme { }  // starts SSE listener + loads startup palette

    PanelWindow { ... }
}
```

3. Use `Theme.<role>` anywhere in your QML:
```qml
import "./theme" as ThemeModule

Rectangle {
    color: ThemeModule.Theme.base

    Text {
        color: ThemeModule.Theme.text
        text:  "Hello RUSTIQ"
    }
}
```

### Color property reference

```qml
// Surfaces
Theme.base       // main background
Theme.mantle     // panels, sidebars
Theme.crust      // window chrome, outermost borders
Theme.surface0   // cards, input backgrounds
Theme.surface1   // hover states
Theme.surface2   // pressed/active states

// Text
Theme.text       // primary readable text
Theme.subtext1   // captions, descriptions
Theme.subtext0   // placeholders, disabled

// Accents
Theme.primary    // main accent — focus rings, active tabs
Theme.secondary  // links, secondary highlights
Theme.tertiary   // success, connected, positive states
Theme.error      // errors, disconnected, critical battery
Theme.warning    // low battery, caution
Theme.info       // notifications, informational

// Overlays
Theme.overlay0   // subtle separators
Theme.overlay1   // inactive borders
Theme.overlay2   // focused input borders

// Derived
Theme.isDark         // bool — true when variant is dark
Theme.primarySubtle  // primary at 20% opacity (for hover backgrounds)
Theme.errorSubtle    // error at 20% opacity
Theme.baseAlpha      // base at 87% opacity (for blur panels)
Theme.surface0Alpha  // surface0 at 80% opacity

// Metadata
Theme.variant        // "dark" | "light"
Theme.sourceKind     // "predefined" | "dynamic"
Theme.sourceName     // theme name or wallpaper path
Theme.wallpaper      // current wallpaper path
Theme.generating     // true while matugen is running
```

### Animated color transitions

Add `Behavior` blocks to any property you want to animate:

```qml
Rectangle {
    color: Theme.base

    Behavior on color {
        ColorAnimation {
            duration:   350
            easing.type: Easing.OutCubic
        }
    }
}
```

Or use the `ThemeTransition` wrapper for cross-fading entire sections.

### Generating spinner

```qml
// Show a subtle indicator while matugen is running
Rectangle {
    visible: Theme.generating
    color:   Theme.surface0
    radius:  4

    Text {
        text:  "⟳ generating palette..."
        color: Theme.subtext1
    }
}
```

---

## Adding to crawl-daemon

1. Add `crawl-theme` to `crawl-daemon/Cargo.toml` dependencies
2. Add `theme: crawl_theme::Config` field to `DaemonConfig` in `config.rs`
3. Add `theme_state: Arc<Mutex<ThemeState>>` to `AppState`
4. Spawn the domain in `spawn_domains()` in `main.rs`
5. Add routes from `router_theme.rs` to `router.rs`
6. Add `crawl theme` subcommand from `cli_cmd_theme.rs` to `crawl-cli`

---

## Adding to crawl-ipc events

Add the `Theme` variant to `CrawlEvent` in `crawl-ipc/src/events.rs`:

```rust
// In CrawlEvent enum:
Theme(ThemeEvent),

// ThemeEvent re-exported from crawl-theme:
pub use crawl_theme::ThemeEvent;
```

Then the SSE stream naturally carries `{"domain":"theme","data":{...}}`.
