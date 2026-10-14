use std::collections::HashSet;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use anyhow::{Context, Result};
use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Html;
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::routing::get;
use notify::Watcher;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::{Stream, StreamExt};
use tower_http::services::ServeDir;

use crate::build;
use crate::cli::ServeArgs;
use crate::site::Site;

/// Shared between the rebuild thread and the SSE handler: a build version
/// that bumps on every successful rebuild, and a channel announcing bumps.
struct ReloadState {
    version: AtomicU64,
    announce: broadcast::Sender<u64>,
}

pub fn run(args: &ServeArgs) -> Result<()> {
    let root = args.build.root.clone();
    let out = args.build.out_dir();
    let options = build::BuildOptions {
        live_reload: true,
        allow_dangling_links: args.build.allow_dangling_links,
        strict: args.build.strict,
        render_llms_full: args.build.render_llms_full,
        base: String::new(),
        cachebust: args.build.cbpath.clone(),
    };
    // A broken site shouldn't kill the dev server: report, serve whatever
    // output already exists, and let the watcher rebuild once it's fixed.
    match Site::load(&root, &out).and_then(|site| build::build_all(&site, &out, &options)) {
        Ok(_) => {}
        Err(error) => eprintln!("initial build failed (serving previous output): {error:#}"),
    }

    let (announce, _) = broadcast::channel(16);
    let state = Arc::new(ReloadState {
        version: AtomicU64::new(1),
        announce,
    });

    {
        let state = state.clone();
        let root = root.clone();
        let out = out.clone();
        let options = options.clone();
        std::thread::spawn(move || watch_and_rebuild(&root, &out, &options, &state));
    }

    let runtime = tokio::runtime::Runtime::new().context("starting async runtime")?;
    runtime.block_on(serve(args.addr, out, root, state))
}

async fn serve(
    addr: SocketAddr,
    out: PathBuf,
    root: PathBuf,
    state: Arc<ReloadState>,
) -> Result<()> {
    // Misses serve the built 404 page with a real 404 status, exactly as
    // a static host configured for /404.html would.
    let missing_page = out.join("404.html");
    let not_found = axum::routing::any(move || {
        let page = missing_page.clone();
        async move {
            let body = tokio::fs::read_to_string(&page)
                .await
                .unwrap_or_else(|_| "404 — page not found".to_string());
            (StatusCode::NOT_FOUND, Html(body))
        }
    });
    let app = Router::new()
        .route("/~trail/reload", get(reload_events))
        .fallback_service(ServeDir::new(&out).not_found_service(not_found))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("binding {addr}"))?;
    println!(
        "serving {} at http://{} (watching {} for changes)",
        out.display(),
        listener.local_addr()?,
        root.display()
    );
    axum::serve(listener, app).await.context("serving")?;
    Ok(())
}

async fn reload_events(
    State(state): State<Arc<ReloadState>>,
) -> Sse<impl Stream<Item = Result<SseEvent, Infallible>>> {
    // The first event tells the client the current version; a later event
    // with a different version (including after a reconnect) means reload.
    let current = state.version.load(Ordering::SeqCst);
    let updates = BroadcastStream::new(state.announce.subscribe())
        .filter_map(|version| version.ok().map(as_event));
    let stream = tokio_stream::once(as_event(current)).chain(updates);
    Sse::new(stream).keep_alive(KeepAlive::default())
}

fn as_event(version: u64) -> Result<SseEvent, Infallible> {
    Ok(SseEvent::default().data(version.to_string()))
}

fn watch_and_rebuild(
    root: &Path,
    out: &Path,
    options: &build::BuildOptions,
    state: &ReloadState,
) {
    // Event paths arrive absolute; canonicalize for the ignore checks.
    let canonical_root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let canonical_out = std::fs::canonicalize(out).unwrap_or_else(|_| out.to_path_buf());

    let (event_tx, event_rx) = std::sync::mpsc::channel();
    let mut watcher =
        match notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
            if let Ok(event) = result {
                let _ = event_tx.send(event);
            }
        }) {
            Ok(watcher) => watcher,
            Err(error) => {
                eprintln!("cannot watch for changes: {error}");
                return;
            }
        };
    if let Err(error) = watcher.watch(&canonical_root, notify::RecursiveMode::Recursive) {
        eprintln!("cannot watch {}: {error}", canonical_root.display());
        return;
    }
    let mut watched_dirs = HashSet::new();
    let mut external = watch_dependencies(&mut watcher, &canonical_root, &mut watched_dirs);

    while let Ok(first) = event_rx.recv() {
        let is_change =
            |event: &notify::Event| is_content_change(event, &canonical_root, &canonical_out, &external);
        let mut relevant = is_change(&first);
        // Debounce: editors fire bursts; wait for 200ms of quiet.
        while let Ok(event) = event_rx.recv_timeout(Duration::from_millis(200)) {
            relevant |= is_change(&event);
        }
        if !relevant {
            continue;
        }
        match Site::load(root, out).and_then(|site| build::build_all(&site, out, options)) {
            Ok(pages) => {
                let version = state.version.fetch_add(1, Ordering::SeqCst) + 1;
                let _ = state.announce.send(version);
                println!("rebuilt {pages} pages");
            }
            Err(error) => {
                eprintln!("rebuild failed (still serving the last good build): {error:#}")
            }
        }
        // A rebuild can change what plugins read, failed or not.
        external = watch_dependencies(&mut watcher, &canonical_root, &mut watched_dirs);
    }
}

/// Watch the files plugins depend on that live outside the site root (the
/// root is already watched recursively), returning them for the change
/// filter. Their directories are watched rather than the files, since
/// editors save by renaming a new file over the old one.
fn watch_dependencies(
    watcher: &mut impl Watcher,
    root: &Path,
    watched_dirs: &mut HashSet<PathBuf>,
) -> HashSet<PathBuf> {
    let external: HashSet<PathBuf> = crate::plugins::dependencies(root)
        .into_iter()
        .filter(|path| !path.starts_with(root))
        .collect();
    for path in &external {
        let Some(dir) = path.parent() else { continue };
        if watched_dirs.insert(dir.to_path_buf())
            && let Err(error) = watcher.watch(dir, notify::RecursiveMode::NonRecursive)
        {
            eprintln!("cannot watch {}: {error}", dir.display());
        }
    }
    external
}

/// A change matters unless it is a read (rebuilds read every content file,
/// and inotify reports those as Access events — reacting to them loops
/// forever), inside the output directory (our own writes), or under a
/// dot-entry (.git and friends). Outside the root, only the files plugins
/// depend on matter — their directories may be busy with other things.
fn is_content_change(
    event: &notify::Event,
    root: &Path,
    out: &Path,
    external: &HashSet<PathBuf>,
) -> bool {
    if matches!(event.kind, notify::EventKind::Access(_)) {
        return false;
    }
    event.paths.iter().any(|path| {
        if !path.starts_with(root) {
            return external.contains(path);
        }
        !path.starts_with(out)
            && !path
                .components()
                .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
    })
}
