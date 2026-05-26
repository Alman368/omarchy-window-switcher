use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use gtk::gdk;
use gtk::glib::{self, ControlFlow, Propagation};
use gtk::pango;
use gtk::prelude::*;
use gtk4 as gtk;
use gtk4_layer_shell::{KeyboardMode, Layer, LayerShell};
use serde::Deserialize;
use std::cell::RefCell;
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::process::Command;
use std::rc::Rc;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

const APP_ID: &str = "dev.alman.OmarchyWindowSwitcher";
const NAMESPACE: &str = "omarchy_window_switcher";
const SOCKET_NAME: &str = "omarchy-window-switcher.sock";
const CLOSE_RACE_GRACE: Duration = Duration::from_millis(350);
const CSS: &str = r#"
window {
  background: transparent;
}

.switcher-panel {
  background: rgba(18, 20, 24, 0.94);
  border: 1px solid rgba(210, 218, 230, 0.22);
  border-radius: 10px;
  padding: 16px;
  box-shadow: 0 18px 48px rgba(0, 0, 0, 0.42);
}

.switcher-row {
  border-spacing: 10px;
}

.switcher-card {
  background: rgba(42, 47, 55, 0.92);
  border: 2px solid rgba(210, 218, 230, 0.16);
  border-radius: 8px;
  padding: 12px;
  min-width: 156px;
  min-height: 112px;
}

.switcher-card.selected {
  background: rgba(54, 68, 88, 0.98);
  border-color: rgba(96, 169, 255, 0.96);
}

.switcher-icon {
  margin-bottom: 8px;
}

.switcher-title {
  color: rgba(248, 250, 252, 0.98);
  font-size: 13px;
  font-weight: 600;
}

.switcher-meta {
  color: rgba(185, 195, 210, 0.86);
  font-size: 11px;
}

.switcher-heading {
  color: rgba(248, 250, 252, 0.98);
  font-size: 14px;
  font-weight: 700;
  margin-bottom: 12px;
}
"#;

#[derive(Debug, Parser)]
#[command(name = "omarchy-window-switcher")]
#[command(about = "Windows-style MRU Alt-Tab switcher for Omarchy/Hyprland")]
struct Cli {
    #[command(subcommand)]
    command: CliCommand,
}

