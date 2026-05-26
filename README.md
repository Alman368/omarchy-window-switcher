# Omarchy Window Switcher

Switcher de ventanas tipo Windows Alt-Tab para Omarchy/Hyprland.

## Que hace

Reemplaza el flujo de `ALT+TAB` y `SUPER+TAB` por un selector rapido con orden MRU
`most recently used`.

- `ALT+TAB` cambia entre ventanas recientes. Por defecto lo hace de forma global,
  independientemente del workspace.
- `SUPER+TAB` cambia entre workspaces recientes, limitado al monitor actual.
- La interfaz visual aparece como una capa GTK4/layer-shell centrada en pantalla.
- El daemon recibe los atajos por socket Unix para que el cambio sea inmediato.

## Por que es util

Hyprland ya permite ciclar ventanas y workspaces, pero el comportamiento por
defecto no replica bien el Alt-Tab clasico: MRU real, interfaz visible y foco al
soltar el modificador.

Este proyecto deja solo esa ruta critica, sin launcher, plugins ni indexado de
aplicaciones. Esta pensado para Omarchy y para tocar lo minimo de la configuracion
del sistema: solo los binds de `ALT+TAB`, `SUPER+TAB` y el autostart del daemon.

## Requisitos

- Omarchy / Hyprland
- `hyprctl`
- Rust 1.91+
- `gtk4`
- `gtk4-layer-shell`

## Como usarlo

Para instalar el binario:

```bash
./install.sh
```

Esto instala `omarchy-window-switcher` en `~/.local/bin`.

Ejecuta el daemon:

```bash
omarchy-window-switcher run
```

Comandos principales:

```bash
omarchy-window-switcher open --profile alt next
omarchy-window-switcher open --profile alt prev
omarchy-window-switcher open --profile super next
omarchy-window-switcher open --profile super prev
omarchy-window-switcher close
omarchy-window-switcher cancel
```

Para ver que detecta el switcher:

```bash
omarchy-window-switcher list
omarchy-window-switcher list --profile super
```

## Configuracion

El comportamiento por defecto es:

```toml
[alt]
target = "windows"
scope = "all"
same_class = false
include_special_workspaces = false

[super]
target = "workspaces"
scope = "current-monitor"
include_empty_workspaces = false
include_special_workspaces = false
```

Para cambiarlo, copia `config.example.toml` a:

```bash
~/.config/omarchy-window-switcher/config.toml
```

Valores soportados:

- `target`: `windows`, `workspaces`, `monitors`
- `scope`: `all`, `current-workspace`, `current-monitor`
- `same_class`: `true` para limitar ventanas a la misma app/clase
- `include_special_workspaces`: `true` para incluir scratchpads/special workspaces
- `include_empty_workspaces`: `true` para incluir workspaces sin ventanas al usar `target = "workspaces"`

Configuraciones habituales:

```toml
# Nuestro uso: Alt-Tab global entre ventanas, Super-Tab entre workspaces del monitor actual.
[alt]
target = "windows"
scope = "all"

[super]
target = "workspaces"
scope = "current-monitor"
```

```toml
# Super-Tab entre ventanas del workspace actual.
[super]
target = "windows"
scope = "current-workspace"
```

```toml
# Alt-Tab limitado al workspace actual.
[alt]
target = "windows"
scope = "current-workspace"
```

```toml
# Alt-Tab entre ventanas del monitor actual.
[alt]
target = "windows"
scope = "current-monitor"
```

```toml
# Cambiar solo entre ventanas de la misma app.
[alt]
target = "windows"
scope = "all"
same_class = true
```

```toml
# Super-Tab entre monitores.
[super]
target = "monitors"
scope = "all"
```

## Hyprland

Autostart en `~/.config/hypr/autostart.conf`:

```ini
exec-once = /home/your-user/.local/bin/omarchy-window-switcher run
```

Binds en `~/.config/hypr/bindings.conf`:

