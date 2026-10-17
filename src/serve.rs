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

/// Retain the authored site and its topology between rebuilds. Content
/// fingerprints, rather than watcher paths or mtimes, decide what changed.
#[derive(Default)]
struct RebuildState {
    raw: Option<Site>,
    #[cfg(test)]
    full_loads: usize,
    inputs: std::collections::BTreeMap<PathBuf, String>,
    dependencies: std::collections::BTreeMap<PathBuf, Option<String>>,
}

impl RebuildState {
    fn unchanged(&self, root: &Path, out: &Path) -> bool {
        self.raw.is_some()
            && source_inputs(root, out).is_ok_and(|inputs| inputs == self.inputs)
            && self
                .dependencies
                .iter()
                .all(|(path, hash)| crate::plugins::fingerprint(path) == *hash)
    }

    fn rebuild(
        &mut self,
        root: &Path,
        out: &Path,
        options: &build::BuildOptions,
        no_cache: bool,
        full: bool,
    ) -> Result<usize> {
        let cache = crate::cache::Session::new(root, !no_cache);
        let inputs = source_inputs(root, out)?;
        let changed: HashSet<_> = inputs
            .iter()
            .filter(|(path, hash)| self.inputs.get(*path) != Some(*hash))
            .map(|(path, _)| path.clone())
            .collect();
        let removed = self.inputs.keys().any(|path| !inputs.contains_key(path));
        let reuse = !no_cache
            && !full
            && !removed
            && self
                .raw
                .as_mut()
                .is_some_and(|site| site.refresh_articles(&changed).unwrap_or(false));
        if !reuse {
            self.raw = Some(Site::load_raw(root, out)?);
            #[cfg(test)]
            {
                self.full_loads += 1;
            }
        }
        self.inputs = inputs;
        let result = (|| {
            let site = self.raw.as_ref().unwrap().clone().finish()?;
            let pages = build::build_all(&site, out, options)?;
            cache.finish();
            self.dependencies = crate::plugins::dependencies(root)
                .into_iter()
                .map(|path| {
                    let hash = crate::plugins::fingerprint(&path);
                    (path, hash)
                })
                .collect();
            Ok(pages)
        })();
        if result.is_err() {
            self.raw = None;
        }
        result
    }
}

/// Reconcile all source files on every rebuild, catching coalesced/missed
/// events and same-size edits with restored mtimes. External plugin inputs
/// remain validated by plugin expansion on every build.
fn source_inputs(root: &Path, out: &Path) -> Result<std::collections::BTreeMap<PathBuf, String>> {
    fn walk(
        dir: &Path,
        out: &Path,
        seen: &mut HashSet<PathBuf>,
        inputs: &mut std::collections::BTreeMap<PathBuf, String>,
    ) -> Result<()> {
        let canonical = std::fs::canonicalize(dir)?;
        if canonical == out || !seen.insert(canonical) {
            return Ok(());
        }
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let name = entry.file_name();
            if name.to_string_lossy().starts_with('.') {
                continue;
            }
            let path = entry.path();
            if std::fs::canonicalize(&path).ok().as_deref() == Some(out) {
                continue;
            }
            if path.is_dir() {
                inputs.insert(path.clone(), "directory".into());
                walk(&path, out, seen, inputs)?;
            } else {
                inputs.insert(path.clone(), crate::memo::key(&[&std::fs::read(&path)?]));
            }
        }
        Ok(())
    }
    let out = std::fs::canonicalize(out).unwrap_or_else(|_| out.to_path_buf());
    let mut inputs = std::collections::BTreeMap::new();
    let mut seen = HashSet::new();
    for tree in std::iter::once(root.to_path_buf()).chain(crate::site::merged_roots(root)) {
        walk(&tree, &out, &mut seen, &mut inputs)?;
        inputs.extend(crate::attachments::source_fingerprints(&tree, &out)?);
    }
    // Explicit chrome paths can be hidden or outside the source tree. In
    // particular head_html is loaded into the retained model, not read by
    // the output writer, so its content must participate in invalidation.
    let config: toml::Value = toml::from_str(&std::fs::read_to_string(root.join("trail.toml"))?)?;
    for name in ["head_html", "custom_css", "favicon"] {
        if let Some(path) = config.get(name).and_then(toml::Value::as_str) {
            let path = root.join(path);
            inputs.insert(path.clone(), crate::memo::key(&[&std::fs::read(path)?]));
        }
    }
    Ok(inputs)
}

