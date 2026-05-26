#!/usr/bin/env bash
set -euo pipefail

prefix="${PREFIX:-$HOME/.local}"
bindir="$prefix/bin"
source_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

mkdir -p "$bindir"
install -m 0755 "$source_dir/bin/omarchy-window-switcher" "$bindir/omarchy-window-switcher"

cat <<EOF
Installed: $bindir/omarchy-window-switcher

Suggested Hyprland bindings:

unbind = ALT, TAB
unbind = ALT SHIFT, TAB
bindd = ALT, TAB, Window switcher, exec, uwsm-app -- $bindir/omarchy-window-switcher choose --all-monitors
bindd = ALT SHIFT, TAB, Previous recent window, exec, uwsm-app -- $bindir/omarchy-window-switcher prev --all-monitors

Optional:
bindd = SUPER, GRAVE, Window list, exec, uwsm-app -- $bindir/omarchy-window-switcher choose --all-monitors
EOF
