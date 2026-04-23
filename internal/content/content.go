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
	Kind        string // "docs" or "spec"
	Pages       []*Page
	Categories  []*Category
	Pathways    []config.Pathway
	Versions    []config.Version
	VersionSlug string // current version being rendered (e.g. "v0.20")
}

type Page struct {
	Title       string   `yaml:"title"`
	Type        string   `yaml:"type"`
	Description string   `yaml:"description"`
	Updated     string   `yaml:"updated"`
	Draft       bool     `yaml:"draft"`
	Related     []string `yaml:"related"`
	Slug        string   // full slug: "peios/identity/understanding-identity"
	Category    string   // e.g. "identity"
	ProductSlug string   // e.g. "peios" (empty for single-product)
	Body        []byte
	SectionNum  string // e.g. "1.2" — set from numeric file prefixes for spec products
	Order       int    // from numeric file prefix, used for sorting
}

type Category struct {
	Name        string
	Title       string
	Order       int // from numeric dir prefix
	ProductSlug string
	Pages       []*Page
	SectionNum  string // e.g. "1" — set from numeric dir prefix for spec products
}

func Load(dir string, cfg *config.Config) (*Site, error) {
	site := &Site{
		PageMap: make(map[string]*Page),
	}

	for _, prod := range cfg.Products {
		products, err := loadProduct(prod)
		if err != nil {
			return nil, fmt.Errorf("loading product %s: %w", prod.Slug, err)
		}
		for _, product := range products {
			site.Products = append(site.Products, product)
			site.Pages = append(site.Pages, product.Pages...)
			site.Categories = append(site.Categories, product.Categories...)
			for _, p := range product.Pages {
				site.PageMap[p.Slug] = p
			}
		}
	}

	return site, nil
}

func loadProduct(prod config.Product) ([]*Product, error) {
	if prod.IsSpec() && len(prod.Versions) > 0 {
		return loadSpecProduct(prod)
	}

	contentDir := filepath.Join(prod.Dir, "content")
	pages, categories, err := loadContent(contentDir, prod.Slug, false)
	if err != nil {
		return nil, err
	}

	return []*Product{{
		Name:        prod.Name,
		Slug:        prod.Slug,
		Description: prod.Description,
		Kind:        prod.Kind,
		Pages:       pages,
		Categories:  categories,
		Pathways:    prod.Pathways,
		Versions:    prod.Versions,
	}}, nil
}

func loadSpecProduct(prod config.Product) ([]*Product, error) {
	var products []*Product

	for _, ver := range prod.Versions {
		versionSlug := "spec/" + prod.Slug + "/" + ver.Name

		pages, categories, err := loadContent(ver.Dir, versionSlug, true)
		if err != nil {
			return nil, fmt.Errorf("loading version %s: %w", ver.Name, err)
		}

		products = append(products, &Product{
			Name:        prod.Name,
			Slug:        versionSlug,
			Description: prod.Description,
			Kind:        "spec",
			Pages:       pages,
			Categories:  categories,
			Versions:    prod.Versions,
			VersionSlug: ver.Name,
		})
	}

	return products, nil
}

func loadContent(contentDir, productSlug string, isSpec bool) ([]*Page, []*Category, error) {
	var pages []*Page
	catMap := make(map[string]*Category)

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

		page, err := loadPage(path, rel, productSlug, isSpec)
		if err != nil {
			return fmt.Errorf("loading %s: %w", rel, err)
		}

		if page.Draft {
			return nil
		}

		pages = append(pages, page)

		catName := page.Category
		if _, ok := catMap[catName]; !ok {
			// Extract category order and section number from the raw directory name
			relSlash := filepath.ToSlash(rel)
			parts := strings.SplitN(relSlash, "/", 2)
			catOrder := 0
			catSectionNum := ""
			if len(parts) == 2 {
				catOrder = config.NumericPrefix(parts[0])
				if isSpec && catOrder > 0 {
					catSectionNum = fmt.Sprintf("%d", catOrder)
				}
			}
			catMap[catName] = &Category{
				Name:        catName,
				Title:       humanize(catName),
				Order:       catOrder,
				ProductSlug: productSlug,
				SectionNum:  catSectionNum,
			}
		}
		catMap[catName].Pages = append(catMap[catName].Pages, page)

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

	sort.Slice(categories, func(i, j int) bool {
		return categories[i].Order < categories[j].Order
	})

	return pages, categories, nil
}

func loadPage(path, rel, productSlug string, isSpec bool) (*Page, error) {
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

	relSlash := filepath.ToSlash(rel)
	rawSlug := strings.TrimSuffix(relSlash, ".md")

	// Split into raw path components and strip numeric prefixes for the slug
	rawParts := strings.Split(rawSlug, "/")
	strippedParts := make([]string, len(rawParts))
	for i, part := range rawParts {
		strippedParts[i] = config.StripNumericPrefix(part)
	}
	slug := strings.Join(strippedParts, "/")

	// Extract page order from the file name's numeric prefix
	fileName := rawParts[len(rawParts)-1]
	page.Order = config.NumericPrefix(fileName)

	// Set section number for specs from the filesystem prefixes
	if isSpec && len(rawParts) >= 2 {
		catPrefix := config.NumericPrefix(rawParts[0])
		filePrefix := config.NumericPrefix(rawParts[len(rawParts)-1])
		if catPrefix > 0 && filePrefix > 0 {
			page.SectionNum = fmt.Sprintf("%d.%d", catPrefix, filePrefix)
		}
	}

	// Prefix with product slug for the full slug
	if productSlug != "" {
		page.Slug = productSlug + "/" + slug
	} else {
		page.Slug = slug
	}

	// Category is the first component of the stripped slug
	if len(strippedParts) >= 2 {
		page.Category = strippedParts[0]
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
