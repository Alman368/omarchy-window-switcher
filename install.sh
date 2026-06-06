#!/usr/bin/env bash
set -euo pipefail

prefix="${PREFIX:-$HOME/.local}"
bindir="$prefix/bin"
source_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is required to build mogtab" >&2
  exit 1
fi

cargo build --release --locked --manifest-path "$source_dir/Cargo.toml"

mkdir -p "$bindir"
install -m 0755 "$source_dir/target/release/mogtab" "$bindir/mogtab"
install -m 0755 "$source_dir/target/release/mogtabctl" "$bindir/mogtabctl"

cat <<EOF
Installed: $bindir/mogtab
Installed: $bindir/mogtabctl

Suggested Hyprland bindings:

exec-once = $bindir/mogtab run

unbind = ALT, TAB
unbind = ALT SHIFT, TAB
unbind = SUPER, TAB
unbind = SUPER SHIFT, TAB

bindd = ALT, TAB, Window switcher next, exec, $bindir/mogtabctl open --profile alt next
bindd = ALT SHIFT, TAB, Window switcher previous, exec, $bindir/mogtabctl open --profile alt prev
bindd = SUPER, TAB, Window switcher workspace next, exec, $bindir/mogtabctl open --profile super next
bindd = SUPER SHIFT, TAB, Window switcher workspace previous, exec, $bindir/mogtabctl open --profile super prev

# Do not add bindr/binddrt close bindings. The GTK overlay captures modifier
# release itself; Hyprland release binds can fire when Tab is released.

Optional:
bindd = SUPER, GRAVE, Window switcher cancel, exec, $bindir/mogtabctl cancel
EOF
