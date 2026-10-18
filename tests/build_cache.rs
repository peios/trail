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

#[test]
fn recursive_merges_keep_provenance_and_invalidate_cached_navigation() {
    let dir = tempfile::tempdir().unwrap();
    let primary = dir.path().join("site");
    let extra = dir.path().join("component");
    let topic = "peios.antho/100--accounts.shelf/100--accounts.topic";
    let write = |root: &Path, relative: &str, text: &str| {
        let file = root.join(relative);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, text).unwrap();
    };
    for root in [&primary, &extra] {
        write(
            root,
            "peios.antho/trail.toml",
            "title = 'Peios'\ndescription = 'Peios documentation.'\n",
        );
    }
    write(
        &primary,
        "trail.toml",
        "sitename = 'Learn'\ntitle = 'Learn'\ndescription = 'Learn'\nfooter = 'Learn'\nedit_url = 'https://example.com/primary/{path}'\n[[merge]]\npath = '../component'\n",
    );
    write(
        &extra,
        "trail.toml",
        "sitename = 'Component'\ntitle = 'Component'\ndescription = 'Component'\nfooter = 'Component'\nedit_url = 'https://example.com/component/{path}'\n[plugins.local]\ncommand = ['python3', '.tools/plugin.py']\n",
    );
    write(
        &extra,
        ".tools/plugin.py",
        r#"import json, pathlib, sys
request = json.load(sys.stdin)
assert pathlib.Path(request['root']).resolve() == pathlib.Path.cwd()
assert request['blocks'][0]['file'].startswith('peios.antho/')
with open('.runs', 'a') as runs: runs.write('run\n')
body = pathlib.Path('.input').read_text()
json.dump({'blocks': [{'id': 0, 'markdown': body}], 'depends_on': ['.input']}, sys.stdout)
"#,
    );
    write(&extra, ".input", "Component generated content.");
    write(
        &primary,
        &format!("{topic}/100--overview.md"),
        "---\ntitle: Overview\ntype: concept\ndescription: Account overview.\nrelated: [peios/accounts/manage]\n---\n\nRead [management](~peios/accounts/manage).\n",
    );
    write(
        &primary,
        &format!("{topic}/300--alias.link"),
        "target = '~peios/accounts/manage'\n",
    );
    let manage = format!("{topic}/200--manage.md");
    let source = "---\ntitle: Managing Accounts\ntype: how-to\ndescription: Manage an account.\n---\n\nSee the [overview](~peios/accounts/overview).\n\n![Account diagram](diagram.svg)\n\n```{local}\n```\n";
    write(&extra, &manage, source);
    write(
        &extra,
        &format!("{topic}/diagram.svg"),
        "<svg xmlns='http://www.w3.org/2000/svg'><title>Accounts</title></svg>",
    );
    let good = |args: &[&str]| {
        let output = build(&primary, args, "recursive");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    let out = dir.path().join("out");
    good(&[]);
    good(&[]);
    assert_eq!(
        fs::read_to_string(extra.join(".runs"))
            .unwrap()
            .lines()
            .count(),
        1
    );
    let page = fs::read_to_string(out.join("peios/accounts/manage/index.html")).unwrap();
    assert!(page.contains("Component generated content."));
    assert!(page.contains("https://example.com/component/peios.antho/100--accounts.shelf/100--accounts.topic/200--manage.md"));
    let alias = fs::read_to_string(out.join("peios/accounts/alias/index.html")).unwrap();
    assert!(alias.contains("https://example.com/component/"));
    assert!(out.join("peios/accounts/diagram.svg").is_file());
    assert!(!out.join("peios/peios").exists());
    let index = fs::read_to_string(out.join("site.json")).unwrap();
    assert!(index.contains("\"kind\": \"anthology\""));
    write(&extra, ".input", "Updated generated content.");
    good(&[]);
    assert!(
        fs::read_to_string(out.join("peios/accounts/manage/index.html"))
            .unwrap()
            .contains("Updated generated content.")
    );
    let additional = format!("{topic}/250--extra.md");
    write(
        &extra,
        &additional,
        "---\ntitle: Extra\ntype: concept\ndescription: Extra content.\n---\n\nAdditional account guidance.\n",
    );
    good(&[]);
    let cached = fs::read(out.join("peios/accounts/manage/index.html")).unwrap();
    assert!(String::from_utf8_lossy(&cached).contains("/peios/accounts/extra"));
    good(&["--no-cache"]);
    assert_eq!(
        cached,
        fs::read(out.join("peios/accounts/manage/index.html")).unwrap()
    );
    fs::remove_file(extra.join(&additional)).unwrap();
    good(&[]);
    assert!(!out.join("peios/accounts/extra").exists());
    assert!(
        !fs::read_to_string(out.join("peios/accounts/manage/index.html"))
            .unwrap()
            .contains("/peios/accounts/extra")
    );

    // Moving the owning source tree changes edit URLs and image resolution,
    // even when the page body and its published URL remain unchanged.
    fs::rename(extra.join(&manage), primary.join(&manage)).unwrap();
    fs::rename(
        extra.join(format!("{topic}/diagram.svg")),
        primary.join(format!("{topic}/diagram.svg")),
    )
    .unwrap();
    good(&[]);
    for name in ["manage", "alias"] {
        let page =
            fs::read_to_string(out.join(format!("peios/accounts/{name}/index.html"))).unwrap();
        assert!(page.contains("https://example.com/primary/peios.antho/"));
        assert!(!page.contains("https://example.com/component/peios.antho/"));
    }
    write(
        &primary,
        &manage,
        &source.replace("~peios/accounts/overview", "~peios/accounts/missing"),
    );
    let failure = build(&primary, &[], "recursive");
    assert!(!failure.status.success());
    assert!(String::from_utf8_lossy(&failure.stderr).contains("missing"));
}