#[derive(Debug, Subcommand)]
enum CliCommand {
    /// Run the resident GTK/layer-shell switcher daemon.
    Run,
    /// Open the switcher or move selection while it is open.
    Open {
        #[arg(value_enum, default_value_t = Direction::Next)]
        direction: Direction,
    },
    /// Close the switcher and focus the selected window.
    Close,
    /// Close the switcher without switching.
    Cancel,
    /// Print the current MRU window list.
    List,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Direction {
    Next,
    Prev,
}

#[derive(Debug, Clone, Copy)]
enum IpcCommand {
    Open(Direction),
    Close,
    Cancel,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct HyprWorkspace {
    #[serde(default)]
    id: i64,
    #[serde(default)]
    name: String,
}

#[derive(Debug, Clone, Deserialize)]
struct HyprClient {
    #[serde(default)]
    address: String,
    #[serde(default = "default_true")]
    mapped: bool,
    #[serde(default)]
    hidden: bool,
    #[serde(default = "default_true", rename = "acceptsInput")]
    accepts_input: bool,
    #[serde(default)]
    workspace: HyprWorkspace,
    #[serde(default)]
    monitor: i64,
    #[serde(default, rename = "class")]
    wm_class: String,
    #[serde(default, rename = "initialClass")]
    initial_class: String,
    #[serde(default)]
    title: String,
    #[serde(default, rename = "focusHistoryID")]
    focus_history_id: Option<i64>,
    #[serde(default)]
    pinned: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct HyprActiveWindow {
    #[serde(default)]
    address: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WindowInfo {
    address: String,
    title: String,
    wm_class: String,
    workspace_id: i64,
    workspace_name: String,
    monitor: i64,
    focus_history_id: i64,
    pinned: bool,
}

struct SwitcherState {
    window: gtk::ApplicationWindow,
    row: gtk::Box,
    heading: gtk::Label,
    windows: Vec<WindowInfo>,
    selected: usize,
    open: bool,
    pending_close_until: Option<Instant>,
}

fn default_true() -> bool {
    true
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        CliCommand::Run => run_daemon(),
        CliCommand::Open { direction } => {
            send_or_start(IpcCommand::Open(direction), true)?;
            Ok(())
        }
        CliCommand::Close => {
            let _ = send_ipc(IpcCommand::Close);
            Ok(())
        }
        CliCommand::Cancel => {
            let _ = send_ipc(IpcCommand::Cancel);
            Ok(())
        }
        CliCommand::List => {
            for window in collect_windows()? {
                println!(
                    "{}  [ws {} | mon {}]",
                    window.display_name(),
                    window.workspace_name,
                    window.monitor
                );
            }
            Ok(())
        }
    }
}

fn run_daemon() -> Result<()> {
    let app = gtk::Application::builder().application_id(APP_ID).build();

    app.connect_activate(|app| {
        // This is a resident hidden-window app; keep it alive between switcher opens.
        std::mem::forget(app.hold());
        install_css();

        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .title("Omarchy Window Switcher")
            .decorated(false)
            .resizable(false)
            .default_width(920)
            .default_height(220)
            .build();
        window.init_layer_shell();
        window.set_namespace(Some(NAMESPACE));
        window.set_layer(Layer::Overlay);
        window.set_keyboard_mode(KeyboardMode::Exclusive);

        let panel = gtk::Box::new(gtk::Orientation::Vertical, 0);
        panel.add_css_class("switcher-panel");
        panel.set_halign(gtk::Align::Center);
        panel.set_valign(gtk::Align::Center);

        let heading = gtk::Label::new(Some("Switch windows"));
        heading.add_css_class("switcher-heading");
        heading.set_halign(gtk::Align::Start);
        heading.set_ellipsize(pango::EllipsizeMode::End);
        panel.append(&heading);

        let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        row.add_css_class("switcher-row");
        row.set_halign(gtk::Align::Center);
        panel.append(&row);

        window.set_child(Some(&panel));
        window.set_visible(false);

        let (tx, rx) = mpsc::channel::<IpcCommand>();
        if let Err(err) = spawn_ipc_listener(tx) {
            eprintln!("omarchy-window-switcher: failed to start IPC listener: {err:#}");
            app.quit();
            return;
        }

        let state = Rc::new(RefCell::new(SwitcherState {
            window: window.clone(),
            row: row.clone(),
            heading: heading.clone(),
            windows: Vec::new(),
            selected: 0,
            open: false,
            pending_close_until: None,
        }));

        setup_keyboard_controller(&window, Rc::clone(&state));

        glib::timeout_add_local(Duration::from_millis(12), move || {
            while let Ok(command) = rx.try_recv() {
                state.borrow_mut().handle(command);
            }
            ControlFlow::Continue
        });
    });

    app.run_with_args::<&str>(&[]);
    Ok(())
}

fn setup_keyboard_controller(window: &gtk::ApplicationWindow, state: Rc<RefCell<SwitcherState>>) {
    let controller = gtk::EventControllerKey::new();
    let state_pressed = Rc::clone(&state);
    controller.connect_key_pressed(move |_, key, _, _| match key {
        gdk::Key::Tab | gdk::Key::Right | gdk::Key::l => {
            state_pressed
                .borrow_mut()
                .handle(IpcCommand::Open(Direction::Next));
            Propagation::Stop
        }
        gdk::Key::ISO_Left_Tab | gdk::Key::Left | gdk::Key::h | gdk::Key::grave => {
            state_pressed
                .borrow_mut()
                .handle(IpcCommand::Open(Direction::Prev));
            Propagation::Stop
        }
        gdk::Key::Escape => {
            state_pressed.borrow_mut().handle(IpcCommand::Cancel);
            Propagation::Stop
        }
        gdk::Key::Return | gdk::Key::KP_Enter => {
            state_pressed.borrow_mut().handle(IpcCommand::Close);
            Propagation::Stop
        }
        _ => Propagation::Proceed,
    });

    let state_released = Rc::clone(&state);
    controller.connect_key_released(move |_, key, _, _| {
        if matches!(
            key,
            gdk::Key::Alt_L | gdk::Key::Alt_R | gdk::Key::Super_L | gdk::Key::Super_R
        ) {
            state_released.borrow_mut().handle(IpcCommand::Close);
        }
    });

    window.add_controller(controller);
}

impl SwitcherState {
    fn handle(&mut self, command: IpcCommand) {
        match command {
            IpcCommand::Open(direction) => self.open_or_advance(direction),
            IpcCommand::Close => self.close(true),
            IpcCommand::Cancel => self.close(false),
        }
    }