pub fn run(args: &ServeArgs) -> Result<()> {
    let no_cache = args.build.no_cache;
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
    let mut builds = RebuildState::default();
    match builds.rebuild(&root, &out, &options, no_cache, true) {
        Ok(_) => {}
        Err(error) => crate::diagnostic::warning(format!(
            "initial build failed (serving previous output): {error:#}"
        )),
    }

    let (announce, _) = broadcast::channel(16);
    let state = Arc::new(ReloadState {
        version: AtomicU64::new(1),
        announce,
    });

    // The CLI owns this watcher for the process lifetime. The server runs
    // until an error or process termination; main exits on either. notify and
    // its callback are owned by this thread, and the OS reclaims them at exit.
    {
        let state = state.clone();
        let root = root.clone();
        let out = out.clone();
        let options = options.clone();
        std::thread::spawn(move || {
            watch_and_rebuild(&root, &out, &options, &state, no_cache, builds)
        });
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
    no_cache: bool,
    mut builds: RebuildState,
) {
    // Event paths arrive absolute; canonicalize for the ignore checks.
    let canonical_root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let canonical_out = std::fs::canonicalize(out).unwrap_or_else(|_| out.to_path_buf());

    let (event_tx, event_rx) = std::sync::mpsc::channel();
    let mut watcher =
        match notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
            // Errors/overflow require reconciliation too; do not drop them.
            // A closed receiver means the owner is shutting down.
            let _ = event_tx.send(result);
        }) {
            Ok(watcher) => watcher,
            Err(error) => {
                crate::diagnostic::warning(format!("file watcher: {error}"));
                return;
            }
        };
    if let Err(error) = watcher.watch(&canonical_root, notify::RecursiveMode::Recursive) {
        crate::diagnostic::warning(format!(
            "{}: cannot watch: {error}",
            canonical_root.display()
        ));
        return;
    }
    let mut watched_dirs = HashSet::new();
    let mut external = watch_dependencies(&mut watcher, &canonical_root, &mut watched_dirs);

    // Reconcile after registering watches, closing the initial-build gap.
    let mut reconcile_at = std::time::Instant::now();
    loop {
        let mut reconcile = std::time::Instant::now() >= reconcile_at;
        let first = if reconcile {
            reconcile_at = std::time::Instant::now() + Duration::from_secs(30);
            Ok(Ok(notify::Event::new(notify::EventKind::Any)))
        } else {
            event_rx.recv_timeout(reconcile_at.saturating_duration_since(std::time::Instant::now()))
        };
        let first = match first {
            Ok(event) => event,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // Periodic reconciliation also validates external plugin inputs.
                reconcile = true;
                reconcile_at = std::time::Instant::now() + Duration::from_secs(30);
                Ok(notify::Event::new(notify::EventKind::Any))
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        };
        if reconcile && builds.unchanged(root, out) {
            continue;
        }
        let is_change = |event: &notify::Event| {
            is_content_change(event, &canonical_root, &canonical_out, &external)
        };
        let uncertain = |event: &notify::Result<notify::Event>| {
            event.as_ref().map_or(true, |e| {
                e.need_rescan()
                    || e.paths.is_empty()
                    || matches!(e.kind, notify::EventKind::Any | notify::EventKind::Other)
            })
        };
        let mut full = uncertain(&first);
        let mut relevant = full || first.as_ref().is_ok_and(&is_change);
        // Debounce editor bursts, but ongoing access events must not starve
        // rebuilds or periodic reconciliation.
        let debounce_end = std::time::Instant::now() + Duration::from_secs(1);
        while let Ok(event) = event_rx.recv_timeout(Duration::from_millis(200)) {
            full |= uncertain(&event);
            relevant |= full || event.as_ref().is_ok_and(&is_change);
            if std::time::Instant::now() >= debounce_end {
                break;
            }
        }
        if !relevant {
            continue;
        }
        match builds.rebuild(root, out, options, no_cache, full) {
            Ok(pages) => {
                let version = state.version.fetch_add(1, Ordering::SeqCst) + 1;
                // No connected browser is a normal broadcast send failure.
                let _ = state.announce.send(version);
                println!("rebuilt {pages} pages");
            }
            Err(error) => crate::diagnostic::warning(format!(
                "rebuild failed (still serving the last good build): {error:#}"
            )),
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
            crate::diagnostic::warning(format!("{}: cannot watch: {error}", tree.display()));
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
            crate::diagnostic::warning(format!("{}: cannot watch: {error}", dir.display()));
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
            if let Some(rest) = external
                .trees
                .iter()
                .find_map(|tree| path.strip_prefix(tree).ok())
            {
                return !rest.starts_with("dist") && !dotted(rest);
            }
            return external.files.contains(path)
                || external
                    .dirs
                    .iter()
                    .any(|dir| path.strip_prefix(dir).is_ok_and(|rest| !dotted(rest)));
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
    fn retained_downloads_track_hidden_files_and_executable_modes() {
        use std::fs;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        crate::attachments::tests::fixture(root);
        let out = root.join("dist");
        let mut state = RebuildState::default();
        let options = build::BuildOptions::default();
        state.rebuild(root, &out, &options, false, false).unwrap();
        assert!(state.unchanged(root, &out));
        let archive = out.join("attachments/tools/tools.tar.gz");
        let first = fs::read(&archive).unwrap();
        fs::write(root.join("bundles/tools/.settings"), "changed\n").unwrap();
        assert!(!state.unchanged(root, &out));
        state.rebuild(root, &out, &options, false, false).unwrap();
        assert_ne!(first, fs::read(&archive).unwrap());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let before = fs::read(&archive).unwrap();
            fs::set_permissions(
                root.join("bundles/tools/run.sh"),
                fs::Permissions::from_mode(0o755),
            )
            .unwrap();
            assert!(!state.unchanged(root, &out));
            state.rebuild(root, &out, &options, false, false).unwrap();
            assert_ne!(before, fs::read(&archive).unwrap());
        }
        fs::remove_file(root.join("bundles/tools/.settings")).unwrap();
        assert!(!state.unchanged(root, &out));
        state.rebuild(root, &out, &options, false, false).unwrap();
    }

    #[test]
    fn retained_authored_pages_reexpand_plugins_and_check_external_inputs() {
        use std::fs;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("site");
        crate::site::write_fixture(&root);
        let config = root.join("trail.toml");
        fs::write(
            &config,
            fs::read_to_string(&config).unwrap()
                + "\n[plugins.p]\ncommand = ['python3', '../plugin.py']\n",
        )
        .unwrap();
        fs::write(dir.path().join("plugin.py"), r#"import json, pathlib, sys
r = json.load(sys.stdin)
with open('../runs', 'a') as f: f.write('run\n')
body = pathlib.Path('../dep.txt').read_text()
json.dump({'blocks': [{'id': b['id'], 'markdown': body} for b in r['blocks']], 'depends_on': ['../dep.txt', '../plugin.py']}, sys.stdout)
"#).unwrap();
        let dep = dir.path().join("dep.txt");
        fs::write(&dep, "First dependency.").unwrap();
        let page = root.join("alpha.product/5--loose.topic/1--x1.md");
        let original = fs::read_to_string(&page).unwrap() + "\n```{p}\n```\n";
        fs::write(&page, &original).unwrap();
        let out = root.join("dist");
        let mut state = RebuildState::default();
        let options = build::BuildOptions::default();
        state.rebuild(&root, &out, &options, false, false).unwrap();
        assert!(state.unchanged(&root, &out));
        fs::write(&page, original + "\nMore authored text.\n").unwrap();
        state.rebuild(&root, &out, &options, false, false).unwrap();
        assert_eq!(state.full_loads, 1);
        assert_eq!(
            fs::read_to_string(dir.path().join("runs"))
                .unwrap()
                .lines()
                .count(),
            1
        );
        assert!(
            fs::read_to_string(out.join("alpha/loose/x1/index.html"))
                .unwrap()
                .contains("First dependency.")
        );
        fs::write(&dep, "Other dependency.").unwrap();
        assert!(!state.unchanged(&root, &out));
        state.rebuild(&root, &out, &options, false, false).unwrap();
        assert_eq!(state.full_loads, 1);
        assert_eq!(
            fs::read_to_string(dir.path().join("runs"))
                .unwrap()
                .lines()
                .count(),
            2
        );
        assert!(
            fs::read_to_string(out.join("alpha/loose/x1/index.html"))
                .unwrap()
                .contains("Other dependency.")
        );
    }

    #[test]
    fn retained_site_matches_full_builds_and_recovers_from_failures() {
        use std::fs;
        fn hashes(root: &Path) -> std::collections::BTreeMap<PathBuf, String> {
            fn walk(
                root: &Path,
                dir: &Path,
                map: &mut std::collections::BTreeMap<PathBuf, String>,
            ) {
                for entry in fs::read_dir(dir).unwrap() {
                    let p = entry.unwrap().path();
                    if p.is_dir() {
                        walk(root, &p, map);
                    } else {
                        map.insert(
                            p.strip_prefix(root).unwrap().to_owned(),
                            crate::memo::key(&[&fs::read(p).unwrap()]),
                        );
                    }
                }
            }
            let mut map = Default::default();
            walk(root, root, &mut map);
            map
        }
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("site");
        let out = root.join("dist");
        let clean = dir.path().join("clean");
        crate::site::write_fixture(&root);
        let options = build::BuildOptions {
            live_reload: true,
            cachebust: Some("revision".into()),
            ..Default::default()
        };
        let mut state = RebuildState::default();
        let check = |state: &mut RebuildState, full: bool| {
            state.rebuild(&root, &out, &options, false, full).unwrap();
            let _session = crate::cache::Session::new(&root, false);
            let site = Site::load(&root, &out).unwrap();
            build::build_all(&site, &clean, &options).unwrap();
            assert_eq!(hashes(&out), hashes(&clean));
            assert!(state.unchanged(&root, &out));
        };
        check(&mut state, false);
        assert_eq!(state.full_loads, 1);
        let page = root.join("alpha.product/5--loose.topic/1--x1.md");
        let original = fs::read_to_string(&page).unwrap();
        let mtime = fs::metadata(&page).unwrap().modified().unwrap();
        fs::write(
            &page,
            original.clone() + "\n## Changed Heading\n\nChanged prose.\n",
        )
        .unwrap();
        fs::File::options()
            .write(true)
            .open(&page)
            .unwrap()
            .set_modified(mtime)
            .unwrap();
        assert!(!state.unchanged(&root, &out));
        check(&mut state, false);
        assert_eq!(state.full_loads, 1, "ordinary edits retain topology");
        let book = root.join("alpha.product/7--manual.book/2--setup/1--install.md");
        let text = fs::read_to_string(&book).unwrap();
        fs::write(&book, text + "\n## Another Heading\n\nMore book text.\n").unwrap();
        check(&mut state, false);
        assert_eq!(state.full_loads, 1);
        fs::write(&page, "---\ntitle: [broken\n---\n").unwrap();
        assert!(state.rebuild(&root, &out, &options, false, false).is_err());
        fs::write(&page, &original).unwrap();
        check(&mut state, false);
        let added = page.with_file_name("99--new.md");
        fs::write(&added, &original).unwrap();
        check(&mut state, false);
        let count = state.full_loads;
        fs::rename(&added, page.with_file_name("99--renamed.md")).unwrap();
        check(&mut state, false);
        assert_eq!(state.full_loads, count + 1);
        fs::remove_file(page.with_file_name("99--renamed.md")).unwrap();
        check(&mut state, false);
        let config = root.join("trail.toml");
        fs::write(
            &config,
            fs::read_to_string(&config).unwrap() + "\nhead_html = '.head.html'\n",
        )
        .unwrap();
        let head = root.join(".head.html");
        fs::write(&head, "<meta name=\"test\" content=\"one\">").unwrap();
        check(&mut state, false);
        fs::write(&head, "<meta name=\"test\" content=\"two\">").unwrap();
        assert!(!state.unchanged(&root, &out));
        check(&mut state, false);
        let count = state.full_loads;
        check(&mut state, true);
        assert_eq!(state.full_loads, count + 1, "rescan forces a full load");
        state.rebuild(&root, &out, &options, true, false).unwrap();
        assert_eq!(
            state.full_loads,
            count + 2,
            "no-cache bypasses retained topology"
        );
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
        assert!(change(
            "/kernel/docs/kernel.product/1--guide.topic/1--page.md"
        ));
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
        assert!(!change("/site/.trail-cache/v1/cache-file"));
    }
}
