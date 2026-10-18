//! Combine authored containers before aliases, plugins and derived metadata.
//!
//! Leaves retain their actual source paths. No staging tree or rewritten paths
//! intervene between a page and its repository's assets, plugins or edit URL.
use std::collections::HashMap;

use anyhow::{Result, bail, ensure};

use super::{
    Anthology, AnthologyChild, AnthologyItem, Article, Book, BookChild, ImageAsset, LinkStub,
    Product, ProductChild, ProductItem, Topic, TopicChild, TopicFolder, check_duplicates,
    check_flat_slugs, sort_book_children,
};

macro_rules! same {
    ($left:ident, $right:ident, $($field:ident),+ $(,)?) => {
        $(ensure!(
            $left.$field == $right.$field,
            "merge conflict at '{}': {} differs",
            $left.slug,
            stringify!($field),
        );)+
    };
}

/// Merge equal identities, then validate sibling order independently of source
/// tree order. Each container's merge function checks its type and metadata.
fn combine<T>(
    left: &mut Vec<T>,
    right: Vec<T>,
    slug: impl Fn(&T) -> &str,
    merge: impl Fn(&mut T, T) -> Result<()>,
) -> Result<()> {
    let mut positions: HashMap<String, usize> = left
        .iter()
        .enumerate()
        .map(|(index, item)| (slug(item).to_string(), index))
        .collect();
    for incoming in right {
        if let Some(index) = positions.get(slug(&incoming)).copied() {
            merge(&mut left[index], incoming)?;
        } else {
            positions.insert(slug(&incoming).to_string(), left.len());
            left.push(incoming);
        }
    }
    Ok(())
}

pub(super) fn collections(left: &mut Vec<Product>, right: Vec<Product>) -> Result<()> {
    combine(
        left,
        right,
        |item| &item.slug,
        |left, right| {
            same!(
                left,
                right,
                kind,
                title,
                monogram,
                color,
                description,
                inline_refs
            );
            product_children(&mut left.children, right.children)?;
            check_flat_slugs("grouping", left.items().map(ProductItem::slug))
        },
    )
}

fn product_children(left: &mut Vec<ProductChild>, right: Vec<ProductChild>) -> Result<()> {
    combine(left, right, ProductChild::slug, |left, right| {
        match (left, right) {
            (ProductChild::Item(left), ProductChild::Item(right)) => product_item(left, right),
            (ProductChild::Shelf(left), ProductChild::Shelf(right)) => {
                same!(left, right, order, title);
                combine(
                    &mut left.items,
                    right.items,
                    ProductItem::slug,
                    product_item,
                )?;
                left.items.sort_by_key(ProductItem::order);
                check_duplicates("item", &left.items, ProductItem::order, ProductItem::slug)
            }
            (left, _) => bail!(
                "merge conflict at '{}': container type differs",
                left.slug()
            ),
        }
    })?;
    left.sort_by_key(ProductChild::order);
    check_duplicates("grouping", left, ProductChild::order, ProductChild::slug)
}

fn product_item(left: &mut ProductItem, right: ProductItem) -> Result<()> {
    match (left, right) {
        (
            ProductItem::Anthology { anthology: left },
            ProductItem::Anthology { anthology: right },
        ) => anthology(left, right),
        (ProductItem::Topic { topic: left }, ProductItem::Topic { topic: right }) => {
            topic(left, right)
        }
        (ProductItem::Book { book: left }, ProductItem::Book { book: right }) => book(left, right),
        (left, _) => bail!(
            "merge conflict at '{}': container type differs",
            left.slug()
        ),
    }
}

