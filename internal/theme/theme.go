package theme

import (
	"fmt"
	"html/template"
	"os"
	"path/filepath"
	"strings"

	"github.com/peios/trail/internal/config"
	"github.com/peios/trail/internal/content"
)

type Templates struct {
	Page                *template.Template
	SpecPage            *template.Template
	Homepage            *template.Template
	Category            *template.Template
	NotFound            *template.Template
	Print               *template.Template
	PrintGlobal         *template.Template
	PathwaysPage        *template.Template
	ProductPage         *template.Template
	ProductPathwaysPage *template.Template
	SpecProductPage     *template.Template
}

func LoadTemplates(cfg *config.Config) (*Templates, error) {
	basePath := cfg.BasePath()
	funcMap := template.FuncMap{
		"bp": func(path string) string {
			if strings.HasPrefix(path, "http://") || strings.HasPrefix(path, "https://") {
				return path
			}
			return basePath + strings.TrimPrefix(path, "/")
		},
		"catPath": func(productSlug, name string) template.URL {
			if productSlug != "" {
				return template.URL(basePath + productSlug + "/" + name + "/")
			}
			return template.URL(basePath + name + "/")
		},
		"pathwayURL": func(product, page, pathwaySlug string) template.URL {
			p := basePath
			if product != "" {
				p += product + "/"
			}
			p += page + "/?pathway=" + pathwaySlug
			return template.URL(p)
		},
		"firstPageSlug": func(prod *content.Product) string {
			if len(prod.Categories) > 0 && len(prod.Categories[0].Pages) > 0 {
				return prod.Categories[0].Pages[0].Slug
			}
			return prod.Slug
		},
		"docsProducts": func(products []*content.Product) []*content.Product {
			var out []*content.Product
			for _, p := range products {
				if p.Kind != "spec" {
					out = append(out, p)
				}
			}
			return out
		},
		"specProducts": func(products []*content.Product) []*content.Product {
			var out []*content.Product
			seen := make(map[string]bool)
			for _, p := range products {
				if p.Kind == "spec" {
					// Deduplicate versioned specs — show only once using base name
					baseName := p.Name
					if !seen[baseName] {
						seen[baseName] = true
						out = append(out, p)
					}
				}
			}
			return out
		},
		"firstN": func(n int, pages []*content.Page) []*content.Page {
			if len(pages) <= n {
				return pages
			}
			return pages[:n]
		},
		"gt": func(a, b int) bool {
			return a > b
		},
		"featuredPathways": func(pathways []config.Pathway) []config.Pathway {
			var out []config.Pathway
			for _, p := range pathways {
				if p.Featured {
					out = append(out, p)
				}
			}
			return out
		},
		"typeLabel": func(t string) string {
			switch t {
			case "concept":
				return "Concept"
			case "how-to":
				return "How-to"
			default:
				return t
			}
		},
		"typeIcon": func(t string) template.HTML {
			switch t {
			case "concept":
				return `<svg class="w-3.5 h-3.5 inline-block" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253"/></svg>`
			case "how-to":
				return `<svg class="w-3.5 h-3.5 inline-block" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M11.42 15.17l-5.384-3.107A2 2 0 005 13.894V20a2 2 0 002.828 0l2.586-2.586m5.006-5.844l5.384-3.107A2 2 0 0019 6.106V4a2 2 0 00-2.828 0L13.586 6.586m-5.172 5.172a2 2 0 112.828 2.828 2 2 0 01-2.828-2.828z"/></svg>`
			default:
				return ""
			}
		},
	}

	// Parse base template
	base, err := template.New("base").Funcs(funcMap).Parse(baseTemplate)
	if err != nil {
		return nil, fmt.Errorf("parsing base template: %w", err)
	}

	// Clone base and add page-specific blocks
	pageTmpl, err := template.Must(base.Clone()).Parse(pageTemplate)
	if err != nil {
		return nil, fmt.Errorf("parsing page template: %w", err)
	}

	// Clone base and add homepage-specific blocks
	homepageTmpl, err := template.Must(base.Clone()).Parse(homepageTemplate)
	if err != nil {
		return nil, fmt.Errorf("parsing homepage template: %w", err)
	}

	categoryTmpl, err := template.Must(base.Clone()).Parse(categoryTemplate)
	if err != nil {
		return nil, fmt.Errorf("parsing category template: %w", err)
	}

	notFoundTmpl, err := template.Must(base.Clone()).Parse(notFoundTemplate)
	if err != nil {
		return nil, fmt.Errorf("parsing 404 template: %w", err)
	}

	printTmpl, err := template.Must(base.Clone()).Parse(printTemplate)
	if err != nil {
		return nil, fmt.Errorf("parsing print template: %w", err)
	}

	printGlobalTmpl, err := template.Must(base.Clone()).Parse(printGlobalTemplate)
	if err != nil {
		return nil, fmt.Errorf("parsing global print template: %w", err)
	}

	pathwaysPageTmpl, err := template.Must(base.Clone()).Parse(pathwaysPageTemplate)
	if err != nil {
		return nil, fmt.Errorf("parsing pathways page template: %w", err)
	}

	productPageTmpl, err := template.Must(base.Clone()).Parse(productPageTemplate)
	if err != nil {
		return nil, fmt.Errorf("parsing product page template: %w", err)
	}

	productPathwaysPageTmpl, err := template.Must(base.Clone()).Parse(productPathwaysPageTemplate)
	if err != nil {
		return nil, fmt.Errorf("parsing product pathways page template: %w", err)
	}

	specPageTmpl, err := template.Must(base.Clone()).Parse(specPageTemplate)
	if err != nil {
		return nil, fmt.Errorf("parsing spec page template: %w", err)
	}

	specProductPageTmpl, err := template.Must(base.Clone()).Parse(specProductPageTemplate)
	if err != nil {
		return nil, fmt.Errorf("parsing spec product page template: %w", err)
	}

	return &Templates{
		Page:                pageTmpl,
		Homepage:            homepageTmpl,
		Category:            categoryTmpl,
		NotFound:            notFoundTmpl,
		Print:               printTmpl,
		PrintGlobal:         printGlobalTmpl,
		PathwaysPage:        pathwaysPageTmpl,
		ProductPage:         productPageTmpl,
		ProductPathwaysPage: productPathwaysPageTmpl,
		SpecPage:            specPageTmpl,
		SpecProductPage:     specProductPageTmpl,
	}, nil
}

