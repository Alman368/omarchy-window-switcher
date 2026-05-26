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
use std::collections::HashMap;
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
const DUPLICATE_ADVANCE_GRACE: Duration = Duration::from_millis(45);
const CSS: &str = r#"
window {
  background: transparent;
}

.switcher-panel {
  background: rgba(18, 20, 24, 0.98);
  border: 1px solid rgba(210, 218, 230, 0.18);
  border-radius: 8px;
  padding: 14px;
  box-shadow: 0 14px 38px rgba(0, 0, 0, 0.38);
}

.switcher-row {
  border-spacing: 10px;
}

.switcher-card {
  background: rgba(37, 41, 49, 0.98);
  border: 2px solid rgba(210, 218, 230, 0.14);
  border-radius: 8px;
  padding: 14px;
  min-width: 176px;
  min-height: 138px;
}

.switcher-card.selected {
  background: rgba(48, 59, 76, 1.0);
  border-color: rgba(88, 166, 255, 0.96);
}

.switcher-icon {
  margin-bottom: 9px;
}

.switcher-app {
  color: rgba(221, 228, 238, 0.96);
  font-size: 12px;
  font-weight: 700;
  margin-bottom: 5px;
}

.switcher-title {
  color: rgba(248, 250, 252, 0.98);
  font-size: 13px;
  font-weight: 500;
}

