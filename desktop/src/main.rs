//! Harmony desktop: the same server, in-process, behind a native window.
//!
//! Embedded mode binds the axum app to a random localhost port on a tokio
//! runtime and opens a system webview on it. `--url` skips the server and just
//! shows a running instance.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod compact;
mod config;
mod ipc;
mod settings;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use tao::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
#[cfg(windows)]
use tao::platform::windows::WindowExtWindows;
use tao::window::{Icon, Window, WindowBuilder};
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;
use wry::WebViewBuilder;

use harmony::storage::Storage;
use harmony::{AppState, router};

use compact::{COMPACT_SIZE, Monitor, place};
use config::{Mode, app_data_dir, resolve_mode};
use ipc::IpcMessage;
use settings::Settings;

const ICON_RGBA: &[u8] = include_bytes!("../icons/harmony-64.rgba");
const ICON_SIZE: u32 = 64;
const BACKGROUND: (u8, u8, u8, u8) = (30, 32, 48, 255);
/// Logical sizes of the full window.
const FULL_SIZE: (f64, f64) = (960.0, 720.0);
const MIN_SIZE: (f64, f64) = (480.0, 400.0);

fn main() {
    load_dotenv();
    let data_dir = dirs::data_local_dir();
    let log_path = init_logging(data_dir.as_deref());

    // Everything that can fail happens here; the window always opens so a
    // hidden-console release build still tells you what went wrong.
    let content = match start(data_dir.as_deref()) {
        Ok(url) => Content::Url(url),
        Err(e) => {
            error!("startup failed: {e:#}");
            Content::Error(error_page(&format!("{e:#}"), log_path.as_deref()))
        }
    };

    let settings_path = app_data_dir(data_dir.as_deref()).ok().map(|d| d.join(settings::FILE_NAME));
    if let Err(e) = run_window(content, settings_path) {
        error!("window failed: {e:#}");
        std::process::exit(1);
    }
}

enum Content {
    Url(String),
    Error(String),
}

/// `.env` next to the exe first, then the working directory. Neither is
/// required, and existing environment variables always win.
fn load_dotenv() {
    if let Some(dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(PathBuf::from)) {
        let _ = dotenvy::from_path(dir.join(".env"));
    }
    let _ = dotenvy::dotenv();
}

/// Stderr in debug builds, `<data dir>/Harmony/harmony-desktop.log` in release.
fn init_logging(data_dir: Option<&std::path::Path>) -> Option<PathBuf> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| "harmony=info,harmony_desktop=info".into());
    if cfg!(debug_assertions) {
        tracing_subscriber::fmt().with_env_filter(filter).init();
        return None;
    }
    let path = app_data_dir(data_dir).ok().map(|d| d.join("harmony-desktop.log"));
    let file = path.as_ref().and_then(|p| {
        std::fs::create_dir_all(p.parent()?).ok()?;
        std::fs::OpenOptions::new().create(true).append(true).open(p).ok()
    });
    match file {
        Some(f) => tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_ansi(false)
            .with_writer(Arc::new(f))
            .init(),
        None => tracing_subscriber::fmt().with_env_filter(filter).init(),
    }
    path
}

/// Resolve the mode and, for embedded mode, start the server. Returns the URL
/// the window should open.
fn start(data_dir: Option<&std::path::Path>) -> anyhow::Result<String> {
    let mode = resolve_mode(std::env::args().skip(1), |k| std::env::var(k).ok(), data_dir)?;
    match mode {
        Mode::Attach { url } => {
            info!("attaching to {url}");
            Ok(url)
        }
        Mode::Embedded { storage_spec } => {
            let storage = Storage::from_spec(&storage_spec)?;
            info!("loading data from {}", storage.describe());
            let runtime = tokio::runtime::Runtime::new().context("starting tokio")?;
            let (listener, state) = runtime.block_on(async {
                let (data, etag) = storage.load().await?;
                info!(
                    projects = data.projects.len(),
                    tasks = data.tasks.len(),
                    sessions = data.sessions.len(),
                    "loaded"
                );
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                    .await
                    .context("binding a localhost port")?;
                anyhow::Ok((listener, Arc::new(AppState::new(data, etag, storage))))
            })?;
            let addr = listener.local_addr()?;
            let url = format!("http://{addr}/");
            info!("serving on {url}");
            runtime.spawn(async move {
                if let Err(e) = axum::serve(listener, router(state)).await {
                    error!("server stopped: {e}");
                }
            });
            // The runtime must outlive the window; the event loop never returns
            // (it ends the process), so leaking it is the simplest correct thing.
            std::mem::forget(runtime);
            Ok(url)
        }
    }
}

/// The one window, which is either the full app or the floating compact strip.
struct Shell {
    window: Window,
    settings: Settings,
    /// `None` when there is no data directory; preferences then last one run.
    settings_path: Option<PathBuf>,
    /// The full window's shape while compact; `None` means not compact.
    restore: Option<Restore>,
}

struct Restore {
    size: PhysicalSize<u32>,
    position: Option<PhysicalPosition<i32>>,
    maximized: bool,
}

impl Shell {
    fn handle(&mut self, message: IpcMessage) {
        match message {
            IpcMessage::Compact => self.compact(),
            IpcMessage::Expand => self.expand(),
            IpcMessage::Drag => {
                if self.restore.is_some() {
                    let _ = self.window.drag_window();
                }
            }
            IpcMessage::SetCompactOnStart { value } => {
                self.settings.compact_on_start = value;
                self.save_settings();
            }
        }
    }