```ini
unbind = ALT, TAB
unbind = ALT SHIFT, TAB
unbind = SUPER, TAB
unbind = SUPER SHIFT, TAB

bindd = ALT, TAB, Window switcher next, exec, /home/your-user/.local/bin/omarchy-window-switcher open --profile alt next
bindd = ALT SHIFT, TAB, Window switcher previous, exec, /home/your-user/.local/bin/omarchy-window-switcher open --profile alt prev
bindd = SUPER, TAB, Window switcher workspace next, exec, /home/your-user/.local/bin/omarchy-window-switcher open --profile super next
bindd = SUPER SHIFT, TAB, Window switcher workspace previous, exec, /home/your-user/.local/bin/omarchy-window-switcher open --profile super prev

binddrt = ALT, Alt_L, Window switcher close, exec, /home/your-user/.local/bin/omarchy-window-switcher close
binddrt = ALT, Alt_R, Window switcher close, exec, /home/your-user/.local/bin/omarchy-window-switcher close
binddrt = SUPER, Super_L, Window switcher close, exec, /home/your-user/.local/bin/omarchy-window-switcher close
binddrt = SUPER, Super_R, Window switcher close, exec, /home/your-user/.local/bin/omarchy-window-switcher close

bind = ALT, TAB, submap, omarchy-window-switcher
bind = ALT SHIFT, TAB, submap, omarchy-window-switcher
bind = SUPER, TAB, submap, omarchy-window-switcher
bind = SUPER SHIFT, TAB, submap, omarchy-window-switcher

submap = omarchy-window-switcher
bindd = ALT, TAB, Window switcher next, exec, /home/your-user/.local/bin/omarchy-window-switcher open --profile alt next
bindd = ALT SHIFT, TAB, Window switcher previous, exec, /home/your-user/.local/bin/omarchy-window-switcher open --profile alt prev
bindd = SUPER, TAB, Window switcher workspace next, exec, /home/your-user/.local/bin/omarchy-window-switcher open --profile super next
bindd = SUPER SHIFT, TAB, Window switcher workspace previous, exec, /home/your-user/.local/bin/omarchy-window-switcher open --profile super prev
bindd = , RIGHT, Window switcher next, exec, /home/your-user/.local/bin/omarchy-window-switcher open --profile alt next
bindd = , LEFT, Window switcher previous, exec, /home/your-user/.local/bin/omarchy-window-switcher open --profile alt prev
bindd = , ESCAPE, Window switcher cancel, exec, /home/your-user/.local/bin/omarchy-window-switcher cancel
bind = , ESCAPE, submap, reset
bindd = , RETURN, Window switcher accept, exec, /home/your-user/.local/bin/omarchy-window-switcher close
bind = , RETURN, submap, reset
binddrt = ALT, Alt_L, Window switcher close, exec, /home/your-user/.local/bin/omarchy-window-switcher close
bindr = ALT, Alt_L, submap, reset
binddrt = ALT, Alt_R, Window switcher close, exec, /home/your-user/.local/bin/omarchy-window-switcher close
bindr = ALT, Alt_R, submap, reset
binddrt = SUPER, Super_L, Window switcher close, exec, /home/your-user/.local/bin/omarchy-window-switcher close
bindr = SUPER, Super_L, submap, reset
binddrt = SUPER, Super_R, Window switcher close, exec, /home/your-user/.local/bin/omarchy-window-switcher close
bindr = SUPER, Super_R, submap, reset
submap = reset
```

## Desarrollo

Antes de subir cambios:

```bash
cargo fmt
cargo test
cargo clippy -- -D warnings
cargo build --release
```

Si se cambia configuracion de Hyprland:

```bash
hyprctl reload
hyprctl configerrors
```

## Contribuir

Las pull requests son bienvenidas. Para cambios grandes, abre primero una issue
explicando el comportamiento esperado y como probarlo en Hyprland/Omarchy.

## Licencia

MIT. Ver `LICENSE`.
