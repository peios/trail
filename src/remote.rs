//! Merged trees fetched from Git repositories.
//!
//! A `[[merge]]` table naming a `repo` is satisfied by a sparse, shallow
//! checkout of that repository's `dir`, kept under the site's
//! `.trail-cache/`. Trail runs the system `git`, so whatever credentials
//! it already has reach private repositories. A process resolves each
//! checkout once: `serve` rebuilds make no network calls.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};

use anyhow::{Context, Result, bail, ensure};

use super::MergeConfig;

/// Where a merged tree's files come from.
pub struct Source {
    /// The tree's root directory.
    pub root: PathBuf,
    /// The edit link template derived from the repository, used when the
    /// tree's own trail.toml sets no `edit_url`.
    pub edit_url: Option<String>,
}

/// A fetched checkout, as this process left it.
#[derive(Clone)]
struct Checkout {
    branch: String,
}

/// Checkouts this process has already brought up to date, by directory.
fn resolved() -> &'static Mutex<HashMap<PathBuf, Checkout>> {
    static RESOLVED: OnceLock<Mutex<HashMap<PathBuf, Checkout>>> = OnceLock::new();
    RESOLVED.get_or_init(Default::default)
}

impl MergeConfig {
    /// The table's name in messages: its local path, or its repository.
    pub fn label(&self) -> &str {
        self.path
            .as_deref()
            .or(self.repo.as_deref())
            .unwrap_or_default()
    }
}

/// Find a merge table's tree: its local `path` when that exists, otherwise
/// a checkout of its `repo`.
pub fn resolve(site_root: &Path, merge: &MergeConfig) -> Result<Source> {
    validate(merge)?;
    let dir = merge.dir.as_deref().unwrap_or_default();
    if let Some(path) = &merge.path {
        let root = site_root.join(path);
        if root.is_dir() {
            let branch = merge.branch.clone().or_else(|| local_branch(&root));
            let edit_url = merge
                .repo
                .as_deref()
                .zip(branch.as_deref())
                .and_then(|(repo, branch)| edit_url(repo, branch, dir));
            return Ok(Source { root, edit_url });
        }
        ensure!(
            merge.repo.is_some(),
            "merge path '{path}' is not a directory (paths are relative to the site root)"
        );
    }
    let repo = merge.repo.as_deref().unwrap_or_default();
    let checkout = checkout_dir(site_root, repo, merge.branch.as_deref(), dir);
    let wanted = Wanted {
        repo,
        branch: merge.branch.as_deref(),
        dir,
        sparse: merge.sparse.unwrap_or(true),
    };
    let state = fetch(&checkout, &wanted).with_context(|| format!("fetching {repo}"))?;
    let root = checkout.join(dir);
    ensure!(
        root.is_dir(),
        "{repo} has no directory '{dir}' on branch '{}'",
        state.branch
    );
    Ok(Source {
        root,
        edit_url: edit_url(repo, &state.branch, dir),
    })
}

fn validate(merge: &MergeConfig) -> Result<()> {
    ensure!(
        merge.path.is_some() || merge.repo.is_some(),
        "a merge needs a path, a repo, or both"
    );
    ensure!(
        merge.path.as_deref() != Some(""),
        "a merge path cannot be empty"
    );
    ensure!(
        merge.repo.as_deref() != Some(""),
        "a merge repo cannot be empty"
    );
    if merge.repo.is_none() {
        ensure!(merge.dir.is_none(), "a merge's dir needs a repo");
        ensure!(merge.branch.is_none(), "a merge's branch needs a repo");
        ensure!(merge.sparse.is_none(), "a merge's sparse needs a repo");
    }
    if let Some(dir) = &merge.dir {
        ensure!(
            !dir.is_empty()
                && Path::new(dir)
                    .components()
                    .all(|c| matches!(c, Component::Normal(_))),
            "merge dir '{dir}' must be a plain relative path inside the repository"
        );
    }
    ensure!(
        merge.branch.as_deref() != Some(""),
        "a merge branch cannot be empty"
    );
    Ok(())
}

/// One checkout per repository, branch and directory, so two tables can
/// take different directories from the same repository.
fn checkout_dir(site_root: &Path, repo: &str, branch: Option<&str>, dir: &str) -> PathBuf {
    let key = format!("{repo}\n{}\n{dir}", branch.unwrap_or_default());
    let hash = blake3::hash(key.as_bytes()).to_hex();
    site_root
        .join(".trail-cache")
        .join("repos")
        .join(&hash.as_str()[..16])
}

