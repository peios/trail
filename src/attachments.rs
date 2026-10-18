//! Declared downloads and deterministic source bundles. Attachment inputs are
//! data: packaging never executes their scripts or follows symbolic links.
use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::build::Output;
use crate::site::{Product, Site, Tree};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub enum Format {
    #[default]
    #[serde(rename = "file")]
    File,
    #[serde(rename = "tar.gz")]
    TarGz,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttachmentConfig {
    pub id: String,
    pub title: String,
    pub description: String,
    pub source: String,
    pub filename: String,
    #[serde(default)]
    pub format: Format,
    /// Exact relative paths or directory subtrees, not glob patterns.
    #[serde(default)]
    pub exclude: Vec<String>,
}

fn safe_name(name: &str) -> bool {
    name.bytes()
        .next()
        .is_some_and(|c| c.is_ascii_alphanumeric())
        && name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
}

fn relative_path(path: &str) -> Result<&Path> {
    ensure!(
        !path.is_empty() && !path.contains('\\') && !path.chars().any(char::is_control),
        "attachment path '{path}' must be a nonempty relative path"
    );
    let parsed = Path::new(path);
    ensure!(
        parsed
            .components()
            .all(|part| matches!(part, Component::Normal(_))),
        "attachment path '{path}' must stay inside its source tree"
    );
    Ok(parsed)
}

/// Resolve existing ancestors even when the output directory has not been made.
fn output_location(path: &Path) -> Result<PathBuf> {
    let mut resolved = PathBuf::new();
    for part in std::path::absolute(path)?.components() {
        match part {
            Component::ParentDir => {
                resolved.pop();
            }
            Component::CurDir => {}
            _ => {
                resolved.push(part);
                if resolved.exists() {
                    resolved = resolved.canonicalize()?;
                }
            }
        }
    }
    Ok(resolved)
}

impl AttachmentConfig {
    pub fn url(&self) -> String {
        format!("/attachments/{}/{}", self.id, self.filename)
    }

    pub fn metadata_url(&self) -> String {
        format!("/attachments/{}/metadata.json", self.id)
    }

    pub fn validate(&self, root: &Path, out: &Path) -> Result<()> {
        ensure!(
            safe_name(&self.id) && self.id != "index.json",
            "invalid or reserved attachment id '{}'",
            self.id
        );
        ensure!(
            safe_name(&self.filename) && self.filename != "metadata.json",
            "invalid or reserved attachment filename '{}'",
            self.filename
        );
        ensure!(
            !self.title.trim().is_empty() && !self.description.trim().is_empty(),
            "attachment '{}' needs a title and description",
            self.id
        );
        let relative = relative_path(&self.source)?;
        let first = relative
            .components()
            .next()
            .unwrap()
            .as_os_str()
            .to_string_lossy();
        ensure!(
            !first.ends_with(".product") && !first.ends_with(".antho") && first != "trail.toml",
            "attachment sources must be outside collection directories and trail.toml"
        );
        let mut source = root.to_path_buf();
        for part in relative.components() {
            source.push(part);
            ensure!(
                !fs::symlink_metadata(&source)?.file_type().is_symlink(),
                "attachment source '{}' is a symbolic link",
                source.display()
            );
        }
        let canonical = source.canonicalize()?;
        let output = output_location(out)?;
        ensure!(
            !canonical.starts_with(&output) && !output.starts_with(&canonical),
            "attachment '{}' overlaps the build output",
            self.id
        );
        match self.format {
            Format::File => ensure!(
                source.is_file() && self.exclude.is_empty(),
                "file attachment '{}' requires a regular file and no exclusions",
                self.id
            ),
            Format::TarGz => {
                ensure!(
                    source.is_dir() && self.filename.ends_with(".tar.gz"),
                    "tar.gz attachment '{}' requires a directory and .tar.gz filename",
                    self.id
                );
                for excluded in &self.exclude {
                    relative_path(excluded)?;
                }
            }
        }
        Ok(())
    }
}

pub fn validate_site(trees: &[Tree], products: &[Product], favicon: Option<&Path>) -> Result<()> {
    let mut ids = HashSet::new();
    for attachment in trees.iter().flat_map(|tree| &tree.attachments) {
        ensure!(
            ids.insert(&attachment.id),
            "duplicate attachment id '{}' across site trees",
            attachment.id
        );
    }
    if ids.is_empty() {
        return Ok(());
    }
    ensure!(
        !products.iter().any(|p| p.slug == "attachments"),
        "product 'attachments' collides with declared downloads"
    );
    ensure!(
        favicon
            .and_then(Path::file_name)
            .is_none_or(|name| name != "attachments"),
        "favicon collides with declared downloads"
    );
    for path in trees.iter().flat_map(|tree| &tree.passthrough) {
        ensure!(
            Path::new(path)
                .components()
                .next()
                .is_none_or(|part| part.as_os_str() != "attachments"),
            "passthrough '{path}' collides with declared downloads"
        );
    }
    Ok(())
}

struct Input {
    name: String,
    directory: bool,
    mode: u32,
    bytes: Vec<u8>,
}

fn inputs(source: &Path, exclude: &[String]) -> Result<Vec<Input>> {
    fn visit(root: &Path, path: &Path, exclude: &[String], result: &mut Vec<Input>) -> Result<()> {
        let relative = path.strip_prefix(root)?;
        if exclude.iter().any(|p| relative.starts_with(p)) {
            return Ok(());
        }
        let name = relative
            .to_str()
            .context("attachment filenames must be UTF-8")?;
        relative_path(name)?;
        let metadata = fs::symlink_metadata(path)?;
        ensure!(
            metadata.is_dir() || metadata.is_file(),
            "attachment input '{}' must be a regular file or directory, not a link or special file",
            path.display()
        );
        let directory = metadata.is_dir();
        #[cfg(unix)]
        let executable = {
            use std::os::unix::fs::PermissionsExt;
            metadata.permissions().mode() & 0o111 != 0
        };
        #[cfg(not(unix))]
        let executable = false;
        result.push(Input {
            name: name.into(),
            directory,
            mode: if directory || executable {
                0o755
            } else {
                0o644
            },
            bytes: if directory {
                Vec::new()
            } else {
                fs::read(path)?
            },
        });
        if directory {
            for entry in fs::read_dir(path)? {
                visit(root, &entry?.path(), exclude, result)?;
            }
        }
        Ok(())
    }
    let mut result = Vec::new();
    for entry in fs::read_dir(source)? {
        visit(source, &entry?.path(), exclude, &mut result)?;
    }
    result.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(result)
}

fn archive(id: &str, entries: &[Input]) -> Result<Vec<u8>> {
    let encoder = flate2::GzBuilder::new()
        .mtime(0)
        .operating_system(255)
        .write(Vec::new(), flate2::Compression::default());
    let mut builder = tar::Builder::new(encoder);
    // Explicit headers avoid embedding host ownership, timestamps or paths.
    for (name, directory, mode, bytes) in std::iter::once((id.to_owned(), true, 0o755, &[][..]))
        .chain(entries.iter().map(|e| {
            (
                format!("{id}/{}", e.name),
                e.directory,
                e.mode,
                e.bytes.as_slice(),
            )
        }))
    {
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(if directory {
            tar::EntryType::Directory
        } else {
            tar::EntryType::Regular
        });
        header.set_size(bytes.len() as u64);
        header.set_mode(mode);
        header.set_uid(0);
        header.set_gid(0);
        header.set_mtime(0);
        builder.append_data(&mut header, name, bytes)?;
    }
    Ok(builder.into_inner()?.finish()?)
}

#[derive(Serialize)]
pub struct Metadata {
    schema_version: u32,
    id: String,
    title: String,
    description: String,
    filename: String,
    url: String,
    metadata_url: String,
    format: Format,
    media_type: &'static str,
    size_bytes: u64,
    sha256: String,
}

pub struct Published {
    config: AttachmentConfig,
    bytes: Vec<u8>,
    sha256: String,
}

pub fn prepare(site: &Site) -> Result<Vec<Published>> {
    let mut published = Vec::new();
    for tree in &site.trees {
        for config in &tree.attachments {
            let source = tree.root.join(&config.source);
            let bytes = match config.format {
                Format::File => fs::read(&source)?,
                Format::TarGz => {
                    let entries = inputs(&source, &config.exclude)
                        .with_context(|| format!("reading attachment '{}'", config.id))?;
                    let hashes: Vec<_> = entries
                        .iter()
                        .map(|e| (&e.name, e.directory, e.mode, crate::memo::key(&[&e.bytes])))
                        .collect();
                    let key = crate::cache::key_json(&(&config.id, hashes));
                    match crate::cache::read_bytes("attachment", &key) {
                        Some(bytes) => bytes,
                        None => {
                            let bytes = archive(&config.id, &entries)?;
                            crate::cache::write_bytes("attachment", &key, &bytes);
                            bytes
                        }
                    }
                }
            };
            let sha256 = format!("{:x}", Sha256::digest(&bytes));
            published.push(Published {
                config: config.clone(),
                bytes,
                sha256,
            });
        }
    }
    published.sort_by(|a, b| a.config.id.cmp(&b.config.id));
    Ok(published)
}

pub fn publish(attachments: &[Published], out: &Output, base: &str) -> Result<()> {
    if attachments.is_empty() {
        return Ok(());
    }
    let mut all = Vec::new();
    for attachment in attachments {
        let config = &attachment.config;
        let metadata = Metadata {
            schema_version: 1,
            id: config.id.clone(),
            title: config.title.clone(),
            description: config.description.clone(),
            filename: config.filename.clone(),
            url: format!("{base}{}", config.url()),
            metadata_url: format!("{base}{}", config.metadata_url()),
            format: config.format,
            media_type: if config.format == Format::TarGz {
                "application/gzip"
            } else {
                "application/octet-stream"
            },
            size_bytes: attachment.bytes.len() as u64,
            sha256: attachment.sha256.clone(),
        };
        out.write(
            &out.dir().join(config.url().trim_start_matches('/')),
            &attachment.bytes,
        )?;
        out.write(
            &out.dir()
                .join(config.metadata_url().trim_start_matches('/')),
            &serde_json::to_vec_pretty(&metadata)?,
        )?;
        all.push(metadata);
    }
    out.write(
        &out.dir().join("attachments/index.json"),
        &serde_json::to_vec_pretty(&all)?,
    )
}

/// Include hidden declared inputs and executable modes in serve's reconciliation.
pub fn source_fingerprints(root: &Path, out: &Path) -> Result<Vec<(PathBuf, String)>> {
    let path = root.join("trail.toml");
    if !path.exists() {
        return Ok(Vec::new());
    }
    #[derive(Deserialize)]
    struct Config {
        #[serde(default)]
        attachments: Vec<AttachmentConfig>,
    }
    let config: Config = toml::from_str(&fs::read_to_string(path)?)?;
    let mut result = Vec::new();
    for a in config.attachments {
        a.validate(root, out)?;
        let source = root.join(&a.source);
        let key = if a.format == Format::File {
            crate::memo::key(&[&fs::read(&source)?])
        } else {
            let entries = inputs(&source, &a.exclude)?;
            crate::cache::key_json(
                &entries
                    .iter()
                    .map(|e| (&e.name, e.directory, e.mode, crate::memo::key(&[&e.bytes])))
                    .collect::<Vec<_>>(),
            )
        };
        result.push((source, key));
    }
    Ok(result)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{build, cache, links::LinkIndex, markdown, site};
    use std::io::Read;

    pub(crate) fn fixture(root: &Path) {
        site::write_fixture(root);
        fs::create_dir_all(root.join("bundles/tools/empty")).unwrap();
        fs::write(root.join("bundles/tools/run.sh"), b"echo hello\n").unwrap();
        fs::write(root.join("bundles/tools/.settings"), b"hidden input\n").unwrap();
        fs::write(root.join("bundles/tools/ignored"), b"not published\n").unwrap();
        fs::write(root.join("bundles/example.txt"), b"raw download\n").unwrap();
        let path = root.join("trail.toml");
        let mut config = fs::read_to_string(&path).unwrap();
        config.push_str(
            r#"
[[attachments]]
id = "tools"
title = "Tools"
description = "Example source tools."
source = "bundles/tools"
filename = "tools.tar.gz"
format = "tar.gz"
exclude = ["ignored"]

[[attachments]]
id = "example"
title = "Example"
description = "A raw file."
source = "bundles/example.txt"
filename = "example.txt"
"#,
        );
        fs::write(path, config).unwrap();
    }

    #[test]
    fn archives_are_reproducible_and_metadata_describes_actual_bytes() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        fixture(a.path());
        fixture(b.path());
        #[cfg(unix)]
        for root in [a.path(), b.path()] {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                root.join("bundles/tools/run.sh"),
                fs::Permissions::from_mode(0o751),
            )
            .unwrap();
        }
        let prepare_root =
            |root: &Path| prepare(&Site::load(root, &root.join("dist")).unwrap()).unwrap();
        let aa = prepare_root(a.path());
        let bb = prepare_root(b.path());
        assert_eq!(aa[1].bytes, bb[1].bytes);
        let out = Output::new(&a.path().join("dist"));
        publish(&aa, &out, "/revision").unwrap();
        let bytes = fs::read(out.dir().join("attachments/tools/tools.tar.gz")).unwrap();
        let meta: serde_json::Value = serde_json::from_slice(
            &fs::read(out.dir().join("attachments/tools/metadata.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(meta["sha256"], format!("{:x}", Sha256::digest(&bytes)));
        assert_eq!(meta["size_bytes"], bytes.len() as u64);
        assert_eq!(meta["url"], "/revision/attachments/tools/tools.tar.gz");
        assert!(
            !meta
                .to_string()
                .contains(&a.path().to_string_lossy().to_string())
        );
        let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(bytes.as_slice()));
        let mut names = Vec::new();
        for entry in archive.entries().unwrap() {
            let mut entry = entry.unwrap();
            let name = entry.path().unwrap().to_string_lossy().to_string();
            assert_eq!(entry.header().mtime().unwrap(), 0);
            assert_eq!(entry.header().uid().unwrap(), 0);
            if name == "tools/run.sh" {
                #[cfg(unix)]
                assert_eq!(entry.header().mode().unwrap(), 0o755);
                let mut text = String::new();
                entry.read_to_string(&mut text).unwrap();
                assert_eq!(text, "echo hello\n");
            }
            names.push(name);
        }
        assert_eq!(
            names,
            ["tools", "tools/.settings", "tools/empty", "tools/run.sh"]
        );
        assert_eq!(
            fs::read(out.dir().join("attachments/example/example.txt")).unwrap(),
            b"raw download\n"
        );
    }

    #[test]
    fn downloads_resolve_in_html_print_and_markdown_and_invalidate_cached_links() {
        let dir = tempfile::tempdir().unwrap();
        fixture(dir.path());
        let source = "[Tools](~attachments/tools) [Details](~attachments/tools/metadata)";
        let session = cache::Session::new(dir.path(), true);
        let site = Site::load(dir.path(), &dir.path().join("dist")).unwrap();
        let links = LinkIndex::new(&site);
        let options = markdown::RenderOptions {
            strict: true,
            base: "/rev",
            ..Default::default()
        };
        let rendered = markdown::render(source, &links, options).unwrap();
        assert!(rendered.broken.is_empty() && rendered.warnings.is_empty());
        assert!(
            rendered
                .html
                .contains("/rev/attachments/tools/tools.tar.gz")
        );
        let mirrors = markdown::rewrite_source(source, &links, None, None, "/rev");
        assert!(mirrors.contains("(/rev/attachments/tools/tools.tar.gz)"));
        assert!(!mirrors.contains(".gz.md") && !mirrors.contains(".json.md"));
        assert_eq!(
            markdown::rewrite_source("[Tools](<~attachments/tools>)", &links, None, None, ""),
            "[Tools](</attachments/tools/tools.tar.gz>)"
        );
        let reference = "[Tools][bundle]\n\n[bundle]: <~attachments/tools> \"Source bundle\"\n";
        let mirror = markdown::rewrite_source(reference, &links, None, None, "/rev");
        assert!(
            mirror.contains("[bundle]: </rev/attachments/tools/tools.tar.gz> \"Source bundle\"")
        );
        let anchors = Default::default();
        let print = markdown::render(
            source,
            &links,
            markdown::RenderOptions {
                print: Some(markdown::PrintScope {
                    anchors: &anchors,
                    cache_key: "test",
                    base: Some("https://docs.example"),
                }),
                ..options
            },
        )
        .unwrap();
        assert!(
            print
                .html
                .contains("https://docs.example/rev/attachments/tools/tools.tar.gz")
        );
        let missing = markdown::render("[Missing](~attachments/absent)", &links, options).unwrap();
        assert_eq!(missing.broken.len(), 1);
        let mut changed = site.clone();
        changed.trees[0].attachments[0].filename = "new.tar.gz".into();
        let updated = markdown::render(source, &LinkIndex::new(&changed), options).unwrap();
        assert!(updated.html.contains("new.tar.gz"));
        changed.trees[0].attachments.clear();
        assert!(
            !markdown::render(source, &LinkIndex::new(&changed), options)
                .unwrap()
                .broken
                .is_empty()
        );
        session.finish();
    }

    #[test]
    fn incremental_bundles_track_edits_removals_and_prune_retired_downloads() {
        let dir = tempfile::tempdir().unwrap();
        fixture(dir.path());
        let out = dir.path().join("dist");
        let session = cache::Session::new(dir.path(), true);
        let build = || {
            let site = Site::load(dir.path(), &out).unwrap();
            build::build_all(
                &site,
                &out,
                &build::BuildOptions {
                    cachebust: Some("rev".into()),
                    ..Default::default()
                },
            )
            .unwrap();
            fs::read(out.join("attachments/tools/tools.tar.gz")).unwrap()
        };
        let first = build();
        let mtime = fs::metadata(out.join("attachments/tools/tools.tar.gz"))
            .unwrap()
            .modified()
            .unwrap();
        assert_eq!(first, build());
        assert!(session.hits("attachment") > 0);
        assert_eq!(
            mtime,
            fs::metadata(out.join("attachments/tools/tools.tar.gz"))
                .unwrap()
                .modified()
                .unwrap()
        );
        fs::write(
            dir.path().join("bundles/tools/.settings"),
            b"edited input\n",
        )
        .unwrap();
        let edited = build();
        assert_ne!(first, edited);
        fs::remove_file(dir.path().join("bundles/tools/run.sh")).unwrap();
        assert_ne!(edited, build());
        let bytes = fs::read(out.join("attachments/tools/tools.tar.gz")).unwrap();
        assert_eq!(
            bytes,
            fs::read(out.join("rev/attachments/tools/tools.tar.gz")).unwrap()
        );
        {
            let _uncached = cache::Session::new(dir.path(), false);
            assert_eq!(bytes, build());
        }
        let mut site = Site::load(dir.path(), &out).unwrap();
        site.trees[0].attachments.clear();
        build::build_all(&site, &out, &Default::default()).unwrap();
        assert!(!out.join("attachments/tools/tools.tar.gz").exists());
        assert!(!out.join("attachments/index.json").exists());
    }

    #[test]
    fn invalid_sources_duplicates_and_output_collisions_fail() {
        let dir = tempfile::tempdir().unwrap();
        fixture(dir.path());
        let root = dir.path();
        let site = Site::load(root, &root.join("dist")).unwrap();
        let original = &site.trees[0].attachments[0];
        assert!(
            build::build_all(
                &site,
                &root.join("dist"),
                &build::BuildOptions {
                    cachebust: Some("attachments".into()),
                    ..Default::default()
                }
            )
            .is_err()
        );
        for source in [
            "../outside",
            "/etc",
            "bundles/missing",
            "alpha.product",
            "peios.antho",
            "trail.toml",
        ] {
            let mut config = original.clone();
            config.source = source.into();
            assert!(
                config.validate(root, &root.join("dist")).is_err(),
                "{source}"
            );
        }
        for filename in ["../escape.tar.gz", "metadata.json", "wrong.zip"] {
            let mut config = original.clone();
            config.filename = filename.into();
            assert!(config.validate(root, &root.join("dist")).is_err());
        }
        assert!(
            original
                .validate(root, &root.join("bundles/tools/output"))
                .is_err()
        );
        let mut trees = site.trees.clone();
        trees.push(trees[0].clone());
        assert!(validate_site(&trees, &site.products, None).is_err());
        trees.pop();
        trees[0].passthrough.push("attachments/tools".into());
        assert!(validate_site(&trees, &site.products, None).is_err());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(root.join("bundles/tools"), root.join("output-alias"))
                .unwrap();
            assert!(
                original
                    .validate(root, &root.join("output-alias/new-output"))
                    .is_err()
            );
            std::os::unix::fs::symlink("/etc/passwd", root.join("bundles/tools/leak")).unwrap();
            assert!(prepare(&site).is_err());
            let mut config = original.clone();
            config.source = "bundles/tools/leak".into();
            config.format = Format::File;
            config.exclude.clear();
            assert!(config.validate(root, &root.join("dist")).is_err());
        }
    }

    #[test]
    fn merged_trees_keep_their_attachment_roots() {
        let dir = tempfile::tempdir().unwrap();
        fixture(dir.path());
        let extra = dir.path().join("extra");
        fs::create_dir(&extra).unwrap();
        fs::write(extra.join("sample.bin"), b"merged").unwrap();
        fs::write(extra.join("trail.toml"), "sitename='Extra'\ntitle='Extra'\ndescription='Extra'\nfooter=''\n[[attachments]]\nid='merged'\ntitle='Merged'\ndescription='Merged download.'\nsource='sample.bin'\nfilename='sample.bin'\n").unwrap();
        let config = dir.path().join("trail.toml");
        let text = fs::read_to_string(&config).unwrap() + "\n[[merge]]\npath='extra'\n";
        fs::write(config, text).unwrap();
        let site = Site::load(dir.path(), &dir.path().join("dist")).unwrap();
        let attachments = prepare(&site).unwrap();
        assert_eq!(
            attachments
                .iter()
                .find(|a| a.config.id == "merged")
                .unwrap()
                .bytes,
            b"merged"
        );
        assert_eq!(
            LinkIndex::new(&site).resolve("attachments/merged").unwrap(),
            "/attachments/merged/sample.bin"
        );
    }
}
