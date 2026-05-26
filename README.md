# Omarchy Window Switcher

A fast Windows-style MRU Alt-Tab switcher for Omarchy/Hyprland.

It runs a tiny GTK4/layer-shell daemon and uses a Unix socket for hotkey
commands. `ALT+TAB` and `SUPER+TAB` cycle through windows in most-recently-used
order, then focus the selected window when the modifier key is released.

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
omarchy-window-switcher open next
omarchy-window-switcher open prev
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
```

## Test

```bash
cargo test
```

## Hyprland Binding

Add this to `~/.config/hypr/bindings.conf`:

```ini
exec-once = /home/your-user/.local/bin/omarchy-window-switcher run

unbind = ALT, TAB
unbind = ALT SHIFT, TAB
unbind = SUPER, TAB
unbind = SUPER SHIFT, TAB

binded = ALT, TAB, Window switcher next, exec, /home/your-user/.local/bin/omarchy-window-switcher open next
binded = ALT SHIFT, TAB, Window switcher previous, exec, /home/your-user/.local/bin/omarchy-window-switcher open prev
binded = SUPER, TAB, Window switcher next, exec, /home/your-user/.local/bin/omarchy-window-switcher open next
binded = SUPER SHIFT, TAB, Window switcher previous, exec, /home/your-user/.local/bin/omarchy-window-switcher open prev

binddr = ALT, Alt_L, Window switcher close, exec, /home/your-user/.local/bin/omarchy-window-switcher close
binddr = ALT, Alt_R, Window switcher close, exec, /home/your-user/.local/bin/omarchy-window-switcher close
binddr = SUPER, Super_L, Window switcher close, exec, /home/your-user/.local/bin/omarchy-window-switcher close
binddr = SUPER, Super_R, Window switcher close, exec, /home/your-user/.local/bin/omarchy-window-switcher close
```

## Why

Hyprshell is a capable Rust/GTK4 project, but it also includes a launcher,
plugins, config editor, desktop-file indexing and more. This project keeps only
the fast MRU switcher path: Hyprland JSON in, small layer-shell overlay out.