fn anthology(left: &mut Anthology, right: Anthology) -> Result<()> {
    same!(left, right, order, title, description, inline_refs);
    combine(
        &mut left.children,
        right.children,
        AnthologyChild::slug,
        |left, right| match (left, right) {
            (AnthologyChild::Item(left), AnthologyChild::Item(right)) => {
                anthology_item(left, right)
            }
            (AnthologyChild::Shelf(left), AnthologyChild::Shelf(right)) => {
                same!(left, right, order, title);
                combine(
                    &mut left.items,
                    right.items,
                    AnthologyItem::slug,
                    anthology_item,
                )?;
                left.items.sort_by_key(AnthologyItem::order);
                check_duplicates(
                    "item",
                    &left.items,
                    AnthologyItem::order,
                    AnthologyItem::slug,
                )
            }
            (left, _) => bail!(
                "merge conflict at '{}': container type differs",
                left.slug()
            ),
        },
    )?;
    left.children.sort_by_key(AnthologyChild::order);
    check_duplicates(
        "grouping",
        &left.children,
        AnthologyChild::order,
        AnthologyChild::slug,
    )?;
    check_flat_slugs("item", left.items().map(AnthologyItem::slug))
}

fn anthology_item(left: &mut AnthologyItem, right: AnthologyItem) -> Result<()> {
    match (left, right) {
        (
            AnthologyItem::Anthology { anthology: left },
            AnthologyItem::Anthology { anthology: right },
        ) => anthology(left, right),
        (AnthologyItem::Topic { topic: left }, AnthologyItem::Topic { topic: right }) => {
            topic(left, right)
        }
        (AnthologyItem::Book { book: left }, AnthologyItem::Book { book: right }) => {
            book(left, right)
        }
        (left, _) => bail!(
            "merge conflict at '{}': container type differs",
            left.slug()
        ),
    }
}

fn topic(left: &mut Topic, right: Topic) -> Result<()> {
    same!(left, right, order, title, inline_refs);
    combine(
        &mut left.children,
        right.children,
        TopicChild::slug,
        |left, right| match (left, right) {
            (TopicChild::Folder { folder: left }, TopicChild::Folder { folder: right }) => {
                folder(left, right)
            }
            (TopicChild::Article { article: left }, TopicChild::Article { article: right }) => {
                duplicate_article(left, right)
            }
            (left, _) => bail!(
                "merge conflict at '{}': container type differs",
                left.slug()
            ),
        },
    )?;
    left.links.extend(right.links);
    left.children.sort_by_key(TopicChild::order);
    slots(
        left.children.iter().map(|item| (item.order(), item.slug())),
        &left.links,
    )
}

fn folder(left: &mut TopicFolder, right: TopicFolder) -> Result<()> {
    same!(left, right, order, title, inline_refs);
    combine(
        &mut left.articles,
        right.articles,
        |article| &article.slug,
        duplicate_article,
    )?;
    left.links.extend(right.links);
    left.articles.sort_by_key(|article| article.order);
    left.entry = left.articles.first().map(|article| article.path.clone());
    slots(
        left.articles
            .iter()
            .map(|article| (article.order, article.slug.as_str())),
        &left.links,
    )
}

fn duplicate_article(left: &mut Article, right: Article) -> Result<()> {
    bail!(
        "duplicate article '{}': {} and {}",
        left.path,
        left.source_file.display(),
        right.source_file.display(),
    )
}

fn slots<'a>(items: impl Iterator<Item = (u32, &'a str)>, links: &'a [LinkStub]) -> Result<()> {
    let mut slots: Vec<_> = items
        .chain(links.iter().map(|link| (link.order, link.slug.as_str())))
        .collect();
    slots.sort_by_key(|item| item.0);
    check_duplicates("entry", &slots, |item| item.0, |item| item.1)
}

fn book(left: &mut Book, right: Book) -> Result<()> {
    same!(left, right, order, title, short, description, inline_refs);
    book_children(&mut left.children, right.children)?;
    left.links.extend(right.links);
    book_slots(&left.children, &left.links)
}

fn book_children(left: &mut Vec<BookChild>, right: Vec<BookChild>) -> Result<()> {
    combine(left, right, BookChild::slug, |left, right| {
        match (left, right) {
            (BookChild::Chapter { chapter: left }, BookChild::Chapter { chapter: right }) => {
                same!(left, right, order, appendix, title, inline_refs);
                book_children(&mut left.children, right.children)?;
                left.links.extend(right.links);
                left.entry = match left.children.first() {
                    Some(BookChild::Article { article }) => article.path.clone(),
                    Some(BookChild::Chapter { chapter }) => chapter.entry.clone(),
                    None => String::new(),
                };
                book_slots(&left.children, &left.links)
            }
            (BookChild::Article { article: left }, BookChild::Article { article: right }) => {
                duplicate_article(left, right)
            }
            (left, _) => bail!(
                "merge conflict at '{}': container type differs",
                left.slug()
            ),
        }
    })?;
    sort_book_children(left);
    Ok(())
}