    fn compact(&mut self) {
        if self.restore.is_some() {
            return;
        }
        let w = &self.window;
        let maximized = w.is_maximized();
        if maximized {
            w.set_maximized(false);
        }
        self.restore = Some(Restore { size: w.inner_size(), position: w.outer_position().ok(), maximized });

        let size = LogicalSize::new(COMPACT_SIZE.0, COMPACT_SIZE.1);
        w.set_decorations(false);
        #[cfg(windows)]
        w.set_undecorated_shadow(true);
        w.set_min_inner_size(Some(size));
        w.set_inner_size(size);
        w.set_resizable(false);
        w.set_always_on_top(true);

        let monitor = |m: tao::monitor::MonitorHandle| Monitor {
            x: m.position().x,
            y: m.position().y,
            width: m.size().width,
            height: m.size().height,
            scale: m.scale_factor(),
        };
        if let Some(current) = w.current_monitor().or_else(|| w.primary_monitor()).map(monitor) {
            let all: Vec<Monitor> = w.available_monitors().map(monitor).collect();
            let physical = size.to_physical::<u32>(current.scale);
            let (x, y) = place(self.settings.compact_position, (physical.width, physical.height), current, &all);
            w.set_outer_position(PhysicalPosition::new(x, y));
        }
    }

    fn expand(&mut self) {
        let Some(restore) = self.restore.take() else { return };
        self.remember_compact_position();
        let w = &self.window;
        w.set_always_on_top(false);
        w.set_resizable(true);
        w.set_decorations(true);
        w.set_min_inner_size(Some(LogicalSize::new(MIN_SIZE.0, MIN_SIZE.1)));
        w.set_inner_size(restore.size);
        if let Some(position) = restore.position {
            w.set_outer_position(position);
        }
        if restore.maximized {
            w.set_maximized(true);
        }
        w.set_focus();
    }

    fn remember_compact_position(&mut self) {
        if let Ok(p) = self.window.outer_position() {
            self.settings.compact_position = Some((p.x, p.y));
            self.save_settings();
        }
    }

    fn save_settings(&self) {
        if let Some(path) = &self.settings_path
            && let Err(e) = self.settings.save(path)
        {
            warn!("saving settings: {e:#}");
        }
    }
}

fn run_window(content: Content, settings_path: Option<PathBuf>) -> anyhow::Result<()> {
    let settings = settings_path.as_deref().map(Settings::load).unwrap_or_default();
    let event_loop = EventLoopBuilder::<IpcMessage>::with_user_event().build();
    let icon = Icon::from_rgba(ICON_RGBA.to_vec(), ICON_SIZE, ICON_SIZE).ok();
    let window = WindowBuilder::new()
        .with_title("Harmony")
        .with_inner_size(LogicalSize::new(FULL_SIZE.0, FULL_SIZE.1))
        .with_min_inner_size(LogicalSize::new(MIN_SIZE.0, MIN_SIZE.1))
        .with_window_icon(icon)
        .build(&event_loop)
        .context("creating the window")?;

    // The page learns it is inside the desktop window (and its saved
    // preferences) before its own scripts run, and talks back over `window.ipc`.
    let init = serde_json::json!({ "compactOnStart": settings.compact_on_start });
    let proxy = event_loop.create_proxy();
    let builder = WebViewBuilder::new()
        .with_background_color(BACKGROUND)
        .with_devtools(cfg!(debug_assertions))
        .with_initialization_script(format!("window.__HARMONY_DESKTOP__ = {init};"))
        .with_ipc_handler(move |request| {
            if let Some(message) = ipc::parse(request.body()) {
                let _ = proxy.send_event(message);
            }
        });
    let builder = match content {
        Content::Url(url) => builder.with_url(url),
        Content::Error(html) => builder.with_html(html),
    };
    let webview = builder.build(&window).context("creating the webview (is the WebView2 runtime installed?)")?;

    let mut shell = Shell { window, settings, settings_path, restore: None };
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        // Keep the webview alive for as long as the loop runs.
        let _keep = &webview;
        match event {
            Event::UserEvent(message) => shell.handle(message),
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                if shell.restore.is_some() {
                    shell.remember_compact_position();
                }
                info!("window closed");
                *control_flow = ControlFlow::Exit;
            }
            _ => {}
        }
    });
}

fn error_page(message: &str, log_path: Option<&std::path::Path>) -> String {
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let log = log_path
        .map(|p| format!("<p class=\"muted\">Log: <code>{}</code></p>", esc(&p.display().to_string())))
        .unwrap_or_default();
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Harmony</title><style>\
         body{{margin:0;background:#1e2030;color:#cad3f5;font:15px system-ui,sans-serif;padding:32px}}\
         h1{{font-size:18px;margin:0 0 12px}}pre{{white-space:pre-wrap;background:#24273a;border:1px solid #494d64;\
         border-radius:8px;padding:12px}}code{{color:#c6a0f6}}.muted{{color:#a5adcb}}</style></head><body>\
         <h1>Harmony could not start</h1><pre>{}</pre>{log}\
         <p class=\"muted\">Check <code>HARMONY_STORAGE</code> in the environment or a <code>.env</code> next to the exe, \
         or pass <code>--url http://localhost:31415</code> to attach to a running instance.</p></body></html>",
        esc(message)
    )
}
