//! Exercise persistent plugin reuse across actual CLI process boundaries.
#![cfg(unix)]
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn build(root: &Path, extra: &[&str], environment: &str) -> Output {
    Command::new(fs::canonicalize(env!("CARGO_BIN_EXE_trail")).unwrap())
        .arg("build")
        .arg(root)
        .args(["--strict", "--out"])
        .arg(root.parent().unwrap().join("out"))
        .args(extra)
        .env("TRAIL_CACHE_TEST", environment)
        .output()
        .unwrap()
}

#[test]
fn plugin_cache_tracks_inputs_and_replays_diagnostics_across_processes() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("site");
    let topic = root.join("demo.product/1--guide.topic");
    fs::create_dir_all(&topic).unwrap();
    fs::create_dir(dir.path().join("inputs")).unwrap();
    fs::write(root.join("trail.toml"), "sitename = 'Test'\ntitle = 'Test'\ndescription = 'Test'\nfooter = 'Test'\n[plugins.p]\ncommand = ['python3', '../plugin.py']\n").unwrap();
    fs::write(
        root.join("demo.product/trail.toml"),
        "title = 'Demo'\nmonogram = 'D'\ncolor = '#123456'\ndescription = 'Demo'\n",
    )
    .unwrap();
    let page = topic.join("1--page.md");
    let source = "---\ntitle: Page\ntype: concept\ndescription: Page\n---\n\n```{p}\n```\n";
    fs::write(&page, source).unwrap();
    let plugin = dir.path().join("plugin.py");
    fs::write(&plugin, r#"import json, pathlib, sys
r = json.load(sys.stdin)
with open('../runs', 'a') as f: f.write('run\n')
print('plugin warning at line ' + str(r['blocks'][0]['line']), file=sys.stderr)
body = pathlib.Path('../dep.txt').read_text()
json.dump({'blocks': [{'id': 0, 'markdown': body}], 'depends_on': ['../dep.txt', '../plugin.py', '../inputs']}, sys.stdout)
"#).unwrap();
    let dep = dir.path().join("dep.txt");
    fs::write(&dep, "first").unwrap();
    let runs = || {
        fs::read_to_string(dir.path().join("runs"))
            .unwrap()
            .lines()
            .count()
    };
    let good = |args: &[&str], env| {
        let result = build(&root, args, env);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        result
    };
    let first = good(&[], "one");
    let second = good(&[], "one");
    assert_eq!(runs(), 1);
    assert_eq!(first.stderr, second.stderr);
    fs::write(&page, source.replace("```{p}", "Prose.\n\n```{p}")).unwrap();
    let moved = good(&[], "one");
    assert_eq!(runs(), 2);
    assert_ne!(moved.stderr, first.stderr);
    let modified = fs::metadata(&dep).unwrap().modified().unwrap();
    fs::write(&dep, "other").unwrap();
    fs::File::options()
        .write(true)
        .open(&dep)
        .unwrap()
        .set_modified(modified)
        .unwrap();
    good(&[], "one");
    assert_eq!(runs(), 3);
    fs::write(dir.path().join("inputs/new"), "new").unwrap();
    good(&[], "one");
    assert_eq!(runs(), 4);
    good(&[], "two");
    assert_eq!(runs(), 5);
    good(&["--no-cache"], "two");
    assert_eq!(runs(), 6);
    good(&[], "two");
    assert_eq!(runs(), 6);
    // A changed helper must invalidate despite an unchanged command.
    fs::write(
        &plugin,
        fs::read_to_string(&plugin).unwrap() + "\n# changed helper\n",
    )
    .unwrap();
    good(&[], "two");
    assert_eq!(runs(), 7);
    // The dependency's spelling is retained as well as its canonical path.
    let target = dir.path().join("target.txt");
    fs::write(&target, "symlink value").unwrap();
    fs::remove_file(&dep).unwrap();
    std::os::unix::fs::symlink(&target, &dep).unwrap();
    good(&[], "two");
    assert_eq!(runs(), 8);
    let next = dir.path().join("next.txt");
    fs::write(&next, "retargeted value").unwrap();
    fs::remove_file(&dep).unwrap();
    std::os::unix::fs::symlink(&next, &dep).unwrap();
    good(&[], "two");
    assert_eq!(runs(), 9);
    let namespace = fs::read_dir(root.join(".trail-cache"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    for entry in fs::read_dir(namespace.join("plugins")).unwrap() {
        fs::write(entry.unwrap().path(), "corrupt").unwrap();
    }
    good(&[], "two");
    assert_eq!(runs(), 10);
    let cached_html = fs::read(dir.path().join("out/demo/guide/page/index.html")).unwrap();
    good(&["--no-cache"], "two");
    assert_eq!(
        cached_html,
        fs::read(dir.path().join("out/demo/guide/page/index.html")).unwrap()
    );
}