func WriteAssets(outDir string) error {
	assetsDir := filepath.Join(outDir, "assets")
	if err := os.MkdirAll(assetsDir, 0o755); err != nil {
		return err
	}

	if err := os.WriteFile(filepath.Join(assetsDir, "pathway.js"), []byte(pathwayJS), 0o644); err != nil {
		return err
	}

	if err := os.WriteFile(filepath.Join(assetsDir, "theme.js"), []byte(themeJS), 0o644); err != nil {
		return err
	}

	if err := os.WriteFile(filepath.Join(assetsDir, "search.js"), []byte(searchJS), 0o644); err != nil {
		return err
	}

	if err := os.WriteFile(filepath.Join(assetsDir, "mobile.js"), []byte(mobileJS), 0o644); err != nil {
		return err
	}

	if err := os.WriteFile(filepath.Join(assetsDir, "copycode.js"), []byte(copyCodeJS), 0o644); err != nil {
		return err
	}

	if err := os.WriteFile(filepath.Join(assetsDir, "tabs.js"), []byte(tabsJS), 0o644); err != nil {
		return err
	}

	if err := os.WriteFile(filepath.Join(assetsDir, "scrollspy.js"), []byte(scrollSpyJS), 0o644); err != nil {
		return err
	}

	if err := os.WriteFile(filepath.Join(assetsDir, "backtotop.js"), []byte(backToTopJS), 0o644); err != nil {
		return err
	}

	if err := os.WriteFile(filepath.Join(assetsDir, "highlight.js"), []byte(highlightJS), 0o644); err != nil {
		return err
	}

	return os.WriteFile(filepath.Join(assetsDir, "fontsize.js"), []byte(fontSizeJS), 0o644)
}
