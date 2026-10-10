use super::*;
use std::fs;
use std::path::PathBuf;

fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("trail.toml"),
        "sitename = 'Print anchors'\ntitle = 'Test docs'\ndescription = 'Fixture'\nfooter = 'Test'\n",
    )
    .unwrap();
    dir
}

fn write_article(root: &Path, path: &str, body: &str) {
    let file = root.join(path);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    let mut parent = PathBuf::from(root);
    for component in Path::new(path).parent().unwrap() {
        parent.push(component);
        let name = component.to_str().unwrap();
        let config = if name.ends_with(".product") {
            Some("title = 'Example'\ndescription = 'Fixture'\nmonogram = 'Ex'\ncolor = '#123456'\n")
        } else if name.ends_with(".antho") || name.ends_with(".book") {
            Some("title = 'Container'\ndescription = 'Fixture'\n")
        } else {
            None
        };
        if let Some(config) = config {
            fs::write(parent.join("trail.toml"), config).unwrap();
        }
    }
    let kind = if path.contains(".book/") {
        ""
    } else {
        "type: concept\n"
    };
    fs::write(
        file,
        format!("---\ntitle: Article\n{kind}description: Fixture\n---\n\n## Same\n\n{body}\n"),
    )
    .unwrap();
}

fn build(root: &Path, options: &BuildOptions) -> Result<usize> {
    let out = root.join("dist");
    let site = Site::load(root, &out)?;
    crate::build::build_all(&site, &out, options)
}

#[test]
fn rejects_flattened_paths_in_each_actual_print_scope() {
    // In the ancestor cases, each smaller child bundle is unambiguous.
    // Only the bundle which actually brings both paths together must fail.
    for (files, scope, paths, anchor) in [
        (
            [
                "example.product/10--manual.topic/10--a-b/10--c.md",
                "example.product/10--manual.topic/20--a/10--b-c.md",
            ],
            "/example/manual",
            ["/example/manual/a-b/c", "/example/manual/a/b-c"],
            "a-b-c",
        ),
        (
            [
                "example.product/10--manual.book/10--a-b/10--c.md",
                "example.product/10--manual.book/20--a/10--b-c.md",
            ],
            "/example/manual",
            ["/example/manual/a-b/c", "/example/manual/a/b-c"],
            "a-b-c",
        ),
        (
            [
                "example.product/10--outer.antho/10--inner.antho/10--a-b.topic/10--c.md",
                "example.product/10--outer.antho/10--inner.antho/20--a.topic/10--b-c.md",
            ],
            "/example/outer/inner",
            ["/example/outer/inner/a-b/c", "/example/outer/inner/a/b-c"],
            "a-b-c",
        ),
        (
            [
                "example.product/10--outer.antho/10--inner-a.antho/10--b.topic/10--c.md",
                "example.product/10--outer.antho/20--inner.antho/10--a-b.topic/10--c.md",
            ],
            "/example/outer",
            ["/example/outer/inner-a/b/c", "/example/outer/inner/a-b/c"],
            "inner-a-b-c",
        ),
        (
            [
                "example.product/10--a-b.topic/10--c.md",
                "example.product/20--a.topic/10--b-c.md",
            ],
            "/example",
            ["/example/a-b/c", "/example/a/b-c"],
            "a-b-c",
        ),
    ] {
        let dir = fixture();
        for file in files {
            write_article(dir.path(), file, "Target.");
        }
        let error = build(
            dir.path(),
            &BuildOptions {
                strict: true,
                ..Default::default()
            },
        )
        .unwrap_err()
        .to_string();
        assert_eq!(
            error,
            format!(
                "print anchor collision in '{scope}/print/': articles '{}' and '{}' both map to '#{anchor}'; rename an article or parent path to give each article a unique print anchor",
                paths[0], paths[1]
            ),
        );
        for file in ["print/index.html", "print.md", "llms-full.txt"] {
            assert!(
                !dir.path()
                    .join("dist")
                    .join(scope.trim_start_matches('/'))
                    .join(file)
                    .exists()
            );
        }
    }
}

#[test]
fn collisions_are_errors_without_strict_and_with_dangling_links_allowed() {
    for allow_dangling_links in [false, true] {
        let dir = fixture();
        write_article(
            dir.path(),
            "example.product/10--manual.topic/10--a-b/10--c.md",
            "First target.",
        );
        write_article(
            dir.path(),
            "example.product/10--manual.topic/20--a/10--b-c.md",
            "Second target.",
        );
        write_article(
            dir.path(),
            "example.product/10--manual.topic/30--caller.md",
            "[First](~example/manual/a-b/c#same) [Second](~example/manual/a/b-c#same)",
        );
        let error = build(
            dir.path(),
            &BuildOptions {
                allow_dangling_links,
                render_llms_full: true,
                base: "/preview".into(),
                ..Default::default()
            },
        )
        .unwrap_err()
        .to_string();
        assert!(
            error.starts_with("print anchor collision in '/example/manual/print/':"),
            "{error}"
        );
        assert!(
            !dir.path()
                .join("dist/example/manual/print/index.html")
                .exists()
        );
        assert!(!dir.path().join("dist/example/manual/print.md").exists());
        assert!(
            !dir.path()
                .join("dist/example/manual/llms-full.txt")
                .exists()
        );
    }
}

#[test]
fn repeated_prefixes_in_separate_bundles_keep_existing_fragments() {
    let dir = fixture();
    // Both topics use a-b-c, but each product prefix still distinguishes them.
    // The other product deliberately repeats those complete relative paths.
    for product in ["example", "other"] {
        for (topic, folder, article) in [
            ("10--first", "10--a-b", "10--c"),
            ("20--second", "10--a", "10--b-c"),
        ] {
            write_article(
                dir.path(),
                &format!("{product}.product/{topic}.topic/{folder}/{article}.md"),
                "[Local](#same)",
            );
        }
    }
    build(
        dir.path(),
        &BuildOptions {
            strict: true,
            ..Default::default()
        },
    )
    .unwrap();
    for product in ["example", "other"] {
        for topic in ["first", "second"] {
            let html = fs::read_to_string(
                dir.path()
                    .join(format!("dist/{product}/{topic}/print/index.html")),
            )
            .unwrap();
            for (expected, count) in [
                ("id=\"a-b-c\"", 1),
                ("id=\"a-b-c--same\"", 1),
                ("href=\"#a-b-c--same\"", 2),
            ] {
                assert_eq!(
                    html.matches(expected).count(),
                    count,
                    "{product}/{topic}: {expected}"
                );
            }
        }
        let html = fs::read_to_string(dir.path().join(format!("dist/{product}/print/index.html")))
            .unwrap();
        for prefix in ["first-a-b-c", "second-a-b-c"] {
            for (expected, count) in [
                (format!("id=\"{prefix}\""), 1),
                (format!("id=\"{prefix}--same\""), 1),
                (format!("href=\"#{prefix}--same\""), 2),
            ] {
                assert_eq!(
                    html.matches(&expected).count(),
                    count,
                    "{product}: {expected}"
                );
            }
        }
        let html = fs::read_to_string(
            dir.path()
                .join(format!("dist/{product}/first/a-b/c/index.html")),
        )
        .unwrap();
        assert!(html.contains("id=\"same\""));
        assert!(html.contains("href=\"#same\""));
        assert!(!html.contains("a-b-c--same"));
    }
}
