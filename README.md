# MogTab

Visual MRU Alt-Tab window switcher for Hyprland, built and tested on Omarchy.

## What It Does

Adds a visual most-recently-used (MRU) Alt-Tab switcher for Hyprland setups.

In Omarchy, it can replace the default `ALT+TAB` bindings, which use Hyprland's regular window cycling.

- The example config maps `ALT+TAB` to the next most recently used window on the current monitor, across workspaces.
- `ALT+SHIFT+TAB` cycles in reverse.
- `SUPER+TAB` can be used to switch between recent workspaces.
- The UI runs as a GTK4/layer-shell daemon and receives shortcuts through a Unix socket for fast response.

## Why It Is Useful

Hyprland provides low-level window cycling dispatchers, and some setups, including Omarchy, bind `ALT+TAB` to regular cycling by default. Hyprland tracks focus history, but cycling bindings focus windows immediately, and static bindings cannot keep a visual MRU selection open until `Alt` is released.

This project exists for that gap: it adds the classic MRU Alt-Tab workflow on top of Hyprland with a small external daemon. Windows are ordered by recent focus history: hold `Alt`, press `Tab` multiple times, hover a card with the mouse to select it, click a card to switch immediately, or release the modifier to focus the selected item.

## How To Use It

You need `hyprctl`, Rust, GTK4, and gtk4-layer-shell available.

Install the binaries:

```bash
./install.sh
```

This installs:

```bash
~/.local/bin/mogtab
~/.local/bin/mogtabctl
```

Add the daemon to your Hyprland autostart:

```ini
exec-once = /home/your-user/.local/bin/mogtab run
```

Configure the bindings:

```ini
unbind = ALT, TAB
unbind = ALT SHIFT, TAB
unbind = SUPER, TAB
unbind = SUPER SHIFT, TAB

bindd = ALT, TAB, Window switcher next, exec, /home/your-user/.local/bin/mogtabctl open --profile alt next
bindd = ALT SHIFT, TAB, Window switcher previous, exec, /home/your-user/.local/bin/mogtabctl open --profile alt prev
bindd = SUPER, TAB, Window switcher workspace next, exec, /home/your-user/.local/bin/mogtabctl open --profile super next
bindd = SUPER SHIFT, TAB, Window switcher workspace previous, exec, /home/your-user/.local/bin/mogtabctl open --profile super prev
```

Replace `/home/your-user` with your actual home directory.

Do not add `bindr`/`binddrt` bindings to close on `Alt` or `Super` release. The overlay captures that release through GTK; in Hyprland those bindings can fire when `Tab` is released.

Reload Hyprland:

```bash
hyprctl reload
hyprctl configerrors
```

Start the daemon for the current session, or log out and back in:

```bash
mogtab run
```

Inspect the detected windows:

```bash
mogtab list --profile alt
```

Configure the behavior:

```bash
mkdir -p ~/.config/mogtab
cp config.example.toml ~/.config/mogtab/config.toml
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

## Contributing

Issues and pull requests are welcome. If a change touches focus behavior or Hyprland bindings, explain how to reproduce it.

Before submitting changes:

```bash
cargo fmt
cargo test
cargo clippy -- -D warnings
cargo build --release
```

## License

MIT. See `LICENSE`.
