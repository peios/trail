package config

import (
	"fmt"
	"os"
	"path/filepath"
	"sort"

	"github.com/BurntSushi/toml"
)

type Config struct {
	Title         string    `toml:"title"`
	Description   string    `toml:"description"`
	BaseURL       string    `toml:"base_url"`
	RepoURL       string    `toml:"repo_url"`
	Favicon       string    `toml:"favicon"`
	HeadExtra     string    `toml:"head_extra"`
	Announcement  string    `toml:"announcement"`
	Nav           []NavItem `toml:"nav"`
	CategoryOrder []string  `toml:"category_order"`

	Pathways []Pathway
}

type NavItem struct {
	Label string `toml:"label"`
	URL   string `toml:"url"`
}

type Pathway struct {
	Name        string   `toml:"name"`
	Slug        string   // derived from filename
	Description string   `toml:"description"`
	Featured    bool     `toml:"featured"`
	Order       int      `toml:"order"`
	Pages       []string `toml:"pages"` // slugs like "identity/how-tokens-work"
}

func Load(dir string) (*Config, error) {
	cfg := &Config{}

	cfgPath := filepath.Join(dir, "trail.toml")
	if _, err := toml.DecodeFile(cfgPath, cfg); err != nil {
		return nil, fmt.Errorf("reading %s: %w", cfgPath, err)
	}

	pathways, err := loadPathways(filepath.Join(dir, "pathways"))
	if err != nil {
		return nil, err
	}
	cfg.Pathways = pathways

	return cfg, nil
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
			// 0 means unset — sort after ordered items
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
