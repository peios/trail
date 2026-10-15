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
    let watched: Vec<String> = std::iter::once(root.clone())
        .chain(crate::site::merged_roots(&root))
        .map(|tree| tree.display().to_string())
        .collect();
    println!(
        "serving {} at http://{} (watching {} for changes)",
        out.display(),
        listener.local_addr()?,
        watched.join(", ")
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

/// What the site is built from outside its root (the root is already
/// watched recursively).
#[derive(Default)]
struct External {
    /// Merged trees: everything beneath one is content, except its own
    /// build output.
    trees: Vec<PathBuf>,
    files: HashSet<PathBuf>,
    /// Directories a plugin scans: anything beneath one may be an input.
    dirs: Vec<PathBuf>,
}

/// Watch merged trees and what plugins depend on outside the site root,
/// returning them for the change filter. A file's directory is watched
/// rather than the file, since editors save by renaming a new file over
/// the old one; a directory dependency is watched with everything beneath
/// it.
fn watch_dependencies(
    watcher: &mut impl Watcher,
    root: &Path,
    watched: &mut HashSet<(PathBuf, bool)>,
) -> External {
    let mut external = External::default();
    for tree in crate::site::merged_roots(root) {
        let tree = std::fs::canonicalize(&tree).unwrap_or(tree);
        if tree.starts_with(root) {
            continue;
        }
        if watched.insert((tree.clone(), true))
            && let Err(error) = watcher.watch(&tree, notify::RecursiveMode::Recursive)
        {
            eprintln!("cannot watch {}: {error}", tree.display());
        }
        external.trees.push(tree);
    }
    for path in crate::plugins::dependencies(root) {
        if path.starts_with(root) || external.trees.iter().any(|tree| path.starts_with(tree)) {
            continue;
        }
        let (dir, recursive) = if path.is_dir() {
            external.dirs.push(path.clone());
            (path, true)
        } else {
            let Some(dir) = path.parent().map(Path::to_path_buf) else {
                continue;
            };
            external.files.insert(path);
            (dir, false)
        };
        let mode = if recursive {
            notify::RecursiveMode::Recursive
        } else {
            notify::RecursiveMode::NonRecursive
        };
        if watched.insert((dir.clone(), recursive))
            && let Err(error) = watcher.watch(&dir, mode)
        {
            eprintln!("cannot watch {}: {error}", dir.display());
        }
    }
    external
}

/// A change matters unless it is a read (rebuilds read every content file,
/// and inotify reports those as Access events — reacting to them loops
/// forever), inside the output directory (our own writes), or under a
/// dot-entry (.git and friends). Outside the root, only merged trees (less
/// their own `dist/`) and what plugins depend on matter — the directories
/// around it may be busy with other things.
fn is_content_change(event: &notify::Event, root: &Path, out: &Path, external: &External) -> bool {
    if matches!(event.kind, notify::EventKind::Access(_)) {
        return false;
    }
    let dotted = |path: &Path| {
        path.components()
            .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
    };
    event.paths.iter().any(|path| {
        if !path.starts_with(root) {
            if let Some(rest) = external.trees.iter().find_map(|tree| path.strip_prefix(tree).ok()) {
                return !rest.starts_with("dist") && !dotted(rest);
            }
            return external.files.contains(path)
                || external.dirs.iter().any(|dir| {
                    path.strip_prefix(dir).is_ok_and(|rest| !dotted(rest))
                });
        }
        !path.starts_with(out) && !dotted(path)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{CreateKind, EventKind};

    fn created(path: &str) -> notify::Event {
        notify::Event::new(EventKind::Create(CreateKind::File)).add_path(PathBuf::from(path))
    }

    #[test]
    fn outside_the_root_only_dependencies_are_changes() {
        let external = External {
            trees: vec![PathBuf::from("/kernel/docs")],
            files: HashSet::from([PathBuf::from("/src/MAINTAINERS")]),
            dirs: vec![PathBuf::from("/src/ABI")],
        };
        let (root, out) = (Path::new("/site"), Path::new("/site/dist"));
        let change = |path| is_content_change(&created(path), root, out, &external);
        assert!(change("/kernel/docs/kernel.product/1--guide.topic/1--page.md"));
        assert!(change("/kernel/docs/trail.toml"));
        assert!(!change("/kernel/docs/dist/kernel/index.html"));
        assert!(!change("/kernel/docs/.git/index"));
        assert!(!change("/kernel/README"));
        assert!(change("/src/MAINTAINERS"));
        assert!(!change("/src/README"));
        assert!(change("/src/ABI/testing/sysfs-new"));
        assert!(!change("/src/ABI/testing/.sysfs-new.swp"));
        assert!(!change("/src/ABIX/file"));
        assert!(change("/site/page.md"));
        assert!(!change("/site/dist/page/index.html"));
    }
}
