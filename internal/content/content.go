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
	Pages      []*Page
	Categories []*Category
	PageMap    map[string]*Page // slug → page
}

type Page struct {
	Title       string   `yaml:"title"`
	Type        string   `yaml:"type"` // "concept" or "how-to"
	Order       int      `yaml:"order"`
	Description string   `yaml:"description"`
	Updated     string   `yaml:"updated"`
	Draft       bool     `yaml:"draft"`
	Related     []string `yaml:"related"`
	Slug        string   // e.g. "identity/how-tokens-work"
	Category    string   // e.g. "identity"
	Body        []byte   // raw markdown body (after frontmatter)
}

type Category struct {
	Name  string // directory name, e.g. "identity"
	Title string // human-readable, derived from first page or directory name
	Order int
	Pages []*Page
}

func Load(dir string, cfg *config.Config) (*Site, error) {
	contentDir := filepath.Join(dir, "content")

	site := &Site{
		PageMap: make(map[string]*Page),
	}

	catMap := make(map[string]*Category)
	catOrder := 0

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

		page, err := loadPage(path, rel)
		if err != nil {
			return fmt.Errorf("loading %s: %w", rel, err)
		}

		if page.Draft {
			return nil
		}

		site.Pages = append(site.Pages, page)
		site.PageMap[page.Slug] = page

		catName := page.Category
		cat, ok := catMap[catName]
		if !ok {
			cat = &Category{
				Name:  catName,
				Title: humanize(catName),
				Order: catOrder,
			}
			catOrder++
			catMap[catName] = cat
		}
		cat.Pages = append(cat.Pages, page)

		return nil
	})
	if err != nil {
		return nil, err
	}

	for _, cat := range catMap {
		sort.Slice(cat.Pages, func(i, j int) bool {
			if cat.Pages[i].Order != cat.Pages[j].Order {
				return cat.Pages[i].Order < cat.Pages[j].Order
			}
			return cat.Pages[i].Slug < cat.Pages[j].Slug
		})
		site.Categories = append(site.Categories, cat)
	}
	// Sort categories by configured order, then discovery order
	if len(cfg.CategoryOrder) > 0 {
		orderMap := make(map[string]int)
		for i, name := range cfg.CategoryOrder {
			orderMap[name] = i
		}
		sort.Slice(site.Categories, func(i, j int) bool {
			oi, oki := orderMap[site.Categories[i].Name]
			oj, okj := orderMap[site.Categories[j].Name]
			if oki && okj {
				return oi < oj
			}
			if oki {
				return true
			}
			if okj {
				return false
			}
			return site.Categories[i].Order < site.Categories[j].Order
		})
	} else {
		sort.Slice(site.Categories, func(i, j int) bool {
			return site.Categories[i].Order < site.Categories[j].Order
		})
	}

	return site, nil
}

func loadPage(path, rel string) (*Page, error) {
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

	// Derive slug and category from path
	// rel is like "identity/how-tokens-work.md"
	slug := strings.TrimSuffix(rel, ".md")
	slug = filepath.ToSlash(slug)
	page.Slug = slug

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
