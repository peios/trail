package cmd

import (
	"fmt"
	"path/filepath"

	"github.com/peios/trail/internal/build"
	"github.com/peios/trail/internal/config"
	"github.com/peios/trail/internal/content"
	"github.com/peios/trail/internal/dictionary"
)

func runBuild(f flags) error {
	dir, err := filepath.Abs(f.dir)
	if err != nil {
		return fmt.Errorf("resolving directory: %w", err)
	}

	outDir, err := filepath.Abs(f.output)
	if err != nil {
		return fmt.Errorf("resolving output directory: %w", err)
	}

	cfg, err := config.Load(dir)
	if err != nil {
		return fmt.Errorf("loading config: %w", err)
	}

	site, err := content.Load(dir, cfg)
	if err != nil {
		return fmt.Errorf("loading content: %w", err)
	}

	dict, err := dictionary.Load(cfg.Dictionary.DictDir(dir))
	if err != nil {
		return fmt.Errorf("loading dictionary: %w", err)
	}

	if err := build.Build(site, cfg, dict, dir, outDir); err != nil {
		return fmt.Errorf("building site: %w", err)
	}

	if dict.IsEmpty() {
		fmt.Printf("Built %d pages in %d categories with %d pathways → %s\n",
			len(site.Pages), len(site.Categories), len(cfg.AllPathways()), outDir)
	} else {
		fmt.Printf("Built %d pages in %d categories with %d pathways and %d dictionary terms → %s\n",
			len(site.Pages), len(site.Categories), len(cfg.AllPathways()), len(dict.Terms), outDir)
	}
	return nil
}
