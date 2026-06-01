# Omarchy Window Switcher

Switcher de ventanas tipo Alt-Tab para Omarchy/Hyprland.

## Que hace

Reemplaza el Alt-Tab de Hyprland por un selector MRU, similar al comportamiento clasico de Windows.

- `ALT+TAB` cambia entre ventanas recientes del monitor actual.
- `ALT+SHIFT+TAB` recorre la lista en sentido inverso.
- `SUPER+TAB` puede usarse para cambiar entre workspaces recientes.
- La interfaz corre como daemon GTK4/layer-shell y recibe los atajos por socket Unix para que la respuesta sea rapida.

## Por que es util

Hyprland ya permite ciclar ventanas, pero no replica bien el flujo clasico de Alt-Tab: mantener `Alt`, pulsar `Tab` varias veces y cambiar solo al soltar el modificador.

Este proyecto resuelve ese caso concreto para Omarchy, incluyendo ventanas del mismo monitor y evitando races habituales de Hyprland con binds de release.

## Como usarlo

Para instalar los binarios:

```bash
./install.sh
```

Necesitas tener disponibles `hyprctl`, Rust, GTK4 y gtk4-layer-shell.

Esto instala:

```bash
~/.local/bin/omarchy-window-switcher
~/.local/bin/omarchy-window-switcherctl
```

Anade el daemon al autostart de Hyprland:

```ini
exec-once = /home/your-user/.local/bin/omarchy-window-switcher run
```

Y configura los binds:

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

No anadas binds `bindr`/`binddrt` para cerrar al soltar `Alt` o `Super`. El overlay captura ese release por GTK; en Hyprland esos binds pueden dispararse al soltar `Tab`.

Para ver que ventanas detecta:

```bash
omarchy-window-switcher list --profile alt
```

Para configurar el comportamiento:

```bash
mkdir -p ~/.config/omarchy-window-switcher
cp config.example.toml ~/.config/omarchy-window-switcher/config.toml
```

La configuracion por defecto usa:

```toml
[alt]
target = "windows"
scope = "current-monitor"

[super]
target = "workspaces"
scope = "current-monitor"
```

Antes de subir cambios:

```bash
cargo fmt
cargo test
cargo clippy -- -D warnings
cargo build --release
```

## Contribuir

Las issues y pull requests son bienvenidas. Si el cambio toca comportamiento de foco o binds de Hyprland, explica como reproducirlo.

## Licencia

MIT. Ver `LICENSE`.
