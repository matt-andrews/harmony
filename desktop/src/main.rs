//! Harmony desktop: the same server, in-process, behind a native window.
//!
//! Embedded mode binds the axum app to a random localhost port on a tokio
//! runtime and opens a system webview on it. `--url` skips the server and just
//! shows a running instance.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop};
use tao::window::{Icon, WindowBuilder};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;
use wry::WebViewBuilder;

use harmony::storage::Storage;
use harmony::{AppState, router};

use config::{Mode, app_data_dir, resolve_mode};

const ICON_RGBA: &[u8] = include_bytes!("../icons/harmony-64.rgba");
const ICON_SIZE: u32 = 64;
const BACKGROUND: (u8, u8, u8, u8) = (30, 32, 48, 255);

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

    if let Err(e) = run_window(content) {
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

fn run_window(content: Content) -> anyhow::Result<()> {
    let event_loop = EventLoop::new();
    let icon = Icon::from_rgba(ICON_RGBA.to_vec(), ICON_SIZE, ICON_SIZE).ok();
    let window = WindowBuilder::new()
        .with_title("Harmony")
        .with_inner_size(LogicalSize::new(960.0, 720.0))
        .with_min_inner_size(LogicalSize::new(480.0, 400.0))
        .with_window_icon(icon)
        .build(&event_loop)
        .context("creating the window")?;

    let builder = WebViewBuilder::new()
        .with_background_color(BACKGROUND)
        .with_devtools(cfg!(debug_assertions));
    let builder = match content {
        Content::Url(url) => builder.with_url(url),
        Content::Error(html) => builder.with_html(html),
    };
    let webview = builder.build(&window).context("creating the webview (is the WebView2 runtime installed?)")?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        // Keep the webview alive for as long as the loop runs.
        let _keep = &webview;
        if let Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } = event
        {
            info!("window closed");
            *control_flow = ControlFlow::Exit;
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
