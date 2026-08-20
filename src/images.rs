use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};

/// File extensions the loaders recognise as co-located images. Anything
/// else that isn't an article or a link stays an "unexpected file" error.
pub const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "svg", "webp", "avif"];

/// An image file discovered during site loading, co-located with the
/// articles that use it: where it sits on disk, and the site-absolute URL
/// it publishes at (its container's URL plus its own file name).
#[derive(Debug)]
pub struct ImageAsset {
    pub source: PathBuf,
    pub url: String,
}

/// Every image in the tree, looked up by normalized source path when an
/// article's relative `![](...)` destination resolves. Tracks which
/// images were actually referenced, so the build copies those — and only
/// those — into the output and can warn about the rest.
#[derive(Debug)]
pub struct ImageIndex {
    by_source: HashMap<PathBuf, ImageInfo>,
    used: RefCell<HashSet<PathBuf>>,
}

#[derive(Debug, Clone)]
pub struct ImageInfo {
    /// Site-absolute URL of the published image.
    pub url: String,
    /// Pixel dimensions from the file header, emitted as width/height
    /// attributes so pages don't shift layout while images load. None
    /// for SVGs (which scale) and headers that can't be read.
    pub width: Option<u32>,
    pub height: Option<u32>,
}

/// What one article's images resolve against: the shared index plus the
/// article's own source directory — relative destinations are relative
/// to the .md file, exactly as an editor preview resolves them.
#[derive(Debug, Clone, Copy)]
pub struct ImageScope<'a> {
    pub index: &'a ImageIndex,
    pub dir: &'a Path,
}

impl ImageIndex {
    pub fn new(assets: &[ImageAsset]) -> ImageIndex {
        let mut by_source = HashMap::new();
        for asset in assets {
            let size = imagesize::size(&asset.source).ok();
            by_source.insert(
                normalize(&asset.source),
                ImageInfo {
                    url: asset.url.clone(),
                    width: size.and_then(|s| u32::try_from(s.width).ok()),
                    height: size.and_then(|s| u32::try_from(s.height).ok()),
                },
            );
        }
        ImageIndex {
            by_source,
            used: RefCell::new(HashSet::new()),
        }
    }

    /// Resolve a relative destination against `dir`. A hit marks the
    /// image as used; a miss means the file doesn't exist in the tree
    /// (or escapes it) — the caller decides how loudly to complain.
    pub fn resolve(&self, dir: &Path, reference: &str) -> Option<&ImageInfo> {
        let path = normalize(&dir.join(reference));
        let info = self.by_source.get(&path)?;
        self.used.borrow_mut().insert(path);
        Some(info)
    }

    pub fn is_used(&self, asset: &ImageAsset) -> bool {
        self.used.borrow().contains(&normalize(&asset.source))
    }
}

/// Lexical normalization — resolves `.` and `..` without touching the
/// filesystem, so index keys and lookups agree however the root was
/// spelled.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolution_normalizes_dotted_paths_and_tracks_use() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("a/b")).unwrap();
        std::fs::write(dir.path().join("a/pic.svg"), "<svg></svg>").unwrap();
        let assets = [ImageAsset {
            source: dir.path().join("a/pic.svg"),
            url: "/alpha/a/pic.svg".into(),
        }];
        let index = ImageIndex::new(&assets);

        assert!(
            index
                .resolve(&dir.path().join("a"), "missing.png")
                .is_none()
        );
        assert!(!index.is_used(&assets[0]));
        let info = index
            .resolve(&dir.path().join("a/b"), "../pic.svg")
            .unwrap();
        assert_eq!(info.url, "/alpha/a/pic.svg");
        // SVG headers carry no fixed pixel size.
        assert_eq!(info.width, None);
        assert!(index.is_used(&assets[0]));
    }

    #[test]
    fn raster_dimensions_come_from_the_file_header() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("dot.png"), crate::site::TEST_PNG).unwrap();
        let assets = [ImageAsset {
            source: dir.path().join("dot.png"),
            url: "/x/dot.png".into(),
        }];
        let index = ImageIndex::new(&assets);
        let info = index.resolve(dir.path(), "dot.png").unwrap();
        assert_eq!((info.width, info.height), (Some(2), Some(1)));
    }
}
