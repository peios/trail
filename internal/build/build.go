package build

import (
	"encoding/json"
	"fmt"
	"html/template"
	"io"
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

func Build(site *content.Site, cfg *config.Config, srcDir, outDir string) error {
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

	// Build product index pages
	for _, prod := range site.Products {
		if err := buildProductIndex(tmpl, site, cfg, prod, outDir); err != nil {
			return fmt.Errorf("building product %s: %w", prod.Slug, err)
		}
	}

	// Build spec version redirects (e.g. /kacs/ → /kacs/v0.20/)
	if err := buildSpecRedirects(cfg, outDir); err != nil {
		return fmt.Errorf("building spec redirects: %w", err)
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

	// Build pathways pages (per-product or global)
	if len(site.Products) > 0 {
		for _, prod := range site.Products {
			if len(prod.Pathways) > 0 {
				if err := buildProductPathwaysPage(tmpl, site, cfg, prod, outDir); err != nil {
					return fmt.Errorf("building pathways for %s: %w", prod.Slug, err)
				}
			}
		}
	} else {
		if err := buildPathwaysPage(tmpl, site, cfg, outDir); err != nil {
			return fmt.Errorf("building pathways page: %w", err)
		}
	}

	// Build 404 page
	if err := build404(tmpl, site, cfg, outDir); err != nil {
		return fmt.Errorf("building 404: %w", err)
	}

	// Build print-all pages
	if len(site.Products) > 0 {
		// Per-product print pages
		for _, prod := range site.Products {
			if err := buildProductPrintAll(md, tmpl, site, cfg, prod, outDir); err != nil {
				return fmt.Errorf("building print for %s: %w", prod.Slug, err)
			}
		}
		// Global print page grouped by product
		if err := buildGlobalPrintAll(md, tmpl, site, cfg, outDir); err != nil {
			return fmt.Errorf("building global print page: %w", err)
		}
	} else {
		if err := buildPrintAll(md, tmpl, site, cfg, outDir); err != nil {
			return fmt.Errorf("building print page: %w", err)
		}
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

	// Copy static directory if it exists
	if err := copyStatic(srcDir, outDir); err != nil {
		return fmt.Errorf("copying static assets: %w", err)
	}

	// Write static assets (JS, CSS)
	if err := theme.WriteAssets(outDir); err != nil {
		return fmt.Errorf("writing assets: %w", err)
	}

	// Validate internal links
	validateLinks(site, outDir)

	return nil
}

type pageData struct {
	Site     siteData
	Page     pageContent
	Category *content.Category
	Product  *content.Product
}

type pageContent struct {
	Title        string
	Type         string
	Slug         string
	Description  string
	Updated      string
	SectionNum   string
	IsSpecCover  bool // first page of a spec product
	ReadingTime  int  // minutes
	HTML         template.HTML
	Headings     []heading
	Related      []relatedPage
}

type relatedPage struct {
	Title string
	Slug  string
}

type heading struct {
	ID         string
	Text       string
	Level      int // 2 or 3
	SectionNum string // e.g. "2.1.1" — set for spec pages
}

type siteData struct {
	Title        string
	Description  string
	BaseURL      string
	BasePath     string
	RepoURL      string
	Favicon      string
	HeadExtra    template.HTML
	Announcement string
	Nav          []config.NavItem
	Products     []*content.Product
	Categories   []*content.Category
	Pathways     []config.Pathway
}

type homepageData struct {
	Site siteData
}

func newSiteData(site *content.Site, cfg *config.Config) siteData {
	return siteData{
		Title:        cfg.Title,
		Description:  cfg.Description,
		BaseURL:      cfg.BaseURL,
		BasePath:     cfg.BasePath(),
		RepoURL:      cfg.RepoURL,
		Favicon:      cfg.Favicon,
		HeadExtra:    template.HTML(cfg.HeadExtra),
		Announcement: cfg.Announcement,
		Nav:          cfg.Nav,
		Products:     site.Products,
		Categories:   site.Categories,
		Pathways:     cfg.AllPathways(),
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
		if c.Name == page.Category && c.ProductSlug == page.ProductSlug {
			cat = c
			break
		}
	}

	var prod *content.Product
	for _, p := range site.Products {
		if p.Slug == page.ProductSlug {
			prod = p
			break
		}
	}

	isSpec := prod != nil && prod.Kind == "spec"

	renderedHTML := string(buf)
	renderedHTML = resolvePageLinks(renderedHTML, site, cfg.BasePath())
	renderedHTML = transformAdmonitions(renderedHTML)
	if isSpec {
		renderedHTML = highlightRFCKeywords(renderedHTML)
	}
	renderedHTML = wrapTables(renderedHTML)
	renderedHTML = transformTabGroups(renderedHTML)
	renderedHTML = transformMermaid(renderedHTML)
	headings := extractHeadings(renderedHTML)
	if isSpec && page.SectionNum != "" {
		headings = assignHeadingSectionNums(headings, page.SectionNum)
		renderedHTML = injectSectionNumbers(renderedHTML, headings)
	}
	renderedHTML = injectAnchorLinks(renderedHTML)
	isSpecCover := isSpec && prod != nil && len(prod.Categories) > 0 &&
		len(prod.Categories[0].Pages) > 0 && prod.Categories[0].Pages[0].Slug == page.Slug

	data := pageData{
		Site: newSiteData(site, cfg),
		Page: pageContent{
			Title:       page.Title,
			Type:        page.Type,
			Slug:        page.Slug,
			Description: page.Description,
			Updated:     page.Updated,
			SectionNum:  page.SectionNum,
			IsSpecCover: isSpecCover,
			ReadingTime: wordCount(page.Body) / 200,
			HTML:        template.HTML(renderedHTML),
			Headings:    headings,
			Related:     resolveRelated(page.Related, site),
		},
		Category: cat,
		Product:  prod,
	}

	t := tmpl.Page
	if isSpec {
		t = tmpl.SpecPage
	}
	return t.ExecuteTemplate(f, "base", data)
}

type categoryData struct {
	Site     siteData
	Category *content.Category
	Product  *content.Product
}

type productData struct {
	Site    siteData
	Product *content.Product
}

func buildProductIndex(tmpl *theme.Templates, site *content.Site, cfg *config.Config, prod *content.Product, outDir string) error {
	prodDir := filepath.Join(outDir, prod.Slug)
	if err := os.MkdirAll(prodDir, 0o755); err != nil {
		return err
	}

	// Spec products redirect to first page instead of rendering an index
	if prod.Kind == "spec" && len(prod.Categories) > 0 && len(prod.Categories[0].Pages) > 0 {
		firstPage := prod.Categories[0].Pages[0]
		target := cfg.BasePath() + firstPage.Slug + "/"
		redirectHTML := `<!DOCTYPE html><html><head><script>location.replace('` + target + `')</script><link rel="canonical" href="` + target + `"><noscript><meta http-equiv="refresh" content="0;url=` + target + `"></noscript></head><body><a href="` + target + `">` + firstPage.Title + `</a></body></html>`
		return os.WriteFile(filepath.Join(prodDir, "index.html"), []byte(redirectHTML), 0o644)
	}

	outPath := filepath.Join(prodDir, "index.html")
	f, err := os.Create(outPath)
	if err != nil {
		return err
	}
	defer f.Close()

	data := productData{
		Site:    newSiteData(site, cfg),
		Product: prod,
	}

	return tmpl.ProductPage.ExecuteTemplate(f, "base", data)
}

func buildCategoryIndex(tmpl *theme.Templates, site *content.Site, cfg *config.Config, cat *content.Category, outDir string) error {
	catDir := outDir
	if cat.ProductSlug != "" {
		catDir = filepath.Join(outDir, cat.ProductSlug, cat.Name)
	} else {
		catDir = filepath.Join(outDir, cat.Name)
	}
	if err := os.MkdirAll(catDir, 0o755); err != nil {
		return err
	}

	outPath := filepath.Join(catDir, "index.html")
	f, err := os.Create(outPath)
	if err != nil {
		return err
	}
	defer f.Close()

	var prod *content.Product
	for _, p := range site.Products {
		if p.Slug == cat.ProductSlug {
			prod = p
			break
		}
	}

	data := categoryData{
		Site:     newSiteData(site, cfg),
		Category: cat,
		Product:  prod,
	}

	return tmpl.Category.ExecuteTemplate(f, "base", data)
}

func buildPathwaysPage(tmpl *theme.Templates, site *content.Site, cfg *config.Config, outDir string) error {
	pathwaysDir := filepath.Join(outDir, "pathways")
	if err := os.MkdirAll(pathwaysDir, 0o755); err != nil {
		return err
	}

	outPath := filepath.Join(pathwaysDir, "index.html")
	f, err := os.Create(outPath)
	if err != nil {
		return err
	}
	defer f.Close()

	data := homepageData{
		Site: newSiteData(site, cfg),
	}

	return tmpl.PathwaysPage.ExecuteTemplate(f, "base", data)
}

type productPathwaysData struct {
	Site    siteData
	Product *content.Product
}

func buildProductPathwaysPage(tmpl *theme.Templates, site *content.Site, cfg *config.Config, prod *content.Product, outDir string) error {
	pathwaysDir := filepath.Join(outDir, prod.Slug, "pathways")
	if err := os.MkdirAll(pathwaysDir, 0o755); err != nil {
		return err
	}

	outPath := filepath.Join(pathwaysDir, "index.html")
	f, err := os.Create(outPath)
	if err != nil {
		return err
	}
	defer f.Close()

	data := productPathwaysData{
		Site:    newSiteData(site, cfg),
		Product: prod,
	}

	return tmpl.ProductPathwaysPage.ExecuteTemplate(f, "base", data)
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

type printPageEntry struct {
	Title    string
	Type     string
	Category string
	HTML     template.HTML
}

type printData struct {
	Site        siteData
	ProductName string
	Pages       []printPageEntry
	Sections    []printSection // for multi-product global print
}

type printSection struct {
	Name  string
	Pages []printPageEntry
}

func buildPrintAll(md goldmark.Markdown, tmpl *theme.Templates, site *content.Site, cfg *config.Config, outDir string) error {
	printDir := filepath.Join(outDir, "print")
	if err := os.MkdirAll(printDir, 0o755); err != nil {
		return err
	}

	outPath := filepath.Join(printDir, "index.html")
	f, err := os.Create(outPath)
	if err != nil {
		return err
	}
	defer f.Close()

	var pages []printPageEntry
	for _, page := range site.Pages {
		var buf []byte
		w := newBytesWriter(&buf)
		if err := md.Convert(page.Body, w); err != nil {
			continue
		}
		pages = append(pages, printPageEntry{
			Title:    page.Title,
			Type:     page.Type,
			Category: page.Category,
			HTML:     template.HTML(buf),
		})
	}

	data := printData{
		Site:  newSiteData(site, cfg),
		Pages: pages,
	}

	return tmpl.Print.ExecuteTemplate(f, "base", data)
}

func buildGlobalPrintAll(md goldmark.Markdown, tmpl *theme.Templates, site *content.Site, cfg *config.Config, outDir string) error {
	printDir := filepath.Join(outDir, "print")
	if err := os.MkdirAll(printDir, 0o755); err != nil {
		return err
	}

	outPath := filepath.Join(printDir, "index.html")
	f, err := os.Create(outPath)
	if err != nil {
		return err
	}
	defer f.Close()

	var sections []printSection
	for _, prod := range site.Products {
		var pages []printPageEntry
		for _, page := range prod.Pages {
			var buf []byte
			w := newBytesWriter(&buf)
			if err := md.Convert(page.Body, w); err != nil {
				continue
			}
			pages = append(pages, printPageEntry{
				Title:    page.Title,
				Type:     page.Type,
				Category: page.Category,
				HTML:     template.HTML(buf),
			})
		}
		sections = append(sections, printSection{
			Name:  prod.Name,
			Pages: pages,
		})
	}

	data := printData{
		Site:     newSiteData(site, cfg),
		Sections: sections,
	}

	return tmpl.PrintGlobal.ExecuteTemplate(f, "base", data)
}

func buildProductPrintAll(md goldmark.Markdown, tmpl *theme.Templates, site *content.Site, cfg *config.Config, prod *content.Product, outDir string) error {
	printDir := filepath.Join(outDir, prod.Slug, "print")
	if err := os.MkdirAll(printDir, 0o755); err != nil {
		return err
	}

	outPath := filepath.Join(printDir, "index.html")
	f, err := os.Create(outPath)
	if err != nil {
		return err
	}
	defer f.Close()

	var pages []printPageEntry
	for _, page := range prod.Pages {
		var buf []byte
		w := newBytesWriter(&buf)
		if err := md.Convert(page.Body, w); err != nil {
			continue
		}
		pages = append(pages, printPageEntry{
			Title:    page.Title,
			Type:     page.Type,
			Category: page.Category,
			HTML:     template.HTML(buf),
		})
	}

	data := printData{
		Site:        newSiteData(site, cfg),
		ProductName: prod.Name,
		Pages:       pages,
	}

	return tmpl.Print.ExecuteTemplate(f, "base", data)
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
	for _, p := range cfg.AllPathways() {
		refs := make([]pageRef, 0, len(p.Pages))
		for _, slug := range p.Pages {
			// Try the slug as-is, then prefixed with product
			fullSlug := slug
			if p.Product != "" {
				fullSlug = p.Product + "/" + slug
			}
			title := slug
			if page, ok := site.PageMap[fullSlug]; ok {
				title = page.Title
			}
			refs = append(refs, pageRef{Slug: fullSlug, Title: title})
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

func wordCount(body []byte) int {
	return len(strings.Fields(string(body)))
}

func resolveRelated(slugs []string, site *content.Site) []relatedPage {
	if len(slugs) == 0 {
		return nil
	}
	pages := make([]relatedPage, 0, len(slugs))
	for _, slug := range slugs {
		title := slug
		if p, ok := site.PageMap[slug]; ok {
			title = p.Title
		}
		pages = append(pages, relatedPage{Title: title, Slug: slug})
	}
	return pages
}

var pageLinkRe = regexp.MustCompile(`href="~([^"]+)"`)

func resolvePageLinks(html string, site *content.Site, basePath string) string {
	return pageLinkRe.ReplaceAllStringFunc(html, func(match string) string {
		sub := pageLinkRe.FindStringSubmatch(match)
		if len(sub) < 2 {
			return match
		}
		slug := sub[1]
		return `href="` + basePath + slug + `/"`
	})
}

func assignHeadingSectionNums(headings []heading, pageBase string) []heading {
	h2count := 0
	h3count := 0
	for i := range headings {
		if headings[i].Level == 2 {
			h2count++
			h3count = 0
			headings[i].SectionNum = fmt.Sprintf("%s.%d", pageBase, h2count)
		} else if headings[i].Level == 3 {
			h3count++
			headings[i].SectionNum = fmt.Sprintf("%s.%d.%d", pageBase, h2count, h3count)
		}
	}
	return headings
}

var sectionHeadingRe = regexp.MustCompile(`(<h([23])\s+id="([^"]*)"[^>]*>)`)

func injectSectionNumbers(html string, headings []heading) string {
	headingIdx := 0
	return sectionHeadingRe.ReplaceAllStringFunc(html, func(match string) string {
		if headingIdx >= len(headings) {
			return match
		}
		h := headings[headingIdx]
		headingIdx++
		// Replace the id with the section number and add a section number span
		sub := sectionHeadingRe.FindStringSubmatch(match)
		if len(sub) < 4 {
			return match
		}
		level := sub[2]
		// Rewrite the tag with section number id and visible prefix
		return `<h` + level + ` id="` + h.SectionNum + `">` +
			`<span class="section-num">§` + h.SectionNum + `</span> `
	})
}

func buildSpecRedirects(cfg *config.Config, outDir string) error {
	basePath := cfg.BasePath()
	for _, prod := range cfg.Products {
		if !prod.IsSpec() || len(prod.Versions) == 0 {
			continue
		}
		// Find current version (latest non-superseded)
		var current string
		for i := len(prod.Versions) - 1; i >= 0; i-- {
			if prod.Versions[i].Status != "superseded" {
				current = prod.Versions[i].Name
				break
			}
		}
		if current == "" {
			current = prod.Versions[len(prod.Versions)-1].Name
		}

		// Write redirect page at /spec/slug/index.html
		redirectDir := filepath.Join(outDir, "spec", prod.Slug)
		if err := os.MkdirAll(redirectDir, 0o755); err != nil {
			return err
		}
		target := basePath + "spec/" + prod.Slug + "/" + current + "/"
		redirectHTML := `<!DOCTYPE html><html><head><script>location.replace('` + target + `')</script><link rel="canonical" href="` + target + `"><noscript><meta http-equiv="refresh" content="0;url=` + target + `"></noscript></head><body><a href="` + target + `">` + current + `</a></body></html>`
		if err := os.WriteFile(filepath.Join(redirectDir, "index.html"), []byte(redirectHTML), 0o644); err != nil {
			return err
		}
	}
	return nil
}

var rfcKeywordRe = regexp.MustCompile(`\b(MUST NOT|SHALL NOT|SHOULD NOT|MUST|SHALL|SHOULD|REQUIRED|MAY|OPTIONAL)\b`)

func highlightRFCKeywords(html string) string {
	// Don't highlight inside HTML tags or code blocks
	var result strings.Builder
	inTag := false
	inCode := false
	i := 0
	for i < len(html) {
		if html[i] == '<' {
			// Check for code/pre opening/closing
			rest := html[i:]
			if strings.HasPrefix(rest, "<code") || strings.HasPrefix(rest, "<pre") {
				inCode = true
			} else if strings.HasPrefix(rest, "</code") || strings.HasPrefix(rest, "</pre") {
				inCode = false
			}
			inTag = true
			result.WriteByte(html[i])
			i++
			continue
		}
		if html[i] == '>' {
			inTag = false
			result.WriteByte(html[i])
			i++
			continue
		}
		if inTag || inCode {
			result.WriteByte(html[i])
			i++
			continue
		}
		// Try to match RFC keyword at current position
		loc := rfcKeywordRe.FindStringIndex(html[i:])
		if loc != nil && loc[0] == 0 {
			keyword := html[i : i+loc[1]]
			result.WriteString(`<span class="rfc-keyword">` + keyword + `</span>`)
			i += loc[1]
		} else {
			result.WriteByte(html[i])
			i++
		}
	}
	return result.String()
}

var admonitionRe = regexp.MustCompile(`(?s)<blockquote>\s*<p>\[!(NOTE|WARNING|IMPORTANT|TIP|CAUTION|INFORMATIVE|DEFINITION)\]\s*\n?(.*?)</p>\s*</blockquote>`)

var admonitionStyles = map[string]struct{ icon, border, bg, title string }{
	"NOTE":        {"&#8505;", "border-brand-400 dark:border-brand-600", "bg-brand-50 dark:bg-brand-900/30", "Note"},
	"TIP":         {"&#128161;", "border-green-400 dark:border-green-600", "bg-green-50 dark:bg-green-900/30", "Tip"},
	"IMPORTANT":   {"&#10071;", "border-purple-400 dark:border-purple-600", "bg-purple-50 dark:bg-purple-900/30", "Important"},
	"WARNING":     {"&#9888;", "border-yellow-400 dark:border-yellow-600", "bg-yellow-50 dark:bg-yellow-900/30", "Warning"},
	"CAUTION":     {"&#9888;", "border-red-400 dark:border-red-600", "bg-red-50 dark:bg-red-900/30", "Caution"},
	"INFORMATIVE": {"&#9432;", "border-gray-300 dark:border-gray-600", "bg-gray-50 dark:bg-gray-900/30", "Informative"},
	"DEFINITION":  {"&#8801;", "border-brand-300 dark:border-brand-700", "bg-white dark:bg-gray-900", "Definition"},
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

var tabGroupRe = regexp.MustCompile(`(?s)<!-- tabs -->(.*?)<!-- /tabs -->`)
var tabMarkerRe = regexp.MustCompile(`<!-- tab:(.+?) -->`)

var tabGroupCounter int

func transformTabGroups(html string) string {
	return tabGroupRe.ReplaceAllStringFunc(html, func(match string) string {
		// Extract inner content (between <!-- tabs --> and <!-- /tabs -->)
		inner := tabGroupRe.FindStringSubmatch(match)
		if len(inner) < 2 {
			return match
		}
		body := inner[1]

		// Split on tab markers
		markers := tabMarkerRe.FindAllStringSubmatchIndex(body, -1)
		if len(markers) == 0 {
			return match
		}

		type tab struct {
			name    string
			content string
		}
		var tabs []tab
		for i, m := range markers {
			name := body[m[2]:m[3]]
			contentStart := m[1]
			var contentEnd int
			if i+1 < len(markers) {
				contentEnd = markers[i+1][0]
			} else {
				contentEnd = len(body)
			}
			tabs = append(tabs, tab{
				name:    strings.TrimSpace(name),
				content: strings.TrimSpace(body[contentStart:contentEnd]),
			})
		}

		tabGroupCounter++
		groupID := fmt.Sprintf("tabgroup-%d", tabGroupCounter)

		var b strings.Builder
		b.WriteString(`<div class="not-prose my-6 border border-gray-200 dark:border-gray-700 rounded-lg overflow-hidden" data-tab-group="` + groupID + `">`)
		b.WriteString(`<div class="flex border-b border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800">`)

		for i, t := range tabs {
			active := ""
			if i == 0 {
				active = " tab-active"
			}
			b.WriteString(`<button class="px-4 py-2 text-sm font-medium border-b-2 -mb-px transition-colors` + active + `" data-tab="` + groupID + `-` + fmt.Sprint(i) + `">` + t.name + `</button>`)
		}
		b.WriteString(`</div>`)

		for i, t := range tabs {
			hidden := ""
			if i > 0 {
				hidden = " hidden"
			}
			b.WriteString(`<div class="` + hidden + `" data-tab-panel="` + groupID + `-` + fmt.Sprint(i) + `">` + t.content + `</div>`)
		}
		b.WriteString(`</div>`)
		return b.String()
	})
}

var tableRe = regexp.MustCompile(`(?s)(<table>.*?</table>)`)

func wrapTables(html string) string {
	return tableRe.ReplaceAllString(html, `<div class="table-wrapper">$1</div>`)
}

var mermaidRe = regexp.MustCompile(`(?s)<pre[^>]*><code class="language-mermaid">(.*?)</code></pre>`)

func transformMermaid(html string) string {
	return mermaidRe.ReplaceAllStringFunc(html, func(match string) string {
		sub := mermaidRe.FindStringSubmatch(match)
		if len(sub) < 2 {
			return match
		}
		// The content may have HTML entities from goldmark
		content := sub[1]
		return `<div class="mermaid my-4">` + content + `</div>`
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

var internalLinkRe = regexp.MustCompile(`href="(/[^"]*?)"`)

func validateLinks(site *content.Site, outDir string) {
	var broken []string

	filepath.Walk(outDir, func(path string, info os.FileInfo, err error) error {
		if err != nil || info.IsDir() || filepath.Ext(path) != ".html" {
			return nil
		}

		data, err := os.ReadFile(path)
		if err != nil {
			return nil
		}

		rel, _ := filepath.Rel(outDir, path)
		matches := internalLinkRe.FindAllStringSubmatch(string(data), -1)
		for _, m := range matches {
			href := m[1]

			// Skip asset/special paths
			if strings.HasPrefix(href, "/assets/") || href == "/" {
				continue
			}

			// Strip query params and hash
			clean := strings.SplitN(href, "?", 2)[0]
			clean = strings.SplitN(clean, "#", 2)[0]

			// Check if the target exists
			target := filepath.Join(outDir, clean)
			if _, err := os.Stat(target); err == nil {
				continue
			}
			// Try as directory with index.html
			if _, err := os.Stat(filepath.Join(target, "index.html")); err == nil {
				continue
			}
			// Try without trailing slash
			trimmed := strings.TrimRight(target, "/")
			if _, err := os.Stat(filepath.Join(trimmed, "index.html")); err == nil {
				continue
			}

			broken = append(broken, fmt.Sprintf("  %s → %s", rel, href))
		}
		return nil
	})

	if len(broken) > 0 {
		fmt.Printf("\nWarning: %d broken internal link(s):\n", len(broken))
		for _, b := range broken {
			fmt.Println(b)
		}
		fmt.Println()
	}
}

func copyStatic(srcDir, outDir string) error {
	staticDir := filepath.Join(srcDir, "static")
	info, err := os.Stat(staticDir)
	if os.IsNotExist(err) {
		return nil
	}
	if err != nil {
		return err
	}
	if !info.IsDir() {
		return nil
	}

	return filepath.Walk(staticDir, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return err
		}

		rel, err := filepath.Rel(staticDir, path)
		if err != nil {
			return err
		}

		dest := filepath.Join(outDir, rel)
		if info.IsDir() {
			return os.MkdirAll(dest, 0o755)
		}

		src, err := os.Open(path)
		if err != nil {
			return err
		}
		defer src.Close()

		dst, err := os.Create(dest)
		if err != nil {
			return err
		}
		defer dst.Close()

		_, err = io.Copy(dst, src)
		return err
	})
}
