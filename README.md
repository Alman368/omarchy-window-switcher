# Omarchy Window Switcher

A small Hyprland window switcher built for Omarchy.

It avoids a background daemon: every invocation reads `hyprctl clients -j`, lets
`walker --dmenu` handle the menu, then focuses the selected window by Hyprland
address.

## Requirements

- Omarchy / Hyprland
- `hyprctl`
- `walker`
- Python 3.11+

## Install

```bash
./install.sh
```

This installs `omarchy-window-switcher` to `~/.local/bin`.

## Usage

Open an interactive window list:

```bash
omarchy-window-switcher choose --all-monitors
```

Focus the next recent window without opening a menu:

```bash
omarchy-window-switcher next --all-monitors
```

Show the windows the tool can see:

```bash
omarchy-window-switcher list --all-monitors
```

## Test

```bash
python3 -m pytest -q
```

## Hyprland Binding

Add this to `~/.config/hypr/bindings.conf`:

```ini
unbind = ALT, TAB
unbind = ALT SHIFT, TAB
bindd = ALT, TAB, Window switcher, exec, uwsm-app -- /home/your-user/.local/bin/omarchy-window-switcher choose --all-monitors
bindd = ALT SHIFT, TAB, Previous recent window, exec, uwsm-app -- /home/your-user/.local/bin/omarchy-window-switcher prev --all-monitors
```

If you want to keep Omarchy's default `ALT+TAB` behavior, bind the menu to a
different key instead:

```ini
bindd = SUPER, GRAVE, Window list, exec, uwsm-app -- /home/your-user/.local/bin/omarchy-window-switcher choose --all-monitors
```

The installer prints bindings with your actual install path. This matters under
UWSM because its service environment may not include `~/.local/bin` in `PATH`.

## Why

Hyprland already exposes the truth through JSON. This tool keeps the moving
parts minimal and delegates the menu to Walker, which Omarchy already ships.
