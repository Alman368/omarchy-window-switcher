# Omarchy Window Switcher

A fast Windows-style MRU Alt-Tab switcher for Omarchy/Hyprland.

It runs a tiny GTK4/layer-shell daemon and uses a Unix socket for hotkey
commands. `ALT+TAB` defaults to global most-recently-used windows across every
workspace. `SUPER+TAB` defaults to most-recently-used workspaces on the current
monitor.

## Requirements

- Omarchy / Hyprland
- `hyprctl`
- Rust 1.91+
- `gtk4`
- `gtk4-layer-shell`

## Install

```bash
./install.sh
```

This installs `omarchy-window-switcher` to `~/.local/bin`.

## Usage

Run the resident daemon:

```bash
omarchy-window-switcher run
```

Open or advance the switcher:

```bash
omarchy-window-switcher open --profile alt next
omarchy-window-switcher open --profile alt prev
omarchy-window-switcher open --profile super next
omarchy-window-switcher open --profile super prev
```

Close and focus the selected window:

```bash
omarchy-window-switcher close
```

Cancel without switching:

```bash
omarchy-window-switcher cancel
```

Show the windows the tool can see:

```bash
omarchy-window-switcher list
omarchy-window-switcher list --profile super
```

## Configuration

The default behavior is:

```toml
[alt]
target = "windows"
scope = "all"

[super]
target = "workspaces"
scope = "current-monitor"
```

To change it, copy `config.example.toml` to:

```bash
~/.config/omarchy-window-switcher/config.toml
```

Supported targets are `windows` and `workspaces`. Supported scopes are `all`,
`current-workspace` and `current-monitor`.

For Alt-Tab limited to the current workspace:

```toml
[alt]
target = "windows"
scope = "current-workspace"
```

## Test

```bash
cargo test
```

## Hyprland Binding

Add the key bindings to `~/.config/hypr/bindings.conf` and the daemon autostart
to `~/.config/hypr/autostart.conf`:

```ini
exec-once = /home/your-user/.local/bin/omarchy-window-switcher run

unbind = ALT, TAB
unbind = ALT SHIFT, TAB
unbind = SUPER, TAB
unbind = SUPER SHIFT, TAB

binded = ALT, TAB, Window switcher next, exec, /home/your-user/.local/bin/omarchy-window-switcher open --profile alt next
binded = ALT SHIFT, TAB, Window switcher previous, exec, /home/your-user/.local/bin/omarchy-window-switcher open --profile alt prev
binded = SUPER, TAB, Window switcher workspace next, exec, /home/your-user/.local/bin/omarchy-window-switcher open --profile super next
binded = SUPER SHIFT, TAB, Window switcher workspace previous, exec, /home/your-user/.local/bin/omarchy-window-switcher open --profile super prev

binddr = ALT, Alt_L, Window switcher close, exec, /home/your-user/.local/bin/omarchy-window-switcher close
binddr = ALT, Alt_R, Window switcher close, exec, /home/your-user/.local/bin/omarchy-window-switcher close
binddr = SUPER, Super_L, Window switcher close, exec, /home/your-user/.local/bin/omarchy-window-switcher close
binddr = SUPER, Super_R, Window switcher close, exec, /home/your-user/.local/bin/omarchy-window-switcher close
```

## Why

Hyprshell is a capable Rust/GTK4 project, but it also includes a launcher,
plugins, config editor, desktop-file indexing and more. This project keeps only
the fast MRU switcher path: Hyprland JSON in, small layer-shell overlay out.
