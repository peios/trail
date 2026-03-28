package config

import (
	"fmt"
	"os"
	"path/filepath"
	"sort"

	"github.com/BurntSushi/toml"
)

type Config struct {
	Title        string    `toml:"title"`
	Description  string    `toml:"description"`
	BaseURL      string    `toml:"base_url"`
	RepoURL      string    `toml:"repo_url"`
	Favicon      string    `toml:"favicon"`
	HeadExtra    string    `toml:"head_extra"`
	Announcement string    `toml:"announcement"`
	Nav          []NavItem `toml:"nav"`
	Products     []Product `toml:"products"`

	// Legacy single-product fields (used when no products defined)
	CategoryOrder []string `toml:"category_order"`
	Pathways      []Pathway
}

type NavItem struct {
	Label string `toml:"label"`
	URL   string `toml:"url"`
}

type Product struct {
	Name          string   `toml:"name"`
	Slug          string   `toml:"slug"`
	Description   string   `toml:"description"`
	Order         int      `toml:"order"`
	CategoryOrder []string `toml:"category_order"`

	Pathways []Pathway
}

type Pathway struct {
	Name        string   `toml:"name"`
	Slug        string   // derived from filename
	Description string   `toml:"description"`
	Featured    bool     `toml:"featured"`
	Order       int      `toml:"order"`
	Product     string   // which product this belongs to
	Pages       []string `toml:"pages"`
}

func Load(dir string) (*Config, error) {
	cfg := &Config{}

	cfgPath := filepath.Join(dir, "trail.toml")
	if _, err := toml.DecodeFile(cfgPath, cfg); err != nil {
		return nil, fmt.Errorf("reading %s: %w", cfgPath, err)
	}

	if len(cfg.Products) > 0 {
		// Multi-product mode: load pathways per product
		for i := range cfg.Products {
			p := &cfg.Products[i]
			pathways, err := loadPathways(filepath.Join(dir, "pathways", p.Slug))
			if err != nil {
				return nil, fmt.Errorf("loading pathways for %s: %w", p.Slug, err)
			}
			for j := range pathways {
				pathways[j].Product = p.Slug
			}
			p.Pathways = pathways
		}
		sort.Slice(cfg.Products, func(i, j int) bool {
			oi, oj := cfg.Products[i].Order, cfg.Products[j].Order
			if oi != oj {
				if oi == 0 {
					return false
				}
				if oj == 0 {
					return true
				}
				return oi < oj
			}
			return cfg.Products[i].Name < cfg.Products[j].Name
		})
	} else {
		// Single-product mode: load pathways from root
		pathways, err := loadPathways(filepath.Join(dir, "pathways"))
		if err != nil {
			return nil, err
		}
		cfg.Pathways = pathways
	}

	return cfg, nil
}

func (c *Config) AllPathways() []Pathway {
	if len(c.Products) == 0 {
		return c.Pathways
	}
	var all []Pathway
	for _, p := range c.Products {
		all = append(all, p.Pathways...)
	}
	return all
}

func (c *Config) CategoryOrderFor(product string) []string {
	if len(c.Products) == 0 {
		return c.CategoryOrder
	}
	for _, p := range c.Products {
		if p.Slug == product {
			return p.CategoryOrder
		}
	}
	return nil
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