.switcher-meta {
  color: rgba(185, 195, 210, 0.86);
  font-size: 11px;
  margin-top: 7px;
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
        #[arg(long, value_enum, default_value_t = Profile::Alt)]
        profile: Profile,
        #[arg(value_enum, default_value_t = Direction::Next)]
        direction: Direction,
    },
    /// Close the switcher and focus the selected window.
    Close,
    /// Close the switcher without switching.
    Cancel,
    /// Print the current MRU window list.
    List {
        #[arg(long, value_enum, default_value_t = Profile::Alt)]
        profile: Profile,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Profile {
    Alt,
    Super,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Direction {
    Next,
    Prev,
}

#[derive(Debug, Clone, Copy)]
enum IpcCommand {
    Open(OpenRequest),
    OpenFromKeyboard(OpenRequest),
    Close,
    Cancel,
}

#[derive(Debug, Clone, Copy)]
struct OpenRequest {
    profile: Profile,
    direction: Direction,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum Target {
    #[default]
    Windows,
    Workspaces,
    Monitors,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum Scope {
    #[default]
    All,
    CurrentWorkspace,
    CurrentMonitor,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(default)]
struct ProfileSettings {
    target: Target,
    scope: Scope,
    same_class: bool,
    include_special_workspaces: bool,
    include_empty_workspaces: bool,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(default)]
struct AppConfig {
    alt: ProfileSettings,
    #[serde(rename = "super")]
    super_profile: ProfileSettings,
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
struct HyprMonitorInfo {
    #[serde(default)]
    id: i64,
    #[serde(default)]
    name: String,
    #[serde(default, rename = "activeWorkspace")]
    active_workspace: HyprWorkspace,
    #[serde(default)]
    focused: bool,
    #[serde(default)]
    disabled: bool,
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

#[derive(Debug, Clone, Deserialize)]
struct HyprActiveWorkspace {
    #[serde(default)]
    id: i64,
    #[serde(default, rename = "monitorID")]
    monitor_id: i64,
}

#[derive(Debug, Clone, Deserialize)]
struct HyprWorkspaceInfo {
    #[serde(default)]
    id: i64,
    #[serde(default)]
    name: String,
    #[serde(default, rename = "monitorID")]
    monitor_id: i64,
    #[serde(default)]
    windows: i64,
    #[serde(default)]
    lastwindowtitle: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorkspaceInfo {
    id: i64,
    name: String,
    monitor: i64,
    windows: i64,
    last_title: String,
    best_focus_history_id: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MonitorInfo {
    id: i64,
    name: String,
    active_workspace_name: String,
    focused: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SwitchItem {
    Window(WindowInfo),
    Workspace(WorkspaceInfo),
    Monitor(MonitorInfo),
}

struct SwitcherState {
    window: gtk::ApplicationWindow,
    row: gtk::Box,
    items: Vec<SwitchItem>,
    selected: usize,
    open: bool,
    pending_close_until: Option<Instant>,
    last_advance_at: Option<Instant>,
    active_profile: Option<Profile>,
    config: AppConfig,
}

fn default_true() -> bool {
    true
}

impl Default for ProfileSettings {
    fn default() -> Self {
        Self {
            target: Target::Windows,
            scope: Scope::All,
            same_class: false,
            include_special_workspaces: false,
            include_empty_workspaces: false,
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            alt: ProfileSettings {
                target: Target::Windows,
                scope: Scope::All,
                same_class: false,
                include_special_workspaces: false,
                include_empty_workspaces: false,
            },
            super_profile: ProfileSettings {
                target: Target::Workspaces,
                scope: Scope::CurrentMonitor,
                same_class: false,
                include_special_workspaces: false,
                include_empty_workspaces: false,
            },
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        CliCommand::Run => run_daemon(),
        CliCommand::Open { profile, direction } => {
            send_or_start(IpcCommand::Open(OpenRequest { profile, direction }), true)?;
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
        CliCommand::List { profile } => {
            let config = load_config();
            for item in collect_items(config.profile(profile))? {
                println!("{}  [{}]", item.display_name(), item.meta_text());
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
            .default_width(1080)
            .default_height(204)
            .build();
        window.init_layer_shell();
        window.set_namespace(Some(NAMESPACE));
        window.set_layer(Layer::Overlay);
        window.set_keyboard_mode(KeyboardMode::Exclusive);

        let panel = gtk::Box::new(gtk::Orientation::Vertical, 0);
        panel.add_css_class("switcher-panel");
        panel.set_halign(gtk::Align::Center);
        panel.set_valign(gtk::Align::Center);
        panel.set_size_request(1080, 180);

        let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        row.add_css_class("switcher-row");
        row.set_halign(gtk::Align::Center);
        row.set_valign(gtk::Align::Center);
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
            items: Vec::new(),
            selected: 0,
            open: false,
            pending_close_until: None,
            last_advance_at: None,
            active_profile: None,
            config: load_config(),
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
            let profile = state_pressed
                .borrow()
                .active_profile
                .unwrap_or(Profile::Alt);
            state_pressed
                .borrow_mut()
                .handle(IpcCommand::OpenFromKeyboard(OpenRequest {
                    profile,
                    direction: Direction::Next,
                }));
            Propagation::Stop
        }
        gdk::Key::ISO_Left_Tab | gdk::Key::Left | gdk::Key::h | gdk::Key::grave => {
            let profile = state_pressed
                .borrow()
                .active_profile
                .unwrap_or(Profile::Alt);
            state_pressed
                .borrow_mut()
                .handle(IpcCommand::OpenFromKeyboard(OpenRequest {
                    profile,
                    direction: Direction::Prev,
                }));
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
            IpcCommand::Open(request) | IpcCommand::OpenFromKeyboard(request) => {
                self.open_or_advance(request)
            }
            IpcCommand::Close => self.close(true),
            IpcCommand::Cancel => self.close(false),
        }
    }

    fn open_or_advance(&mut self, request: OpenRequest) {
        if self.open {
            let now = Instant::now();
            if is_duplicate_advance(self.last_advance_at, now) {
                return;
            }
            self.last_advance_at = Some(now);
            self.advance(request.direction);
            self.render();
            return;
        }

        self.config = load_config();
        let settings = self.config.profile(request.profile);
        match collect_items(settings) {
            Ok(items) if !items.is_empty() => {
                self.items = items;
                self.selected = initial_selection(self.items.len(), request.direction);
                self.open = true;
                self.last_advance_at = Some(Instant::now());
                self.active_profile = Some(request.profile);
                self.render();
                self.window.present();
                self.window.grab_focus();
                if self.consume_pending_close() {
                    self.close(true);
                }
            }
            Ok(_) => {}
            Err(err) => eprintln!("omarchy-window-switcher: failed to collect items: {err:#}"),
        }
    }

    fn advance(&mut self, direction: Direction) {
        if self.items.is_empty() {
            self.selected = 0;
            return;
        }

        self.selected = match direction {
            Direction::Next => (self.selected + 1) % self.items.len(),
            Direction::Prev => {
                if self.selected == 0 {
                    self.items.len() - 1
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
        self.last_advance_at = None;
        self.active_profile = None;
        self.open = false;
        self.window.set_visible(false);

        let selected = self.items.get(self.selected).cloned();
        self.clear();
        self.items.clear();
        self.selected = 0;

        if should_focus
            && let Some(item) = selected
            && let Err(err) = focus_item(&item)
        {
            eprintln!("omarchy-window-switcher: failed to focus item: {err:#}");
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

        for (idx, item) in self.items.iter().enumerate() {
            self.row.append(&item_card(item, idx == self.selected));
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

fn is_duplicate_advance(last_advance_at: Option<Instant>, now: Instant) -> bool {
    last_advance_at.is_some_and(|last| now.duration_since(last) < DUPLICATE_ADVANCE_GRACE)
}

fn item_card(item: &SwitchItem, selected: bool) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    card.add_css_class("switcher-card");
    if selected {
        card.add_css_class("selected");
    }

    let icon = gtk::Image::from_icon_name(item.icon_name());
    icon.add_css_class("switcher-icon");
    icon.set_pixel_size(44);
    icon.set_halign(gtk::Align::Center);
    card.append(&icon);

    let app = gtk::Label::new(Some(&item.app_label()));
    app.add_css_class("switcher-app");
    app.set_ellipsize(pango::EllipsizeMode::End);
    app.set_max_width_chars(20);
    app.set_halign(gtk::Align::Center);
    card.append(&app);

    let title = gtk::Label::new(Some(&item.short_title()));
    title.add_css_class("switcher-title");
    title.set_ellipsize(pango::EllipsizeMode::End);
    title.set_max_width_chars(24);
    title.set_lines(2);
    title.set_wrap(true);
    title.set_halign(gtk::Align::Center);
    title.set_justify(gtk::Justification::Center);
    card.append(&title);

    let meta_text = item.meta_text();
    let meta = gtk::Label::new(Some(&meta_text));
    meta.add_css_class("switcher-meta");
    meta.set_halign(gtk::Align::Center);
    card.append(&meta);

    card
}

impl AppConfig {
    fn profile(&self, profile: Profile) -> ProfileSettings {
        match profile {
            Profile::Alt => self.alt,
            Profile::Super => self.super_profile,
        }
    }
}

impl SwitchItem {
    fn display_name(&self) -> String {
        match self {
            Self::Window(window) => window.display_name(),
            Self::Workspace(workspace) => workspace.display_name(),
            Self::Monitor(monitor) => monitor.display_name(),
        }
    }

    fn short_title(&self) -> String {
        match self {
            Self::Window(window) => window.short_title(),
            Self::Workspace(workspace) => workspace.short_title(),
            Self::Monitor(monitor) => monitor.short_title(),
        }
    }

    fn app_label(&self) -> String {
        match self {
            Self::Window(window) => app_display_name(&window.wm_class).to_string(),
            Self::Workspace(workspace) => format!("Workspace {}", workspace.name),
            Self::Monitor(_) => "Monitor".to_string(),
        }
    }

    fn meta_text(&self) -> String {
        match self {
            Self::Window(window) => {
                if window.pinned {
                    format!("ws {} - pinned", window.workspace_name)
                } else {
                    format!("ws {}", window.workspace_name)
                }
            }
            Self::Workspace(workspace) => {
                let count = if workspace.windows == 1 {
                    "window"
                } else {
                    "windows"
                };
                format!("ws {} - {} {}", workspace.name, workspace.windows, count)
            }
            Self::Monitor(monitor) => format!("ws {}", monitor.active_workspace_name),
        }
    }

    fn icon_name(&self) -> &'static str {
        match self {
            Self::Window(window) => icon_name(&window.wm_class),
            Self::Workspace(_) => "view-grid-symbolic",
            Self::Monitor(_) => "video-display-symbolic",
        }
    }
}

impl WindowInfo {
    fn display_name(&self) -> String {
        if !self.title.trim().is_empty() && !self.wm_class.trim().is_empty() {
            format!(
                "{}: {}",
                app_display_name(&self.wm_class),
                clean_display_title(&self.title)
            )
        } else if !self.title.trim().is_empty() {
            clean_display_title(&self.title)
        } else if !self.wm_class.trim().is_empty() {
            app_display_name(&self.wm_class).to_string()
        } else {
            self.address.clone()
        }
    }

    fn short_title(&self) -> String {
        if !self.title.trim().is_empty() {
            clean_display_title(&self.title)
        } else if !self.wm_class.trim().is_empty() {
            app_display_name(&self.wm_class).to_string()
        } else {
            self.address.clone()
        }
    }
}

impl WorkspaceInfo {
    fn display_name(&self) -> String {
        format!(
            "Workspace {}: {}",
            self.name,
            clean_display_title(&self.last_title)
        )
    }

    fn short_title(&self) -> String {
        if self.last_title.trim().is_empty() {
            format!("Workspace {}", self.name)
        } else {
            clean_display_title(&self.last_title)
        }
    }
}

impl MonitorInfo {
    fn display_name(&self) -> String {
        format!(
            "Monitor {}: workspace {}",
            self.name, self.active_workspace_name
        )
    }

    fn short_title(&self) -> String {
        self.name.clone()
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

fn app_display_name(class_name: &str) -> &'static str {
    let normalized = class_name.to_ascii_lowercase();
    match normalized.as_str() {
        "zen" => "Zen Browser",
        "firefox" => "Firefox",
        "chromium" => "Chromium",
        "google-chrome" | "chrome" => "Chrome",
        "chrome-github.com__-default" => "GitHub",
        "code" | "visual studio code" => "VS Code",
        "dev.zed.zed" | "zed" => "Zed",
        "alacritty" => "Alacritty",
        "kitty" => "Kitty",
        "org.gnome.nautilus" | "nautilus" => "Files",
        "signal" | "signal-desktop" => "Signal",
        "spotify" => "Spotify",
        "obsidian" => "Obsidian",
        _ => "Application",
    }
}

fn clean_display_title(title: &str) -> String {
    title
        .replace(" --- ", " | ")
        .replace(" -- ", " | ")
        .replace(" — ", " | ")
        .replace(" – ", " | ")
}

fn load_config() -> AppConfig {
    let path = config_path();
    let Ok(contents) = fs::read_to_string(&path) else {
        return AppConfig::default();
    };

    match toml::from_str::<AppConfig>(&contents) {
        Ok(config) => config,
        Err(err) => {
            eprintln!(
                "omarchy-window-switcher: failed to parse {}: {err}",
                path.display()
            );
            AppConfig::default()
        }
    }
}

fn config_path() -> PathBuf {
    env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(env::temp_dir)
        .join("omarchy-window-switcher")
        .join("config.toml")
}

fn collect_items(settings: ProfileSettings) -> Result<Vec<SwitchItem>> {
    match settings.target {
        Target::Windows => collect_windows(settings)
            .map(|windows| windows.into_iter().map(SwitchItem::Window).collect()),
        Target::Workspaces => collect_workspaces(settings)
            .map(|workspaces| workspaces.into_iter().map(SwitchItem::Workspace).collect()),
        Target::Monitors => collect_monitors()
            .map(|monitors| monitors.into_iter().map(SwitchItem::Monitor).collect()),
    }
}

fn collect_windows(settings: ProfileSettings) -> Result<Vec<WindowInfo>> {
    let clients: Vec<HyprClient> = hyprctl_json(&["clients", "-j"])?;
    let active: Option<HyprActiveWindow> = hyprctl_json(&["activewindow", "-j"]).ok();
    let active_workspace: Option<HyprActiveWorkspace> =
        hyprctl_json(&["activeworkspace", "-j"]).ok();
    let active_address = active.as_ref().map(|a| a.address.clone());
    let active_workspace_id = active_workspace.as_ref().map(|workspace| workspace.id);
    let active_monitor_id = active_workspace
        .as_ref()
        .map(|workspace| workspace.monitor_id);
    let active_class = active_address.as_ref().and_then(|active_address| {
        clients
            .iter()
            .find(|client| &client.address == active_address)
            .map(client_class)
    });

    let mut windows = clients
        .into_iter()
        .filter(|client| {
            !client.address.is_empty()
                && client.mapped
                && !client.hidden
                && client.accepts_input
                && (settings.include_special_workspaces || client.workspace.id >= 0)
                && (!settings.same_class
                    || active_class
                        .as_ref()
                        .is_some_and(|active_class| client_class(client) == *active_class))
                && settings.scope.includes(
                    client.workspace.id,
                    client.monitor,
                    active_workspace_id,
                    active_monitor_id,
                )
        })
        .map(|client| WindowInfo {
            address: client.address,
            title: strip_markup(&client.title),
            wm_class: resolved_class(client.wm_class, client.initial_class),
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

fn collect_workspaces(settings: ProfileSettings) -> Result<Vec<WorkspaceInfo>> {
    let clients = collect_windows(ProfileSettings {
        target: Target::Windows,
        scope: Scope::All,
        same_class: false,
        include_special_workspaces: settings.include_special_workspaces,
        include_empty_workspaces: false,
    })?;
    let workspaces: Vec<HyprWorkspaceInfo> = hyprctl_json(&["workspaces", "-j"])?;
    let active_workspace: HyprActiveWorkspace = hyprctl_json(&["activeworkspace", "-j"])?;
    let active_workspace_id = Some(active_workspace.id);
    let active_monitor_id = Some(active_workspace.monitor_id);

    let mut best_focus_by_workspace = HashMap::<i64, i64>::new();
    for client in clients {
        best_focus_by_workspace
            .entry(client.workspace_id)
            .and_modify(|best| *best = (*best).min(client.focus_history_id))
            .or_insert(client.focus_history_id);
    }

    let mut items = workspaces
        .into_iter()
        .filter(|workspace| {
            (settings.include_special_workspaces || workspace.id >= 0)
                && (settings.include_empty_workspaces || workspace.windows > 0)
                && settings.scope.includes(
                    workspace.id,
                    workspace.monitor_id,
                    active_workspace_id,
                    active_monitor_id,
                )
        })
        .map(|workspace| WorkspaceInfo {
            id: workspace.id,
            name: if workspace.name.is_empty() {
                workspace.id.to_string()
            } else {
                workspace.name
            },
            monitor: workspace.monitor_id,
            windows: workspace.windows,
            last_title: strip_markup(&workspace.lastwindowtitle),
            best_focus_history_id: best_focus_by_workspace
                .get(&workspace.id)
                .copied()
                .unwrap_or(999_999),
        })
        .collect::<Vec<_>>();

    items.sort_by(|a, b| {
        a.best_focus_history_id
            .cmp(&b.best_focus_history_id)
            .then_with(|| a.id.cmp(&b.id))
    });

    if let Some(active_index) = items
        .iter()
        .position(|workspace| workspace.id == active_workspace.id)
    {
        let active = items.remove(active_index);
        items.insert(0, active);
    }

    Ok(items)
}

fn collect_monitors() -> Result<Vec<MonitorInfo>> {
    let mut monitors = hyprctl_json::<Vec<HyprMonitorInfo>>(&["monitors", "-j"])?
        .into_iter()
        .filter(|monitor| !monitor.disabled && !monitor.name.is_empty())
        .map(|monitor| MonitorInfo {
            id: monitor.id,
            name: monitor.name,
            active_workspace_name: if monitor.active_workspace.name.is_empty() {
                monitor.active_workspace.id.to_string()
            } else {
                monitor.active_workspace.name
            },
            focused: monitor.focused,
        })
        .collect::<Vec<_>>();

    monitors.sort_by(|a, b| {
        b.focused
            .cmp(&a.focused)
            .then_with(|| a.id.cmp(&b.id))
            .then_with(|| a.name.cmp(&b.name))
    });

    Ok(monitors)
}

fn resolved_class(wm_class: String, initial_class: String) -> String {
    if wm_class.trim().is_empty() {
        initial_class
    } else {
        wm_class
    }
}

fn client_class(client: &HyprClient) -> String {
    if client.wm_class.trim().is_empty() {
        client.initial_class.clone()
    } else {
        client.wm_class.clone()
    }
}

impl Scope {
    fn includes(
        self,
        workspace_id: i64,
        monitor_id: i64,
        active_workspace_id: Option<i64>,
        active_monitor_id: Option<i64>,
    ) -> bool {
        match self {
            Self::All => true,
            Self::CurrentWorkspace => active_workspace_id == Some(workspace_id),
            Self::CurrentMonitor => active_monitor_id == Some(monitor_id),
        }
    }
}

fn focus_item(item: &SwitchItem) -> Result<()> {
    match item {
        SwitchItem::Window(window) => focus_window(window),
        SwitchItem::Workspace(workspace) => focus_workspace(workspace.id),
        SwitchItem::Monitor(monitor) => focus_monitor(&monitor.name),
    }
}

fn focus_window(window: &WindowInfo) -> Result<()> {
    if window.workspace_id > 0 {
        focus_workspace(window.workspace_id)?;
    }
    hyprctl(&[
        "dispatch",
        "focuswindow",
        &format!("address:{}", window.address),
    ])?;
    let _ = hyprctl(&["dispatch", "bringactivetotop"]);
    Ok(())
}

fn focus_workspace(workspace_id: i64) -> Result<()> {
    hyprctl(&["dispatch", "workspace", &workspace_id.to_string()])
}

fn focus_monitor(monitor_name: &str) -> Result<()> {
    hyprctl(&["dispatch", "focusmonitor", monitor_name])
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
        IpcCommand::Open(OpenRequest {
            profile: Profile::Alt,
            direction: Direction::Next,
        })
        | IpcCommand::OpenFromKeyboard(OpenRequest {
            profile: Profile::Alt,
            direction: Direction::Next,
        }) => "open alt next\n",
        IpcCommand::Open(OpenRequest {
            profile: Profile::Alt,
            direction: Direction::Prev,
        })
        | IpcCommand::OpenFromKeyboard(OpenRequest {
            profile: Profile::Alt,
            direction: Direction::Prev,
        }) => "open alt prev\n",
        IpcCommand::Open(OpenRequest {
            profile: Profile::Super,
            direction: Direction::Next,
        })
        | IpcCommand::OpenFromKeyboard(OpenRequest {
            profile: Profile::Super,
            direction: Direction::Next,
        }) => "open super next\n",
        IpcCommand::Open(OpenRequest {
            profile: Profile::Super,
            direction: Direction::Prev,
        })
        | IpcCommand::OpenFromKeyboard(OpenRequest {
            profile: Profile::Super,
            direction: Direction::Prev,
        }) => "open super prev\n",
        IpcCommand::Close => "close\n",
        IpcCommand::Cancel => "cancel\n",
    }
}

fn parse_ipc_payload(payload: &str) -> Option<IpcCommand> {
    match payload {
        "open next" | "open alt next" => Some(IpcCommand::Open(OpenRequest {
            profile: Profile::Alt,
            direction: Direction::Next,
        })),
        "open prev" | "open alt prev" => Some(IpcCommand::Open(OpenRequest {
            profile: Profile::Alt,
            direction: Direction::Prev,
        })),
        "open super next" => Some(IpcCommand::Open(OpenRequest {
            profile: Profile::Super,
            direction: Direction::Next,
        })),
        "open super prev" => Some(IpcCommand::Open(OpenRequest {
            profile: Profile::Super,
            direction: Direction::Prev,
        })),
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
    fn duplicate_advance_guard_catches_initial_overlay_key_event() {
        let opened_at = Instant::now();

        assert!(is_duplicate_advance(
            Some(opened_at),
            opened_at + Duration::from_millis(20)
        ));
        assert!(!is_duplicate_advance(
            Some(opened_at),
            opened_at + DUPLICATE_ADVANCE_GRACE + Duration::from_millis(1)
        ));
        assert!(!is_duplicate_advance(None, opened_at));
    }

    #[test]
    fn ipc_payloads_round_trip() {
        for command in [
            IpcCommand::Open(OpenRequest {
                profile: Profile::Alt,
                direction: Direction::Next,
            }),
            IpcCommand::Open(OpenRequest {
                profile: Profile::Alt,
                direction: Direction::Prev,
            }),
            IpcCommand::Open(OpenRequest {
                profile: Profile::Super,
                direction: Direction::Next,
            }),
            IpcCommand::Open(OpenRequest {
                profile: Profile::Super,
                direction: Direction::Prev,
            }),
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
    fn legacy_open_payloads_use_alt_profile() {
        let Some(IpcCommand::Open(request)) = parse_ipc_payload("open next") else {
            panic!("legacy payload did not parse");
        };

        assert_eq!(request.profile, Profile::Alt);
        assert_eq!(request.direction, Direction::Next);
    }

    #[test]
    fn default_config_is_global_alt_and_monitor_scoped_super() {
        let config = AppConfig::default();

        assert_eq!(
            config.profile(Profile::Alt),
            ProfileSettings {
                target: Target::Windows,
                scope: Scope::All,
                same_class: false,
                include_special_workspaces: false,
                include_empty_workspaces: false,
            }
        );
        assert_eq!(
            config.profile(Profile::Super),
            ProfileSettings {
                target: Target::Workspaces,
                scope: Scope::CurrentMonitor,
                same_class: false,
                include_special_workspaces: false,
                include_empty_workspaces: false,
            }
        );
    }

    #[test]
    fn partial_config_preserves_profile_defaults() {
        let config = toml::from_str::<AppConfig>(
            r#"
            [alt]
            scope = "current-workspace"
            "#,
        )
        .expect("config should parse");

        assert_eq!(
            config.profile(Profile::Alt),
            ProfileSettings {
                target: Target::Windows,
                scope: Scope::CurrentWorkspace,
                same_class: false,
                include_special_workspaces: false,
                include_empty_workspaces: false,
            }
        );
        assert_eq!(
            config.profile(Profile::Super),
            ProfileSettings {
                target: Target::Workspaces,
                scope: Scope::CurrentMonitor,
                same_class: false,
                include_special_workspaces: false,
                include_empty_workspaces: false,
            }
        );
    }

    #[test]
    fn extended_profile_config_parses() {
        let config = toml::from_str::<AppConfig>(
            r#"
            [super]
            target = "windows"
            scope = "current-workspace"
            same_class = true
            include_special_workspaces = true
            include_empty_workspaces = true
            "#,
        )
        .expect("config should parse");

        assert_eq!(
            config.profile(Profile::Super),
            ProfileSettings {
                target: Target::Windows,
                scope: Scope::CurrentWorkspace,
                same_class: true,
                include_special_workspaces: true,
                include_empty_workspaces: true,
            }
        );
    }

    #[test]
    fn scope_current_monitor_matches_only_active_monitor() {
        assert!(Scope::CurrentMonitor.includes(6, 1, Some(6), Some(1)));
        assert!(!Scope::CurrentMonitor.includes(2, 0, Some(6), Some(1)));
    }

    #[test]
    fn scope_current_workspace_matches_only_active_workspace() {
        assert!(Scope::CurrentWorkspace.includes(6, 1, Some(6), Some(1)));
        assert!(!Scope::CurrentWorkspace.includes(2, 1, Some(6), Some(1)));
    }

    #[test]
    fn markup_is_stripped_from_titles() {
        assert_eq!(strip_markup("one <b>two</b> three"), "one two three");
    }

    #[test]
    fn display_titles_use_pipe_separators() {
        assert_eq!(clean_display_title("Desktop — rnn.py"), "Desktop | rnn.py");
        assert_eq!(
            clean_display_title("one --- two -- three – four"),
            "one | two | three | four"
        );
    }
}
