package build

import (
	"encoding/json"
	"fmt"
	"html/template"
	"os"
	"path/filepath"
	"regexp"
	"strings"

	"github.com/yuin/goldmark"
	"github.com/yuin/goldmark/extension"
	"github.com/yuin/goldmark/parser"
	"github.com/yuin/goldmark/renderer/html"

	highlighting "github.com/yuin/goldmark-highlighting/v2"

	"github.com/peios/trail/internal/config"
	"github.com/peios/trail/internal/content"
	"github.com/peios/trail/internal/theme"
)

func Build(site *content.Site, cfg *config.Config, outDir string) error {
	md := goldmark.New(
		goldmark.WithExtensions(
			extension.Table,
			extension.Strikethrough,
			highlighting.NewHighlighting(
				highlighting.WithStyle("dracula"),
				highlighting.WithFormatOptions(),
			),
		),
		goldmark.WithParserOptions(parser.WithAutoHeadingID()),
		goldmark.WithRendererOptions(html.WithUnsafe()),
	)

	tmpl, err := theme.LoadTemplates(cfg)
	if err != nil {
		return fmt.Errorf("loading templates: %w", err)
	}

	if err := os.RemoveAll(outDir); err != nil {
		return fmt.Errorf("cleaning output directory: %w", err)
	}
	if err := os.MkdirAll(outDir, 0o755); err != nil {
		return fmt.Errorf("creating output directory: %w", err)
	}

	// Build each page
	for _, page := range site.Pages {
		if err := buildPage(md, tmpl, site, cfg, page, outDir); err != nil {
			return fmt.Errorf("building %s: %w", page.Slug, err)
		}
	}

	// Build category index pages
	for _, cat := range site.Categories {
		if err := buildCategoryIndex(tmpl, site, cfg, cat, outDir); err != nil {
			return fmt.Errorf("building category %s: %w", cat.Name, err)
		}
	}

	// Build homepage
	if err := buildHomepage(tmpl, site, cfg, outDir); err != nil {
		return fmt.Errorf("building homepage: %w", err)
	}

	// Build 404 page
	if err := build404(tmpl, site, cfg, outDir); err != nil {
		return fmt.Errorf("building 404: %w", err)
	}

	// Build pathway manifest for JS navigation
	if err := buildPathwayManifest(site, cfg, outDir); err != nil {
		return fmt.Errorf("building pathway manifest: %w", err)
	}

	// Build sitemap
	if err := buildSitemap(site, cfg, outDir); err != nil {
		return fmt.Errorf("building sitemap: %w", err)
	}

	// Build search index
	if err := buildSearchIndex(site, outDir); err != nil {
		return fmt.Errorf("building search index: %w", err)
	}

	// Write robots.txt
	robotsTxt := "User-agent: *\nAllow: /\nSitemap: " + strings.TrimRight(cfg.BaseURL, "/") + "/sitemap.xml\n"
	if err := os.WriteFile(filepath.Join(outDir, "robots.txt"), []byte(robotsTxt), 0o644); err != nil {
		return fmt.Errorf("writing robots.txt: %w", err)
	}

	// Write static assets (JS, CSS)
	if err := theme.WriteAssets(outDir); err != nil {
		return fmt.Errorf("writing assets: %w", err)
	}

	return nil
}

type pageData struct {
	Site     siteData
	Page     pageContent
	Category *content.Category
}

type pageContent struct {
	Title       string
	Type        string
	Slug        string
	Description string
	HTML        template.HTML
	Headings    []heading
}

type heading struct {
	ID    string
	Text  string
	Level int // 2 or 3
}

type siteData struct {
	Title       string
	Description string
	BaseURL     string
	RepoURL     string
	Nav         []config.NavItem
	Categories  []*content.Category
	Pathways    []config.Pathway
}

type homepageData struct {
	Site siteData
}

func newSiteData(site *content.Site, cfg *config.Config) siteData {
	return siteData{
		Title:       cfg.Title,
		Description: cfg.Description,
		BaseURL:     cfg.BaseURL,
		RepoURL:     cfg.RepoURL,
		Nav:         cfg.Nav,
		Categories:  site.Categories,
		Pathways:    cfg.Pathways,
	}
}