/// What a checkout should hold.
struct Wanted<'a> {
    repo: &'a str,
    branch: Option<&'a str>,
    dir: &'a str,
    /// Whether to check out only `dir` and the root files.
    sparse: bool,
}

/// Bring `checkout` to the tip of the wanted branch, or of the
/// repository's default branch, once per process. An existing checkout
/// fetches only what changed; one that cannot be updated is replaced.
fn fetch(checkout: &Path, wanted: &Wanted) -> Result<Checkout> {
    if let Some(state) = resolved().lock().unwrap().get(checkout) {
        return Ok(state.clone());
    }
    let (branch, commit) = remote_tip(wanted.repo, wanted.branch)?;
    let current = checkout
        .join(".git")
        .is_dir()
        .then(|| git(checkout, &["rev-parse", "HEAD"]).ok())
        .flatten();
    if current.as_deref() != Some(commit.as_str()) {
        if update(checkout, wanted, &branch).is_err() {
            if checkout.exists() {
                std::fs::remove_dir_all(checkout).with_context(|| {
                    format!("removing the stale checkout {}", checkout.display())
                })?;
            }
            update(checkout, wanted, &branch)?;
        }
        let fetched = git(checkout, &["rev-parse", "HEAD"])?;
        println!(
            "fetched {} {branch} at {}",
            wanted.repo,
            &fetched[..fetched.len().min(12)]
        );
    }
    let state = Checkout { branch };
    resolved()
        .lock()
        .unwrap()
        .insert(checkout.to_path_buf(), state.clone());
    Ok(state)
}

/// Fetch the tip of `branch` into `checkout`, creating it if needed, and
/// check it out. A shallow fetch into an existing checkout transfers only
/// what changed since its last commit.
fn update(checkout: &Path, wanted: &Wanted, branch: &str) -> Result<()> {
    if !checkout.join(".git").is_dir() {
        std::fs::create_dir_all(checkout)
            .with_context(|| format!("creating {}", checkout.display()))?;
        git(checkout, &["init", "--quiet"])?;
        git(checkout, &["remote", "add", "origin", wanted.repo])?;
    }
    if wanted.sparse && !wanted.dir.is_empty() {
        git(checkout, &["sparse-checkout", "set", wanted.dir])?;
    } else {
        git(checkout, &["sparse-checkout", "disable"])?;
    }
    let refspec = format!("refs/heads/{branch}");
    // A sparse checkout takes only its own blobs, on demand; a whole one
    // takes them all in the same pack.
    let mut fetch = vec!["fetch", "--quiet", "--depth", "1"];
    if wanted.sparse {
        fetch.push("--filter=blob:none");
    }
    git(checkout, &[&fetch[..], &["origin", &refspec]].concat())?;
    git(
        checkout,
        &["checkout", "--quiet", "--force", "--detach", "FETCH_HEAD"],
    )?;
    Ok(())
}

/// The branch to fetch and the commit at its tip.
fn remote_tip(repo: &str, branch: Option<&str>) -> Result<(String, String)> {
    let query = match branch {
        Some(branch) => format!("refs/heads/{branch}"),
        None => "HEAD".to_string(),
    };
    let listing = git(Path::new("."), &["ls-remote", "--symref", repo, &query])?;
    let mut default = None;
    let mut commit = None;
    for line in listing.lines() {
        if let Some(target) = line.strip_prefix("ref: ") {
            default = target
                .split('\t')
                .next()
                .and_then(|target| target.strip_prefix("refs/heads/"))
                .map(str::to_string);
        } else if let Some((sha, name)) = line.split_once('\t')
            && name == query
        {
            commit = Some(sha.to_string());
        }
    }
    let Some(commit) = commit else {
        match branch {
            Some(branch) => bail!("{repo} has no branch '{branch}'"),
            None => bail!("{repo} has no default branch"),
        }
    };
    let branch = match branch {
        Some(branch) => branch.to_string(),
        None => default.context("the repository did not report its default branch")?,
    };
    Ok((branch, commit))
}