    fn open_or_advance(&mut self, direction: Direction) {
        if self.open {
            self.advance(direction);
            self.render();
            return;
        }

        match collect_windows() {
            Ok(windows) if !windows.is_empty() => {
                self.windows = windows;
                self.selected = initial_selection(self.windows.len(), direction);
                self.open = true;
                self.render();
                self.window.present();
                self.window.grab_focus();
                if self.consume_pending_close() {
                    self.close(true);
                }
            }
            Ok(_) => {}
            Err(err) => eprintln!("omarchy-window-switcher: failed to collect windows: {err:#}"),
        }
    }

    fn advance(&mut self, direction: Direction) {
        if self.windows.is_empty() {
            self.selected = 0;
            return;
        }

        self.selected = match direction {
            Direction::Next => (self.selected + 1) % self.windows.len(),
            Direction::Prev => {
                if self.selected == 0 {
                    self.windows.len() - 1
                } else {
                    self.selected - 1
                }
            }
        };
    }

    fn close(&mut self, should_focus: bool) {
        if !self.open {
            if should_focus {
                self.pending_close_until = Some(Instant::now() + CLOSE_RACE_GRACE);
            }
            return;
        }

        self.pending_close_until = None;
        self.open = false;
        self.window.set_visible(false);

        let selected = self.windows.get(self.selected).cloned();
        self.clear();
        self.windows.clear();
        self.selected = 0;

        if should_focus
            && let Some(window) = selected
            && let Err(err) = focus_window(&window)
        {
            eprintln!("omarchy-window-switcher: failed to focus window: {err:#}");
        }
    }

    fn consume_pending_close(&mut self) -> bool {
        let Some(deadline) = self.pending_close_until.take() else {
            return false;
        };

        Instant::now() <= deadline
    }

    fn render(&self) {
        self.clear();

        if let Some(selected) = self.windows.get(self.selected) {
            self.heading.set_label(&format!(
                "{} windows - {}",
                self.windows.len(),
                selected.display_name()
            ));
        }

        for (idx, window) in self.windows.iter().enumerate() {
            self.row.append(&window_card(window, idx == self.selected));
        }
    }

    fn clear(&self) {
        while let Some(child) = self.row.first_child() {
            self.row.remove(&child);
        }
    }
}

fn initial_selection(len: usize, direction: Direction) -> usize {
    if len <= 1 {
        0
    } else if direction == Direction::Prev {
        len - 1
    } else {
        1
    }
}

fn window_card(window: &WindowInfo, selected: bool) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    card.add_css_class("switcher-card");
    if selected {
        card.add_css_class("selected");
    }

    let icon = gtk::Image::from_icon_name(icon_name(&window.wm_class));
    icon.add_css_class("switcher-icon");
    icon.set_pixel_size(44);
    icon.set_halign(gtk::Align::Center);
    card.append(&icon);

    let title = gtk::Label::new(Some(&window.short_title()));
    title.add_css_class("switcher-title");
    title.set_ellipsize(pango::EllipsizeMode::End);
    title.set_max_width_chars(18);
    title.set_lines(2);
    title.set_wrap(true);
    title.set_halign(gtk::Align::Center);
    title.set_justify(gtk::Justification::Center);
    card.append(&title);

    let meta_text = if window.pinned {
        format!("ws {} - pinned", window.workspace_name)
    } else {
        format!("ws {}", window.workspace_name)
    };
    let meta = gtk::Label::new(Some(&meta_text));
    meta.add_css_class("switcher-meta");
    meta.set_halign(gtk::Align::Center);
    card.append(&meta);