func buildPage(md goldmark.Markdown, tmpl *theme.Templates, site *content.Site, cfg *config.Config, page *content.Page, outDir string) error {
	var buf []byte
	w := newBytesWriter(&buf)
	if err := md.Convert(page.Body, w); err != nil {
		return fmt.Errorf("converting markdown: %w", err)
	}

	pageDir := filepath.Join(outDir, page.Slug)
	if err := os.MkdirAll(pageDir, 0o755); err != nil {
		return err
	}

	outPath := filepath.Join(pageDir, "index.html")
	f, err := os.Create(outPath)
	if err != nil {
		return err
	}
	defer f.Close()

	var cat *content.Category
	for _, c := range site.Categories {
		if c.Name == page.Category {
			cat = c
			break
		}
	}

	renderedHTML := string(buf)
	renderedHTML = resolvePageLinks(renderedHTML, site)
	renderedHTML = transformAdmonitions(renderedHTML)
	renderedHTML = injectAnchorLinks(renderedHTML)
	data := pageData{
		Site: newSiteData(site, cfg),
		Page: pageContent{
			Title:       page.Title,
			Type:        page.Type,
			Slug:        page.Slug,
			Description: page.Description,
			HTML:        template.HTML(renderedHTML),
			Headings:    extractHeadings(renderedHTML),
		},
		Category: cat,
	}

	return tmpl.Page.ExecuteTemplate(f, "base", data)
}

type categoryData struct {
	Site     siteData
	Category *content.Category
}

func buildCategoryIndex(tmpl *theme.Templates, site *content.Site, cfg *config.Config, cat *content.Category, outDir string) error {
	catDir := filepath.Join(outDir, cat.Name)
	if err := os.MkdirAll(catDir, 0o755); err != nil {
		return err
	}

	outPath := filepath.Join(catDir, "index.html")
	f, err := os.Create(outPath)
	if err != nil {
		return err
	}
	defer f.Close()

	data := categoryData{
		Site:     newSiteData(site, cfg),
		Category: cat,
	}

	return tmpl.Category.ExecuteTemplate(f, "base", data)
}

func build404(tmpl *theme.Templates, site *content.Site, cfg *config.Config, outDir string) error {
	outPath := filepath.Join(outDir, "404.html")
	f, err := os.Create(outPath)
	if err != nil {
		return err
	}
	defer f.Close()

	data := homepageData{
		Site: newSiteData(site, cfg),
	}

	return tmpl.NotFound.ExecuteTemplate(f, "base", data)
}

func buildHomepage(tmpl *theme.Templates, site *content.Site, cfg *config.Config, outDir string) error {
	outPath := filepath.Join(outDir, "index.html")
	f, err := os.Create(outPath)
	if err != nil {
		return err
	}
	defer f.Close()

	data := homepageData{
		Site: newSiteData(site, cfg),
	}

	return tmpl.Homepage.ExecuteTemplate(f, "base", data)
}

func buildPathwayManifest(site *content.Site, cfg *config.Config, outDir string) error {
	type pageRef struct {
		Slug  string `json:"slug"`
		Title string `json:"title"`
	}
	type pathwayEntry struct {
		Name        string    `json:"name"`
		Slug        string    `json:"slug"`
		Description string    `json:"description"`
		Pages       []pageRef `json:"pages"`
	}

	var entries []pathwayEntry
	for _, p := range cfg.Pathways {
		refs := make([]pageRef, 0, len(p.Pages))
		for _, slug := range p.Pages {
			title := slug
			if page, ok := site.PageMap[slug]; ok {
				title = page.Title
			}
			refs = append(refs, pageRef{Slug: slug, Title: title})
		}
		entries = append(entries, pathwayEntry{
			Name:        p.Name,
			Slug:        p.Slug,
			Description: p.Description,
			Pages:       refs,
		})
	}

	data, err := json.MarshalIndent(entries, "", "  ")
	if err != nil {
		return err
	}

	return os.WriteFile(filepath.Join(outDir, "pathways.json"), data, 0o644)
}

func buildSearchIndex(site *content.Site, outDir string) error {
	type searchEntry struct {
		Title       string `json:"title"`
		Slug        string `json:"slug"`
		Category    string `json:"category"`
		Type        string `json:"type"`
		Description string `json:"description,omitempty"`
		Content     string `json:"content"`
	}

	entries := make([]searchEntry, 0, len(site.Pages))
	for _, page := range site.Pages {
		// Strip markdown formatting for plain text content
		text := tagStripRe.ReplaceAllString(string(page.Body), "")
		text = strings.Join(strings.Fields(text), " ")
		// Truncate to keep index size reasonable
		if len(text) > 500 {
			text = text[:500]
		}
		entries = append(entries, searchEntry{
			Title:       page.Title,
			Slug:        page.Slug,
			Category:    page.Category,
			Type:        page.Type,
			Description: page.Description,
			Content:     text,
		})
	}

	data, err := json.Marshal(entries)
	if err != nil {
		return err
	}

	return os.WriteFile(filepath.Join(outDir, "search-index.json"), data, 0o644)
}