/// A local checkout's default branch, as its `origin` reports it, looked
/// up once per process.
fn local_branch(tree: &Path) -> Option<String> {
    static BRANCHES: OnceLock<Mutex<HashMap<PathBuf, Option<String>>>> = OnceLock::new();
    let branches = BRANCHES.get_or_init(Default::default);
    if let Some(branch) = branches.lock().unwrap().get(tree) {
        return branch.clone();
    }
    let branch = origin_head(tree);
    branches
        .lock()
        .unwrap()
        .insert(tree.to_path_buf(), branch.clone());
    branch
}

fn origin_head(tree: &Path) -> Option<String> {
    let name = git(
        tree,
        &[
            "symbolic-ref",
            "--quiet",
            "--short",
            "refs/remotes/origin/HEAD",
        ],
    )
    .ok()?;
    name.strip_prefix("origin/").map(str::to_string)
}

/// The edit link template for a GitHub repository. Other hosts get none;
/// a tree can still set its own `edit_url`.
fn edit_url(repo: &str, branch: &str, dir: &str) -> Option<String> {
    let rest = repo.strip_prefix("https://github.com/")?;
    let rest = rest.trim_end_matches('/');
    let rest = rest.strip_suffix(".git").unwrap_or(rest);
    let (owner, name) = rest.split_once('/')?;
    if owner.is_empty() || name.is_empty() || name.contains('/') {
        return None;
    }
    let prefix = if dir.is_empty() {
        String::new()
    } else {
        format!("{dir}/")
    };
    Some(format!(
        "https://github.com/{owner}/{name}/edit/{branch}/{prefix}{{path}}"
    ))
}

