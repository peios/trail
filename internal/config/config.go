package config

import (
	"fmt"
	"net/url"
	"os"
	"path/filepath"
	"sort"
	"strconv"
	"strings"

	"github.com/BurntSushi/toml"
)

type Config struct {
	Title        string           `toml:"title"`
	Description  string           `toml:"description"`
	BaseURL      string           `toml:"base_url"`
	RepoURL      string           `toml:"repo_url"`
	Favicon      string           `toml:"favicon"`
	HeadExtra    string           `toml:"head_extra"`
	Announcement string           `toml:"announcement"`
	Nav          []NavItem        `toml:"nav"`
	Dictionary   DictionaryConfig `toml:"dictionary"`
	Products     []Product
}

// DictionaryConfig controls the dictionary feature.
type DictionaryConfig struct {
	Dir      string                              `toml:"dir"`
	AutoLink bool                                `toml:"auto_link"`
	Products map[string]DictionaryProductConfig   `toml:"products"`
}

// DictionaryProductConfig holds per-product dictionary overrides.
type DictionaryProductConfig struct {
	AutoLink *bool `toml:"auto_link"`
}

// DictDir returns the absolute path to the dictionary directory.
func (c *DictionaryConfig) DictDir(siteDir string) string {
	dir := c.Dir
	if dir == "" {
		dir = "dict"
	}
	return filepath.Join(siteDir, dir)
}

// AutoLinkForProduct reports whether auto-linking is enabled for a product.
// Per-product settings override the global default.
func (c *DictionaryConfig) AutoLinkForProduct(productSlug string) bool {
	if p, ok := c.Products[productSlug]; ok && p.AutoLink != nil {
		return *p.AutoLink
	}
	return c.AutoLink
}

type NavItem struct {
	Label string `toml:"label"`
	URL   string `toml:"url"`
}

type Product struct {
	Name        string
	Slug        string
	Description string
	Order       int
	Kind        string // "docs" or "spec"
	Dir         string // full path to product directory
	SpecID      string // e.g. "psd-004" — parsed from "id--name" directory convention

	Versions []Version
	Pathways []Pathway
}

type Version struct {
	Name   string
	Status string
	Date   string
	Dir    string // full path to version directory
}

func (p *Product) IsSpec() bool {
	return p.Kind == "spec"
}

func (p *Product) CurrentVersion() *Version {
	for i := len(p.Versions) - 1; i >= 0; i-- {
		if p.Versions[i].Status != "superseded" && p.Versions[i].Status != "withdrawn" {
			return &p.Versions[i]
		}
	}
	if len(p.Versions) > 0 {
		return &p.Versions[len(p.Versions)-1]
	}
	return nil
}

type Pathway struct {
	Name        string   `toml:"name"`
	Slug        string
	Description string   `toml:"description"`
	Featured    bool     `toml:"featured"`
	Order       int      `toml:"order"`
	Product     string
	Pages       []string `toml:"pages"`
}

// productConfig is the per-product trail.toml (name + description only).
type productConfig struct {
	Name        string `toml:"name"`
	Description string `toml:"description"`
}

// versionConfig is the per-version trail.toml (status + date only).
type versionConfig struct {
	Status string `toml:"status"`
	Date   string `toml:"date"`
}

func Load(dir string) (*Config, error) {
	cfg := &Config{}

	cfgPath := filepath.Join(dir, "trail.toml")
	if _, err := toml.DecodeFile(cfgPath, cfg); err != nil {
		return nil, fmt.Errorf("reading %s: %w", cfgPath, err)
	}

	// Discover docs products
	docsDir := filepath.Join(dir, "docs")
	if entries, err := os.ReadDir(docsDir); err == nil {
		for _, entry := range entries {
			if !entry.IsDir() {
				continue
			}
			product, err := loadDocsProduct(docsDir, entry.Name())
			if err != nil {
				return nil, fmt.Errorf("loading docs product %s: %w", entry.Name(), err)
			}
			cfg.Products = append(cfg.Products, product)
		}
	}

	// Discover spec products
	specsDir := filepath.Join(dir, "specs")
	if entries, err := os.ReadDir(specsDir); err == nil {
		for _, entry := range entries {
			if !entry.IsDir() {
				continue
			}
			product, err := loadSpecProductConfig(specsDir, entry.Name())
			if err != nil {
				return nil, fmt.Errorf("loading spec product %s: %w", entry.Name(), err)
			}
			cfg.Products = append(cfg.Products, product)
		}
	}

	sort.SliceStable(cfg.Products, func(i, j int) bool {
		if cfg.Products[i].Order != cfg.Products[j].Order {
			return cfg.Products[i].Order < cfg.Products[j].Order
		}
		if cfg.Products[i].SpecID != cfg.Products[j].SpecID {
			return cfg.Products[i].SpecID < cfg.Products[j].SpecID
		}
		return cfg.Products[i].Slug < cfg.Products[j].Slug
	})

	return cfg, nil
}

