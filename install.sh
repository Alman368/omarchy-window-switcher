#!/usr/bin/env bash
set -euo pipefail

prefix="${PREFIX:-$HOME/.local}"
bindir="$prefix/bin"
source_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

cargo build --release --manifest-path "$source_dir/Cargo.toml"

mkdir -p "$bindir"
install -m 0755 "$source_dir/target/release/omarchy-window-switcher" "$bindir/omarchy-window-switcher"

cat <<EOF
Installed: $bindir/omarchy-window-switcher

Suggested Hyprland bindings:

exec-once = $bindir/omarchy-window-switcher run

unbind = ALT, TAB
unbind = ALT SHIFT, TAB
unbind = SUPER, TAB
unbind = SUPER SHIFT, TAB

binded = ALT, TAB, Window switcher next, exec, $bindir/omarchy-window-switcher open --profile alt next
binded = ALT SHIFT, TAB, Window switcher previous, exec, $bindir/omarchy-window-switcher open --profile alt prev
binded = SUPER, TAB, Window switcher workspace next, exec, $bindir/omarchy-window-switcher open --profile super next
binded = SUPER SHIFT, TAB, Window switcher workspace previous, exec, $bindir/omarchy-window-switcher open --profile super prev

binddrt = ALT, Alt_L, Window switcher close, exec, $bindir/omarchy-window-switcher close
binddrt = ALT, Alt_R, Window switcher close, exec, $bindir/omarchy-window-switcher close
binddrt = SUPER, Super_L, Window switcher close, exec, $bindir/omarchy-window-switcher close
binddrt = SUPER, Super_R, Window switcher close, exec, $bindir/omarchy-window-switcher close

Optional:
bindd = SUPER, GRAVE, Window switcher cancel, exec, $bindir/omarchy-window-switcher cancel
EOF