/// Run git in `dir` without prompting for credentials, returning its
/// trimmed standard output.
fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .context("running git (is it installed?)")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git {} failed: {}", args.join(" "), stderr.trim());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::site::{Site, write_fixture};

    /// A repository whose `docs/learn` holds one product, and a site
    /// fixture beside it that merges `merge` (TOML lines).
    fn setup(merge: &str) -> (tempfile::TempDir, PathBuf, String) {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        let topic = source.join("docs/learn/gamma.product/1--guide.topic");
        fs::create_dir_all(&topic).unwrap();
        fs::create_dir_all(source.join("src")).unwrap();
        fs::write(source.join("src/main.c"), "not documentation").unwrap();
        fs::write(
            source.join("docs/learn/gamma.product/trail.toml"),
            "title = \"Gamma\"\nmonogram = \"G\"\ncolor = \"#3b82f6\"\ndescription = \"d\"\n",
        )
        .unwrap();
        fs::write(topic.join("trail.toml"), "title = \"Guide\"\n").unwrap();
        fs::write(
            topic.join("1--start.md"),
            "---\ntitle: Start\ntype: concept\n---\n\nFirst.\n",
        )
        .unwrap();
        git(dir.path(), &["init", "--quiet", "-b", "trunk", "source"]).unwrap();
        commit(&source, "first");
        let url = format!("file://{}", source.display());
        let root = dir.path().join("site");
        write_fixture(&root);
        let config = fs::read_to_string(root.join("trail.toml")).unwrap();
        let merge = merge.replace("{url}", &url);
        fs::write(
            root.join("trail.toml"),
            format!("{config}\n[[merge]]\n{merge}\n"),
        )
        .unwrap();
        (dir, root, url)
    }

    fn commit(repo: &Path, message: &str) {
        git(repo, &["add", "--all"]).unwrap();
        git(
            repo,
            &[
                "-c",
                "user.name=Trail",
                "-c",
                "user.email=trail@example.invalid",
                "commit",
                "--quiet",
                "-m",
                message,
            ],
        )
        .unwrap();
    }

    fn load(root: &Path) -> Result<Site> {
        Site::load(root, &root.join("dist"))
    }

    #[test]
    fn a_repo_tree_is_fetched_sparsely_and_refreshed() {
        let (dir, root, _) = setup("repo = \"{url}\"\ndir = \"docs/learn\"");
        let site = load(&root).unwrap();
        assert!(site.products.iter().any(|p| p.slug == "gamma"));
        let tree = &site.trees[1];
        assert!(tree.root.starts_with(root.join(".trail-cache/repos")));
        assert!(tree.root.ends_with("docs/learn"));
        // Only the tree's directory is checked out, besides root files.
        let checkout = tree.root.parent().unwrap().parent().unwrap().to_path_buf();
        assert!(!checkout.join("src").exists());
        // A file:// repository is not on GitHub, so it gets no edit link.
        assert_eq!(tree.edit_url, None);

        // The process keeps its checkout; a new one fetches the new tip.
        let topic = dir
            .path()
            .join("source/docs/learn/gamma.product/1--guide.topic");
        fs::write(
            topic.join("2--next.md"),
            "---\ntitle: Next\ntype: concept\n---\n\nSecond.\n",
        )
        .unwrap();
        commit(&dir.path().join("source"), "second");
        assert!(
            !load(&root).unwrap().trees[1]
                .root
                .join("gamma.product/1--guide.topic/2--next.md")
                .exists()
        );
        resolved().lock().unwrap().remove(&checkout);
        fs::write(checkout.join(".git/marker"), "kept").unwrap();
        let site = load(&root).unwrap();
        assert!(
            site.trees[1]
                .root
                .join("gamma.product/1--guide.topic/2--next.md")
                .is_file()
        );
        // It was updated in place, not cloned again.
        assert!(checkout.join(".git/marker").is_file());
    }

    #[test]
    fn a_tree_can_take_the_whole_repository() {
        let (_dir, root, _) = setup("repo = \"{url}\"\ndir = \"docs/learn\"\nsparse = false");
        let site = load(&root).unwrap();
        let checkout = site.trees[1].root.parent().unwrap().parent().unwrap();
        assert!(checkout.join("src/main.c").is_file());
    }

    #[test]
    fn a_local_path_is_preferred_to_the_repo() {
        let (dir, root, _) = setup(
            "path = \"../source/docs/learn\"\nrepo = \"file:///nonexistent/trail-test\"\n\
             dir = \"docs/learn\"",
        );
        let site = load(&root).unwrap();
        assert!(is_same_tree(
            &site.trees[1].root,
            &dir.path().join("source/docs/learn")
        ));
        assert!(!root.join(".trail-cache/repos").exists());
    }

    fn is_same_tree(a: &Path, b: &Path) -> bool {
        fs::canonicalize(a).unwrap() == fs::canonicalize(b).unwrap()
    }

    #[test]
    fn a_missing_local_path_falls_back_to_the_repo() {
        let (_dir, root, _) =
            setup("path = \"../elsewhere\"\nrepo = \"{url}\"\ndir = \"docs/learn\"");
        let site = load(&root).unwrap();
        assert!(
            site.trees[1]
                .root
                .starts_with(root.join(".trail-cache/repos"))
        );
    }

    #[test]
    fn misconfigured_merges_fail_to_load() {
        for (merge, message) in [
            ("dir = \"docs\"", "needs a path, a repo, or both"),
            ("path = \"../source\"\ndir = \"docs\"", "dir needs a repo"),
            (
                "path = \"../source\"\nbranch = \"trunk\"",
                "branch needs a repo",
            ),
            (
                "path = \"../source\"\nsparse = false",
                "sparse needs a repo",
            ),
            ("repo = \"\"", "repo cannot be empty"),
            ("repo = \"{url}\"\ndir = \"../up\"", "plain relative path"),
            (
                "repo = \"{url}\"\ndir = \"docs/nowhere\"",
                "has no directory 'docs/nowhere'",
            ),
            (
                "repo = \"{url}\"\nbranch = \"gone\"",
                "has no branch 'gone'",
            ),
            ("path = \"../nope\"", "is not a directory"),
        ] {
            let (_dir, root, _) = setup(merge);
            let error = format!("{:#}", load(&root).unwrap_err());
            assert!(error.contains(message), "{merge}: {error}");
        }
    }

    #[test]
    fn github_repositories_get_edit_links() {
        let link = |repo, dir| edit_url(repo, "peios/main", dir);
        let expected = "https://github.com/peios/kernel/edit/peios/main/docs/{path}";
        assert_eq!(
            link("https://github.com/peios/kernel", "docs").unwrap(),
            expected
        );
        assert_eq!(
            link("https://github.com/peios/kernel.git", "docs").unwrap(),
            expected
        );
        assert_eq!(
            link("https://github.com/peios/kernel/", "docs").unwrap(),
            expected
        );
        assert_eq!(
            link("https://github.com/peios/kernel", "").unwrap(),
            "https://github.com/peios/kernel/edit/peios/main/{path}"
        );
        assert_eq!(link("https://gitlab.com/peios/kernel", "docs"), None);
        assert_eq!(link("https://github.com/peios", "docs"), None);
        assert_eq!(link("git@github.com:peios/kernel.git", "docs"), None);
    }
}
