# Hyprland MRU Window Switcher

Visual MRU Alt-Tab window switcher for Hyprland, built and tested on Omarchy.

## What It Does

Adds a visual most-recently-used (MRU) Alt-Tab switcher for Hyprland setups.

In Omarchy, it can replace the default `ALT+TAB` bindings, which use Hyprland's regular window cycling.

- The example config maps `ALT+TAB` to the next most recently used window on the current monitor, across workspaces.
- `ALT+SHIFT+TAB` cycles in reverse.
- `SUPER+TAB` can be used to switch between recent workspaces.
- The UI runs as a GTK4/layer-shell daemon and receives shortcuts through a Unix socket for fast response.

## Why It Is Useful

Hyprland provides low-level window cycling and focus-history dispatchers, and some setups, including Omarchy, bind `ALT+TAB` to regular cycling by default. Those bindings focus windows immediately and do not provide a visual switcher overlay.

This project adds the classic MRU Alt-Tab workflow on top of Hyprland: hold `Alt`, press `Tab` multiple times, and focus the selected window only when the modifier is released.

## How To Use It

Install the binaries:

```bash
./install.sh
```

You need `hyprctl`, Rust, GTK4, and gtk4-layer-shell available.

This installs:

```bash
~/.local/bin/omarchy-window-switcher
~/.local/bin/omarchy-window-switcherctl
```

Add the daemon to your Hyprland autostart:

```ini
exec-once = /home/your-user/.local/bin/omarchy-window-switcher run
```

Configure the bindings:

```ini
unbind = ALT, TAB
unbind = ALT SHIFT, TAB
unbind = SUPER, TAB
unbind = SUPER SHIFT, TAB

bindd = ALT, TAB, Window switcher next, exec, /home/your-user/.local/bin/omarchy-window-switcherctl open --profile alt next
bindd = ALT SHIFT, TAB, Window switcher previous, exec, /home/your-user/.local/bin/omarchy-window-switcherctl open --profile alt prev
bindd = SUPER, TAB, Window switcher workspace next, exec, /home/your-user/.local/bin/omarchy-window-switcherctl open --profile super next
bindd = SUPER SHIFT, TAB, Window switcher workspace previous, exec, /home/your-user/.local/bin/omarchy-window-switcherctl open --profile super prev
```

Do not add `bindr`/`binddrt` bindings to close on `Alt` or `Super` release. The overlay captures that release through GTK; in Hyprland those bindings can fire when `Tab` is released.

Inspect the detected windows:

```bash
omarchy-window-switcher list --profile alt
```

Configure the behavior:

```bash
mkdir -p ~/.config/omarchy-window-switcher
cp config.example.toml ~/.config/omarchy-window-switcher/config.toml
```

The default configuration uses:

```toml
[alt]
target = "windows"
scope = "current-monitor"

[super]
target = "workspaces"
scope = "current-monitor"
```

You can change this in `config.toml`. Supported targets are `windows`, `workspaces`, and `monitors`; supported scopes are `current-monitor`, `current-workspace`, and `all`.

Before submitting changes:

```bash
cargo fmt
cargo test
cargo clippy -- -D warnings
cargo build --release
```

## Contributing

Issues and pull requests are welcome. If a change touches focus behavior or Hyprland bindings, explain how to reproduce it.

## License

MIT. See `LICENSE`.