    card
}

impl WindowInfo {
    fn display_name(&self) -> String {
        if !self.title.trim().is_empty() && !self.wm_class.trim().is_empty() {
            format!("{}: {}", self.wm_class, self.title)
        } else if !self.title.trim().is_empty() {
            self.title.clone()
        } else if !self.wm_class.trim().is_empty() {
            self.wm_class.clone()
        } else {
            self.address.clone()
        }
    }

    fn short_title(&self) -> String {
        if !self.title.trim().is_empty() {
            self.title.clone()
        } else if !self.wm_class.trim().is_empty() {
            self.wm_class.clone()
        } else {
            self.address.clone()
        }
    }
}

fn icon_name(class_name: &str) -> &'static str {
    let normalized = class_name.to_ascii_lowercase();
    match normalized.as_str() {
        "zen" => "zen-browser",
        "firefox" => "firefox",
        "chromium" => "chromium",
        "google-chrome" | "chrome" => "google-chrome",
        "code" | "visual studio code" => "visual-studio-code",
        "dev.zed.zed" | "zed" => "zed",
        "alacritty" => "Alacritty",
        "kitty" => "kitty",
        "org.gnome.nautilus" | "nautilus" => "org.gnome.Nautilus",
        "signal" | "signal-desktop" => "signal-desktop",
        "spotify" => "spotify",
        "obsidian" => "obsidian",
        _ => "application-x-executable",
    }
}

fn collect_windows() -> Result<Vec<WindowInfo>> {
    let clients: Vec<HyprClient> = hyprctl_json(&["clients", "-j"])?;
    let active: Option<HyprActiveWindow> = hyprctl_json(&["activewindow", "-j"]).ok();
    let active_address = active.map(|a| a.address);

    let mut windows = clients
        .into_iter()
        .filter(|client| {
            !client.address.is_empty()
                && client.mapped
                && !client.hidden
                && client.accepts_input
                && client.workspace.id >= 0
        })
        .map(|client| WindowInfo {
            address: client.address,
            title: strip_markup(&client.title),
            wm_class: if client.wm_class.trim().is_empty() {
                client.initial_class
            } else {
                client.wm_class
            },
            workspace_id: client.workspace.id,
            workspace_name: if client.workspace.name.is_empty() {
                client.workspace.id.to_string()
            } else {
                client.workspace.name
            },
            monitor: client.monitor,
            focus_history_id: client.focus_history_id.unwrap_or(999_999),
            pinned: client.pinned,
        })
        .collect::<Vec<_>>();

    windows.sort_by(|a, b| {
        a.focus_history_id
            .cmp(&b.focus_history_id)
            .then_with(|| a.workspace_id.cmp(&b.workspace_id))
            .then_with(|| a.display_name().cmp(&b.display_name()))
    });

    if let Some(active_address) = active_address
        && let Some(active_index) = windows
            .iter()
            .position(|window| window.address == active_address)
    {
        let active = windows.remove(active_index);
        windows.insert(0, active);
    }

    Ok(windows)
}

fn focus_window(window: &WindowInfo) -> Result<()> {
    if window.workspace_id > 0 {
        hyprctl(&["dispatch", "workspace", &window.workspace_id.to_string()])?;
    }
    hyprctl(&[
        "dispatch",
        "focuswindow",
        &format!("address:{}", window.address),
    ])?;
    let _ = hyprctl(&["dispatch", "bringactivetotop"]);
    Ok(())
}

fn hyprctl(args: &[&str]) -> Result<()> {
    let output = Command::new("hyprctl")
        .args(args)
        .output()
        .with_context(|| format!("failed to run hyprctl {}", args.join(" ")))?;

    if !output.status.success() {
        bail!(
            "hyprctl {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    Ok(())
}

fn hyprctl_json<T>(args: &[&str]) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    let output = Command::new("hyprctl")
        .args(args)
        .output()
        .with_context(|| format!("failed to run hyprctl {}", args.join(" ")))?;

    if !output.status.success() {
        bail!(
            "hyprctl {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    serde_json::from_slice(&output.stdout)
        .with_context(|| format!("hyprctl {} returned invalid JSON", args.join(" ")))
}

fn send_or_start(command: IpcCommand, start_if_missing: bool) -> Result<()> {
    match send_ipc(command) {
        Ok(()) => Ok(()),
        Err(first_err) if start_if_missing => {
            let exe = env::current_exe().context("failed to resolve current executable")?;
            Command::new(exe)
                .arg("run")
                .spawn()
                .context("failed to auto-start switcher daemon")?;

            for _ in 0..20 {
                thread::sleep(Duration::from_millis(25));
                if send_ipc(command).is_ok() {
                    return Ok(());
                }
            }

            Err(first_err).context("daemon was not reachable after auto-start")
        }
        Err(err) => Err(err),
    }
}

fn send_ipc(command: IpcCommand) -> Result<()> {
    let mut stream =
        UnixStream::connect(socket_path()).context("switcher daemon is not running")?;
    stream
        .write_all(ipc_payload(command).as_bytes())
        .context("failed to send IPC command")?;
    Ok(())
}

fn spawn_ipc_listener(tx: mpsc::Sender<IpcCommand>) -> Result<()> {
    let path = socket_path();
    if path.exists() {
        if UnixStream::connect(&path).is_ok() {
            bail!("another daemon is already listening on {}", path.display());
        }
        fs::remove_file(&path)
            .with_context(|| format!("failed to remove stale {}", path.display()))?;
    }

    let listener =
        UnixListener::bind(&path).with_context(|| format!("failed to bind {}", path.display()))?;

    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else {
                continue;
            };
            let mut payload = String::new();
            if stream.read_to_string(&mut payload).is_ok()
                && let Some(command) = parse_ipc_payload(payload.trim())
            {
                let _ = tx.send(command);
            }
        }
    });

    Ok(())
}

fn ipc_payload(command: IpcCommand) -> &'static str {
    match command {
        IpcCommand::Open(Direction::Next) => "open next\n",
        IpcCommand::Open(Direction::Prev) => "open prev\n",
        IpcCommand::Close => "close\n",
        IpcCommand::Cancel => "cancel\n",
    }
}

