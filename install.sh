#!/usr/bin/env bash
set -euo pipefail

prefix="${PREFIX:-$HOME/.local}"
bindir="$prefix/bin"
source_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

cargo build --release --manifest-path "$source_dir/Cargo.toml"

mkdir -p "$bindir"
install -m 0755 "$source_dir/target/release/omarchy-window-switcher" "$bindir/omarchy-window-switcher"
install -m 0755 "$source_dir/target/release/omarchy-window-switcherctl" "$bindir/omarchy-window-switcherctl"

cat <<EOF
Installed: $bindir/omarchy-window-switcher
Installed: $bindir/omarchy-window-switcherctl

Suggested Hyprland bindings:

exec-once = $bindir/omarchy-window-switcher run

unbind = ALT, TAB
unbind = ALT SHIFT, TAB
unbind = SUPER, TAB
unbind = SUPER SHIFT, TAB

bindd = ALT, TAB, Window switcher next, exec, $bindir/omarchy-window-switcherctl open --profile alt next
bindd = ALT SHIFT, TAB, Window switcher previous, exec, $bindir/omarchy-window-switcherctl open --profile alt prev
bindd = SUPER, TAB, Window switcher workspace next, exec, $bindir/omarchy-window-switcherctl open --profile super next
bindd = SUPER SHIFT, TAB, Window switcher workspace previous, exec, $bindir/omarchy-window-switcherctl open --profile super prev

# Do not add bindr/binddrt close bindings. The GTK overlay captures modifier
# release itself; Hyprland release binds can fire when Tab is released.

Optional:
bindd = SUPER, GRAVE, Window switcher cancel, exec, $bindir/omarchy-window-switcherctl cancel
EOF