func loadDocsProduct(docsDir, dirName string) (Product, error) {
	slug := StripNumericPrefix(dirName)
	order := NumericPrefix(dirName)
	productDir := filepath.Join(docsDir, dirName)

	var pc productConfig
	tomlPath := filepath.Join(productDir, "trail.toml")
	if _, err := toml.DecodeFile(tomlPath, &pc); err != nil {
		return Product{}, fmt.Errorf("reading %s: %w", tomlPath, err)
	}

	pathways, err := loadPathways(filepath.Join(productDir, "pathways"))
	if err != nil {
		return Product{}, fmt.Errorf("loading pathways for %s: %w", slug, err)
	}
	for i := range pathways {
		pathways[i].Product = slug
	}

	return Product{
		Name:        pc.Name,
		Slug:        slug,
		Description: pc.Description,
		Order:       order,
		Kind:        "docs",
		Dir:         productDir,
		Pathways:    pathways,
	}, nil
}

func loadSpecProductConfig(specsDir, dirName string) (Product, error) {
	specID, _ := ParseSpecDirName(dirName)
	productDir := filepath.Join(specsDir, dirName)

	var pc productConfig
	tomlPath := filepath.Join(productDir, "trail.toml")
	if _, err := toml.DecodeFile(tomlPath, &pc); err != nil {
		return Product{}, fmt.Errorf("reading %s: %w", tomlPath, err)
	}

	entries, err := os.ReadDir(productDir)
	if err != nil {
		return Product{}, fmt.Errorf("reading %s: %w", productDir, err)
	}

	var versions []Version
	for _, entry := range entries {
		if !entry.IsDir() || !strings.HasPrefix(entry.Name(), "v") {
			continue
		}

		var vc versionConfig
		vTomlPath := filepath.Join(productDir, entry.Name(), "trail.toml")
		if _, err := toml.DecodeFile(vTomlPath, &vc); err != nil {
			return Product{}, fmt.Errorf("reading %s: %w", vTomlPath, err)
		}

		versions = append(versions, Version{
			Name:   entry.Name(),
			Status: vc.Status,
			Date:   vc.Date,
			Dir:    filepath.Join(productDir, entry.Name()),
		})
	}

	sort.Slice(versions, func(i, j int) bool {
		return versions[i].Name < versions[j].Name
	})

	return Product{
		Name:        pc.Name,
		Slug:        specID,
		Description: pc.Description,
		Kind:        "spec",
		Dir:         productDir,
		SpecID:      specID,
		Versions:    versions,
	}, nil
}

func (c *Config) AllPathways() []Pathway {
	var all []Pathway
	for _, p := range c.Products {
		all = append(all, p.Pathways...)
	}
	return all
}

func (c *Config) BasePath() string {
	if c.BaseURL == "" {
		return "/"
	}
	u, err := url.Parse(c.BaseURL)
	if err != nil {
		return "/"
	}
	p := u.Path
	if p == "" || p == "/" {
		return "/"
	}
	if !strings.HasPrefix(p, "/") {
		p = "/" + p
	}
	if !strings.HasSuffix(p, "/") {
		p += "/"
	}
	return p
}

func loadPathways(dir string) ([]Pathway, error) {
	entries, err := os.ReadDir(dir)
	if os.IsNotExist(err) {
		return nil, nil
	}
	if err != nil {
		return nil, fmt.Errorf("reading pathways directory: %w", err)
	}

	var pathways []Pathway
	for _, entry := range entries {
		if entry.IsDir() || filepath.Ext(entry.Name()) != ".toml" {
			continue
		}

		var p Pathway
		path := filepath.Join(dir, entry.Name())
		if _, err := toml.DecodeFile(path, &p); err != nil {
			return nil, fmt.Errorf("reading pathway %s: %w", path, err)
		}

		p.Slug = entry.Name()[:len(entry.Name())-len(".toml")]
		pathways = append(pathways, p)
	}

	sort.Slice(pathways, func(i, j int) bool {
		oi, oj := pathways[i].Order, pathways[j].Order
		if oi != oj {
			if oi == 0 {
				return false
			}
			if oj == 0 {
				return true
			}
			return oi < oj
		}
		return pathways[i].Name < pathways[j].Name
	})

	return pathways, nil
}

// ParseSpecDirName splits a spec directory name on "--" into a spec ID and slug.
// "psd-004--kacs" → ("psd-004", "kacs"). If no "--" is present, the spec ID is
// empty and the full name is returned as the slug.
func ParseSpecDirName(dirName string) (specID, slug string) {
	if id, name, ok := strings.Cut(dirName, "--"); ok {
		return id, name
	}
	return "", dirName
}

// StripNumericPrefix removes a leading "N-" numeric prefix from a name.
// "1-introduction" → "introduction", "v0.20" → "v0.20", "foo" → "foo"
func StripNumericPrefix(name string) string {
	idx := strings.IndexByte(name, '-')
	if idx <= 0 {
		return name
	}
	if _, err := strconv.Atoi(name[:idx]); err != nil {
		return name
	}
	return name[idx+1:]
}

// NumericPrefix extracts the leading numeric prefix from a name.
// "1-introduction" → 1, "100-architecture" → 100, "v0.20" → 0
func NumericPrefix(name string) int {
	idx := strings.IndexByte(name, '-')
	if idx <= 0 {
		return 0
	}
	n, err := strconv.Atoi(name[:idx])
	if err != nil {
		return 0
	}
	return n
}