func buildSitemap(site *content.Site, cfg *config.Config, outDir string) error {
	baseURL := strings.TrimRight(cfg.BaseURL, "/")
	var b strings.Builder
	b.WriteString(`<?xml version="1.0" encoding="UTF-8"?>` + "\n")
	b.WriteString(`<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">` + "\n")

	// Homepage
	b.WriteString("  <url><loc>" + baseURL + "/</loc></url>\n")

	// Category pages
	for _, cat := range site.Categories {
		b.WriteString("  <url><loc>" + baseURL + "/" + cat.Name + "/</loc></url>\n")
	}

	// Content pages
	for _, page := range site.Pages {
		b.WriteString("  <url><loc>" + baseURL + "/" + page.Slug + "/</loc></url>\n")
	}

	b.WriteString("</urlset>\n")
	return os.WriteFile(filepath.Join(outDir, "sitemap.xml"), []byte(b.String()), 0o644)
}

var pageLinkRe = regexp.MustCompile(`href="~([^"]+)"`)

func resolvePageLinks(html string, site *content.Site) string {
	return pageLinkRe.ReplaceAllStringFunc(html, func(match string) string {
		sub := pageLinkRe.FindStringSubmatch(match)
		if len(sub) < 2 {
			return match
		}
		slug := sub[1]
		if _, ok := site.PageMap[slug]; ok {
			return `href="/` + slug + `/"`
		}
		// Leave unresolved links as-is (will 404, author can fix)
		return `href="/` + slug + `/"`
	})
}

var admonitionRe = regexp.MustCompile(`(?s)<blockquote>\s*<p>\[!(NOTE|WARNING|IMPORTANT|TIP|CAUTION)\]\s*\n?(.*?)</p>\s*</blockquote>`)

var admonitionStyles = map[string]struct{ icon, border, bg, title string }{
	"NOTE":      {"&#8505;", "border-brand-400 dark:border-brand-600", "bg-brand-50 dark:bg-brand-900/30", "Note"},
	"TIP":       {"&#128161;", "border-green-400 dark:border-green-600", "bg-green-50 dark:bg-green-900/30", "Tip"},
	"IMPORTANT": {"&#10071;", "border-purple-400 dark:border-purple-600", "bg-purple-50 dark:bg-purple-900/30", "Important"},
	"WARNING":   {"&#9888;", "border-yellow-400 dark:border-yellow-600", "bg-yellow-50 dark:bg-yellow-900/30", "Warning"},
	"CAUTION":   {"&#9888;", "border-red-400 dark:border-red-600", "bg-red-50 dark:bg-red-900/30", "Caution"},
}

func transformAdmonitions(html string) string {
	return admonitionRe.ReplaceAllStringFunc(html, func(match string) string {
		sub := admonitionRe.FindStringSubmatch(match)
		if len(sub) < 3 {
			return match
		}
		kind := sub[1]
		content := strings.TrimSpace(sub[2])
		style, ok := admonitionStyles[kind]
		if !ok {
			return match
		}
		return `<div class="not-prose my-4 border-l-4 ` + style.border + ` ` + style.bg + ` rounded-r-lg p-4">` +
			`<div class="font-semibold text-sm mb-1">` + style.icon + ` ` + style.title + `</div>` +
			`<div class="text-sm text-gray-700 dark:text-gray-300">` + content + `</div></div>`
	})
}

var anchorRe = regexp.MustCompile(`(<h[23]\s+id="([^"]+)"[^>]*>)(.*?)(</h[23]>)`)

func injectAnchorLinks(html string) string {
	return anchorRe.ReplaceAllString(html, `${1}${3} <a href="#${2}" class="anchor" aria-hidden="true">#</a>${4}`)
}

var headingRe = regexp.MustCompile(`<h([23])\s+id="([^"]+)"[^>]*>(.*?)</h[23]>`)
var tagStripRe = regexp.MustCompile(`<[^>]+>`)

func extractHeadings(html string) []heading {
	matches := headingRe.FindAllStringSubmatch(html, -1)
	headings := make([]heading, 0, len(matches))
	for _, m := range matches {
		level := 2
		if m[1] == "3" {
			level = 3
		}
		text := tagStripRe.ReplaceAllString(m[3], "")
		text = strings.TrimSpace(text)
		headings = append(headings, heading{
			ID:    m[2],
			Text:  text,
			Level: level,
		})
	}
	return headings
}

// bytesWriter implements io.Writer by appending to a byte slice.
type bytesWriter struct {
	buf *[]byte
}

func newBytesWriter(buf *[]byte) *bytesWriter {
	return &bytesWriter{buf: buf}
}

func (w *bytesWriter) Write(p []byte) (int, error) {
	*w.buf = append(*w.buf, p...)
	return len(p), nil
}