fn book_slots(children: &[BookChild], links: &[LinkStub]) -> Result<()> {
    let mut slots: Vec<_> = children
        .iter()
        .map(|item| (item.order_label(), item.slug()))
        .chain(links.iter().map(|link| {
            let order = if link.appendix {
                format!("a{}", link.order)
            } else {
                link.order.to_string()
            };
            (order, link.slug.as_str())
        }))
        .collect();
    slots.sort_by(|a, b| a.0.cmp(&b.0));
    check_duplicates("entry", &slots, |item| item.0.clone(), |item| item.1)
}

pub(super) fn images(images: &[ImageAsset]) -> Result<()> {
    let mut owners = HashMap::new();
    for image in images {
        if let Some(previous) = owners.insert(&image.url, &image.source) {
            bail!(
                "duplicate image '{}': {} and {}",
                image.url,
                previous.display(),
                image.source.display()
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use super::super::{CollectionKind, Site};
    use super::*;

    const COLLECTION: &str = "title = 'Demo'\ndescription = 'Documentation.'\n";
    const TOPIC: &str =
        "demo.antho/10--library.shelf/10--guide.antho/10--tasks.shelf/10--tasks.topic";
    const BOOK: &str = "demo.antho/10--library.shelf/10--guide.antho/20--manual.book";

    fn write(root: &Path, path: &str, content: &str) {
        let file = root.join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, content).unwrap();
    }

    fn fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (tree, order) in [("primary", 20), ("extra", 10)] {
            let root = dir.path().join(tree);
            write(
                &root,
                "trail.toml",
                "sitename = 'Test'\ntitle = 'Test'\ndescription = 'Test'\nfooter = 'Test'\n",
            );
            write(&root, "demo.antho/trail.toml", COLLECTION);
            write(
                &root,
                "demo.antho/10--library.shelf/10--guide.antho/trail.toml",
                COLLECTION,
            );
            write(&root, &format!("{BOOK}/trail.toml"), COLLECTION);
            write(
                &root,
                &format!("{TOPIC}/{order}--page-{order}.md"),
                &format!("---\ntitle: Page {order}\ntype: concept\n---\n\nPage {order}.\n"),
            );
            write(
                &root,
                &format!("{TOPIC}/30--folder/{order}--nested-{order}.md"),
                &format!("---\ntitle: Nested {order}\ntype: concept\n---\n\nNested {order}.\n"),
            );
            write(
                &root,
                &format!("{BOOK}/1--chapter/1--nested/{order}--section-{order}.md"),
                &format!("---\ntitle: Section {order}\n---\n\nSection {order}.\n"),
            );
        }
        let primary = dir.path().join("primary");
        let config = fs::read_to_string(primary.join("trail.toml")).unwrap();
        write(
            &primary,
            "trail.toml",
            &format!("{config}\n[[merge]]\npath = '../extra'\n"),
        );
        dir
    }

    fn load(dir: &Path) -> Result<Site> {
        Site::load(&dir.join("primary"), &dir.join("out"))
    }

    fn fails(dir: &Path, message: &str) {
        let error = format!("{:#}", load(dir).unwrap_err());
        assert!(error.contains(message), "{error}");
    }

    #[test]
    fn merges_every_container_level_preserving_paths_order_and_chapter_entries() {
        let dir = fixture();
        let site = load(dir.path()).unwrap();
        assert_eq!(site.products.len(), 1);
        let collection = &site.products[0];
        assert_eq!(collection.kind, CollectionKind::Anthology);
        assert_eq!(collection.path, "/demo");
        let guide = collection.anthologies().next().unwrap();
        let topic = guide.topics().next().unwrap();
        assert_eq!(
            topic
                .pages()
                .map(|page| page.slug.as_str())
                .collect::<Vec<_>>(),
            ["page-10", "page-20", "nested-10", "nested-20"]
        );
        assert!(
            topic
                .pages()
                .next()
                .unwrap()
                .source_file
                .starts_with(dir.path().join("primary/../extra"))
        );
        let book = guide
            .items()
            .find_map(|item| match item {
                AnthologyItem::Book { book } => Some(book),
                _ => None,
            })
            .unwrap();
        let articles = book.articles();
        assert_eq!(articles[0].1.number.as_deref(), Some("1.1.10"));
        assert_eq!(articles[1].1.number.as_deref(), Some("1.1.20"));
        assert_eq!(
            articles[0].0[0].entry,
            "/demo/guide/manual/chapter/nested/section-10"
        );
        assert_eq!(articles[0].0[1].entry, articles[0].0[0].entry);
        // Root anthologies remain independently buildable.
        Site::load(&dir.path().join("extra"), &dir.path().join("standalone")).unwrap();
    }

    #[test]
    fn merges_products_and_rejects_root_kind_or_metadata_disagreements() {
        let dir = fixture();
        for tree in ["primary", "extra"] {
            let root = dir.path().join(tree);
            fs::rename(root.join("demo.antho"), root.join("demo.product")).unwrap();
            write(
                &root,
                "demo.product/trail.toml",
                &format!("{COLLECTION}monogram = 'D'\ncolor = '#123456'\n"),
            );
        }
        assert_eq!(
            load(dir.path()).unwrap().products[0].kind,
            CollectionKind::Product
        );
        let extra = dir.path().join("extra");
        fs::rename(extra.join("demo.product"), extra.join("demo.antho")).unwrap();
        write(&extra, "demo.antho/trail.toml", COLLECTION);
        fails(dir.path(), "kind differs");
        write(
            &extra,
            "demo.antho/trail.toml",
            "title = 'Different'\ndescription = 'Documentation.'\n",
        );
        fs::rename(
            dir.path().join("primary/demo.product"),
            dir.path().join("primary/demo.antho"),
        )
        .unwrap();
        write(
            &dir.path().join("primary"),
            "demo.antho/trail.toml",
            COLLECTION,
        );
        fails(dir.path(), "title differs");
    }

    #[test]
    fn rejects_duplicate_leaves_aliases_assets_and_sibling_orders() {
        for (path, content, error) in [
            (
                format!("{TOPIC}/20--page-20.md"),
                "---\ntitle: Copy\ntype: concept\n---\nCopy.",
                "duplicate article",
            ),
            (
                format!("{TOPIC}/20--other.md"),
                "---\ntitle: Other\ntype: concept\n---\nOther.",
                "share order 20",
            ),
            (
                format!("{TOPIC}/40--page-20.link"),
                "target = '~demo/guide/tasks/page-10'",
                "duplicate entry slug 'page-20'",
            ),
            (
                format!("{BOOK}/1--chapter/1--nested/20--section-20.md"),
                "---\ntitle: Copy\n---\nCopy.",
                "duplicate article",
            ),
        ] {
            let dir = fixture();
            write(&dir.path().join("extra"), &path, content);
            fails(dir.path(), error);
        }
        let dir = fixture();
        for tree in ["primary", "extra"] {
            write(
                &dir.path().join(tree),
                &format!("{TOPIC}/figure.svg"),
                "<svg/>",
            );
        }
        fails(dir.path(), "duplicate image '/demo/guide/tasks/figure.svg'");
        let dir = fixture();
        for tree in ["primary", "extra"] {
            write(
                &dir.path().join(tree),
                &format!("{TOPIC}/40--alias.link"),
                "target = '~demo/guide/tasks/page-10'",
            );
        }
        fails(dir.path(), "share order 40");
    }

    #[test]
    fn rejects_container_order_type_metadata_and_flattened_shelf_conflicts() {
        let dir = fixture();
        let extra = dir.path().join("extra");
        fs::rename(
            extra.join("demo.antho/10--library.shelf"),
            extra.join("demo.antho/20--library.shelf"),
        )
        .unwrap();
        fails(dir.path(), "order differs");
        let dir = fixture();
        write(
            &dir.path().join("extra"),
            &format!("{TOPIC}/trail.toml"),
            "title = 'Other'\n",
        );
        fails(dir.path(), "title differs");
        let dir = fixture();
        let extra = dir.path().join("extra");
        fs::rename(
            extra.join(format!("{TOPIC}/30--folder")),
            extra.join(format!("{TOPIC}/30--moved")),
        )
        .unwrap();
        write(
            &extra,
            &format!("{TOPIC}/40--folder.md"),
            "---\ntitle: Folder\ntype: concept\n---\nBody.\n",
        );
        fails(dir.path(), "container type differs");
        let dir = fixture();
        let extra = dir.path().join("extra");
        fs::rename(
            extra.join("demo.antho/10--library.shelf"),
            extra.join("demo.antho/20--other.shelf"),
        )
        .unwrap();
        fails(dir.path(), "URL space");
    }
    #[test]
    fn nested_aliases_refresh_chapter_entries_and_preserve_original_sources() {
        let dir = fixture();
        let primary = dir.path().join("primary");
        write(
            &primary,
            &format!("{BOOK}/1--chapter/1--nested/1--alias.link"),
            "target = '~demo/guide/manual/chapter/nested/section-10'\n",
        );
        write(
            &primary,
            &format!("{TOPIC}/30--folder/1--alias.link"),
            "target = '~demo/guide/tasks/folder/nested-10'\n",
        );
        for (tree, order) in [("primary", 20), ("extra", 10)] {
            write(
                &dir.path().join(tree),
                &format!("{BOOK}/a1--appendix/{order}--appendix-{order}.md"),
                &format!("---\ntitle: Appendix {order}\n---\n\nAppendix content.\n"),
            );
        }
        let site = load(dir.path()).unwrap();
        let guide = site.products[0].anthologies().next().unwrap();
        let topic = guide.topics().next().unwrap();
        let alias = topic.pages().find(|page| page.slug == "alias").unwrap();
        assert_eq!(
            alias.original.as_deref(),
            Some("/demo/guide/tasks/folder/nested-10")
        );
        assert!(alias.source_file.ends_with("10--nested-10.md"));
        let book = guide
            .items()
            .find_map(|item| match item {
                AnthologyItem::Book { book } => Some(book),
                _ => None,
            })
            .unwrap();
        let articles = book.articles();
        assert_eq!(articles[0].1.number.as_deref(), Some("1.1.1"));
        assert_eq!(articles[0].0[0].entry, articles[0].1.path);
        assert_eq!(articles[0].0[1].entry, articles[0].1.path);
        assert!(articles[0].1.source_file.ends_with("10--section-10.md"));
        assert_eq!(articles[3].1.number.as_deref(), Some("A.10"));
        assert_eq!(articles[4].1.number.as_deref(), Some("A.20"));
    }

    #[test]
    fn rejects_repeated_source_trees_and_malformed_root_anthologies() {
        let dir = fixture();
        let primary = dir.path().join("primary");
        let config = fs::read_to_string(primary.join("trail.toml")).unwrap();
        write(
            &primary,
            "trail.toml",
            &format!("{config}\n[[merge]]\npath = '../extra/.'\n"),
        );
        fails(dir.path(), "duplicate article");
        let dir = fixture();
        let extra = dir.path().join("extra");
        write(&extra, "demo.antho/trail.toml", "title = 'Demo'\n");
        fails(dir.path(), "missing field `description`");
        write(
            &extra,
            "demo.antho/trail.toml",
            &format!("{COLLECTION}monogram = 'D'\n"),
        );
        fails(dir.path(), "unknown field `monogram`");
        write(&extra, "demo.antho/trail.toml", COLLECTION);
        fs::rename(extra.join("demo.antho"), extra.join("Demo.antho")).unwrap();
        fails(dir.path(), "not a valid slug");
    }
}
