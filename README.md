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

[super]
target = "workspaces"
scope = "current-monitor"
```

Para cambiarlo, copia `config.example.toml` a:

```bash
~/.config/omarchy-window-switcher/config.toml
```

Valores soportados:

- `target`: `windows`, `workspaces`
- `scope`: `all`, `current-workspace`, `current-monitor`

Para limitar `ALT+TAB` al workspace actual:

```toml
[alt]
target = "windows"
scope = "current-workspace"
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

binded = ALT, TAB, Window switcher next, exec, /home/your-user/.local/bin/omarchy-window-switcher open --profile alt next
binded = ALT SHIFT, TAB, Window switcher previous, exec, /home/your-user/.local/bin/omarchy-window-switcher open --profile alt prev
binded = SUPER, TAB, Window switcher workspace next, exec, /home/your-user/.local/bin/omarchy-window-switcher open --profile super next
binded = SUPER SHIFT, TAB, Window switcher workspace previous, exec, /home/your-user/.local/bin/omarchy-window-switcher open --profile super prev

binddr = ALT, Alt_L, Window switcher close, exec, /home/your-user/.local/bin/omarchy-window-switcher close
binddr = ALT, Alt_R, Window switcher close, exec, /home/your-user/.local/bin/omarchy-window-switcher close
binddr = SUPER, Super_L, Window switcher close, exec, /home/your-user/.local/bin/omarchy-window-switcher close
binddr = SUPER, Super_R, Window switcher close, exec, /home/your-user/.local/bin/omarchy-window-switcher close
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