fn parse_ipc_payload(payload: &str) -> Option<IpcCommand> {
    match payload {
        "open next" => Some(IpcCommand::Open(Direction::Next)),
        "open prev" => Some(IpcCommand::Open(Direction::Prev)),
        "close" => Some(IpcCommand::Close),
        "cancel" => Some(IpcCommand::Cancel),
        _ => None,
    }
}

fn socket_path() -> PathBuf {
    env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir)
        .join(SOCKET_NAME)
}

fn install_css() {
    let Some(display) = gdk::Display::default() else {
        return;
    };

    let provider = gtk::CssProvider::new();
    provider.load_from_data(CSS);
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

fn strip_markup(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut inside_tag = false;

    for ch in input.chars() {
        match ch {
            '<' => inside_tag = true,
            '>' => inside_tag = false,
            _ if !inside_tag => output.push(ch),
            _ => {}
        }
    }

    output.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_next_selects_previous_mru_window() {
        assert_eq!(initial_selection(0, Direction::Next), 0);
        assert_eq!(initial_selection(1, Direction::Next), 0);
        assert_eq!(initial_selection(4, Direction::Next), 1);
    }

    #[test]
    fn initial_prev_wraps_to_oldest_window() {
        assert_eq!(initial_selection(1, Direction::Prev), 0);
        assert_eq!(initial_selection(4, Direction::Prev), 3);
    }

    #[test]
    fn ipc_payloads_round_trip() {
        for command in [
            IpcCommand::Open(Direction::Next),
            IpcCommand::Open(Direction::Prev),
            IpcCommand::Close,
            IpcCommand::Cancel,
        ] {
            assert_eq!(
                parse_ipc_payload(ipc_payload(command).trim()).map(ipc_payload),
                Some(ipc_payload(command))
            );
        }
    }

    #[test]
    fn markup_is_stripped_from_titles() {
        assert_eq!(strip_markup("one <b>two</b> three"), "one two three");
    }
}
