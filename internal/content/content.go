package content

import (
	"bytes"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"gopkg.in/yaml.v3"

	"github.com/peios/trail/internal/config"
)

type Site struct {
	Products   []*Product
	Pages      []*Page
	Categories []*Category
	PageMap    map[string]*Page // full slug → page
}

type Product struct {
	Name        string
	Slug        string
	Description string
	Pages       []*Page
	Categories  []*Category
	Pathways    []config.Pathway
}

type Page struct {
	Title       string   `yaml:"title"`
	Type        string   `yaml:"type"`
	Order       int      `yaml:"order"`
	Description string   `yaml:"description"`
	Updated     string   `yaml:"updated"`
	Draft       bool     `yaml:"draft"`
	Related     []string `yaml:"related"`
	Slug        string   // full slug: "peios/identity/how-tokens-work" or "identity/how-tokens-work"
	Category    string   // e.g. "identity"
	ProductSlug string   // e.g. "peios" (empty for single-product)
	Body        []byte
}

type Category struct {
	Name        string
	Title       string
	Order       int
	ProductSlug string
	Pages       []*Page
}

func Load(dir string, cfg *config.Config) (*Site, error) {
	site := &Site{
		PageMap: make(map[string]*Page),
	}

	if len(cfg.Products) > 0 {
		for _, prod := range cfg.Products {
			product, err := loadProduct(dir, cfg, prod)
			if err != nil {
				return nil, fmt.Errorf("loading product %s: %w", prod.Slug, err)
			}
			site.Products = append(site.Products, product)
			site.Pages = append(site.Pages, product.Pages...)
			site.Categories = append(site.Categories, product.Categories...)
			for _, p := range product.Pages {
				site.PageMap[p.Slug] = p
			}
		}
	} else {
		// Single-product mode (backwards compatible)
		pages, categories, err := loadContent(filepath.Join(dir, "content"), "", cfg.CategoryOrder)
		if err != nil {
			return nil, err
		}
		site.Pages = pages
		site.Categories = categories
		for _, p := range pages {
			site.PageMap[p.Slug] = p
		}
	}

	return site, nil
}

func loadProduct(dir string, cfg *config.Config, prod config.Product) (*Product, error) {
	contentDir := filepath.Join(dir, "content", prod.Slug)
	pages, categories, err := loadContent(contentDir, prod.Slug, prod.CategoryOrder)
	if err != nil {
		return nil, err
	}

	return &Product{
		Name:        prod.Name,
		Slug:        prod.Slug,
		Description: prod.Description,
		Pages:       pages,
		Categories:  categories,
		Pathways:    prod.Pathways,
	}, nil
}

func loadContent(contentDir, productSlug string, categoryOrder []string) ([]*Page, []*Category, error) {
	var pages []*Page
	catMap := make(map[string]*Category)
	catOrder := 0

	if _, err := os.Stat(contentDir); os.IsNotExist(err) {
		return nil, nil, nil
	}

	err := filepath.Walk(contentDir, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return err
		}
		if info.IsDir() || filepath.Ext(path) != ".md" {
			return nil
		}

		rel, err := filepath.Rel(contentDir, path)
		if err != nil {
			return err
		}

		page, err := loadPage(path, rel, productSlug)
		if err != nil {
			return fmt.Errorf("loading %s: %w", rel, err)
		}

		if page.Draft {
			return nil
		}

		pages = append(pages, page)

		catName := page.Category
		cat, ok := catMap[catName]
		if !ok {
			cat = &Category{
				Name:        catName,
				Title:       humanize(catName),
				Order:       catOrder,
				ProductSlug: productSlug,
			}
			catOrder++
			catMap[catName] = cat
		}
		cat.Pages = append(cat.Pages, page)

		return nil
	})
	if err != nil {
		return nil, nil, err
	}

	var categories []*Category
	for _, cat := range catMap {
		sort.Slice(cat.Pages, func(i, j int) bool {
			if cat.Pages[i].Order != cat.Pages[j].Order {
				return cat.Pages[i].Order < cat.Pages[j].Order
			}
			return cat.Pages[i].Slug < cat.Pages[j].Slug
		})
		categories = append(categories, cat)
	}

	if len(categoryOrder) > 0 {
		orderMap := make(map[string]int)
		for i, name := range categoryOrder {
			orderMap[name] = i
		}
		sort.Slice(categories, func(i, j int) bool {
			oi, oki := orderMap[categories[i].Name]
			oj, okj := orderMap[categories[j].Name]
			if oki && okj {
				return oi < oj
			}
			if oki {
				return true
			}
			if okj {
				return false
			}
			return categories[i].Order < categories[j].Order
		})
	} else {
		sort.Slice(categories, func(i, j int) bool {
			return categories[i].Order < categories[j].Order
		})
	}

	return pages, categories, nil
}

func loadPage(path, rel, productSlug string) (*Page, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}

	page := &Page{}
	body, err := parseFrontmatter(data, page)
	if err != nil {
		return nil, fmt.Errorf("parsing frontmatter: %w", err)
	}
	page.Body = body
	page.ProductSlug = productSlug

	slug := strings.TrimSuffix(rel, ".md")
	slug = filepath.ToSlash(slug)

	// Prefix with product slug for multi-product
	if productSlug != "" {
		page.Slug = productSlug + "/" + slug
	} else {
		page.Slug = slug
	}

	parts := strings.SplitN(slug, "/", 2)
	if len(parts) == 2 {
		page.Category = parts[0]
	} else {
		page.Category = ""
	}

	return page, nil
}

func parseFrontmatter(data []byte, v any) ([]byte, error) {
	if !bytes.HasPrefix(data, []byte("---\n")) {
		return data, nil
	}

	rest := data[4:]
	end := bytes.Index(rest, []byte("\n---\n"))
	if end == -1 {
		return data, nil
	}

	fm := rest[:end]
	body := rest[end+5:]

	if err := yaml.Unmarshal(fm, v); err != nil {
		return nil, err
	}

	return body, nil
}

func humanize(s string) string {
	s = strings.ReplaceAll(s, "-", " ")
	if len(s) == 0 {
		return s
	}
	return strings.ToUpper(s[:1]) + s[1:]
}
